use super::*;
use crate::chats::{Action, Button, ButtonStyle};
use std::io::Read;

pub(super) struct ButtonOffer {
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
            if b.outcome.is_none() {
                let permitted = self.configs.iter().any(|c| {
                    c.kind == b.address.kind
                        && c.channels
                            .iter()
                            .any(|channel| channel.id == b.address.channel)
                }) && b
                    .status
                    .name
                    .split_once('/')
                    .is_some_and(|(repo, _)| self.bound(&b.address.kind, repo));
                let reason = if !permitted {
                    Some("频道绑定已变，按钮已过期，没按 Esc")
                } else {
                    let current = statuses
                        .entry(b.status.name.clone())
                        .or_insert_with(|| corral.status(&b.status.name));
                    match current {
                        Ok(s) => stale(b, s),
                        Err(e) if e.exit == Some(2) => Some("agent 已退出，框没了，没按 Esc"),
                        Err(_) => None,
                    }
                };
                if let Some(reason) = reason {
                    self.state.buttons[i].outcome = Some(reason.into());
                    self.save()?;
                }
            }
            self.update_button(i)?;
        }
        // Match thread retention. Unknown older tokens remain unable to press keys.
        self.state
            .buttons
            .retain(|b| !b.updated || at - b.created <= 30. * 86400.);
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
            || !matches!(status.state.as_str(), "working" | "blocked")
        {
            return Ok(None);
        }
        let mut bytes = [0u8; 16];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut file| file.read_exact(&mut bytes))
            .map_err(|_| Error::new("random_unavailable", "不能生成按钮记号"))?;
        Ok(Some(ButtonOffer {
            button: Button {
                value: bytes.iter().map(|b| format!("{b:02x}")).collect(),
                label: label.into(),
                style: ButtonStyle::Danger,
                confirm: None,
            },
            status: status.clone(),
            screen: (status.state == "blocked")
                .then(|| corral.read(&status.name).ok())
                .flatten(),
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
            .position(|b| b.address.kind == kind && b.button.value == action.value)
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
        let address = &button.address;
        let Some(config) = self.configs.iter().find(|c| c.kind == kind) else {
            return Ok(());
        };
        if !config.channels.iter().any(|b| b.id == address.channel)
            || !button
                .status
                .name
                .split_once('/')
                .is_some_and(|(repo, _)| self.bound(kind, repo))
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
            return self.say(address, reason, false);
        }
        let mut status = match corral.status(&button.status.name) {
            Ok(s) => s,
            Err(_) => return self.finish_button(i, "读不出 agent 状态，没按 Esc", false),
        };
        if let Some(reason) = self
            .stop_reason(&status, Some(&button.status.instance), action.at, at)
            .or_else(|| stale(&button, &status))
        {
            return self.finish_button(i, reason, false);
        }
        if status.state == "blocked" {
            match corral.read(&status.name) {
                Ok(screen) if button.screen.as_ref() == Some(&screen) => {}
                Ok(_) => return self.finish_button(i, "框没了或画面变了，没按 Esc", false),
                Err(_) => return self.finish_button(i, "读不出屏幕，没按 Esc", false),
            }
            status = match corral.status(&button.status.name) {
                Ok(s) => s,
                Err(_) => return self.finish_button(i, "读不出 agent 状态，没按 Esc", false),
            };
            if let Some(reason) = self
                .stop_reason(&status, Some(&button.status.instance), action.at, at)
                .or_else(|| stale(&button, &status))
            {
                return self.finish_button(i, reason, false);
            }
        }
        // Mark the whole message before the shared interrupt path persists and presses Esc.
        self.state.buttons[i].used = true;
        self.state.buttons[i].outcome = Some("已接收，Esc 是否送达待确认；没重按".into());
        let pressed = self.start_interrupt(address, &status, corral, at, false)?;
        let text = if pressed {
            format!("已按：{}（{}）", button.button.label, clock_time(at))
        } else {
            "Esc 发送失败或结果不明，没重按".into()
        };
        self.finish_button(i, &text, true)
    }

    fn finish_button(&mut self, i: usize, text: &str, used: bool) -> Result<()> {
        self.state.buttons[i].outcome = Some(text.into());
        self.state.buttons[i].used = used;
        self.save()?;
        self.update_button(i)
    }

    fn update_button(&mut self, i: usize) -> Result<()> {
        let b = &self.state.buttons[i];
        let Some(outcome) = &b.outcome else {
            return Ok(());
        };
        if b.updated || !self.connected.contains(&b.address.kind) {
            return Ok(());
        }
        let text = format!("{}\n{outcome}", b.text);
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
                    notify: b.notify,
                    buttons: &[],
                },
            );
        match result {
            Ok(()) => self.state.buttons[i].updated = true,
            Err(error) => self.record_adapter_error("button_update_failed", &error),
        }
        self.save()
    }
}

fn stale(button: &ButtonMessage, status: &Status) -> Option<&'static str> {
    let old = &button.status;
    if status.instance != old.instance {
        Some("它重开过，框没了，没按 Esc")
    } else if status.turn_started != old.turn_started {
        Some("它已经开了新的一轮，没按 Esc")
    } else if status.last_human_input != old.last_human_input
        || (old.state == "blocked"
            && status
                .last_human_input
                .is_some_and(|t| t > old.state_started.unwrap_or(button.created)))
    {
        Some("已在电脑上处理，没按 Esc")
    } else if old.state == "blocked"
        && (status.state != "blocked"
            || status.state_started != old.state_started
            || status.last_tool != old.last_tool)
    {
        Some("框没了或换了一个框，没按 Esc")
    } else if !matches!(status.state.as_str(), "working" | "blocked") {
        Some("这一轮已经结束，没按 Esc")
    } else {
        None
    }
}
