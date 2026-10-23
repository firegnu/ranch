use super::*;
use crate::{
    chats::{Action, Button, ButtonStyle, Confirmation},
    core::state::{ApprovalButton, ApprovalCheck},
};
use std::io::Read;

impl Engine {
    pub(super) fn poll_approvals(&mut self, corral: &mut dyn Corral, at: f64) -> Result<()> {
        let mut i = 0;
        while i < self.state.approvals.len() {
            let check = self.state.approvals[i].clone();
            if !self.connected.contains(&check.address.kind) || at < check.poll_at {
                i += 1;
                continue;
            }
            let status = corral.status(&check.status.name);
            let result = match status {
                Ok(s) if s.instance != check.status.instance => {
                    Some("它重开过，没确认原框的反应，没再按；请回电脑看看")
                }
                Ok(s)
                    if at - check.started <= 10.
                        && s.last_output.is_some()
                        && s.last_output != check.status.last_output
                        && corral.read(&s.name).is_ok_and(|screen| {
                            !screen.trim().is_empty()
                                && screen != check.screen
                                && !contains_frame(&screen, &check.screen)
                                && recognize(&check.status, &screen)
                                    != recognize(&check.status, &check.screen)
                        }) =>
                {
                    Some("画面已变化，原来的框已不在屏幕尾部")
                }
                _ if at - check.started < 10. => None,
                _ if check.sent => Some("按了，没看到它有反应，没再按；请回电脑看看"),
                _ => Some("批准按键是否送达不明，没确认它有反应，没再按；请回电脑看看"),
            };
            let Some(result) = result else {
                self.state.approvals[i].poll_at = at + 1.;
                i += 1;
                continue;
            };
            self.state.approvals.remove(i);
            self.save()?;
            self.say(
                &check.address,
                &format!("{} {result}", check.status.name),
                true,
            )?;
        }
        self.save()
    }

    pub(super) fn approval_action(
        &mut self,
        i: usize,
        action: Action,
        corral: &mut dyn Corral,
        at: f64,
    ) -> Result<()> {
        let b = self.state.buttons[i].clone();
        let Some(content) = &b.content else {
            return Ok(());
        };
        let Some(approval) = &content.approval else {
            return self.finish_button(i, "批准按钮已失效，没按", false, at);
        };
        let mut status = match corral.status(&content.status.name) {
            Ok(s) => s,
            Err(_) => return self.finish_button(i, "读不出 agent 状态，没按", false, at),
        };
        if let Some(reason) = stale(&content.status, &status)
            .or_else(|| {
                (action.selected.is_some() || action.at < b.created).then_some("点击内容或时间不对")
            })
            .or_else(|| {
                self.state
                    .interrupts
                    .iter()
                    .any(|a| a.agent == status.name)
                    .then_some("已经按过 Esc，正在等它停下")
            })
            .or_else(|| {
                self.state
                    .approvals
                    .iter()
                    .any(|a| a.status.name == status.name)
                    .then_some("已经发过批准按键，正在看反应")
            })
        {
            return self.finish_button(i, &format!("{reason}，没按"), false, at);
        }
        let screen = match corral.read(&status.name) {
            Ok(s) => s,
            Err(_) => return self.finish_button(i, "读不出屏幕，没按", false, at),
        };
        // Recheck after read as well: a human or the process may have moved on while reading.
        status = match corral.status(&status.name) {
            Ok(s) => s,
            Err(_) => return self.finish_button(i, "读不出 agent 状态，没按", false, at),
        };
        if let Some(reason) = stale(&content.status, &status) {
            return self.finish_button(i, &format!("{reason}，没按"), false, at);
        }
        if content.screen.as_ref() != Some(&screen)
            || !recognize(&status, &screen)
                .is_some_and(|p| p.option == approval.option && p.label == approval.button.label)
        {
            return self.finish_button(
                i,
                "画面和发提醒时不一样或认不出框，没按；请回电脑处理",
                false,
                at,
            );
        }
        self.state.buttons[i].used = true;
        self.state.buttons[i].outcome = Some("已接收，批准按键是否送达待确认；没重按".into());
        // Invalidate other messages for this instance before keys, including older interrupt buttons.
        let siblings: Vec<_> = self
            .state
            .buttons
            .iter()
            .enumerate()
            .filter_map(|(j, other)| {
                (other.outcome.is_none()
                    && other.content.as_ref().is_some_and(|c| {
                        c.status.name == status.name && c.status.instance == status.instance
                    }))
                .then_some(j)
            })
            .collect();
        for j in &siblings {
            self.state.buttons[*j].outcome =
                Some("已从另一条消息接收批准，按键是否送达待确认；没重按".into());
        }
        self.state.approvals.push(ApprovalCheck {
            address: b.address.clone(),
            status: status.clone(),
            screen,
            started: at,
            poll_at: at + 1.,
            sent: false,
        });
        self.save()?;
        let result = corral.approve(&status.name, approval.option);
        self.state.approvals.last_mut().expect("just inserted").sent = result.is_ok();
        let text = if let Err(error) = result {
            self.log.push(format!("corral {}", safe_code(&error.code)));
            "批准按键发送失败或结果不明，没重按；请回电脑看看".into()
        } else {
            format!("已按：{}（{}）", approval.button.label, clock_time(at))
        };
        for j in siblings {
            self.state.buttons[j].outcome = Some(text.clone());
        }
        self.finish_button(i, &text, true, at)
    }
}

pub(super) fn stale(old: &Status, status: &Status) -> Option<&'static str> {
    if status.instance != old.instance || status.name != old.name {
        Some("它重开过")
    } else if status.paused {
        Some("暂停中")
    } else if status.kind.as_deref() != Some("claude") || old.kind != status.kind {
        Some("这家的框不能批准")
    } else if status.state != "blocked" {
        Some("框已经不在了")
    } else if old.state_started.is_none()
        || status.state_started != old.state_started
        || status.last_tool != old.last_tool
        || status.turn_started != old.turn_started
    {
        Some("换了一个框或无法核对等待")
    } else if status.last_human_input != old.last_human_input
        || status
            .last_human_input
            .is_some_and(|t| t > old.state_started.unwrap_or(0.))
    {
        Some("你刚在电脑上操作过")
    } else if old.last_output.is_none() || status.last_output != old.last_output {
        Some("它又有输出，画面可能变了")
    } else {
        None
    }
}

pub(super) fn offer(status: &Status, screen: Option<&str>) -> Result<Option<ApprovalButton>> {
    if status.kind.as_deref() != Some("claude")
        || status.state != "blocked"
        || status.paused
        || status.state_started.is_none()
        || status.last_output.is_none()
        || status.name.chars().count() + "允许  这一次？".chars().count() > 100
        || status
            .last_human_input
            .is_some_and(|t| t > status.state_started.unwrap_or(0.))
    {
        return Ok(None);
    }
    let Some(prompt) = screen.and_then(|s| recognize(status, s)) else {
        return Ok(None);
    };
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|_| Error::new("random_unavailable", "不能生成按钮记号"))?;
    Ok(Some(ApprovalButton {
        option: prompt.option,
        button: Button {
            value: bytes.iter().map(|b| format!("{b:02x}")).collect(),
            label: prompt.label.into(),
            style: ButtonStyle::Primary,
            confirm: Some(Confirmation {
                title: format!("允许 {} 这一次？", status.name),
                text: format!(
                    "{}\n{}\n读屏会吞掉空格，看不清请回电脑",
                    status.last_tool.as_deref().unwrap_or_default(),
                    prompt.command.chars().take(200).collect::<String>()
                ),
                confirm: "允许".into(),
                cancel: "算了".into(),
            }),
        },
    }))
}

fn compact(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

// Extra status output after the old frame is not proof that the frame disappeared.
fn contains_frame(screen: &str, original: &str) -> bool {
    let lines: Vec<_> = original.lines().collect();
    let Some(start) = lines
        .iter()
        .rposition(|line| RULES.iter().any(|rule| compact(line) == rule.header))
    else {
        return true;
    };
    compact(screen).contains(&compact(&lines[start..].join("\n")))
}

// corral read is a text stream, not a screen. Require exactly one complete frame;
// never infer a safe option from a partial "Yes" or the currently selected row.
struct Rule {
    tool: &'static str,
    header: &'static str,
    separators: usize,
    fixed_lines: &'static [&'static str],
    safe: &'static str,
    label: &'static str,
}

// Claude Code 2.1.309: Bash and plans with a body (R47c live samples).
// 2.1.308: ExitPlanMode without a plan file (R42t recorded sample).
const RULES: &[Rule] = &[
    Rule {
        tool: "Bash",
        header: "Bashcommand",
        separators: 2,
        fixed_lines: &[
            "Thiscommandrequiresapproval",
            "Doyouwanttoproceed?",
            "Esctocancel·Tabtoamend",
        ],
        safe: "Yes",
        label: "允许这一次",
    },
    Rule {
        tool: "ExitPlanMode",
        header: "Readytocode?",
        separators: 2,
        fixed_lines: &[
            "HereisClaude'splan:",
            "Claudehaswrittenupaplanandisreadytoexecute.Wouldyouliketoproceed?",
            "shift+tabtoapprovewiththisfeedback",
        ],
        safe: "Yes,manuallyapproveedits",
        label: "批准，之后的修改仍逐个问",
    },
    Rule {
        tool: "ExitPlanMode",
        header: "Exitplanmode?",
        separators: 0,
        fixed_lines: &["Claudewantstoexitplanmode"],
        safe: "Yes,andswitchtodefault(askeachtime)forthissession",
        label: "批准，之后的修改仍逐个问",
    },
];

#[derive(PartialEq)]
struct Prompt {
    option: u8,
    command: String,
    label: &'static str,
}

fn separator(line: &str) -> bool {
    line.chars().count() >= 3 && line.chars().all(|c| c == '╌')
}

fn recognize(status: &Status, screen: &str) -> Option<Prompt> {
    let lines: Vec<_> = screen
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let compact_lines: Vec<_> = lines.iter().map(|line| compact(line)).collect();
    let mut headers = compact_lines
        .iter()
        .enumerate()
        .filter_map(|(i, s)| RULES.iter().find(|r| s == r.header).map(|r| (i, r)));
    let (start, rule) = headers.next()?;
    // Count across the entire window: embedded headers must not hide command text,
    // and old frame remnants also make the current frame ambiguous.
    if headers.next().is_some()
        || status.kind.as_deref() != Some("claude")
        || status.last_tool.as_deref() != Some(rule.tool)
        || lines.iter().filter(|line| separator(line)).count() != rule.separators
        || RULES.iter().any(|known| {
            known.fixed_lines.iter().any(|fixed| {
                compact_lines.iter().filter(|line| line == fixed).count()
                    != usize::from(known.header == rule.header)
            })
        })
        || compact_lines
            .iter()
            .filter(|line| line.starts_with("ctrl+gtoeditin"))
            .count()
            != usize::from(rule.header == "Readytocode?")
    {
        return None;
    }
    let frame = &lines[start + 1..];
    if rule.header == "Exitplanmode?" {
        if compact(frame.first()?) != "Claudewantstoexitplanmode" {
            return None;
        }
        let option = menu(&frame[1..], rule, "No")?;
        return Some(Prompt {
            option,
            command: frame[0].into(),
            label: rule.label,
        });
    }
    let separators: Vec<_> = frame
        .iter()
        .enumerate()
        .filter(|(_, s)| separator(s))
        .map(|(i, _)| i)
        .collect();
    if separators.len() != 2 || separators[1] <= separators[0] + 1 {
        return None;
    }
    let command = frame[separators[0] + 1..separators[1]].join("\n");
    if command.chars().count() > 1000 || command.contains('…') || command.contains("...") {
        return None;
    }
    let tail = &frame[separators[1] + 1..];
    let option = if rule.tool == "Bash" {
        if compact(tail.first()?) != "Thiscommandrequiresapproval"
            || compact(tail.get(1)?) != "Doyouwanttoproceed?"
            || compact(tail.last()?) != "Esctocancel·Tabtoamend"
        {
            return None;
        }
        menu(&tail[2..tail.len() - 1], rule, "No")?
    } else {
        if compact(frame.first()?) != "HereisClaude'splan:"
            || !tail.first()?.chars().all(|c| c == '─')
            || compact(tail.get(1)?)
                != "Claudehaswrittenupaplanandisreadytoexecute.Wouldyouliketoproceed?"
            || compact(tail.get(tail.len().checked_sub(2)?)?)
                != "shift+tabtoapprovewiththisfeedback"
        {
            return None;
        }
        let footer = compact(tail.last()?);
        let (_, path) = footer
            .strip_prefix("ctrl+gtoeditin")?
            .split_once("·~/.claude/plans/")?;
        if !path.strip_suffix(".md").is_some_and(|s| {
            !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        }) {
            return None;
        }
        menu(&tail[2..tail.len() - 2], rule, "TellClaudewhattochange")?
    };
    Some(Prompt {
        option,
        command,
        label: rule.label,
    })
}

fn menu(options: &[&str], rule: &Rule, cancel: &str) -> Option<u8> {
    if !(2..=9).contains(&options.len()) {
        return None;
    }
    let mut selected = 0;
    let mut safe = None;
    for (i, line) in options.iter().enumerate() {
        let line = compact(line);
        let row = if let Some(row) = line.strip_prefix('❯') {
            selected += 1;
            row
        } else {
            &line
        };
        let label = row.strip_prefix(&format!("{}.", i + 1))?;
        if i + 1 == options.len() {
            if label != cancel {
                return None;
            }
        } else if label == rule.safe {
            if safe.replace((i + 1) as u8).is_some() {
                return None;
            }
        } else if !(rule.tool == "Bash"
            && (label.starts_with("Yes,anddon’taskagainfor:")
                || label == "Yes,andswitchtoautomode·automodehandlesthesepromptsforyou")
            || rule.header == "Readytocode?" && label == "Yes,anduseautomode")
        {
            return None;
        }
    }
    if selected == 1 { safe } else { None }
}
