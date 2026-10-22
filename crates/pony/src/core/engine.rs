use super::{
    Error, Result,
    config::{ChatConfig, ConfigFile, name_segment},
    state::{
        Address, Backfill, Cursor, Expired, Pending, Phase, Seen, State, Store, Thread,
        Verification, Watch,
    },
};
use crate::{
    chats::{ChannelInfo, Connection, ConnectionInfo, Event, HistoryAfter, Message, Outgoing},
    corral::{Agent, Corral, Status},
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

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
        let connection = self
            .connections
            .get_mut(&address.kind)
            .ok_or_else(|| Error::new("disconnected", "adapter is not connected"))?;
        let limit = connection.limits().characters;
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
        while let Some((part, i, total, limit)) = work.pop_front() {
            let rendered = part.render(i, total);
            let thread = if !connection.limits().threads {
                None
            } else if top {
                first.as_deref()
            } else {
                Some(address.thread.as_str())
            };
            match connection.send(Outgoing {
                channel: &address.channel,
                thread,
                markdown: &rendered,
                notify: notify && first.is_none(),
            }) {
                Ok(id) => {
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
        if let Err(error) = self.route_message(kind, m, corral, at, offline) {
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
        self.remember(kind, &m);
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
        let text = m.text.trim_start();
        if text.is_empty() && m.attachments {
            self.save()?;
            self.say(&address, "附件没送，只收文字", false)?;
            return Ok(());
        }
        let agents = corral.list().map_err(corral_error)?;
        if matches!(text.trim_end(), "?" | "？") {
            let summary = self.summary(kind, &binding.repo, &agents, corral, at)?;
            self.save()?;
            self.post(&address, &summary, false, false)?;
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
                self.save()?;
                self.post(&address, &format!("{name} 已经不在了，没送"), false, false)?;
                return Ok(());
            }
            Err(e) => return Err(corral_error(e)),
        };
        let reopened = known
            .as_ref()
            .is_some_and(|t| t.instance != status.instance);
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
        let queued = status.paused
            || !matches!(status.state.as_str(), "idle" | "unknown")
            || self.state.pending.iter().any(|p| p.agent == name);
        let explanation = if status.paused {
            "（暂停中，恢复后再送）".into()
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
            announcement: Some(announcement.clone()),
            attachments: m.attachments,
            rejection_reported: false,
            verification: None,
            address: address.clone(),
            message: m.id,
            agent: name.clone(),
            instance: status.instance,
            text: body,
            at,
            queued,
            retry_at: at,
            baseline: status.turn_started,
            phase: Phase::Queued,
        });
        self.save()?;
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
            self.post(address, "仓库名不合规则，没绑：须以 ASCII 字母或数字开头，只能含字母、数字、点、下划线和连字符", false, false)?;
            return Ok(());
        }
        let channel = self
            .connections
            .get_mut(&address.kind)
            .ok_or_else(|| Error::new("disconnected", "adapter is not connected"))?
            .channel(&address.channel);
        match channel.map(|channel| channel.private) {
            Ok(Some(true)) => {}
            Ok(Some(false)) => {
                self.post(address, "这是公开频道，没绑：主控的回复会带仓库代码和路径，整个工作区都看得到。请建私有频道再绑", false, false)?;
                return Ok(());
            }
            Ok(None) | Err(_) => {
                self.post(
                    address,
                    "查不到这个频道是不是私有的，没绑；请稍后再试",
                    false,
                    false,
                )?;
                return Ok(());
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
            self.post(
                address,
                &format!("{repo} 已经绑在 #{display}，没绑"),
                false,
                false,
            )?;
            return Ok(());
        }
        let result = self
            .config_file
            .as_ref()
            .ok_or_else(|| Error::new("config_invalid", "no active configuration file"))
            .and_then(|file| file.bind(&address.kind, &address.channel, repo));
        let text = match result {
            Ok(()) => {
                let name = format!("{repo}/main");
                let status = self
                    .agent_summary(&name, corral, at)
                    .unwrap_or_else(|| format!("{name} 没在跑"));
                format!("已绑 {repo}\n{status}")
            }
            Err(error) => match error.code.as_str() {
                "config_invalid" => "配置现在有错，没绑；请先修好配置".into(),
                "changed" => "配置刚被改过，请再说一次".into(),
                _ => format!("写配置失败，没绑：{}", safe_code(&error.code)),
            },
        };
        self.post(address, &text, false, false)?;
        Ok(())
    }

    fn agent_summary(&self, name: &str, corral: &mut dyn Corral, at: f64) -> Option<String> {
        let s = match corral.status(name) {
            Ok(s) => Some(s),
            Err(e) if e.exit == Some(2) => return None,
            Err(_) => None,
        };
        let label = match s {
            None => "读不出".into(),
            Some(s) if s.paused => "暂停中".into(),
            Some(s) => match s.state.as_str() {
                "working" => format!(
                    "干活中，{} 分钟",
                    minutes(at - s.turn_started.unwrap_or(at))
                ),
                "idle" => if s.interrupted {
                    "空闲（上一轮被打断）"
                } else {
                    "空闲"
                }
                .into(),
                "blocked" => "等你（对话框）".into(),
                "starting" => "启动中".into(),
                _ => "读不出".into(),
            },
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
        &self,
        kind: &str,
        local: &str,
        agents: &[Agent],
        corral: &mut dyn Corral,
        at: f64,
    ) -> Result<String> {
        let config = self
            .configs
            .iter()
            .find(|c| c.kind == kind)
            .expect("active adapter");
        let mut channels = config.channels.clone();
        channels.sort_by_key(|c| c.repo != local);
        let mut lines = Vec::new();
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
                if let Some(line) = self.agent_summary(&agent.name, corral, at) {
                    lines.push(line);
                }
            }
        }
        Ok(lines.join("\n"))
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
            if matches!(input.event, Event::Message(_) | Event::Deleted { .. })
                && !self.connected.contains(&input.kind)
            {
                continue;
            }
            match input.event {
                Event::Message(m) => self.message(&input.kind, m, corral, at, false)?,
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
            let mut response = None;
            let mut notices = Vec::new();
            if stable && !status.paused && (waiting.is_some() || status.turn_started != watch.turn)
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
                            notices.push(format!(
                                "{}{} 新回复\n{text}",
                                if offline {
                                    "（pony 不在线时）"
                                } else {
                                    ""
                                },
                                agent.name
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
                let text = format!(
                    "⚠ {} 在等你（权限框或提问）{}{}。要回电脑上处理；在这里回的话，等它处理完再送。",
                    agent.name,
                    status
                        .last_tool
                        .as_ref()
                        .map(|s| format!(" · 最后的工具 {s}"))
                        .unwrap_or_default(),
                    status
                        .title
                        .as_ref()
                        .map(|s| format!(" · 标题「{s}」"))
                        .unwrap_or_default()
                );
                notices.push(text);
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
            for text in notices {
                self.notice(&agent.name, &status.instance, &text, at)?;
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
    fn notice(&mut self, name: &str, instance: &str, text: &str, at: f64) -> Result<()> {
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
            address.thread = match self.post(&address, text, true, true) {
                Ok(id) => id,
                Err(error) => {
                    self.record_error("delivery_failed", error)?;
                    continue;
                }
            };
            self.state.threads.push(Thread {
                address,
                agent: name.into(),
                instance: instance.into(),
                created: at,
                used: at,
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
fn corral_error(e: crate::corral::Failure) -> Error {
    Error::new(&e.code, e.message)
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
