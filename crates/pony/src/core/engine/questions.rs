use super::super::state::QuestionForm;
use super::*;
use crate::chats::{Action, Button, ButtonStyle, Choice};
use std::io::Read;

pub(super) const TEXT_ANSWER: &str = "已改用线程里的文字回答，选项已撤下";

fn button(label: String) -> Result<Button> {
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|_| Error::new("random_unavailable", "不能生成按钮记号"))?;
    Ok(Button {
        value: bytes.iter().map(|b| format!("{b:02x}")).collect(),
        label,
        style: ButtonStyle::Default,
        confirm: None,
    })
}

// Reserve the largest progress/result line; the full question stays in the notice.
pub(super) fn reserve(status: &Status) -> usize {
    status.questions.as_ref().map_or(128, |qs| {
        128 + qs
            .items
            .iter()
            .map(|q| {
                30 + q
                    .options
                    .iter()
                    .map(|o| o.label.chars().count() + 1)
                    .sum::<usize>()
            })
            .sum::<usize>()
    })
}

impl Engine {
    pub(super) fn question_offer(
        &self,
        address: &Address,
        status: &Status,
        at: f64,
    ) -> Result<Option<ButtonOffer>> {
        let Some(questions) = &status.questions else {
            return Ok(None);
        };
        let Some(connection) = self.connections.get(&address.kind) else {
            return Ok(None);
        };
        if !connection.limits().buttons
            || !connection.limits().threads
            || status.paused
            || status.state_started.is_none()
            || questions.truncated
            || questions.items.is_empty()
            // Two blocks per question, plus notification and the original text.
            || questions.items.len() > 24
            || questions
                .items
                .iter()
                .any(|q| q.options.is_empty() || q.options.len() > 10)
            || reserve(status) + 1000 > connection.limits().characters
        {
            return Ok(None);
        }
        let choices = questions
            .items
            .iter()
            .enumerate()
            .map(|(i, q)| {
                Ok(Choice {
                    label: format!(
                        "{}. {}（{}）",
                        i + 1,
                        q.header,
                        if q.multi_select {
                            "多选，选好后提交"
                        } else {
                            "单选"
                        }
                    ),
                    value: button(String::new())?.value,
                    options: q
                        .options
                        .iter()
                        .enumerate()
                        .map(|(j, o)| button(format!("{} {}", option_letter(j), o.label)))
                        .collect::<Result<_>>()?,
                    multi_select: q.multi_select,
                    selected: Vec::new(),
                    submit: button("提交".into())?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Some(ButtonOffer {
            // Keep the old record shape, but never render its handle. Older readers
            // cannot mistake a choice token for an interrupt button.
            button: button(String::new())?,
            status: status.clone(),
            screen: None,
            at,
            form: Some(QuestionForm {
                dirty: false,
                answered: vec![false; choices.len()],
                action_at: vec![0.; choices.len()],
                choices,
            }),
        }))
    }

    pub(super) fn question_action(
        &mut self,
        i: usize,
        action: Action,
        corral: &mut dyn Corral,
        at: f64,
    ) -> Result<()> {
        let b = &mut self.state.buttons[i];
        let Some(content) = &mut b.content else {
            return Ok(());
        };
        let Some(form) = &mut content.form else {
            return Ok(());
        };
        let Some(q) = form.choices.iter().position(|q| {
            q.value == action.value
                || q.submit.value == action.value
                || q.options.iter().any(|o| o.value == action.value)
        }) else {
            return Ok(());
        };
        if action.at <= form.action_at[q] {
            return Ok(());
        }
        let choice = &mut form.choices[q];
        if choice.multi_select {
            if action.value != choice.value && action.value != choice.submit.value {
                return Ok(());
            }
            if let Some(selected) = action.selected {
                if selected
                    .iter()
                    .any(|v| !choice.options.iter().any(|o| o.value == *v))
                {
                    return Ok(());
                }
                choice.selected = choice
                    .options
                    .iter()
                    .filter(|o| selected.contains(&o.value))
                    .map(|o| o.value.clone())
                    .collect();
            } else if action.value == choice.value {
                return Ok(());
            }
            form.answered[q] = action.value == choice.submit.value && !choice.selected.is_empty();
        } else {
            if action.selected.is_some() || !choice.options.iter().any(|o| o.value == action.value)
            {
                return Ok(());
            }
            choice.selected = vec![action.value];
            form.answered[q] = true;
        }
        form.action_at[q] = action.at;
        form.dirty = true;
        if !form.answered.iter().all(|a| *a) {
            // refresh_buttons renders once after this batch of input is processed.
            return self.save();
        }
        let text = answer(content);
        let address = b.address.clone();
        let message = Message {
            channel: address.channel.clone(),
            id: b.message.clone(),
            thread: Some(address.thread.clone()),
            sender: action.sender,
            mentioned: false,
            text,
            attachments: false,
            at: action.at,
        };
        if let Err(error) = self.route_message(&address.kind, message, corral, at, false, Some(i)) {
            self.record_error("route_failed", error)?;
            if !self.state.buttons[i].used {
                return self.finish_button(i, "读不出 agent 状态，没按 Esc", false, at);
            }
            // The answer and interrupt were persisted. Its announcement is retried
            // by the normal queue; do not replay this click or interrupt again.
        }
        Ok(())
    }

    pub(super) fn take_over_question(&mut self, address: &Address) {
        for b in &mut self.state.buttons {
            if b.address == *address
                && b.outcome.is_none()
                && b.content.as_ref().is_some_and(|c| c.form.is_some())
            {
                b.outcome = Some(TEXT_ANSWER.into());
            }
        }
    }
}

fn selected(content: &ButtonContent) -> Vec<String> {
    let (Some(form), Some(qs)) = (&content.form, &content.status.questions) else {
        return Vec::new();
    };
    form.choices
        .iter()
        .zip(&qs.items)
        .map(|(choice, q)| {
            choice
                .options
                .iter()
                .zip(&q.options)
                .filter(|(o, _)| choice.selected.contains(&o.value))
                .map(|(_, o)| o.label.as_str())
                .collect::<Vec<_>>()
                .join("、")
        })
        .collect()
}

pub(super) fn answer_summary(content: &ButtonContent) -> String {
    selected(content)
        .iter()
        .enumerate()
        .map(|(i, s)| format!("{}. {s}", i + 1))
        .collect::<Vec<_>>()
        .join(" ")
}

fn answer(content: &ButtonContent) -> String {
    let Some(qs) = &content.status.questions else {
        return String::new();
    };
    selected(content)
        .iter()
        .zip(&qs.items)
        .enumerate()
        .map(|(i, (s, q))| format!("{}. {}：{s}", i + 1, q.header))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn progress(content: &ButtonContent) -> String {
    let Some(form) = &content.form else {
        return String::new();
    };
    selected(content)
        .iter()
        .enumerate()
        .map(|(i, s)| {
            format!(
                "{}. {}{}",
                i + 1,
                if s.is_empty() { "未选" } else { s },
                if form.answered[i] {
                    "（已选，可改）"
                } else if !s.is_empty() {
                    "（未提交）"
                } else {
                    ""
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
