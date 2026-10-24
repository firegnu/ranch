use super::*;
use crate::chats::{Action, Button, ButtonStyle};
use std::io::Read;

pub(super) struct ButtonOffer {
    pub approval: Option<super::super::state::ApprovalButton>,
    pub form: Option<super::super::state::QuestionForm>,
    pub button: Button,
    pub status: Status,
    pub screen: Option<String>,
    pub at: f64,
}

impl Engine {
    pub(super) fn interrupt_notice(
        &mut self,
        address: &Address,
        status: &Status,
        corral: &mut dyn Corral,
        at: f64,
        top: bool,
    ) -> Result<()> {
        let offer = self.button_offer(address, status, "打断", corral, at)?;
        let text = format!(
            "{} 当前这一轮：{}。会话留着，只停这一轮。",
            status.name,
            if offer.is_some() {
                "点「打断」或在线程回「停」"
            } else {
                "在线程回「停」可打断"
            }
        );
        let id = self.post_with_button(address, &text, false, top, offer)?;
        if top {
            let mut address = address.clone();
            address.thread = id;
            self.state.threads.push(Thread {
                address,
                agent: status.name.clone(),
                instance: status.instance.clone(),
                created: at,
                used: at,
                question: None,
            });
            self.save()?;
        }
        Ok(())
    }

    pub(super) fn refresh_buttons(&mut self, corral: &mut dyn Corral, at: f64) -> Result<()> {
        let mut statuses = BTreeMap::new();
        for i in 0..self.state.buttons.len() {
            let b = &self.state.buttons[i];
            if !self.connected.contains(&b.address.kind) {
                continue;
            }
            if b.outcome.is_none()
                && let Some(content) = &b.content
            {
                let permitted = self.configs.iter().any(|c| {
                    c.kind == b.address.kind
                        && c.channels.iter().any(|channel| {
                            channel.id == b.address.channel
                                && (content.approval.is_none()
                                    || content
                                        .status
                                        .name
                                        .split_once('/')
                                        .is_some_and(|(repo, _)| repo == channel.repo))
                        })
                }) && content
                    .status
                    .name
                    .split_once('/')
                    .is_some_and(|(repo, _)| self.bound(&b.address.kind, repo));
                let reason = if !permitted {
                    Some("频道绑定已变，按钮已过期")
                } else {
                    let current = statuses
                        .entry(content.status.name.clone())
                        .or_insert_with(|| corral.status(&content.status.name));
                    match current {
                        Ok(s) => stale(b, s),
                        Err(e) if e.exit == Some(2) => Some("agent 已退出，框没了"),
                        Err(_) => None,
                    }
                };
                if let Some(reason) = reason {
                    let handled = statuses
                        .get(&content.status.name)
                        .and_then(|s| s.as_ref().ok())
                        .and_then(|s| kimi::handled(&content.status, s));
                    self.state.buttons[i].outcome = Some(handled.unwrap_or_else(|| reason.into()));
                    self.save()?;
                }
            }
            self.update_button(i, at)?;
        }
        self.save()
    }

    pub(super) fn button_offer(
        &self,
        address: &Address,
        status: &Status,
        label: &str,
        corral: &mut dyn Corral,
        at: f64,
    ) -> Result<Option<ButtonOffer>> {
        if !self
            .connections
            .get(&address.kind)
            .is_some_and(|c| c.limits().buttons)
            || status.paused
            || status.kind.as_deref() == Some("agy")
            || (status.state == "blocked"
                && status.kind.as_deref() == Some("kimi")
                && !status.kimi_dialog())
            || !matches!(status.state.as_str(), "working" | "blocked")
        {
            return Ok(None);
        }
        let screen = if status.state == "blocked" && status.kind.as_deref() != Some("kimi") {
            match corral.read(&status.name) {
                Ok(screen) => Some(screen),
                Err(_) => return Ok(None),
            }
        } else {
            None
        };
        let mut bytes = [0u8; 16];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut file| file.read_exact(&mut bytes))
            .map_err(|_| Error::new("random_unavailable", "不能生成按钮记号"))?;
        Ok(Some(ButtonOffer {
            approval: None,
            form: None,
            button: Button {
                value: bytes.iter().map(|b| format!("{b:02x}")).collect(),
                label: label.into(),
                style: ButtonStyle::Danger,
                confirm: None,
            },
            status: status.clone(),
            screen,
            at,
        }))
    }

    pub(super) fn button_action(
        &mut self,
        kind: &str,
        action: Action,
        corral: &mut dyn Corral,
        at: f64,
    ) -> Result<()> {
        let Some(i) = self
            .state
            .buttons
            .iter()
            .position(|b| b.address.kind == kind && b.matches(&action.value))
        else {
            if action.own_message
                && self.configs.iter().any(|c| {
                    c.kind == kind
                        && c.owner == action.sender
                        && c.channels.iter().any(|b| b.id == action.channel)
                })
            {
                return self.say(
                    &Address {
                        kind: kind.into(),
                        channel: action.channel,
                        thread: action.thread.unwrap_or(action.message),
                    },
                    "这个按钮已失效，没按 Esc；请重新查状态",
                    false,
                );
            }
            return Ok(());
        };
        let button = self.state.buttons[i].clone();
        let approval = button.approval_value.as_deref() == Some(action.value.as_str());
        let address = &button.address;
        let Some(config) = self.configs.iter().find(|c| c.kind == kind) else {
            return Ok(());
        };
        if approval
            && button.content.as_ref().is_some_and(|content| {
                !config.channels.iter().any(|channel| {
                    channel.id == address.channel
                        && content
                            .status
                            .name
                            .split_once('/')
                            .is_some_and(|(repo, _)| repo == channel.repo)
                })
            })
        {
            return self.finish_button(i, "频道绑定已变，没按", false, at);
        }
        if !config.channels.iter().any(|b| b.id == address.channel)
            || button.content.as_ref().is_some_and(|content| {
                !content
                    .status
                    .name
                    .split_once('/')
                    .is_some_and(|(repo, _)| self.bound(kind, repo))
            })
        {
            return Ok(());
        }
        let reason = if action.sender != config.owner {
            Some("只有 owner 能按这个按钮，没按 Esc")
        } else if !action.own_message
            || action.channel != address.channel
            || action.message != button.message
        {
            Some("不是 pony 发的那条消息上的按钮，没按 Esc")
        } else if button.used {
            Some("这个按钮已经按过了，没再按 Esc")
        } else if button.outcome.is_some() {
            Some("这个按钮已过期，没按 Esc")
        } else {
            None
        };
        if let Some(reason) = reason {
            let reason = if approval {
                reason.replace(" Esc", "")
            } else {
                reason.into()
            };
            if approval && !button.used && button.outcome.is_none() {
                return self.finish_button(i, &reason, false, at);
            }
            return self.say(address, &refusal(&button, &reason), false);
        }
        let Some(content) = &button.content else {
            self.state.buttons[i].finish_update();
            self.save()?;
            return self.say(
                address,
                if approval {
                    "这个按钮已过期，没按"
                } else {
                    "这个按钮已过期，没按 Esc"
                },
                false,
            );
        };
        if approval {
            return self.approval_action(i, action, corral, at);
        }
        let mut status = match corral.status(&content.status.name) {
            Ok(s) => s,
            Err(_) => return self.finish_button(i, "读不出 agent 状态，没按 Esc", false, at),
        };
        if let Some(reason) = self
            .stop_reason(&status, Some(&content.status.instance), action.at, at)
            .map(str::to_owned)
            .or_else(|| stale(&button, &status).map(|reason| format!("{reason}，没按 Esc")))
        {
            return self.finish_button(i, &reason, false, at);
        }
        if content.form.is_some() {
            return self.question_action(i, action, corral, at);
        }
        if content.status.state == "blocked" && status.kind.as_deref() != Some("kimi") {
            let screen = match corral.read(&status.name) {
                Ok(screen) => screen,
                Err(_) => return self.finish_button(i, "读不出屏幕，没按 Esc", false, at),
            };
            status = match corral.status(&content.status.name) {
                Ok(s) => s,
                Err(_) => return self.finish_button(i, "读不出 agent 状态，没按 Esc", false, at),
            };
            if let Some(reason) = self
                .stop_reason(&status, Some(&content.status.instance), action.at, at)
                .map(str::to_owned)
                .or_else(|| stale(&button, &status).map(|reason| format!("{reason}，没按 Esc")))
            {
                return self.finish_button(i, &reason, false, at);
            }
            if content.screen.as_ref() != Some(&screen) {
                return self.finish_button(
                    i,
                    "画面和发提醒时不一样，没按；要拒绝请在线程回『停』",
                    false,
                    at,
                );
            }
        }
        // Mark the whole message before the shared interrupt path persists and presses Esc.
        self.state.buttons[i].used = true;
        self.state.buttons[i].outcome = Some("已接收，Esc 是否送达待确认；没重按".into());
        let pressed = self.start_interrupt(address, &status, corral, at, false)?;
        let text = if pressed {
            format!("已按：{}（{}）", content.button.label, clock_time(at))
        } else {
            "Esc 发送失败或结果不明，没重按".into()
        };
        self.finish_button(i, &text, true, at)
    }

    pub(super) fn finish_button(
        &mut self,
        i: usize,
        text: &str,
        used: bool,
        at: f64,
    ) -> Result<()> {
        self.state.buttons[i].outcome = Some(if used {
            text.into()
        } else {
            refusal(&self.state.buttons[i], text)
        });
        self.state.buttons[i].used = used;
        self.save()?;
        self.update_button(i, at)
    }

    pub(super) fn update_button(&mut self, i: usize, at: f64) -> Result<()> {
        let b = &self.state.buttons[i];
        let active_form = b.content.as_ref().and_then(|c| c.form.as_ref());
        if b.outcome.is_none() && !active_form.is_some_and(|f| f.dirty) {
            return Ok(());
        }
        if b.updated || !self.connected.contains(&b.address.kind) {
            return Ok(());
        }
        if b.update_retry
            .as_ref()
            .is_some_and(|retry| at < retry.retry_at)
        {
            return Ok(());
        }
        let Some(content) = &b.content else {
            self.state.buttons[i].finish_update();
            return self.save();
        };
        let text = match &b.outcome {
            Some(outcome) => format!("{}\n{outcome}", content.text),
            None => format!("{}\n{}", content.text, questions::progress(content)),
        };
        let choices = if b.outcome.is_none() {
            active_form.map_or(&[][..], |f| f.choices.as_slice())
        } else {
            &[]
        };
        let result = self
            .connections
            .get_mut(&b.address.kind)
            .expect("connected adapter")
            .update(
                &b.message,
                Outgoing {
                    channel: &b.address.channel,
                    thread: Some(&b.address.thread),
                    markdown: &text,
                    notify: content.notify,
                    buttons: &[],
                    choices,
                },
            );
        match result {
            Ok(()) => {
                let b = &mut self.state.buttons[i];
                b.update_retry = None;
                if b.outcome.is_some() {
                    b.updated = true;
                } else if let Some(form) = b.content.as_mut().and_then(|c| c.form.as_mut()) {
                    form.dirty = false;
                }
            }
            Err(error) => {
                if matches!(
                    error.code.as_str(),
                    "not_found"
                        | "archived"
                        | "not_member"
                        | "message_not_found"
                        | "cant_update_message"
                ) {
                    self.state.buttons[i].updated = true;
                    if self.state.buttons[i].outcome.is_none() {
                        self.state.buttons[i].outcome = Some("消息无法更新，选项已失效".into());
                    }
                } else {
                    let retry = &mut self.state.buttons[i].update_retry;
                    let delay = retry.as_ref().map_or(2., |r| (r.delay * 2.).min(300.));
                    *retry = Some(Verification {
                        started: retry.as_ref().map_or(at, |r| r.started),
                        retry_at: at + delay,
                        delay,
                    });
                }
                self.record_adapter_error("button_update_failed", &error);
            }
        }
        if self.state.buttons[i].updated
            && self.state.buttons[i].outcome.is_some()
            && !self.confirming_kimi(&self.state.buttons[i])
        {
            self.state.buttons[i].finish_update();
        }
        self.save()
    }
}

fn refusal(button: &ButtonMessage, reason: &str) -> String {
    if !button.used && button.content.as_ref().is_some_and(|c| c.form.is_some()) {
        format!("{reason}，选项答案没送；要回答请在线程里写")
    } else {
        reason.into()
    }
}

pub(super) fn stale(button: &ButtonMessage, status: &Status) -> Option<&'static str> {
    let content = button.content.as_ref()?;
    let old = &content.status;
    if content.approval.is_some() {
        return approvals::stale(old, status);
    }
    if status.instance != old.instance {
        Some("它重开过，框没了")
    } else if status.turn_started != old.turn_started && !old.background_question() {
        Some("这一轮已结束，它已经开了新的一轮")
    } else if status.last_human_input != old.last_human_input
        || (old.state == "blocked"
            && status
                .last_human_input
                .is_some_and(|t| t > old.state_started.unwrap_or(button.created)))
    {
        Some("已在电脑上处理")
    } else if old.kind.as_deref() == Some("kimi")
        && old.state == "blocked"
        && (!status.kimi_dialog()
            || old.kind != status.kind
            || old.command != status.command
            || old.questions != status.questions
            || old.action != status.action)
    {
        Some("框已变或有别的框，回电脑处理")
    } else if old.state == "blocked"
        && (status.state != "blocked"
            || status.state_started != old.state_started
            || status.last_tool != old.last_tool
            || (button.content.as_ref()?.form.is_some()
                && (status.kind != old.kind || status.questions != old.questions)))
    {
        Some("框没了或换了一个框")
    } else if !matches!(status.state.as_str(), "working" | "blocked") {
        Some("这一轮已结束")
    } else {
        None
    }
}
