//! Kimi background answers: wait without Esc, persist once, confirm from the main wire.
use super::*;
use crate::{core::state::BackgroundAnswer, corral::QuestionKey};
use serde_json::{Value, json};

fn plan(
    status: &Status,
    text: &str,
    content: Option<&ButtonContent>,
) -> Option<(Vec<QuestionKey>, Value)> {
    let questions = status.questions.as_ref()?;
    if questions.truncated
        || questions.items.is_empty()
        || questions.items.len() > 4
        || questions
            .items
            .iter()
            .any(|q| !(2..=4).contains(&q.options.len()))
    {
        return None;
    }
    let mut keys = Vec::new();
    let mut answers = json!({});
    if let Some(form) = content.and_then(|c| c.form.as_ref()) {
        for (q, choice) in questions.items.iter().zip(&form.choices) {
            let mut selected = Vec::new();
            for (i, (option, control)) in q.options.iter().zip(&choice.options).enumerate() {
                if choice.selected.contains(&control.value) {
                    keys.push(QuestionKey::Choose((i + 1) as u8));
                    selected.push(option.label.as_str());
                }
            }
            if selected.is_empty() {
                return None;
            }
            answers[&q.question] = json!(selected.join(", "));
            if q.multi_select {
                keys.push(QuestionKey::Tab);
            }
        }
    } else {
        let text: String = text.chars().filter(|c| !c.is_control()).collect();
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        let q = &questions.items[0];
        keys.extend([
            QuestionKey::Choose((q.options.len() + 1) as u8),
            QuestionKey::Text(text.into()),
            QuestionKey::Enter,
        ]);
        answers[&q.question] = json!(text);
        for _ in 0..questions.items.len() - 1 + usize::from(q.multi_select) {
            keys.push(QuestionKey::Tab);
        }
    }
    keys.push(QuestionKey::Choose(1)); // Submit; no further keys can reach a queued modal.
    Some((keys, answers))
}

fn stale(old: &Status, current: &Status) -> Option<&'static str> {
    if old.instance != current.instance || old.name != current.name {
        Some("它重开过")
    } else if current.paused {
        Some("暂停中")
    } else if !current.kimi_dialog()
        || current.interaction.as_ref()?.kind.as_deref() != Some("question")
        || current.state_started != old.state_started
        || current.questions != old.questions
        || !current.background_question()
    {
        Some("题目已处理、已换或有别的框")
    } else if current
        .last_human_input
        .is_some_and(|t| t > old.state_started.unwrap_or(0.))
    {
        Some("已在电脑上处理")
    } else {
        None
    }
}

pub(super) fn resolution<'a>(old: &Status, current: &'a Status, kind: &str) -> Option<&'a Value> {
    let result = current.resolved.as_ref()?;
    (old.instance == current.instance
        && result["kind"] == kind
        && result["state_started"].as_f64() == old.state_started)
        .then_some(result)
}

pub(super) fn handled(old: &Status, current: &Status) -> Option<String> {
    let result = current.resolved.as_ref()?;
    if old.kind.as_deref() != Some("kimi")
        || old.instance != current.instance
        || result["state_started"].as_f64() != old.state_started
    {
        return None;
    }
    Some(format!(
        "已在电脑上处理：{}",
        match result["kind"].as_str() {
            Some("approval") => match result["decision"].as_str() {
                Some("approved") => "允许".into(),
                Some("rejected") => "拒绝".into(),
                _ => "结果不明".into(),
            },
            _ if result["answers"].is_null() => "关闭提问".into(),
            _ => format!(
                "答了 {}{}",
                result["answers"],
                if result["truncated"] == true {
                    "（结果不完整）"
                } else {
                    ""
                }
            ),
        }
    ))
}

impl Engine {
    pub(super) fn confirming_kimi(&self, button: &ButtonMessage) -> bool {
        button.content.as_ref().is_some_and(|c| {
            self.state.background_answers.iter().any(|a| {
                a.status.name == c.status.name
                    && a.status.instance == c.status.instance
                    && a.status.state_started == c.status.state_started
            }) || self.state.approvals.iter().any(|a| {
                a.status.kind.as_deref() == Some("kimi")
                    && a.status.name == c.status.name
                    && a.status.instance == c.status.instance
                    && a.status.state_started == c.status.state_started
            }) || self.state.interrupts.iter().any(|a| {
                a.require_interrupted && a.agent == c.status.name && a.instance == c.status.instance
            })
        })
    }

    pub(super) fn finish_kimi_controls(&mut self, old: &Status, outcome: &str) {
        for b in &mut self.state.buttons {
            if b.content.as_ref().is_some_and(|c| {
                c.status.name == old.name
                    && c.status.instance == old.instance
                    && c.status.state_started == old.state_started
            }) {
                b.outcome = Some(outcome.into());
                b.updated = false;
            }
        }
    }
    pub(super) fn queue_background(
        &mut self,
        address: &Address,
        message: &Message,
        status: &Status,
        button: Option<usize>,
        offline: bool,
        at: f64,
    ) -> Result<()> {
        let Some(question) = self
            .state
            .threads
            .iter()
            .find(|t| t.address == *address)
            .and_then(|t| t.question.clone())
        else {
            return Ok(());
        };
        if question.handled
            || self.state.background_answers.iter().any(|a| {
                a.status.name == status.name
                    && a.status.instance == question.instance
                    && a.status.state_started == question.state_started
            })
        {
            self.save()?;
            return self.say(
                address,
                "这道后台提问的答案已提交，没再按；请等确认结果",
                false,
            );
        }
        let mut original = status.clone();
        original.instance = question.instance.clone();
        original.state_started = question.state_started;
        original.questions = question.questions.clone();
        let reason = if offline {
            Some("离线期间的回答")
        } else {
            stale(&original, status)
        };
        let keys = plan(
            status,
            &message.text,
            button.and_then(|i| self.state.buttons[i].content.as_ref()),
        );
        if reason.is_some() || keys.is_none() || human_active(status, at) {
            let text = format!(
                "{}，后台答案没送、没按；请回电脑处理",
                reason.unwrap_or("无法核对完整题目、答案或人工输入")
            );
            if let Some(i) = button {
                return self.finish_button(i, &text, false, at);
            }
            self.save()?;
            return self.say(address, &text, false);
        }
        let (keys, answers) = keys.unwrap();
        self.state.background_answers.push(BackgroundAnswer {
            address: address.clone(),
            message: message.id.clone(),
            status: status.clone(),
            keys,
            answers,
            pressed_at: None,
        });
        for thread in &mut self.state.threads {
            if thread.agent == status.name
                && let Some(q) = &mut thread.question
                && q.instance == status.instance
                && q.state_started == status.state_started
            {
                q.handled = true;
            }
        }
        for (i, b) in self.state.buttons.iter_mut().enumerate() {
            if b.outcome.is_none()
                && b.content.as_ref().is_some_and(|c| {
                    c.status.name == status.name
                        && c.status.instance == status.instance
                        && c.status.state_started == status.state_started
                })
            {
                b.used = button == Some(i);
                b.outcome = Some("后台答案已排队，等待空闲后填写并核对".into());
            }
        }
        self.save()?;
        self.say(
            address,
            "它还在干活时先排着，空下来就替你选；交上去后核对答案",
            false,
        )
    }

    pub(super) fn poll_background(&mut self, corral: &mut dyn Corral, at: f64) -> Result<()> {
        let mut i = 0;
        while i < self.state.background_answers.len() {
            let answer = self.state.background_answers[i].clone();
            if !self.connected.contains(&answer.address.kind) {
                i += 1;
                continue;
            }
            let current = corral.status(&answer.status.name);
            let outcome = if let Some(pressed) = answer.pressed_at {
                match &current {
                    Ok(s) if resolution(&answer.status, s, "question").is_some() => {
                        let result = resolution(&answer.status, s, "question").unwrap();
                        Some(if result["truncated"] == true {
                            "交上的答案不完整，无法核对；请回电脑看".into()
                        } else if result["answers"] == answer.answers {
                            format!("已答：{}（{}）", answer.answers, clock_time(at))
                        } else {
                            format!(
                                "框里交上去的是 {}，和你选的不一样；没再按，请回电脑看",
                                result["answers"]
                            )
                        })
                    }
                    _ if at - pressed < 10. => None,
                    _ => Some("10 秒内没确认后台答案是否交上去，没重按；请回电脑看".into()),
                }
            } else {
                let allowed = self.configs.iter().any(|c| {
                    c.kind == answer.address.kind
                        && c.channels.iter().any(|ch| {
                            ch.id == answer.address.channel
                                && answer.status.name.starts_with(&format!("{}/", ch.repo))
                        })
                });
                match current {
                    _ if !allowed => Some("频道绑定已变，后台答案没送".into()),
                    Ok(s) if stale(&answer.status, &s).is_some() => Some(format!(
                        "{}，后台答案没送、没按；请回电脑处理",
                        stale(&answer.status, &s).unwrap()
                    )),
                    Ok(s)
                        if s.interaction.as_ref().is_some_and(|v| !v.running)
                            && !human_active(&s, at)
                            && !self.state.interrupts.iter().any(|v| v.agent == s.name) =>
                    {
                        // Verify the source still exists, then re-read immediately before keys.
                        let exists = self
                            .connections
                            .get_mut(&answer.address.kind)
                            .unwrap()
                            .exists(
                                &answer.address.channel,
                                Some(&answer.address.thread),
                                &answer.message,
                            );
                        match exists {
                            Ok(false) => Some("回答已撤回，没按".into()),
                            Err(_) => None,
                            Ok(true) => match corral.status(&s.name) {
                                Ok(s)
                                    if stale(&answer.status, &s).is_none()
                                        && !human_active(&s, at)
                                        && s.interaction.as_ref().is_some_and(|v| !v.running) =>
                                {
                                    self.state.background_answers[i].pressed_at = Some(at);
                                    self.save()?;
                                    if let Err(error) =
                                        corral.answer_question(&s.name, &answer.keys)
                                    {
                                        self.log.push(format!("corral {}", safe_code(&error.code)));
                                        self.say(
                                            &answer.address,
                                            "后台回答按键是否送达不明，没重按；继续核对",
                                            false,
                                        )?;
                                    }
                                    None
                                }
                                _ => None,
                            },
                        }
                    }
                    Err(e) if e.exit == Some(2) => Some("agent 已退出，后台答案没送".into()),
                    _ => None,
                }
            };
            if let Some(outcome) = outcome {
                self.state.background_answers.remove(i);
                self.finish_kimi_controls(&answer.status, &outcome);
                self.save()?;
                self.say(&answer.address, &outcome, true)?;
            } else {
                i += 1;
            }
        }
        self.save()
    }
}
