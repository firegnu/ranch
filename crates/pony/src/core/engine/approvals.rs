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
                Ok(s) if check.status.kind.as_deref() == Some("kimi") => {
                    match kimi::resolution(&check.status, &s, "approval")
                        .and_then(|r| r["decision"].as_str())
                    {
                        Some("approved") => Some("已按：允许这一次"),
                        Some("rejected") => Some("框的实际结果是拒绝，没再按；请回电脑看"),
                        _ if at - check.started < 10. => None,
                        _ => Some("10 秒内没确认批准结果，没重按；请回电脑看"),
                    }
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
            if check.status.kind.as_deref() == Some("kimi") {
                self.finish_kimi_controls(
                    &check.status,
                    &format!("{result}（{}）", clock_time(at)),
                );
            }
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
        let screen = if status.kind.as_deref() == Some("kimi") {
            String::new()
        } else {
            match corral.read(&status.name) {
                Ok(s) => s,
                Err(_) => return self.finish_button(i, "读不出屏幕，没按", false, at),
            }
        };
        // Recheck after read as well: a human or the process may have moved on while reading.
        status = match corral.status(&status.name) {
            Ok(s) => s,
            Err(_) => return self.finish_button(i, "读不出 agent 状态，没按", false, at),
        };
        if let Some(reason) = stale(&content.status, &status) {
            return self.finish_button(i, &format!("{reason}，没按"), false, at);
        }
        if if status.kind.as_deref() == Some("kimi") {
            kimi_prompt(&status).is_none() || approval.option != 1
        } else {
            content.screen.as_ref() != Some(&screen)
                || !recognize(&status, &screen).is_some_and(|p| {
                    p.option == approval.option && p.label == approval.button.label
                })
        } {
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
        } else if status.kind.as_deref() == Some("kimi") {
            "批准按键已发送，正在确认结果；没重按".into()
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
    } else if !matches!(status.kind.as_deref(), Some("claude" | "kimi")) || old.kind != status.kind
    {
        Some("这家的框不能批准")
    } else if status.state != "blocked" {
        Some("框已经不在了")
    } else if old.state_started.is_none()
        || status.state_started != old.state_started
        || status.last_tool != old.last_tool
        || status.turn_started != old.turn_started
    {
        Some("换了一个框或无法核对等待")
    } else if old.last_tool.as_deref() == Some("Bash")
        && (old.command.as_ref().is_none_or(|c| c.truncated) || status.command != old.command)
    {
        Some("原始命令变了或无法核对")
    } else if status.last_human_input != old.last_human_input
        || status
            .last_human_input
            .is_some_and(|t| t > old.state_started.unwrap_or(0.))
    {
        Some("你刚在电脑上操作过")
    } else if status.kind.as_deref() == Some("kimi")
        && (!status.kimi_dialog() || kimi_prompt(status).is_none())
    {
        Some("框已变、有别的框或命令不适合远程批准")
    } else if status.kind.as_deref() != Some("kimi")
        && (old.last_output.is_none() || status.last_output != old.last_output)
    {
        Some("它又有输出，画面可能变了")
    } else {
        None
    }
}

pub(super) fn offer(status: &Status, screen: Option<&str>) -> Result<Option<ApprovalButton>> {
    if !matches!(status.kind.as_deref(), Some("claude" | "kimi" | "canter"))
        || status.state != "blocked"
        || status.paused
        || status.state_started.is_none()
        || (!matches!(status.kind.as_deref(), Some("kimi" | "canter"))
            && status.last_output.is_none())
        || status.name.chars().count() + "允许  这一次？".chars().count() > 100
        || status
            .last_human_input
            .is_some_and(|t| t > status.state_started.unwrap_or(0.))
    {
        return Ok(None);
    }
    let Some(prompt) = (if status.kind.as_deref() == Some("canter") {
        canter_prompt(status)
    } else if status.kind.as_deref() == Some("kimi") {
        kimi_prompt(status)
    } else {
        screen.and_then(|s| recognize(status, s))
    }) else {
        return Ok(None);
    };
    let length = prompt.command.chars().count();
    let note = if status.last_tool.as_deref() != Some("Bash")
        && status.kind.as_deref() != Some("canter")
    {
        "\n读屏会吞掉空格，看不清请回电脑".into()
    } else if length > 200 {
        format!("\n未显示全，共 {length} 字；看不全请回电脑")
    } else {
        String::new()
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
                    "{}\n{}{}",
                    status.last_tool.as_deref().unwrap_or_default(),
                    prompt.command.chars().take(200).collect::<String>(),
                    note
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

pub(super) fn canter_file_change(status: &Status) -> Option<(String, Option<&'static str>)> {
    let request = status.canter_request()?;
    let (change, label) = match request.tool.as_deref()? {
        "write" => (request.content.as_ref(), "写入内容"),
        "edit" => (request.edits.as_ref(), "替换内容（oldText → newText）"),
        _ => return None,
    };
    let file = request.file.as_ref();
    let text = format!(
        "路径：{}\n{label}：\n{}",
        file.map_or("无法核对", |f| f.text.as_str()),
        change.map_or("无法核对", |c| if c.text.is_empty() {
            "（空内容）"
        } else {
            &c.text
        }),
    );
    let reason = match (file, change) {
        (Some(file), Some(change)) if !file.text.trim().is_empty() => {
            if file.truncated || change.truncated {
                Some("内容不完整，没给允许按钮；请回电脑处理")
            } else if file.text.chars().count() + change.text.chars().count() > 1000 {
                Some("路径加改动内容超过 1000 字，没给允许按钮；请回电脑处理")
            } else if !readable_text(&text) {
                Some("路径或改动内容含不可读字符，没给允许按钮；请回电脑处理")
            } else if request.tool.as_deref() == Some("edit") {
                let edits = serde_json::from_str::<serde_json::Value>(&change.text).ok();
                match edits.as_ref().and_then(|v| v.as_array()) {
                    Some(edits)
                        if !edits.is_empty()
                            && edits.iter().all(|e| {
                                e["oldText"].as_str().is_some_and(|s| !s.is_empty())
                                    && e["newText"].is_string()
                            }) =>
                    {
                        (!edits.iter().all(|e| {
                            ["oldText", "newText"]
                                .iter()
                                .all(|key| readable_text(e[key].as_str().unwrap()))
                        }))
                        .then_some("改动内容含不可读字符，没给允许按钮；请回电脑处理")
                    }
                    _ => Some("改动内容无法核对，没给允许按钮；请回电脑处理"),
                }
            } else {
                None
            }
        }
        _ => Some("路径或改动内容无法核对，没给允许按钮；请回电脑处理"),
    };
    Some((text, reason))
}

fn canter_prompt(status: &Status) -> Option<Prompt> {
    let request = status.canter_request()?;
    let (detail, label) = match request.kind.as_str() {
        "approval" if request.available_decisions.iter().any(|d| d == "accept") => {
            let detail = match request.tool.as_deref()? {
                "shell" => status.command.as_ref()?,
                "write" | "edit" => {
                    let (text, reason) = canter_file_change(status)?;
                    return reason.is_none().then_some(Prompt {
                        option: 0,
                        command: text,
                        label: "允许这一次",
                    });
                }
                _ => return None,
            };
            (detail, "允许这一次")
        }
        "planApproval" => (request.plan.as_ref()?, "批准，之后的操作仍逐个问"),
        _ => return None,
    };
    if detail.truncated
        || detail.text.trim().is_empty()
        || detail.text.chars().count() > 1000
        || if request.kind == "planApproval" {
            !readable_text(&detail.text)
        } else {
            !readable_command(&detail.text)
        }
    {
        return None;
    }
    Some(Prompt {
        option: 0,
        command: detail.text.clone(),
        label,
    })
}

fn kimi_prompt(status: &Status) -> Option<Prompt> {
    if !status.kimi_dialog()
        || status.interaction.as_ref()?.kind.as_deref() != Some("approval")
        || status.last_tool.as_deref() != Some("Bash")
    {
        return None;
    }
    let command = status.command.as_ref()?;
    if command.truncated
        || command.text.trim().is_empty()
        || command.text.chars().count() > 1000
        || !readable_command(&command.text)
        || dangerous_kimi_command(&command.text)
    {
        return None;
    }
    Some(Prompt {
        option: 1,
        command: command.text.clone(),
        label: "允许这一次",
    })
}

// Conservative equivalents of Kimi 2.3.1's approval-panel danger labels. Quoted
// mentions may also be refused; this is a display gate, not a shell security policy.
fn dangerous_kimi_command(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let words: Vec<_> = lower
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .collect();
    let word = |w: &str| words.contains(&w);
    (word("rm")
        && (lower.contains("--recursive")
            || lower.contains("--force")
            || lower
                .split_whitespace()
                .any(|s| s.starts_with('-') && (s.contains('r') || s.contains('f')))))
        || word("sudo")
        || word("mkfs")
        || (word("dd") && lower.contains("of="))
        || (word("chmod") && word("777"))
        || ((word("curl") || word("wget"))
            && lower.contains('|')
            && (word("sh") || word("bash") || word("zsh")))
        || lower.split('>').skip(1).any(|tail| {
            ["/dev/sd", "/dev/nvme", "/dev/disk", "/dev/hd"]
                .iter()
                .any(|p| tail.trim_start().starts_with(p))
        })
        || compact(&lower).contains(":(){:|:&}")
}

fn readable_command(text: &str) -> bool {
    if text
        .split(|c: char| !c.is_whitespace())
        .any(|run| run.chars().count() > 4)
    {
        return false;
    }
    readable_text(text)
}

fn readable_text(text: &str) -> bool {
    // Unicode 17.0 General_Category=Cf, including supplementary-plane formats.
    // https://www.unicode.org/Public/17.0.0/ucd/extracted/DerivedGeneralCategory.txt
    !text.chars().any(|c| {
        (c.is_control() && !matches!(c, '\n' | '\t'))
            || matches!(c,
                '\u{00ad}' | '\u{0600}'..='\u{0605}' | '\u{061c}' | '\u{06dd}'
                | '\u{070f}' | '\u{0890}'..='\u{0891}' | '\u{08e2}' | '\u{180e}'
                | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}'
                | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{206f}' | '\u{feff}'
                | '\u{fff9}'..='\u{fffb}' | '\u{110bd}' | '\u{110cd}'
                | '\u{13430}'..='\u{1343f}' | '\u{1bca0}'..='\u{1bca3}'
                | '\u{1d173}'..='\u{1d17a}' | '\u{e0001}' | '\u{e0020}'..='\u{e007f}')
    })
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
        fixed_lines: &["Doyouwanttoproceed?", "Esctocancel·Tabtoamend"],
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
    let approval_notices = compact_lines
        .iter()
        .filter(|line| *line == "Thiscommandrequiresapproval")
        .count();
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
        || approval_notices > usize::from(rule.tool == "Bash")
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
    let mut command = frame[separators[0] + 1..separators[1]].join("\n");
    if command.chars().count() > 1000 || command.contains('…') || command.contains("...") {
        return None;
    }
    let tail = &frame[separators[1] + 1..];
    let option = if rule.tool == "Bash" {
        let raw = status.command.as_ref()?;
        if raw.truncated || !readable_command(&raw.text) || compact(&raw.text) != compact(&command)
        {
            return None;
        }
        command.clone_from(&raw.text);
        // The notice is optional, but any occurrence must be here, not in old output
        // or embedded in the command. Keep counting it across the entire window.
        let notice = usize::from(compact(tail.first()?) == "Thiscommandrequiresapproval");
        if approval_notices != notice
            || compact(tail.get(notice)?) != "Doyouwanttoproceed?"
            || compact(tail.last()?) != "Esctocancel·Tabtoamend"
        {
            return None;
        }
        menu(&tail[notice + 1..tail.len() - 1], rule, "No")?
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
        } else if !(rule.tool == "Bash" && label.starts_with("Yes,")
            || rule.header == "Readytocode?" && label == "Yes,anduseautomode")
        {
            return None;
        }
    }
    if selected == 1 { safe } else { None }
}
