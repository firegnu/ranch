use super::{
    Error, Result,
    config::{ChatConfig, ConfigFile, name_segment},
    state::{
        Address, Backfill, ButtonContent, ButtonMessage, Cursor, Expired, Interrupt, Pending,
        Phase, QuestionContext, Seen, State, Store, Thread, Verification, Watch,
    },
};
use crate::{
    chats::{ChannelInfo, Connection, ConnectionInfo, Event, HistoryAfter, Message, Outgoing},
    corral::{self, Agent, Corral, Questions, Status},
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

mod approvals;
mod buttons;
mod questions;
use buttons::ButtonOffer;

pub struct Engine {
    pub state: State,
    pub store: Store,
    pub configs: Vec<ChatConfig>,
    pub config_file: Option<ConfigFile>,
    pub connections: BTreeMap<String, Box<dyn Connection>>,
    pub reports: BTreeMap<String, ConnectionInfo>,
    pub channels: BTreeMap<(String, String), ChannelInfo>,
    pub log: Vec<String>,
    previous: BTreeMap<String, Status>,
    blocked_since: BTreeMap<String, (String, f64)>,
    recovering: BTreeSet<String>,
    connected: BTreeSet<String>,
    last_step: Option<f64>,
}
impl Engine {
    pub fn new(
        store: Store,
        configs: Vec<ChatConfig>,
        connections: BTreeMap<String, Box<dyn Connection>>,
    ) -> Self {
        let mut state = store.state();
        for p in &mut state.pending {
            if p.phase == Phase::Sending {
                p.phase = Phase::Uncertain;
            }
        }
        let recovering = state.watches.keys().cloned().collect();
        Self {
            state,
            store,
            configs,
            config_file: None,
            connections,
            log: Vec::new(),
            reports: BTreeMap::new(),
            channels: BTreeMap::new(),
            previous: BTreeMap::new(),
            blocked_since: BTreeMap::new(),
            recovering,
            connected: BTreeSet::new(),
            last_step: None,
        }
    }
    fn save(&self) -> Result<()> {
        self.store.save(&self.state, None)
    }
    fn record_error(&mut self, context: &str, error: Error) -> Result<()> {
        if matches!(
            error.code.as_str(),
            "io" | "state_invalid" | "secrets_too_open"
        ) {
            return Err(error);
        }
        self.log
            .push(format!("{context} {}", safe_code(&error.code)));
        Ok(())
    }
    fn record_adapter_error(&mut self, context: &str, error: &Error) {
        self.log
            .push(format!("{context} {}", safe_code(&error.code)));
    }
    fn say(&mut self, address: &Address, text: &str, notify: bool) -> Result<()> {
        if let Err(error) = self.post(address, text, notify, false) {
            self.record_error("delivery_failed", error)?;
        }
        Ok(())
    }
    fn bound(&self, kind: &str, repo: &str) -> bool {
        self.configs
            .iter()
            .any(|c| c.kind == kind && c.channels.iter().any(|b| b.repo == repo))
    }
    fn allowed(&self, name: &str) -> bool {
        name.split_once('/').is_some_and(|(repo, _)| {
            self.configs
                .iter()
                .any(|c| c.channels.iter().any(|b| b.repo == repo))
        })
    }
    fn address(kind: &str, message: &Message) -> Address {
        Address {
            kind: kind.into(),
            channel: message.channel.clone(),
            thread: message.thread.clone().unwrap_or_else(|| message.id.clone()),
        }
    }
    fn post(&mut self, address: &Address, text: &str, notify: bool, top: bool) -> Result<String> {
        self.post_with_button(address, text, notify, top, None)
    }
    fn post_with_button(
        &mut self,
        address: &Address,
        text: &str,
        notify: bool,
        top: bool,
        mut offer: Option<ButtonOffer>,
    ) -> Result<String> {
        let connection = self
            .connections
            .get_mut(&address.kind)
            .ok_or_else(|| Error::new("disconnected", "adapter is not connected"))?;
        // Updating this same message must leave room for the result or expiry reason.
        let reserve = offer.as_ref().map_or(0, |o| {
            if o.form.is_some() {
                questions::reserve(&o.status)
            } else {
                128
            }
        });
        let choice_hint = if offer.as_ref().is_some_and(|o| o.form.is_some()) {
            "\n也可以点选项；多选至少选一项后点『提交』。全部题目答完即送出，此前可改选。"
        } else {
            ""
        };
        let limit = connection
            .limits()
            .characters
            .saturating_sub(reserve + choice_hint.chars().count());
        let prefixed;
        let text = if !connection.limits().threads
            && let Some(t) = self.state.threads.iter().find(|t| t.address == *address)
        {
            prefixed = format!("{}: {text}", t.agent);
            &prefixed
        } else {
            text
        };
        let parts = super::split::split(text, limit)?;
        let total = parts.len();
        let mut work: VecDeque<_> = parts
            .into_iter()
            .enumerate()
            .map(|(i, p)| (p, i, total, limit))
            .collect();
        let mut first = None;
        let mut buttons = offer
            .as_ref()
            .filter(|o| o.form.is_none())
            .map(|o| {
                let mut buttons = vec![o.button.clone()];
                if let Some(approval) = &o.approval {
                    buttons.push(approval.button.clone());
                }
                buttons
            })
            .unwrap_or_default();
        while let Some((part, i, total, limit)) = work.pop_front() {
            let mut rendered = part.render(i, total);
            if first.is_none() && offer.is_some() {
                rendered.push_str(choice_hint);
            }
            let thread = if !connection.limits().threads {
                None
            } else if top {
                first.as_deref()
            } else {
                Some(address.thread.as_str())
            };
            let mut result = connection.send(Outgoing {
                channel: &address.channel,
                thread,
                markdown: &rendered,
                notify: notify && first.is_none(),
                buttons: if first.is_none() { &buttons } else { &[] },
                choices: if first.is_none() {
                    offer
                        .as_ref()
                        .and_then(|o| o.form.as_ref())
                        .map_or(&[], |f| &f.choices)
                } else {
                    &[]
                },
            });
            if result.is_err()
                && first.is_none()
                && let Some(offer) = offer.as_mut()
                && offer.approval.take().is_some()
            {
                // Retry without approval, retaining rejection and its receipt.
                // Re-split the full notice so later parts also lose the approval hint.
                buttons.truncate(1);
                let text = text.replace("要允许：点批准按钮并确认", "要允许：回电脑处理");
                let parts = super::split::split(&text, limit)?;
                let total = parts.len();
                work = parts
                    .into_iter()
                    .enumerate()
                    .map(|(i, p)| (p, i, total, limit))
                    .collect();
                continue;
            }
            if result.is_err()
                && first.is_none()
                && offer.as_ref().is_some_and(|o| o.form.is_some())
            {
                // Preserve the question even when the platform rejects its controls.
                // No form receipt is kept for a message posted without choices.
                offer = None;
                rendered.truncate(rendered.len() - choice_hint.len());
                result = connection.send(Outgoing {
                    channel: &address.channel,
                    thread,
                    markdown: &rendered,
                    notify,
                    buttons: &[],
                    choices: &[],
                });
            }
            match result {
                Ok(id) => {
                    if first.is_none()
                        && let Some(offer) = &offer
                    {
                        let mut reply_to = address.clone();
                        if top {
                            reply_to.thread = id.clone();
                        }
                        self.state.buttons.push(ButtonMessage {
                            address: reply_to,
                            message: id.clone(),
                            value: offer.button.value.clone(),
                            approval_value: offer.approval.as_ref().map(|a| a.button.value.clone()),
                            content: Some(ButtonContent {
                                approval: offer.approval.clone(),
                                form: offer.form.clone(),
                                text: rendered,
                                notify,
                                button: offer.button.clone(),
                                status: offer.status.clone(),
                                screen: offer.screen.clone(),
                            }),
                            created: offer.at,
                            outcome: None,
                            used: false,
                            updated: false,
                            update_retry: None,
                        });
                        self.store.save(&self.state, None)?;
                    }
                    first.get_or_insert(id);
                }
                Err(e) if e.resize && limit > 1000 && rendered.chars().count() > 1000 => {
                    let smaller = (limit / 2).max(1000);
                    let pieces = super::split::split(&rendered, smaller)?;
                    let count = pieces.len();
                    for (j, piece) in pieces.into_iter().enumerate().rev() {
                        work.push_front((piece, j, count, smaller));
                    }
                }
                Err(e) => {
                    let code = safe_code(&e.code);
                    self.log.push(format!("delivery_failed {code}"));
                    if !notify {
                        return Err(Error::new("delivery_failed", code));
                    }
                    let explanation = format!("第 {}/{total} 段发不出去：{code}", i + 1);
                    match connection.send(Outgoing {
                        channel: &address.channel,
                        thread,
                        markdown: &explanation,
                        notify: notify && first.is_none(),
                        buttons: &[],
                        choices: &[],
                    }) {
                        Ok(id) => {
                            first.get_or_insert(id);
                        }
                        Err(error) => {
                            self.log
                                .push(format!("delivery_failed {}", safe_code(&error.code)));
                            // Preserve the root of a partially posted notice for thread routing.
                            return first.ok_or_else(|| {
                                Error::new("delivery_failed", "cannot post delivery error")
                            });
                        }
                    }
                }
            }
        }
        first.ok_or_else(|| Error::new("delivery_failed", "no message was posted"))
    }
    fn remember(&mut self, kind: &str, m: &Message) {
        self.state.seen.push(Seen {
            kind: kind.into(),
            channel: m.channel.clone(),
            id: m.id.clone(),
            at: m.at,
        });
        let address = Address {
            kind: kind.into(),
            channel: m.channel.clone(),
            thread: String::new(),
        };
        if !self.state.backfills.iter().any(|b| b.address == address) {
            self.advance_cursor(
                address,
                HistoryAfter {
                    id: Some(m.id.clone()),
                    at: m.at,
                },
            );
        }
    }
    fn advance_cursor(&mut self, address: Address, after: HistoryAfter) {
        if let Some(cursor) = self.state.cursors.iter_mut().find(|c| c.address == address) {
            if after.at > cursor.at {
                cursor.at = after.at;
                cursor.id = after.id;
            }
        } else {
            self.state.cursors.push(Cursor {
                address,
                id: after.id,
                at: after.at,
            });
        }
    }
    fn message(
        &mut self,
        kind: &str,
        m: Message,
        corral: &mut dyn Corral,
        at: f64,
        offline: bool,
    ) -> Result<()> {
        let address = Self::address(kind, &m);
        if let Err(error) = self.route_message(kind, m, corral, at, offline, None) {
            let text = format!("{}: {}", error.code, error.message);
            let delivery = error.code == "delivery_failed";
            self.record_error("route_failed", error)?;
            // Keep the seen marker and any pending acknowledgement, then dequeue this input.
            self.save()?;
            if !delivery {
                self.say(&address, &text, false)?;
            }
        }
        Ok(())
    }
    fn route_message(
        &mut self,
        kind: &str,
        m: Message,
        corral: &mut dyn Corral,
        at: f64,
        offline: bool,
        button: Option<usize>,
    ) -> Result<()> {
        let Some(config) = self.configs.iter().find(|c| c.kind == kind).cloned() else {
            return Ok(());
        };
        if m.sender != config.owner {
            self.log.push("忽略了一条别人的消息".into());
            return Ok(());
        }
        if self
            .state
            .seen
            .iter()
            .any(|s| s.kind == kind && s.channel == m.channel && s.id == m.id)
            || self.state.pending.iter().any(|p| {
                p.address.kind == kind && p.address.channel == m.channel && p.message == m.id
            })
        {
            return Ok(());
        }
        let address = Self::address(kind, &m);
        let threaded = self
            .connections
            .get(kind)
            .is_some_and(|c| c.limits().threads);
        let known = self
            .state
            .threads
            .iter()
            .find(|t| threaded && t.address == address)
            .cloned();
        // A control action is not a new chat message and must not advance history.
        if button.is_none() {
            self.remember(kind, &m);
        }
        let Some(binding) = config.channels.iter().find(|c| c.id == m.channel) else {
            self.save()?;
            if m.mentioned {
                let words: Vec<_> = m.text.split_whitespace().collect();
                if let ["绑", repo] = words.as_slice() {
                    self.bind_channel(&address, repo, corral, at)?;
                } else {
                    self.post(
                        &address,
                        "这个频道没绑仓库。要绑，发 `@pony 绑 <仓库名>`",
                        false,
                        false,
                    )?;
                }
            }
            return Ok(());
        };
        if self.state.expired.iter().any(|e| e.address == address) {
            self.save()?;
            self.post(
                &address,
                "这个线程太久没用，对应关系已清掉，请在频道里重新 @ 我",
                false,
                false,
            )?;
            return Ok(());
        }
        if !m.mentioned && known.is_none() {
            self.save()?;
            return Ok(());
        }
        let text = m.text.trim_start();
        let agents = corral.list().map_err(corral_error)?;
        let words: Vec<_> = text.split_whitespace().collect();
        let stop = match words.as_slice() {
            ["停" | "打断"] => Some(
                known
                    .as_ref()
                    .map_or_else(|| format!("{}/main", binding.repo), |t| t.agent.clone()),
            ),
            ["停" | "打断", target]
                if known.is_none() && (target.contains('/') || name_segment(target)) =>
            {
                if target
                    .split_once('/')
                    .is_some_and(|(repo, _)| repo != binding.repo)
                {
                    self.save()?;
                    return self.say(&address, "只能停本频道仓库的 agent，没按", false);
                }
                let name = if target.contains('/') {
                    (*target).to_owned()
                } else {
                    format!("{}/{target}", binding.repo)
                };
                if !agents.iter().any(|a| a.name == name) {
                    self.save()?;
                    return self.say(&address, &format!("没有 {name} 这个 agent，没按"), false);
                }
                Some(name)
            }
            _ => None,
        };
        if let Some(name) = stop {
            return self.stop_message(&address, &name, &m, corral, at, offline);
        }
        if offline && at - m.at > 12. * 3600. {
            self.save()?;
            self.post(
                &address,
                "这句是 12 小时以前发的，没送；要送请重发",
                false,
                false,
            )?;
            return Ok(());
        }
        if text.is_empty() && m.attachments {
            self.save()?;
            self.say(&address, "附件没送，只收文字", false)?;
            return Ok(());
        }
        if matches!(text.trim_end(), "?" | "？") {
            let (mut summary, controls) = self.summary(kind, &binding.repo, &agents, corral, at)?;
            let buttons = self
                .connections
                .get(kind)
                .is_some_and(|c| c.limits().buttons);
            if !buttons && !controls.is_empty() {
                summary.push_str("\n要打断本频道的 agent：@pony 停 <名字>，会话留着，只停这一轮。");
            }
            self.save()?;
            self.post(&address, &summary, false, false)?;
            if buttons {
                for status in controls {
                    self.interrupt_notice(&address, &status, corral, at, true)?;
                }
            }
            return Ok(());
        }
        let (name, mut body) = if let Some(thread) = &known {
            (thread.agent.clone(), m.text.clone())
        } else {
            let first = text.split_whitespace().next().unwrap_or("");
            let rest = text
                .get(first.len()..)
                .unwrap_or("")
                .trim_start()
                .to_owned();
            if let Some((repo, _)) = first.split_once('/') {
                if !self.bound(kind, repo) {
                    self.save()?;
                    self.post(&address, &self.unbound_message(kind), false, false)?;
                    return Ok(());
                }
                if !agents.iter().any(|a| a.name == first) {
                    self.save()?;
                    self.post(
                        &address,
                        &format!("没有 {first} 这个 agent，没送"),
                        false,
                        false,
                    )?;
                    return Ok(());
                }
                (first.into(), rest)
            } else if name_segment(first)
                && agents
                    .iter()
                    .any(|a| a.name == format!("{}/{first}", binding.repo))
            {
                (format!("{}/{first}", binding.repo), rest)
            } else {
                if !threaded {
                    self.save()?;
                    self.post(
                        &address,
                        "这个平台没有线程，请每句 @ 机器人并写明 agent 名字；? 看状态",
                        false,
                        false,
                    )?;
                    return Ok(());
                }
                let name = format!("{}/main", binding.repo);
                if !agents.iter().any(|a| a.name == name) {
                    self.save()?;
                    self.post(&address, &format!("{name} 没在跑，没送"), false, false)?;
                    return Ok(());
                }
                (name, m.text.clone())
            }
        };
        // Re-check saved threads against the active bindings after configuration changes.
        if !name
            .split_once('/')
            .is_some_and(|(repo, _)| self.bound(kind, repo))
        {
            self.save()?;
            self.post(&address, &self.unbound_message(kind), false, false)?;
            return Ok(());
        }
        if body.trim().is_empty() {
            self.save()?;
            self.post(
                &address,
                if m.attachments {
                    "附件没送，只收文字"
                } else {
                    "用法：@机器人 正文，或 @机器人 agent/全名 正文；? 看状态"
                },
                false,
                false,
            )?;
            return Ok(());
        }
        if button.is_none()
            && self.state.pending.iter().any(|p| {
                p.address == address
                    && p.button_answer
                    && (p.phase == Phase::Queued || m.at <= p.at)
            })
        {
            self.save()?;
            return self.say(
                &address,
                "选项回答已提交，这句没作为第二份答案发送；收到回复后可接着聊",
                false,
            );
        }
        if offline {
            let label = self.connections.get(kind).map_or("聊天", |c| c.name());
            body = format!(
                "（这是你 {} 在 {label} 发的，pony 当时离线，现在才送到）\n{body}",
                clock_time(m.at)
            );
        }
        let status = match corral.status(&name) {
            Ok(s) => s,
            Err(e) if e.exit == Some(2) => {
                let reason = if known.as_ref().is_some_and(|t| t.instance.is_empty()) {
                    "没在跑"
                } else {
                    "已经不在了"
                };
                self.save()?;
                self.post(&address, &format!("{name} {reason}，没送"), false, false)?;
                return Ok(());
            }
            Err(e) => return Err(corral_error(e)),
        };
        if let Some(i) = button {
            let b = &self.state.buttons[i];
            let reason = self
                .stop_reason(
                    &status,
                    b.content.as_ref().map(|c| c.status.instance.as_str()),
                    m.at,
                    at,
                )
                .or_else(|| buttons::stale(b, &status));
            if let Some(reason) = reason {
                return self.finish_button(i, reason, false, at);
            }
        }
        let reopened = known
            .as_ref()
            .is_some_and(|t| !t.instance.is_empty() && t.instance != status.instance);
        let mut question = known.as_ref().and_then(|t| t.question.clone());
        // A handled prompt only covers answers still queued (or its pending Esc).
        // A completed, withdrawn or failed answer must not turn later chat into answers.
        if question.as_ref().is_some_and(|q| q.handled)
            && !self
                .state
                .pending
                .iter()
                .any(|p| p.address == address && p.question_answer && p.phase == Phase::Queued)
            && !self.state.interrupts.iter().any(|i| i.agent == name)
        {
            question = None;
        }
        let question_reason = question.as_ref().and_then(|q| {
            if offline {
                Some("离线期间发的回答，没按 Esc")
            } else if q.handled {
                Some("这条提问已处理过，没再按 Esc")
            } else if q.instance != status.instance {
                Some("它重开过，没按 Esc")
            } else if status.paused {
                Some("暂停中，没按 Esc")
            } else if status.state != "blocked" {
                Some("提问框已经不在了，没按 Esc")
            } else if status.kind.as_deref() != Some("claude")
                || status.last_tool.as_deref() != Some("AskUserQuestion")
                || q.state_started.is_none()
                || q.state_started != status.state_started
            {
                Some("提问框已换或无法核对，没按 Esc")
            } else if human_active(&status, at) {
                Some("你刚在电脑上操作过，没按 Esc")
            } else if self.state.interrupts.iter().any(|i| i.agent == name) {
                Some("已经按过 Esc，正在等它停下，没再按")
            } else {
                None
            }
        });
        let answer = question.is_some();
        let reference = question
            .as_ref()
            .filter(|_| button.is_none())
            .and_then(|q| q.questions.as_ref())
            .and_then(|questions| option_reference(questions, &body));
        let close_question = answer && question_reason.is_none();
        if let Some(i) = button
            && !close_question
        {
            return self.finish_button(
                i,
                question_reason.unwrap_or("提问已处理，没按 Esc"),
                false,
                at,
            );
        }
        if let Some(q) = &mut question {
            q.handled = true;
        }
        let immediate_head = !status.paused
            && matches!(status.state.as_str(), "idle" | "unknown")
            && self
                .state
                .pending
                .iter()
                .find(|p| p.agent == name)
                .is_some_and(|p| {
                    p.phase == Phase::Queued
                        && !p.queued
                        && p.retry_at <= at
                        && p.announcement.is_none()
                });
        let count = self
            .state
            .pending
            .iter()
            .filter(|p| p.agent == name && p.phase == Phase::Queued)
            .count()
            + 1
            - usize::from(immediate_head);
        let queued = answer
            || status.paused
            || !matches!(status.state.as_str(), "idle" | "unknown")
            || self.state.pending.iter().any(|p| p.agent == name);
        let explanation = if status.paused {
            "（暂停中，恢复后再送）".into()
        } else if answer
            && (close_question || self.state.interrupts.iter().any(|i| i.agent == name))
        {
            "（关掉提问框后就送）".into()
        } else if status.state == "blocked" {
            "（在等你处理对话框，处理完再送）".into()
        } else if queued {
            format!("（正在干活，这句排在第 {count} 个，这一轮结束就送）")
        } else if status.state == "unknown" {
            "（它报不出状态，送了，但等不到回复）".into()
        } else {
            String::new()
        };
        self.state.threads.retain(|t| t.address != address);
        self.state.threads.push(Thread {
            address: address.clone(),
            agent: name.clone(),
            instance: status.instance.clone(),
            created: known.as_ref().map_or(at, |t| t.created),
            used: at,
            question,
        });
        let message_id = m.id.clone();
        let announcement = format!(
            "→ {name}{}{}{explanation}",
            if reopened { "（重开过）" } else { "" },
            if offline {
                "（离线期间发的）"
            } else {
                ""
            }
        );
        self.state.pending.push(Pending {
            button_answer: button.is_some(),
            interrupt_offered: false,
            announcement: Some(announcement.clone()),
            attachments: m.attachments,
            rejection_reported: false,
            verification: None,
            question_answer: answer,
            address: address.clone(),
            message: m.id,
            agent: name.clone(),
            instance: status.instance.clone(),
            text: if answer {
                format!("{}\n{body}", answer_prefix(reference.as_deref(), false))
            } else {
                body.clone()
            },
            at,
            queued,
            retry_at: at,
            baseline: status.turn_started,
            phase: Phase::Queued,
        });
        if let Some(i) = button {
            let b = &mut self.state.buttons[i];
            b.used = true;
            b.outcome = Some(format!(
                "已答：{}（{}）",
                questions::answer_summary(b.content.as_ref().expect("validated form")),
                clock_time(at)
            ));
        } else if answer {
            self.take_over_question(&address);
        }
        self.save()?;
        if close_question {
            if self.start_interrupt(&address, &status, corral, at, true)? {
                self.state.pending.last_mut().expect("just queued").text =
                    format!("{}\n{body}", answer_prefix(reference.as_deref(), true));
                self.save()?;
            }
        } else if let Some(reason) = question_reason {
            self.say(
                &address,
                &format!("{reason}；回答排着，等空下来再送"),
                false,
            )?;
        }
        self.log.push(format!(
            "route {} {} {}",
            identifier(&name),
            identifier(&address.channel),
            identifier(&message_id)
        ));
        self.post(&address, &announcement, false, false)?;
        self.state
            .pending
            .last_mut()
            .expect("just queued")
            .announcement = None;
        self.save()?;
        self.report_attachments(self.state.pending.len() - 1)
    }
    fn stop_message(
        &mut self,
        address: &Address,
        name: &str,
        message: &Message,
        corral: &mut dyn Corral,
        at: f64,
        offline: bool,
    ) -> Result<()> {
        // Commit the input ID before any key, including errors and refused requests.
        self.save()?;
        if !name
            .split_once('/')
            .is_some_and(|(repo, _)| self.bound(&address.kind, repo))
        {
            return self.say(address, &self.unbound_message(&address.kind), false);
        }
        if offline {
            return self.say(address, "离线期间发的「停」没执行，没按 Esc", false);
        }
        let status = match corral.status(name) {
            Ok(status) => status,
            Err(_) => return self.say(address, "读不出 agent 状态，没按 Esc", false),
        };
        let known = self
            .state
            .threads
            .iter()
            .find(|t| t.address == *address)
            .cloned();
        self.state.threads.retain(|t| t.address != *address);
        self.state.threads.push(Thread {
            address: address.clone(),
            agent: name.into(),
            instance: status.instance.clone(),
            created: known.as_ref().map_or(at, |t| t.created),
            used: at,
            question: known.as_ref().and_then(|t| t.question.clone()),
        });
        let reason = self.stop_reason(
            &status,
            known.as_ref().map(|t| t.instance.as_str()),
            message.at,
            at,
        );
        self.save()?;
        if let Some(reason) = reason {
            self.say(address, reason, false)
        } else {
            self.start_interrupt(address, &status, corral, at, false)
                .map(|_| ())
        }
    }

    fn stop_reason(
        &self,
        status: &Status,
        instance: Option<&str>,
        sent: f64,
        at: f64,
    ) -> Option<&'static str> {
        if instance.is_some_and(|i| !i.is_empty() && i != status.instance) {
            Some("它重开过，没按 Esc")
        } else if status.kind.as_deref() == Some("agy") {
            Some("agy 被打断后 corral 认不出，打断不了，没按；请回电脑")
        } else if status.paused {
            Some("暂停中，没按 Esc")
        } else if status.state == "idle" {
            Some("已经空闲，没按 Esc")
        } else if status.state == "starting" {
            Some("启动中，没按 Esc")
        } else if !matches!(status.state.as_str(), "working" | "blocked") {
            Some("读不出 agent 当前是否在干活，没按 Esc")
        } else if status.turn_started.is_some_and(|t| t > sent) {
            Some("它已经开了新的一轮，没按 Esc")
        } else if human_active(status, at) {
            Some("你刚在电脑上操作过，没按 Esc")
        } else if self.state.interrupts.iter().any(|i| i.agent == status.name) {
            Some("已经按过 Esc，正在等它停下，没再按")
        } else {
            None
        }
    }

    fn start_interrupt(
        &mut self,
        address: &Address,
        status: &Status,
        corral: &mut dyn Corral,
        at: f64,
        question_answer: bool,
    ) -> Result<bool> {
        for thread in &mut self.state.threads {
            if thread.agent == status.name
                && let Some(q) = &mut thread.question
                && q.instance == status.instance
                && q.state_started == status.state_started
            {
                q.handled = true;
            }
        }
        self.state.interrupts.push(Interrupt {
            address: address.clone(),
            agent: status.name.clone(),
            instance: status.instance.clone(),
            turn: status.turn_started,
            blocked: status.state == "blocked",
            question_answer,
            started: at,
            poll_at: at + 1.,
        });
        let buttons: Vec<_> = self
            .state
            .buttons
            .iter()
            .enumerate()
            .filter_map(|(i, b)| {
                ((b.outcome.is_none() || b.outcome.as_deref() == Some(questions::TEXT_ANSWER))
                    && b.content.as_ref().is_some_and(|c| {
                        c.status.name == status.name && c.status.instance == status.instance
                    })
                    && buttons::stale(b, status).is_none())
                .then_some((i, b.outcome.clone()))
            })
            .collect();
        for (i, _) in &buttons {
            self.state.buttons[*i].outcome = Some("已接收，Esc 是否送达待确认；没重按".into());
        }
        // Persist before keys: a crash can lose a key, but must never repeat it.
        self.save()?;
        let result = corral.escape(&status.name, status.kind.as_deref() == Some("amp"));
        for (i, previous) in buttons {
            let outcome = if result.is_ok() {
                "已在线程里按了 Esc"
            } else {
                "Esc 发送失败或结果不明，没重按"
            };
            self.state.buttons[i].outcome =
                Some(previous.map_or_else(|| outcome.into(), |p| format!("{p}；{outcome}")));
        }
        self.save()?;
        if let Err(error) = result {
            self.log.push(format!("corral {}", safe_code(&error.code)));
            self.say(
                address,
                "Esc 发送失败或结果不明，没重按；继续核对状态",
                false,
            )?;
            return Ok(false);
        }
        Ok(true)
    }

    fn poll_interrupts(&mut self, corral: &mut dyn Corral, at: f64) -> Result<()> {
        let mut i = 0;
        while i < self.state.interrupts.len() {
            let action = self.state.interrupts[i].clone();
            if !self.connected.contains(&action.address.kind) || at < action.poll_at {
                i += 1;
                continue;
            }
            let status = corral.status(&action.agent);
            let mut stopped = false;
            let result = match &status {
                Ok(s) if s.instance != action.instance => {
                    Some("它重开过，没继续按，也没确认原来那轮停下")
                }
                Ok(s) if s.turn_started != action.turn => {
                    Some("它已经开了新的一轮，没继续按，也没确认原来那轮停下")
                }
                Ok(s) if !s.paused && s.state == "idle" && (s.interrupted || action.blocked) => {
                    stopped = true;
                    Some(if action.question_answer {
                        "提问框已关，回答这就送过去"
                    } else if action.blocked {
                        "框已拒绝，会话还在，在这个线程接着说就行"
                    } else {
                        "停下了，会话还在，在这个线程接着说就行"
                    })
                }
                _ if at - action.started < 15. => None,
                Ok(s) if s.paused => Some("按了 Esc，15 秒后仍暂停中，没确认停下，没再按"),
                Ok(s) if s.state == "working" => {
                    Some("按了 Esc，15 秒后还在干活，没再按；排着的话继续等空下来再送")
                }
                Ok(s) if s.state == "blocked" => {
                    Some("按了 Esc，15 秒后还在等你，没再按；排着的话继续等空下来再送")
                }
                _ => Some("按了 Esc，15 秒内没确认停下，没再按；排着的话继续等空下来再送"),
            };
            let Some(result) = result else {
                self.state.interrupts[i].poll_at = at + 1.;
                i += 1;
                continue;
            };
            let mut result = result.to_owned();
            if stopped {
                let before = self.state.pending.len();
                self.state.pending.retain(|p| {
                    !(p.agent == action.agent
                        && p.instance == action.instance
                        && p.address == action.address
                        && p.phase == Phase::Waiting
                        && p.baseline != action.turn)
                });
                if self.state.pending.len() < before {
                    result.push_str("；这一轮被打断了，没有回复");
                }
                if let Some(watch) = self.state.watches.get_mut(&action.agent) {
                    watch.turn = action.turn;
                }
            }
            self.state.interrupts.remove(i);
            self.save()?;
            self.say(&action.address, &format!("{} {result}", action.agent), true)?;
        }
        self.save()
    }

    fn report_attachments(&mut self, i: usize) -> Result<()> {
        if self.state.pending[i].attachments {
            self.state.pending[i].attachments = false;
            let address = self.state.pending[i].address.clone();
            self.save()?;
            self.say(&address, "附件没送", false)?;
        }
        Ok(())
    }

    fn bind_channel(
        &mut self,
        address: &Address,
        repo: &str,
        corral: &mut dyn Corral,
        at: f64,
    ) -> Result<()> {
        if !name_segment(repo) {
            return self.refuse_binding(address, "invalid_repo", "仓库名不合规则，没绑：须以 ASCII 字母或数字开头，只能含字母、数字、点、下划线和连字符");
        }
        let channel = self
            .connections
            .get_mut(&address.kind)
            .ok_or_else(|| Error::new("disconnected", "adapter is not connected"))?
            .channel(&address.channel);
        match channel.map(|channel| channel.private) {
            Ok(Some(true)) => {}
            Ok(Some(false)) => {
                return self.refuse_binding(address, "public_channel", "这是公开频道，没绑：主控的回复会带仓库代码和路径，整个工作区都看得到。请建私有频道再绑");
            }
            Ok(None) => {
                return self.refuse_binding(
                    address,
                    "channel_unknown",
                    "查不到这个频道是不是私有的，没绑；请稍后再试",
                );
            }
            Err(error) => {
                return self.refuse_binding(
                    address,
                    &error.code,
                    "查不到这个频道是不是私有的，没绑；请稍后再试",
                );
            }
        }
        if let Some(binding) = self
            .configs
            .iter()
            .find(|config| config.kind == address.kind)
            .and_then(|config| config.channels.iter().find(|channel| channel.repo == repo))
        {
            let display = self
                .connections
                .get_mut(&address.kind)
                .unwrap()
                .channel(&binding.id)
                .ok()
                .and_then(|channel| channel.name)
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| binding.id.clone());
            return self.refuse_binding(
                address,
                "repo_bound",
                &format!("{repo} 已经绑在 #{display}，没绑"),
            );
        }
        let result = self
            .config_file
            .as_ref()
            .ok_or_else(|| Error::new("config_invalid", "no active configuration file"))
            .and_then(|file| file.bind(&address.kind, &address.channel, repo));
        let text = match result {
            Ok(()) => {
                let name = format!("{repo}/main");
                let status = corral.status(&name);
                let instance = status
                    .as_ref()
                    .map(|s| s.instance.clone())
                    .unwrap_or_default();
                let summary = self
                    .agent_summary(&name, status, true, corral, at)
                    .unwrap_or_else(|| format!("{name} 没在跑"));
                if self.connections[&address.kind].limits().threads {
                    self.state.threads.retain(|t| t.address != *address);
                    self.state.threads.push(Thread {
                        address: address.clone(),
                        agent: name,
                        instance,
                        created: at,
                        used: at,
                        question: None,
                    });
                    self.save()?;
                }
                format!("已绑 {repo}\n{summary}")
            }
            Err(error) => {
                let text = match error.code.as_str() {
                    "config_invalid" => "配置现在有错，没绑；请先修好配置".into(),
                    "changed" => "配置刚被改过，请再说一次".into(),
                    "format_not_preserved" => format!(
                        "配置文件的格式没法原样保留，没绑；请手动加这一行：\n{{ id = \"{}\", repo = \"{repo}\" }}",
                        address.channel
                    ),
                    "config_not_writable" => format!(
                        "配置文件是链接或只读的，没绑；请手动加这一行：\n{{ id = \"{}\", repo = \"{repo}\" }}",
                        address.channel
                    ),
                    _ => format!("写配置失败，没绑：{}", safe_code(&error.code)),
                };
                return self.refuse_binding(address, &error.code, &text);
            }
        };
        self.post(address, &text, false, false)?;
        Ok(())
    }

    fn refuse_binding(&mut self, address: &Address, code: &str, text: &str) -> Result<()> {
        self.log.push(format!("bind_failed {}", safe_code(code)));
        self.post(address, text, false, false).map(|_| ())
    }

    fn observe_blocked(&mut self, status: &Status, at: f64) {
        if status.state != "blocked" {
            self.blocked_since.remove(&status.name);
        } else {
            let entry = self
                .blocked_since
                .entry(status.name.clone())
                .or_insert_with(|| (status.instance.clone(), at));
            if entry.0 != status.instance {
                *entry = (status.instance.clone(), at);
            }
        }
    }

    fn agent_summary(
        &mut self,
        name: &str,
        status: corral::Result<Status>,
        local: bool,
        corral: &mut dyn Corral,
        at: f64,
    ) -> Option<String> {
        let s = match status {
            Ok(s) => Some(s),
            Err(e) if e.exit == Some(2) => return None,
            Err(_) => None,
        };
        let label = match s {
            None => "读不出".into(),
            Some(s) => {
                self.observe_blocked(&s, at);
                let reply = (local && !s.paused && s.state == "idle").then(|| corral.reply(name));
                let interrupted = s.interrupted
                    || reply.as_ref().is_some_and(
                        |r| matches!(r, Err(e) if e.code == "no_reply" && e.interrupted),
                    );
                let last_reply = reply
                    .as_ref()
                    .and_then(|r| r.as_ref().ok())
                    .filter(|r| r.instance == s.instance && !interrupted);
                let known_reply = self
                    .state
                    .watches
                    .get(name)
                    .filter(|w| w.status.instance == s.instance && !interrupted)
                    .and_then(|w| w.reply);
                let (label, since) = if s.paused {
                    ("暂停中", s.paused_at)
                } else {
                    match s.state.as_str() {
                        "working" => ("干活中", s.turn_started),
                        "idle" => (
                            "空闲",
                            last_reply
                                .map(|r| r.at)
                                .or(if local { None } else { known_reply })
                                .or(s.state_started),
                        ),
                        "blocked" => (
                            "等你（对话框）",
                            s.state_started
                                .or_else(|| self.blocked_since.get(name).map(|(_, at)| *at)),
                        ),
                        "starting" => ("启动中", None),
                        _ => ("读不出", None),
                    }
                };
                let mut label = match since {
                    Some(since) => format!("{label}，{} 分钟", minutes(at - since)),
                    None => label.into(),
                };
                if local {
                    if let Some(title) = summary_title(&s) {
                        label.push_str(&format!(" · 「{title}」"));
                    }
                    if let Some(tool) = s.last_tool.as_deref().filter(|t| !t.is_empty()) {
                        label.push_str(&format!(" · 在用 {tool}"));
                    }
                    if !s.paused && s.state == "idle" {
                        if interrupted {
                            label.push_str(" · 上一轮被打断");
                        } else if let Some(line) = last_reply.and_then(|r| {
                            r.text.lines().map(str::trim).find(|line| !line.is_empty())
                        }) {
                            label.push_str(&format!(
                                " · 上一句「{}」",
                                line.chars().take(60).collect::<String>()
                            ));
                        }
                    }
                }
                label
            }
        };
        let count = self
            .state
            .pending
            .iter()
            .filter(|p| p.agent == name && p.phase == Phase::Queued)
            .count();
        Some(format!(
            "• {} — {label}{}",
            name.split_once('/').unwrap().1,
            if count > 0 {
                format!("，排着 {count} 句")
            } else {
                String::new()
            }
        ))
    }
    fn summary(
        &mut self,
        kind: &str,
        local: &str,
        agents: &[Agent],
        corral: &mut dyn Corral,
        at: f64,
    ) -> Result<(String, Vec<Status>)> {
        let config = self
            .configs
            .iter()
            .find(|c| c.kind == kind)
            .expect("active adapter");
        let mut channels = config.channels.clone();
        channels.sort_by_key(|c| c.repo != local);
        let mut lines = Vec::new();
        let mut controls = Vec::new();
        for channel in channels {
            let display = self
                .channels
                .get(&(kind.into(), channel.id.clone()))
                .and_then(|c| c.name.as_deref())
                .unwrap_or(&channel.id);
            lines.push(format!("{}（#{display}）", channel.repo));
            for agent in agents.iter().filter(|a| {
                a.name
                    .split_once('/')
                    .is_some_and(|(r, _)| r == channel.repo)
            }) {
                let status = corral.status(&agent.name);
                if channel.repo == local
                    && let Ok(s) = &status
                    && !s.paused
                    && s.kind.as_deref() != Some("agy")
                    && matches!(s.state.as_str(), "working" | "blocked")
                {
                    controls.push(s.clone());
                }
                if let Some(line) =
                    self.agent_summary(&agent.name, status, channel.repo == local, corral, at)
                {
                    lines.push(line);
                }
            }
        }
        Ok((lines.join("\n"), controls))
    }
    pub fn step(&mut self, corral: &mut dyn Corral, at: f64) -> Result<()> {
        if let Some(previous) = self.last_step.replace(at)
            && at - previous > 60.
        {
            // A suspended process cannot retry. Exclude the gap from retry deadlines,
            // but leave retry_at due so the first awake tick can try immediately.
            for job in &mut self.state.backfills {
                job.started += at - previous;
            }
            for pending in &mut self.state.pending {
                if let Some(check) = &mut pending.verification {
                    check.started += at - previous;
                }
            }
        }
        self.state.threads.retain(|t| {
            if at - t.used > 30. * 86400. {
                self.state.expired.push(Expired {
                    address: t.address.clone(),
                    at,
                });
                false
            } else {
                true
            }
        });
        // Keep one day of IDs, plus all IDs a still-pending backfill can return.
        self.state.seen.retain(|s| {
            at - s.at <= 86400.
                || self.state.backfills.iter().any(|b| {
                    b.address.kind == s.kind && b.address.channel == s.channel && s.at > b.after.at
                })
        });
        // Also expire unfinished button updates while every adapter is offline.
        self.state
            .buttons
            .retain(|b| at - b.created <= 30. * 86400.);
        for button in &mut self.state.buttons {
            if button.updated
                || button
                    .update_retry
                    .as_ref()
                    .is_some_and(|r| at - r.started >= 86400.)
            {
                button.finish_update();
            }
        }
        self.state.expired.retain(|e| at - e.at <= 90. * 86400.);
        self.state.cursors.retain(|c| {
            if c.address.thread.is_empty() {
                self.configs.iter().any(|config| {
                    config.kind == c.address.kind
                        && config.channels.iter().any(|b| b.id == c.address.channel)
                })
            } else {
                self.state.threads.iter().any(|t| t.address == c.address)
            }
        });
        // Removed bindings must never leak old queued text, replies or notices.
        let permitted = |p: &Pending| {
            self.configs.iter().any(|c| {
                c.kind == p.address.kind
                    && c.channels.iter().any(|b| b.id == p.address.channel)
                    && p.agent
                        .split_once('/')
                        .is_some_and(|(repo, _)| c.channels.iter().any(|b| b.repo == repo))
            })
        };
        self.state.pending.retain(permitted);
        self.state.interrupts.retain(|i| {
            self.configs.iter().any(|c| {
                c.kind == i.address.kind
                    && c.channels.iter().any(|b| b.id == i.address.channel)
                    && i.agent
                        .split_once('/')
                        .is_some_and(|(repo, _)| c.channels.iter().any(|b| b.repo == repo))
            })
        });
        self.state.approvals.retain(|a| {
            self.configs.iter().any(|c| {
                c.kind == a.address.kind
                    && c.channels.iter().any(|b| {
                        b.id == a.address.channel
                            && a.status
                                .name
                                .split_once('/')
                                .is_some_and(|(repo, _)| repo == b.repo)
                    })
            })
        });
        self.state.backfills.retain(|b| {
            self.configs.iter().any(|c| {
                c.kind == b.address.kind
                    && c.channels
                        .iter()
                        .any(|channel| channel.id == b.address.channel)
            })
        });
        self.save()?;
        let uncertain: Vec<_> = self
            .state
            .pending
            .iter()
            .filter(|p| p.phase == Phase::Uncertain)
            .cloned()
            .collect();
        for p in uncertain {
            if !self.connected.contains(&p.address.kind) {
                continue;
            }
            self.log.push("send_uncertain".into());
            self.state
                .pending
                .retain(|q| q.address != p.address || q.message != p.message);
            self.save()?;
            self.say(&p.address, "上次发送结果不明，没重送，请确认后重发", false)?;
        }
        let mut backfilled = BTreeSet::new();
        for input in self.store.inbox() {
            if matches!(
                input.event,
                Event::Message(_) | Event::Action(_) | Event::Deleted { .. }
            ) && !self.connected.contains(&input.kind)
            {
                continue;
            }
            match input.event {
                Event::Message(m) => self.message(&input.kind, m, corral, at, false)?,
                Event::Action(action) => self.button_action(&input.kind, action, corral, at)?,
                Event::Ignored => self.log.push("忽略了一条别人的消息".into()),
                Event::Connection(info) => {
                    let connected = info.connection == "connected";
                    self.log.push(format!(
                        "connection {} {}",
                        identifier(&input.kind),
                        safe_code(&info.connection)
                    ));
                    self.reports.insert(input.kind.clone(), info);
                    if connected {
                        self.connected.insert(input.kind.clone());
                        self.backfill(&input.kind, corral, at, true)?;
                        backfilled.insert(input.kind.clone());
                        if !self
                            .state
                            .backfills
                            .iter()
                            .any(|b| b.address.kind == input.kind)
                        {
                            self.state.online.insert(input.kind.clone(), at);
                        }
                        for pending in &mut self.state.pending {
                            if pending.address.kind == input.kind && pending.phase == Phase::Queued
                            {
                                pending.verification = Some(Verification {
                                    started: at,
                                    retry_at: at,
                                    delay: 2.,
                                });
                            }
                        }
                    } else {
                        self.disconnect(&input.kind);
                    }
                }
                Event::Deleted { channel, id } => {
                    let removed: Vec<_> = self
                        .state
                        .pending
                        .iter()
                        .filter(|p| {
                            p.address.kind == input.kind
                                && p.address.channel == channel
                                && p.message == id
                                && p.phase == Phase::Queued
                        })
                        .cloned()
                        .collect();
                    self.state.pending.retain(|p| {
                        !(p.address.kind == input.kind
                            && p.address.channel == channel
                            && p.message == id
                            && p.phase == Phase::Queued)
                    });
                    self.save()?;
                    for p in removed {
                        self.say(&p.address, "撤回了", false)?;
                    }
                }
            }
            self.store.save(&self.state, Some(input.sequence))?;
        }
        if self.connected.is_empty() {
            return self.save();
        }
        for kind in self.connected.clone().difference(&backfilled) {
            self.backfill(kind, corral, at, false)?;
        }
        // Checkpoint online time at most once a minute. This gives cursorless channels
        // a conservative overlap after an unclean stop without forcing writes every tick.
        for kind in &self.connected {
            if !self.state.backfills.iter().any(|b| b.address.kind == *kind)
                && self
                    .state
                    .online
                    .get(kind)
                    .is_none_or(|last| at - last >= 60.)
            {
                self.state.online.insert(kind.clone(), at);
            }
        }
        self.verify_queue(at)?;
        self.refresh_buttons(corral, at)?;
        self.poll_interrupts(corral, at)?;
        self.poll_approvals(corral, at)?;
        for i in 0..self.state.pending.len() {
            let pending = self.state.pending[i].clone();
            if self.connected.contains(&pending.address.kind)
                && let Some(text) = pending.announcement
            {
                if let Err(error) = self.post(&pending.address, &text, false, false) {
                    self.record_error("delivery_failed", error)?;
                    continue;
                }
                self.state.pending[i].announcement = None;
                self.save()?;
                self.report_attachments(i)?;
            }
        }
        let agents: Vec<Agent> = match corral.list() {
            Ok(agents) => agents,
            Err(error) => {
                self.record_error("corral", corral_error(error))?;
                return self.save();
            }
        }
        .into_iter()
        .filter(|a| self.allowed(&a.name))
        .collect();
        let gone: Vec<_> = self
            .state
            .pending
            .iter()
            .filter(|p| !agents.iter().any(|a| a.name == p.agent))
            .map(|p| p.agent.clone())
            .collect();
        for name in gone {
            self.cancel(&name, None)?;
        }
        self.state
            .watches
            .retain(|name, _| agents.iter().any(|a| a.name == *name));
        self.previous
            .retain(|name, _| agents.iter().any(|a| a.name == *name));
        self.blocked_since
            .retain(|name, _| agents.iter().any(|a| a.name == *name));
        let mut settled = BTreeSet::new();
        let mut unavailable = BTreeSet::new();
        for agent in agents {
            if !agent.name.split_once('/').is_some_and(|(repo, _)| {
                self.configs.iter().any(|c| {
                    self.connected.contains(&c.kind) && c.channels.iter().any(|b| b.repo == repo)
                })
            }) {
                continue;
            }
            let status = match corral.status(&agent.name) {
                Ok(s) => s,
                Err(e) if e.exit == Some(2) => {
                    self.cancel(&agent.name, None)?;
                    continue;
                }
                Err(e) => {
                    self.record_error("corral", corral_error(e))?;
                    self.previous.remove(&agent.name);
                    unavailable.insert(agent.name.clone());
                    continue;
                }
            };
            self.observe_blocked(&status, at);
            self.cancel(&agent.name, Some(&status.instance))?;
            let stable = self.previous.get(&agent.name).is_some_and(|p| {
                p.instance == status.instance
                    && p.state == "idle"
                    && status.state == "idle"
                    && p.last_event == status.last_event
                    && p.last_event_at == status.last_event_at
            });
            if stable {
                settled.insert(agent.name.clone());
            }
            let initial = self
                .state
                .watches
                .get(&agent.name)
                .is_none_or(|w| w.status.instance != status.instance);
            let mut watch = if initial {
                let reply = match corral.reply(&agent.name) {
                    Ok(r) if r.instance == status.instance => Some(r.at),
                    Ok(_) => None,
                    Err(e) if e.code == "no_reply" => None,
                    Err(e) => {
                        self.record_error("corral", corral_error(e))?;
                        unavailable.insert(agent.name.clone());
                        continue;
                    }
                };
                self.recovering.remove(&agent.name);
                Watch {
                    status: status.clone(),
                    turn: status.turn_started,
                    reply,
                    blocked: None,
                }
            } else {
                self.state.watches[&agent.name].clone()
            };
            let waiting = self.state.pending.iter().position(|p| {
                p.agent == agent.name
                    && p.phase == Phase::Waiting
                    && status.turn_started != p.baseline
            });
            if status.state == "working"
                && !status.paused
                && status.kind.as_deref() != Some("agy")
                && let Some(i) = waiting
                && !self.state.pending[i].interrupt_offered
                && !self
                    .state
                    .interrupts
                    .iter()
                    .any(|action| action.agent == agent.name)
            {
                self.state.pending[i].interrupt_offered = true;
                let address = self.state.pending[i].address.clone();
                self.save()?;
                if let Err(error) = self.interrupt_notice(&address, &status, corral, at, false) {
                    self.record_error("delivery_failed", error)?;
                }
            }
            let mut response = None;
            let mut notices = Vec::new();
            if stable
                && !status.paused
                && (waiting.is_some() || status.turn_started != watch.turn)
                && !self.state.interrupts.iter().any(|i| i.agent == agent.name)
            {
                let offline = self.recovering.contains(&agent.name);
                match corral.reply(&agent.name) {
                    Ok(reply) if reply.instance == status.instance => {
                        let text = if reply.text.is_empty() {
                            "（空回复）"
                        } else {
                            &reply.text
                        };
                        if let Some(i) = waiting {
                            let pending = self.state.pending.remove(i);
                            response = Some((
                                pending.address,
                                if status.turn_started.is_some_and(|start| reply.at >= start) {
                                    text.to_owned()
                                } else {
                                    format!("{} 这一轮结束了，没有新回复", agent.name)
                                },
                            ));
                        } else if agent.name.ends_with("/main")
                            && watch.reply.is_none_or(|old| reply.at > old)
                        {
                            notices.push((
                                format!(
                                    "{}{} 新回复\n{text}",
                                    if offline {
                                        "（pony 不在线时）"
                                    } else {
                                        ""
                                    },
                                    agent.name
                                ),
                                None,
                            ));
                        }
                        watch.reply = Some(reply.at);
                    }
                    Ok(_) => {
                        continue;
                    }
                    Err(e) if e.code == "no_reply" => {
                        if let Some(i) = waiting {
                            let pending = self.state.pending.remove(i);
                            response = Some((
                                pending.address,
                                if e.interrupted {
                                    format!("{} 这一轮被打断了，没有回复", agent.name)
                                } else {
                                    "（空回复）".into()
                                },
                            ));
                        }
                    }
                    Err(e) => {
                        self.record_error("corral", corral_error(e))?;
                        unavailable.insert(agent.name.clone());
                        continue;
                    }
                }
                watch.turn = status.turn_started;
            }
            let blocked = self.previous.get(&agent.name).is_some_and(|p| {
                p.instance == status.instance && p.state == "blocked" && status.state == "blocked"
            });
            if blocked && watch.blocked != Some(status.state_started.unwrap_or(0.)) {
                let question = is_question(&status).then(|| QuestionContext {
                    instance: status.instance.clone(),
                    state_started: status.state_started,
                    handled: false,
                    questions: status.questions.clone(),
                });
                notices.push((needs_you(&status, corral), question));
                watch.blocked = Some(status.state_started.unwrap_or(0.));
            }
            if status.state != "blocked" {
                watch.blocked = None;
            }
            if stable || status.state != "idle" {
                self.recovering.remove(&agent.name);
            }
            watch.status = status.clone();
            self.state.watches.insert(agent.name.clone(), watch);
            self.previous.insert(agent.name.clone(), status.clone());
            // Commit completion/dedup before any outgoing chunks. Delivery errors must
            // not turn a later tick or restart into a replay of successful chunks.
            self.save()?;
            if let Some((address, text)) = response {
                self.say(&address, &text, true)?;
            }
            for (text, question) in notices {
                self.notice(&status, &text, question, corral, at)?;
            }
        }
        let mut i = 0;
        let mut attempted = BTreeSet::new();
        while i < self.state.pending.len() {
            let p = self.state.pending[i].clone();
            if p.phase != Phase::Queued
                || p.announcement.is_some()
                || p.verification.is_some()
                || self.state.pending[..i]
                    .iter()
                    .any(|earlier| earlier.agent == p.agent)
                || !self.connected.contains(&p.address.kind)
                || p.retry_at > at
                || attempted.contains(&p.agent)
                || unavailable.contains(&p.agent)
                || self.state.interrupts.iter().any(|i| i.agent == p.agent)
                || self.state.pending.iter().any(|w| {
                    w.agent == p.agent
                        && matches!(w.phase, Phase::Waiting | Phase::Sending | Phase::Uncertain)
                })
            {
                i += 1;
                continue;
            }
            attempted.insert(p.agent.clone());
            let status = match corral.status(&p.agent) {
                Ok(status) => status,
                Err(e) if e.exit == Some(2) => {
                    self.cancel(&p.agent, None)?;
                    continue;
                }
                Err(e) => {
                    self.record_error("corral", corral_error(e))?;
                    i += 1;
                    continue;
                }
            };
            if status.instance != p.instance {
                self.cancel(&p.agent, Some(&status.instance))?;
                continue;
            }
            if status.paused || !matches!(status.state.as_str(), "idle" | "unknown") {
                i += 1;
                continue;
            }
            if p.question_answer && status.state != "idle" {
                i += 1;
                continue;
            }
            if p.queued && status.state != "unknown" && !settled.contains(&p.agent) {
                i += 1;
                continue;
            }
            self.state.pending[i].phase = Phase::Sending;
            self.state.pending[i].baseline = status.turn_started;
            self.save()?;
            match corral.send(&p.agent, &p.text) {
                Ok(sent) => {
                    if status.state == "unknown" {
                        self.state.pending.remove(i);
                    } else {
                        self.state.pending[i].phase = Phase::Waiting;
                        i += 1;
                    }
                    if p.question_answer
                        && !self.state.pending.iter().any(|queued| {
                            queued.address == p.address
                                && queued.question_answer
                                && queued.phase == Phase::Queued
                        })
                        && let Some(thread) = self
                            .state
                            .threads
                            .iter_mut()
                            .find(|t| t.address == p.address)
                    {
                        thread.question = None;
                    }
                    self.save()?;
                    if p.queued {
                        self.say(
                            &p.address,
                            &format!("已送给 {}（排了 {} 分钟）", p.agent, minutes(at - p.at)),
                            false,
                        )?;
                    }
                    if sent.merged_with_draft {
                        self.say(
                            &p.address,
                            "输入框里原有没提交的字，和你的话一起送了",
                            false,
                        )?;
                    }
                }
                Err(e) => {
                    self.log.push(format!("corral {}", safe_code(&e.code)));
                    let text = match (e.exit, e.code.as_str()) {
                        (Some(7), "not_idle")
                        | (Some(8), "human_active")
                        | (Some(10), "paused") => {
                            self.state.pending[i].phase = Phase::Queued;
                            self.state.pending[i].queued = true;
                            self.state.pending[i].retry_at =
                                at + if e.exit == Some(8) { 30. } else { 0. };
                            self.state.pending[i].rejection_reported = true;
                            i += 1;
                            if p.rejection_reported {
                                String::new()
                            } else {
                                match e.exit {
                                    Some(8) => "人刚在窗口里操作过，30 秒后再试".into(),
                                    Some(10) => "暂停中，恢复后再送".into(),
                                    _ => "正在干活，放回队首排着".into(),
                                }
                            }
                        }
                        (None, "corral_unavailable") => {
                            self.state.pending.remove(i);
                            format!("corral_unavailable: {}，没送", e.message)
                        }
                        (None, _) => {
                            self.state.pending.remove(i);
                            self.log.push("send_uncertain".into());
                            "上次发送结果不明，没重送，请确认后重发".into()
                        }
                        _ => {
                            self.state.pending.remove(i);
                            match e.exit {
                                Some(3) if e.code == "not_delivered" => format!(
                                    "送了但 {} 没收到（可能停在菜单或对话框上），没重送",
                                    p.agent
                                ),
                                Some(2) => format!("{} 已经不在了，没送", p.agent),
                                _ => format!("{}: {}", e.code, e.message),
                            }
                        }
                    };
                    self.save()?;
                    if !text.is_empty() {
                        self.say(&p.address, &text, false)?;
                    }
                    if e.exit == Some(2) {
                        self.cancel(&p.agent, None)?;
                    }
                }
            }
        }
        self.save()
    }
    fn cancel(&mut self, name: &str, instance: Option<&str>) -> Result<()> {
        let removed: Vec<_> = self
            .state
            .pending
            .iter()
            .filter(|p| p.agent == name && instance.is_none_or(|i| p.instance != i))
            .cloned()
            .collect();
        self.state.pending.retain(|p| {
            !(p.agent == name
                && self.connected.contains(&p.address.kind)
                && instance.is_none_or(|i| p.instance != i))
        });
        self.save()?;
        for p in &removed {
            if !self.connected.contains(&p.address.kind) {
                continue;
            }
            let text = if p.phase == Phase::Waiting {
                format!("{name} 退出了，没等到回复")
            } else {
                format!("{name} 已经不在了，没送")
            };
            self.say(&p.address, &text, false)?;
        }
        Ok(())
    }
    fn notice(
        &mut self,
        status: &Status,
        text: &str,
        question: Option<QuestionContext>,
        corral: &mut dyn Corral,
        at: f64,
    ) -> Result<()> {
        let name = &status.name;
        let repo = name.split_once('/').expect("bound agent").0;
        let addresses: Vec<_> = self
            .configs
            .iter()
            .filter(|c| self.connected.contains(&c.kind))
            .flat_map(|c| {
                c.channels
                    .iter()
                    .filter(|b| b.repo == repo)
                    .map(|b| Address {
                        kind: c.kind.clone(),
                        channel: b.id.clone(),
                        thread: String::new(),
                    })
            })
            .collect();
        for mut address in addresses {
            let mut offer = if status.state == "blocked" && is_question(status) {
                self.question_offer(&address, status, at)?
            } else {
                None
            };
            if offer.is_none() && status.state == "blocked" {
                offer = self.button_offer(&address, status, "拒绝", corral, at)?;
            }
            let mut text = text.to_owned();
            if status.state == "blocked"
                && status.kind.as_deref() == Some("claude")
                && !is_question(status)
            {
                if let Some(offer) = &mut offer {
                    offer.approval = approvals::offer(status, offer.screen.as_deref())?;
                }
                if let Some(offer) = &offer
                    && offer.approval.is_some()
                {
                    text = needs_you_screen(status, offer.screen.as_deref())
                        .replace("要允许：回电脑处理", "要允许：点批准按钮并确认");
                } else {
                    text.push_str("\n认不出框的样子，没给按钮（允许）；要允许请回电脑处理。");
                }
            }
            address.thread = match self.post_with_button(&address, &text, true, true, offer) {
                Ok(id) => id,
                Err(error) => {
                    self.record_error("delivery_failed", error)?;
                    continue;
                }
            };
            self.state.threads.push(Thread {
                address,
                agent: name.into(),
                instance: status.instance.clone(),
                created: at,
                used: at,
                question: question.clone(),
            });
            self.save()?;
        }
        Ok(())
    }
    fn backfill(
        &mut self,
        kind: &str,
        corral: &mut dyn Corral,
        at: f64,
        reconnect: bool,
    ) -> Result<()> {
        let Some(config) = self.configs.iter().find(|c| c.kind == kind).cloned() else {
            return Ok(());
        };
        let mut messages = Vec::new();
        let mut completed = Vec::new();
        for channel in config.channels {
            let address = Address {
                kind: kind.into(),
                channel: channel.id.clone(),
                thread: String::new(),
            };
            let mut pending = self
                .state
                .backfills
                .iter()
                .find(|b| b.address == address)
                .cloned();
            if pending.is_none() && reconnect {
                let after = self
                    .state
                    .cursors
                    .iter()
                    .find(|c| c.address == address)
                    .map(|cursor| HistoryAfter {
                        id: cursor.id.clone(),
                        at: cursor.at,
                    })
                    .or_else(|| {
                        self.state
                            .online
                            .get(kind)
                            .map(|at| HistoryAfter { id: None, at: *at })
                    });
                if let Some(after) = after {
                    let job = Backfill {
                        address: address.clone(),
                        after,
                        started: at,
                        error: "backfill_pending".into(),
                        retry_at: at,
                        delay: 2.,
                    };
                    self.state.backfills.push(job.clone());
                    self.save()?;
                    pending = Some(job);
                }
            }
            if let Some(job) = &pending {
                if !reconnect && at < job.retry_at {
                    continue;
                }
            } else if !reconnect {
                continue;
            }
            let info = match self.connections.get_mut(kind).unwrap().channel(&channel.id) {
                Ok(info) => info,
                Err(error) => {
                    self.backfill_failed(&address, error, at)?;
                    continue;
                }
            };
            self.channels
                .insert((kind.into(), channel.id.clone()), info);
            let Some(job) = pending else {
                continue;
            };
            let found = match self
                .connections
                .get_mut(kind)
                .unwrap()
                .history(&channel.id, &job.after)
            {
                Ok(found) => found,
                Err(error) => {
                    self.backfill_failed(&address, error, at)?;
                    continue;
                }
            };
            let mut through = HistoryAfter { id: None, at };
            for message in found.into_iter().filter(|m| {
                m.sender == config.owner && m.channel == channel.id && m.at > job.after.at
            }) {
                if message.at > through.at {
                    through = HistoryAfter {
                        id: Some(message.id.clone()),
                        at: message.at,
                    };
                }
                messages.push(message);
            }
            completed.push((address, through));
        }
        messages.sort_by(|a, b| a.at.total_cmp(&b.at).then_with(|| a.id.cmp(&b.id)));
        for message in messages {
            // Workers can enqueue live input while history is being fetched.
            if self.store.inbox().iter().any(|input| input.kind == kind
                && matches!(&input.event, Event::Message(live) if live.channel == message.channel && live.id == message.id)) {
                continue;
            }
            self.message(kind, message, corral, at, true)?;
        }
        // Keep the lower bound durable until every returned message has been handled.
        for (address, through) in completed {
            self.state.backfills.retain(|b| b.address != address);
            self.advance_cursor(address, through);
        }
        self.save()
    }
    fn backfill_failed(&mut self, address: &Address, error: Error, at: f64) -> Result<()> {
        self.record_adapter_error("backfill_failed", &error);
        if let Some(job) = self
            .state
            .backfills
            .iter_mut()
            .find(|b| b.address == *address)
        {
            job.error = safe_code(&error.code);
            job.retry_at = at + job.delay;
            job.delay = (job.delay * 2.).min(300.);
            if at - job.started > 12. * 3600. {
                let code = job.error.clone();
                self.state.backfills.retain(|b| b.address != *address);
                self.advance_cursor(address.clone(), HistoryAfter { id: None, at });
                self.save()?;
                self.log.push(format!("backfill_expired {code}"));
                if let Err(error) = self.post(
                    address,
                    &format!("重连后没读到离线期间的消息（{code}），那段时间发的话请重发"),
                    false,
                    true,
                ) {
                    self.record_error("delivery_failed", error)?;
                }
            }
        }
        self.save()
    }
    fn verify_queue(&mut self, at: f64) -> Result<()> {
        let mut i = 0;
        while i < self.state.pending.len() {
            let pending = self.state.pending[i].clone();
            let Some(mut check) = pending.verification else {
                i += 1;
                continue;
            };
            let address = pending.address;
            let id = pending.message;
            if pending.phase != Phase::Queued || !self.connected.contains(&address.kind) {
                i += 1;
                continue;
            }
            if at - check.started > 600. {
                self.state.pending.remove(i);
                self.save()?;
                self.say(&address, "没法核对这句还在不在，先不送；要送请重发", false)?;
                continue;
            }
            if at < check.retry_at {
                i += 1;
                continue;
            }
            let thread = (address.thread != id).then_some(address.thread.as_str());
            match self.connections.get_mut(&address.kind).unwrap().exists(
                &address.channel,
                thread,
                &id,
            ) {
                Ok(false) => {
                    self.state.pending.remove(i);
                    self.save()?;
                    self.say(&address, "撤回了", false)?;
                    continue;
                }
                Ok(true) => self.state.pending[i].verification = None,
                Err(error) => {
                    self.record_adapter_error("queue_check_failed", &error);
                    check.retry_at = at + check.delay;
                    check.delay = (check.delay * 2.).min(300.);
                    self.state.pending[i].verification = Some(check);
                }
            }
            self.save()?;
            i += 1;
        }
        Ok(())
    }
    pub fn disconnect(&mut self, kind: &str) {
        self.connected.remove(kind);
        self.channels.retain(|(k, _), _| k != kind);
        for name in self.state.watches.keys() {
            if name
                .split_once('/')
                .is_some_and(|(repo, _)| self.bound(kind, repo))
            {
                self.recovering.insert(name.clone());
                self.previous.remove(name);
            }
        }
    }
    fn unbound_message(&self, kind: &str) -> String {
        format!(
            "这个仓库没接入{}",
            self.connections.get(kind).map_or("聊天", |c| c.name())
        )
    }
}
fn human_active(status: &Status, at: f64) -> bool {
    status.last_human_input.is_some_and(|t| at - t < 30.)
}

fn is_question(status: &Status) -> bool {
    status.kind.as_deref() == Some("claude")
        && status.last_tool.as_deref() == Some("AskUserQuestion")
        && status.questions.is_some()
}

fn summary_title(status: &Status) -> Option<String> {
    let title = status
        .title
        .as_deref()?
        .trim()
        .trim_start_matches(|c: char| {
            c.is_whitespace()
                || ('\u{2800}'..='\u{28ff}').contains(&c)
                || "◐◑◒◓◴◵◶◷✻✽✶✳✢·\u{fe0e}\u{fe0f}".contains(c)
        });
    if title.is_empty()
        || status
            .kind
            .as_deref()
            .is_some_and(|k| title.eq_ignore_ascii_case(k))
        || matches!(
            title.to_ascii_lowercase().as_str(),
            "claude" | "claude code" | "codex" | "pi" | "omp" | "amp" | "agy"
        )
    {
        None
    } else {
        Some(title.chars().take(40).collect())
    }
}

fn needs_you(status: &Status, corral: &mut dyn Corral) -> String {
    let screen = if is_question(status) {
        None
    } else {
        corral.read(&status.name).ok()
    };
    needs_you_screen(status, screen.as_deref())
}

fn needs_you_screen(status: &Status, screen: Option<&str>) -> String {
    let footer = if status.kind.as_deref() == Some("agy") {
        "要回电脑上处理；在这里回的话，等它处理完再送。"
    } else {
        "要拒绝：在这个线程回『停』；要允许：回电脑处理。在这里回别的话，等它处理完再送。"
    };
    if status.kind.as_deref() == Some("claude")
        && status.last_tool.as_deref() == Some("AskUserQuestion")
        && let Some(questions) = &status.questions
    {
        let mut text = format!("⚠ {} 在问你（{} 道题）", status.name, questions.items.len());
        for (i, question) in questions.items.iter().enumerate() {
            text.push_str(&format!(
                "\n{}. {}（{}）：{}",
                i + 1,
                question.header,
                if question.multi_select {
                    "可多选"
                } else {
                    "单选"
                },
                question.question,
            ));
            for (j, option) in question.options.iter().enumerate() {
                text.push_str(&format!("\n   {} {}", option_letter(j), option.label));
                if !option.description.is_empty() {
                    text.push_str(&format!(" — {}", option.description));
                }
            }
        }
        if questions.truncated {
            text.push_str("\n题目不完整，请回电脑看");
        }
        return format!("{text}\n在这个线程写答案（例如『1 选 B；2 选 A、C』），会直接送过去。");
    }
    let heading = format!(
        "⚠ {} 在等你（权限框或提问）{}{}。",
        status.name,
        status
            .last_tool
            .as_ref()
            .map(|s| format!(" · 最后的工具 {s}"))
            .unwrap_or_default(),
        status
            .title
            .as_ref()
            .map(|s| format!(" · 标题「{s}」"))
            .unwrap_or_default(),
    );
    if let Some(screen) = screen {
        let lines: Vec<_> = screen.lines().collect();
        let mut tail = Vec::new();
        for line in &lines[lines.len().saturating_sub(20)..] {
            if line.trim().is_empty() {
                if tail.last() != Some(&"") {
                    tail.push("");
                }
            } else {
                tail.push(*line);
            }
        }
        let fence = "~".repeat(
            tail.iter()
                .map(|line| line.trim_start().chars().take_while(|c| *c == '~').count() + 1)
                .max()
                .unwrap_or(3)
                .max(3),
        );
        return format!(
            "{heading}\n屏幕末尾：\n{fence}\n{}\n{fence}\n{footer}",
            tail.join("\n")
        );
    }
    format!("{heading}{footer}")
}

fn option_letter(mut index: usize) -> String {
    let mut letters = Vec::new();
    loop {
        letters.push((b'A' + (index % 26) as u8) as char);
        if index < 26 {
            break;
        }
        index = index / 26 - 1;
    }
    letters.into_iter().rev().collect()
}

const OPTION_REFERENCE_MIN: usize = 80;
const OPTION_REFERENCE_MAX: usize = 300;

fn option_reference(questions: &Questions, answer: &str) -> Option<String> {
    if questions.items.iter().all(|q| q.options.is_empty()) {
        return None;
    }
    let budget = (answer.chars().count() * 2).clamp(OPTION_REFERENCE_MIN, OPTION_REFERENCE_MAX);
    let numbered = questions.items.len() > 1;
    let mut text = String::new();
    let mut cut = false;
    for (i, question) in questions.items.iter().enumerate() {
        if question.options.is_empty() {
            continue;
        }
        let mut segment = if numbered {
            format!("{}. ", i + 1)
        } else {
            String::new()
        };
        let mut count = segment.chars().count();
        let mut fitted = false;
        for (j, option) in question.options.iter().enumerate() {
            let token = format!(
                "{}{} {}",
                if fitted { " " } else { "" },
                option_letter(j),
                option.label.trim()
            );
            let token_len = token.chars().count();
            let connector = if text.is_empty() {
                ""
            } else if numbered {
                "；"
            } else {
                " "
            };
            if text.chars().count() + connector.chars().count() + count + token_len > budget {
                cut = true;
                break;
            }
            segment.push_str(&token);
            count += token_len;
            fitted = true;
        }
        if !fitted {
            cut = true;
            break;
        }
        if !text.is_empty() {
            text.push_str(if numbered { "；" } else { " " });
        }
        text.push_str(&segment);
        if cut {
            break;
        }
    }
    if text.is_empty() {
        return cut.then(|| "对照太长，截断".to_string());
    }
    if questions.truncated {
        text.push_str("；题目不完整");
    }
    if cut {
        text.push_str("；对照太长，截断");
    }
    Some(text)
}

fn answer_prefix(reference: Option<&str>, closed: bool) -> String {
    let mut text = String::from("（这是对刚才提问的回答");
    if let Some(reference) = reference {
        text.push_str("；选项：");
        text.push_str(reference);
    }
    if closed {
        text.push_str("；关闭提问框不表示放弃作答");
    }
    text.push('）');
    text
}

fn corral_error(e: crate::corral::Failure) -> Error {
    // Keep corral I/O errors distinct from pony's fatal storage errors.
    let code = if e.code == "io" { "corral_io" } else { &e.code };
    Error::new(code, e.message)
}
fn minutes(seconds: f64) -> u64 {
    (seconds.max(0.) / 60.) as u64
}
fn clock_time(at: f64) -> String {
    let timestamp = at as libc::time_t;
    let mut time: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&timestamp, &mut time) }.is_null() {
        return "未知时间".into();
    }
    format!("{:02}:{:02}", time.tm_hour, time.tm_min)
}
pub fn safe_code(code: &str) -> String {
    if !code.is_empty()
        && code.len() <= 64
        && code.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
    {
        code.into()
    } else {
        "adapter_error".into()
    }
}
fn identifier(value: &str) -> &str {
    if value.len() <= 256
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-/".contains(&b))
    {
        value
    } else {
        "invalid_id"
    }
}
