use super::{
    Error, Result,
    config::ChatConfig,
    state::{Address, Cursor, Pending, Phase, State, Store, Thread, Watch},
};
use crate::{
    chats::{ChannelInfo, Connection, ConnectionInfo, Event, Message, Outgoing},
    corral::{Agent, Corral, Status},
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub struct Engine {
    pub state: State,
    pub store: Store,
    pub configs: Vec<ChatConfig>,
    pub connections: BTreeMap<String, Box<dyn Connection>>,
    pub reports: BTreeMap<String, ConnectionInfo>,
    pub channels: BTreeMap<(String, String), ChannelInfo>,
    pub log: Vec<String>,
    previous: BTreeMap<String, Status>,
    recovering: BTreeSet<String>,
    connected: BTreeSet<String>,
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
            connections,
            log: Vec::new(),
            reports: BTreeMap::new(),
            channels: BTreeMap::new(),
            previous: BTreeMap::new(),
            recovering,
            connected: BTreeSet::new(),
        }
    }
    fn save(&self) -> Result<()> {
        self.store.save(&self.state, None)
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
                    let report =
                        self.reports
                            .entry(address.kind.clone())
                            .or_insert(ConnectionInfo {
                                connection: "failed".into(),
                                reason: None,
                                message: None,
                                workspace: None,
                                bot: None,
                                other_connections: 0,
                            });
                    report.connection = "failed".into();
                    report.reason = Some(code.clone());
                    report.message = Some(format!("第 {}/{total} 段发不出去：{code}", i + 1));
                    if !notify {
                        return Err(Error::new("delivery_failed", code));
                    }
                    let explanation = report.message.as_deref().unwrap();
                    let id = connection
                        .send(Outgoing {
                            channel: &address.channel,
                            thread,
                            markdown: explanation,
                            notify: notify && first.is_none(),
                        })
                        .map_err(|_| Error::new("delivery_failed", "cannot post delivery error"))?;
                    first.get_or_insert(id);
                }
            }
        }
        first.ok_or_else(|| Error::new("delivery_failed", "no message was posted"))
    }
    fn remember(&mut self, kind: &str, m: &Message) {
        self.state
            .seen
            .insert((kind.into(), m.channel.clone(), m.id.clone()));
        let mut addresses = vec![Address {
            kind: kind.into(),
            channel: m.channel.clone(),
            thread: String::new(),
        }];
        addresses.push(Self::address(kind, m));
        for address in addresses {
            if let Some(c) = self.state.cursors.iter_mut().find(|c| c.address == address) {
                if m.at > c.at {
                    c.at = m.at;
                    c.id = m.id.clone();
                }
            } else {
                self.state.cursors.push(Cursor {
                    address,
                    id: m.id.clone(),
                    at: m.at,
                });
            }
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
        let result = self.route_message(kind, m, corral, at, offline);
        if result.is_err() {
            self.state = self.store.state();
        }
        result
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
            .contains(&(kind.into(), m.channel.clone(), m.id.clone()))
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
                self.post(&address, "这个频道没绑仓库", false, false)?;
            }
            return Ok(());
        };
        if self.state.expired.contains(&address) {
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
                "用法：@机器人 正文，或 @机器人 agent/全名 正文；? 看状态",
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
        let count = self
            .state
            .pending
            .iter()
            .filter(|p| p.agent == name && p.phase == Phase::Queued)
            .count()
            + 1;
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
        self.save()
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
                let s = match corral.status(&agent.name) {
                    Ok(s) => s,
                    Err(e) if e.exit == Some(2) => continue,
                    Err(e) => return Err(corral_error(e)),
                };
                let label = if s.paused {
                    "暂停中".into()
                } else {
                    match s.state.as_str() {
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
                    }
                };
                let count = self
                    .state
                    .pending
                    .iter()
                    .filter(|p| p.agent == agent.name && p.phase == Phase::Queued)
                    .count();
                lines.push(format!(
                    "• {} — {label}{}",
                    agent.name.split_once('/').unwrap().1,
                    if count > 0 {
                        format!("，排着 {count} 句")
                    } else {
                        String::new()
                    }
                ));
            }
        }
        Ok(lines.join("\n"))
    }
    pub fn step(&mut self, corral: &mut dyn Corral, at: f64) -> Result<()> {
        self.state.threads.retain(|t| {
            if at - t.used > 30. * 86400. {
                self.state.expired.insert(t.address.clone());
                false
            } else {
                true
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
            self.post(
                &p.address,
                "上次发送结果不明，没重送，请确认后重发",
                false,
                false,
            )?;
            self.log.push("send_uncertain".into());
            self.state
                .pending
                .retain(|q| q.address != p.address || q.message != p.message);
            self.save()?;
        }
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
                        self.backfill(&input.kind, corral, at)?;
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
                        self.post(&p.address, "撤回了", false, false)?;
                    }
                }
            }
            self.store.save(&self.state, Some(input.sequence))?;
        }
        if self.connected.is_empty() {
            return self.save();
        }
        for i in 0..self.state.pending.len() {
            let pending = self.state.pending[i].clone();
            if self.connected.contains(&pending.address.kind)
                && let Some(text) = pending.announcement
            {
                self.post(&pending.address, &text, false, false)?;
                self.state.pending[i].announcement = None;
                self.save()?;
            }
        }
        let agents: Vec<Agent> = corral
            .list()
            .map_err(corral_error)?
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
                Err(e) => return Err(corral_error(e)),
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
                    Err(e) => return Err(corral_error(e)),
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
                            let address = self.state.pending[i].address.clone();
                            self.post(&address, text, true, false)?;
                            self.state.pending.remove(i);
                        } else if agent.name.ends_with("/main")
                            && watch.reply.is_none_or(|old| reply.at > old)
                        {
                            self.notice(
                                &agent.name,
                                &status.instance,
                                &format!(
                                    "{}{} 新回复\n{text}",
                                    if offline {
                                        "（pony 不在线时）"
                                    } else {
                                        ""
                                    },
                                    agent.name
                                ),
                                at,
                            )?;
                        }
                        watch.reply = Some(reply.at);
                    }
                    Ok(_) => {
                        continue;
                    }
                    Err(e) if e.code == "no_reply" => {
                        if let Some(i) = waiting {
                            let address = self.state.pending[i].address.clone();
                            self.post(
                                &address,
                                &if e.interrupted {
                                    format!("{} 这一轮被打断了，没有回复", agent.name)
                                } else {
                                    "（空回复）".into()
                                },
                                true,
                                false,
                            )?;
                            self.state.pending.remove(i);
                        }
                    }
                    Err(e) => return Err(corral_error(e)),
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
                self.notice(&agent.name, &status.instance, &text, at)?;
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
            self.previous.insert(agent.name, status);
            self.save()?;
        }
        let mut i = 0;
        let mut attempted = BTreeSet::new();
        while i < self.state.pending.len() {
            let p = self.state.pending[i].clone();
            if p.phase != Phase::Queued
                || self.state.pending[..i]
                    .iter()
                    .any(|earlier| earlier.agent == p.agent)
                || !self.connected.contains(&p.address.kind)
                || p.retry_at > at
                || attempted.contains(&p.agent)
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
                Err(e) => return Err(corral_error(e)),
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
                        self.post(
                            &p.address,
                            &format!("已送给 {}（排了 {} 分钟）", p.agent, minutes(at - p.at)),
                            false,
                            false,
                        )?;
                    }
                    if sent.merged_with_draft {
                        self.post(
                            &p.address,
                            "输入框里原有没提交的字，和你的话一起送了",
                            false,
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
                            i += 1;
                            match e.exit {
                                Some(8) => "人刚在窗口里操作过，30 秒后再试".into(),
                                Some(10) => "暂停中，恢复后再送".into(),
                                _ => "正在干活，放回队首排着".into(),
                            }
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
                    self.post(&p.address, &text, false, false)?;
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
        for p in &removed {
            if !self.connected.contains(&p.address.kind) {
                continue;
            }
            let text = if p.phase == Phase::Waiting {
                format!("{name} 退出了，没等到回复")
            } else {
                format!("{name} 已经不在了，没送")
            };
            self.post(&p.address, &text, false, false)?;
        }
        self.state.pending.retain(|p| {
            !(p.agent == name
                && self.connected.contains(&p.address.kind)
                && instance.is_none_or(|i| p.instance != i))
        });
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
            address.thread = self.post(&address, text, true, true)?;
            self.state.threads.push(Thread {
                address,
                agent: name.into(),
                instance: instance.into(),
                created: at,
                used: at,
            });
        }
        Ok(())
    }
    fn backfill(&mut self, kind: &str, corral: &mut dyn Corral, at: f64) -> Result<()> {
        let Some(config) = self.configs.iter().find(|c| c.kind == kind).cloned() else {
            return Ok(());
        };
        let mut messages = Vec::new();
        for channel in config.channels {
            let info = self
                .connections
                .get_mut(kind)
                .unwrap()
                .channel(&channel.id)?;
            self.channels
                .insert((kind.into(), channel.id.clone()), info);
            let Some(cursor) = self
                .state
                .cursors
                .iter()
                .find(|c| {
                    c.address.kind == kind
                        && c.address.channel == channel.id
                        && c.address.thread.is_empty()
                })
                .cloned()
            else {
                continue;
            };
            let mut requests = vec![(None, cursor.clone())];
            for thread in self
                .state
                .threads
                .iter()
                .filter(|t| t.address.kind == kind && t.address.channel == channel.id)
            {
                let cursor = self
                    .state
                    .cursors
                    .iter()
                    .find(|c| c.address == thread.address)
                    .cloned()
                    .unwrap_or_else(|| Cursor {
                        address: thread.address.clone(),
                        id: thread.address.thread.clone(),
                        at: thread.created,
                    });
                requests.push((Some(thread.address.thread.clone()), cursor));
            }
            for (thread, cursor) in requests {
                let found = self.connections.get_mut(kind).unwrap().history(
                    &channel.id,
                    thread.as_deref(),
                    &cursor.id,
                )?;
                messages.extend(found.into_iter().filter(|m| {
                    m.sender == config.owner && m.channel == channel.id && m.at > cursor.at
                }));
            }
        }
        messages.sort_by(|a, b| a.at.total_cmp(&b.at).then_with(|| a.id.cmp(&b.id)));
        for message in messages {
            self.message(kind, message, corral, at, true)?;
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
fn name_segment(s: &str) -> bool {
    s.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
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
