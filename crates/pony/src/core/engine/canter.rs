//! canter's request-addressed answers; chat adapters only see ordinary controls.
use super::*;
use crate::{chats::Action, corral::Answer};

const UNKNOWN: &str = "回答结果不明，没重答；请回电脑看看";
pub(super) const UNVERIFIABLE: &str = "无法核对 canter 的框，没答；请回电脑处理";

pub(super) fn unverifiable(status: &Status) -> bool {
    matches!(status.state.as_str(), "unknown" | "starting" | "exiting")
}

pub(super) fn request_gone(old: &Status, current: &Status) -> bool {
    old.instance != current.instance
        || old.name != current.name
        || (!unverifiable(current)
            && (current.state != "blocked"
                || current
                    .canter_request()
                    .is_some_and(|r| Some(&r.id) != old.canter_request().map(|r| &r.id))))
}

pub(super) fn stale(old: &Status, current: &Status) -> Option<&'static str> {
    if unverifiable(current) {
        Some(UNVERIFIABLE)
    } else if old.instance != current.instance || old.name != current.name {
        Some("它重开过，没答")
    } else if current.paused {
        Some("暂停中，没答")
    } else if old.canter_request().is_none()
        || old.state_started.is_none()
        || current.kind != old.kind
    {
        Some("无法核对 canter 的框，没按；请回电脑处理")
    } else if current.state != "blocked"
        || current
            .canter_request()
            .is_some_and(|r| Some(&r.id) != old.canter_request().map(|r| &r.id))
    {
        Some("已经在别处答过，或已换了一个框，没再答")
    } else if current.canter_request().is_none() {
        Some("无法核对 canter 的框，没按；请回电脑处理")
    } else if current.last_human_input != old.last_human_input
        || current
            .last_human_input
            .is_some_and(|t| t > old.state_started.unwrap_or(0.))
    {
        Some("你刚在电脑上操作过，没答；请回电脑处理")
    } else if old.canter_request() != current.canter_request()
        || old.questions != current.questions
        || old.command != current.command
    {
        Some("框的内容变了，没答；请回电脑处理")
    } else {
        None
    }
}

impl Engine {
    // A newer status means the question disappeared before submission: route as chat.
    pub(super) fn canter_text_answer(
        &mut self,
        address: &Address,
        question: &QuestionContext,
        message: &Message,
        corral: &mut dyn Corral,
        offline: bool,
        at: f64,
    ) -> Result<Option<Status>> {
        let old = question.api_status.as_ref().expect("canter question");
        let reason = if question.handled {
            Some("这道题已处理，没送第二份答案；等收到回复后再聊")
        } else if offline {
            Some("离线期间发的回答，没答；请回电脑处理")
        } else if message.text.contains(['\r', '\n']) {
            Some("canter 只收一句话回答，这句没送；请改成一行")
        } else if old.questions.as_ref().is_none_or(|q| q.truncated) {
            Some("题目不完整，没答；请回电脑处理")
        } else {
            None
        };
        if let Some(reason) = reason {
            self.save()?;
            return self.say(address, reason, false).map(|_| None);
        }
        let option = old
            .questions
            .as_ref()
            .and_then(|qs| qs.items.first())
            .and_then(|q| {
                q.options.iter().enumerate().find(|(i, option)| {
                    message.text == option.label || message.text == option_letter(*i)
                })
            });
        let answer = option.map_or_else(
            || Answer::Text(message.text.clone()),
            |(_, option)| Answer::Option(option.label.clone()),
        );
        self.submit_canter(address, old, answer, corral, at)
    }

    pub(super) fn canter_button_action(
        &mut self,
        i: usize,
        action: Action,
        corral: &mut dyn Corral,
        at: f64,
    ) -> Result<()> {
        let b = self.state.buttons[i].clone();
        let content = b.content.as_ref().unwrap();
        let old = &content.status;
        if action.at < b.created || action.selected.is_some() {
            return self.finish_button(i, "点击内容或时间不对，没答", false, at);
        }
        let answer = if content.approval.is_some() {
            // The current safe offer is rechecked before calling this path's API.
            if approvals::offer(old, None)?.is_none() {
                return self.finish_button(i, "无法核对批准内容，没答；请回电脑处理", false, at);
            }
            if old
                .canter_request()
                .is_some_and(|r| r.kind == "planApproval")
            {
                Answer::ApprovePlan
            } else {
                Answer::AllowOnce
            }
        } else {
            let Some(choice) = content.form.as_ref().and_then(|f| f.choices.first()) else {
                return Ok(());
            };
            let Some(index) = choice.options.iter().position(|o| o.value == action.value) else {
                return Ok(());
            };
            let Some(label) = old
                .questions
                .as_ref()
                .and_then(|qs| qs.items.first())
                .and_then(|q| q.options.get(index))
                .map(|o| o.label.clone())
            else {
                return Ok(());
            };
            Answer::Option(label)
        };
        self.submit_canter(&b.address, old, answer, corral, at)
            .map(|_| ())
    }

    fn submit_canter(
        &mut self,
        address: &Address,
        old: &Status,
        answer: Answer,
        corral: &mut dyn Corral,
        at: f64,
    ) -> Result<Option<Status>> {
        if !self.configs.iter().any(|config| {
            config.kind == address.kind
                && config.channels.iter().any(|channel| {
                    channel.id == address.channel
                        && old
                            .name
                            .split_once('/')
                            .is_some_and(|(repo, _)| repo == channel.repo)
                })
        }) {
            return self.say(address, "频道绑定已变，没答", false).map(|_| None);
        }
        let current = corral.status(&old.name);
        let reason = match &current {
            Ok(s) => stale(old, s).or_else(|| {
                human_active(s, at).then_some("你刚在电脑上操作过，没答；请回电脑处理")
            }),
            Err(_) => Some("读不出 agent 状态，没答；请回电脑处理"),
        };
        let controls: Vec<_> = self
            .state
            .buttons
            .iter()
            .enumerate()
            .filter_map(|(i, b)| {
                (b.outcome.is_none()
                    && b.content.as_ref().is_some_and(|c| {
                        c.status.name == old.name && c.status.instance == old.instance
                    }))
                .then_some(i)
            })
            .collect();
        if let Some(reason) = reason {
            for i in controls {
                if self.state.buttons[i]
                    .content
                    .as_ref()
                    .is_some_and(|c| c.status.canter_request() == old.canter_request())
                {
                    self.finish_button(i, reason, false, at)?;
                }
            }
            self.say(address, reason, false)?;
            return Ok(current.ok().filter(|s| request_gone(old, s)));
        }
        if self.state.interrupts.iter().any(|i| i.agent == old.name) {
            return self
                .say(address, "已发过打断，没答；请回电脑处理", false)
                .map(|_| None);
        }
        for thread in &mut self.state.threads {
            if thread.agent == old.name
                && thread.instance == old.instance
                && let Some(q) = &mut thread.question
                && q.api_status
                    .as_ref()
                    .is_some_and(|s| s.canter_request() == old.canter_request())
            {
                q.handled = true;
            }
        }
        for i in &controls {
            let b = &mut self.state.buttons[*i];
            b.used = true;
            b.outcome = Some(UNKNOWN.into());
        }
        // Crash, duplicate click and reconnect all see a consumed receipt before the call.
        self.save()?;
        let request = old.canter_request().unwrap();
        let outcome = match corral.resolve_request(&old.name, &old.instance, &request.id, &answer) {
            Ok(receipt) if receipt.already_resolved => {
                "这个框已经处理过（别处答过或已取消），没再答".into()
            }
            Ok(_) => match answer {
                Answer::Option(text) => format!("已答：{text}（{}）", clock_time(at)),
                Answer::Text(text) => format!(
                    "已答：{}{}（{}）",
                    text.chars().take(200).collect::<String>(),
                    if text.chars().count() > 200 {
                        "…（全文见线程）"
                    } else {
                        ""
                    },
                    clock_time(at)
                ),
                Answer::AllowOnce => format!("已允许这一次（{}）", clock_time(at)),
                Answer::ApprovePlan => format!("已批准，之后的操作仍逐个问（{}）", clock_time(at)),
            },
            Err(e) => {
                self.log.push(format!("corral {}", safe_code(&e.code)));
                match e.code.as_str() {
                    "state_check_failed" => UNVERIFIABLE,
                    "request_changed" => "已经在别处答过，或已换了一个框，没再答",
                    "unsupported" | "bad_op" | "usage" => {
                        "无法核对 canter 的框，没按；请回电脑处理"
                    }
                    "answer_failed" => "canter 拒绝了这次回答，没答上；请回电脑处理",
                    "human_active" => "你刚在电脑上操作过，没答；请回电脑处理",
                    "paused" | "answer_busy" | "restarted" => "当前无法回答，没答；请回电脑处理",
                    _ => UNKNOWN,
                }
                .into()
            }
        };
        for i in controls {
            self.finish_button(i, &outcome, true, at)?;
        }
        self.say(address, &outcome, false).map(|_| None)
    }
}
