use pony::{
    chats::*,
    core::{
        self,
        config::{Channel, ChatConfig, Problem},
        engine::Engine,
        state::Store,
    },
    corral::{self, Agent, Corral, Reply, Sent, Status},
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

#[derive(Clone, Debug)]
struct Posted {
    channel: String,
    thread: Option<String>,
    text: String,
    notify: bool,
    id: String,
}
#[derive(Default)]
struct Memory {
    posted: Vec<Posted>,
    history: Vec<Message>,
    history_reads: Vec<(String, HistoryAfter)>,
    fail_over: Option<usize>,
    fail_after: Option<usize>,
    channel_errors: BTreeSet<String>,
    channel_info: BTreeMap<String, ChannelInfo>,
    channel_reads: Vec<String>,
    history_errors: BTreeSet<String>,
    deleted: BTreeSet<String>,
    exists_errors: BTreeSet<String>,
    exists_reads: Vec<String>,
    adapter_error_code: Option<String>,
}
struct MemoryConnection(Rc<RefCell<Memory>>, usize, bool);
impl Connection for MemoryConnection {
    fn limits(&self) -> Limits {
        Limits {
            characters: self.1,
            threads: self.2,
        }
    }
    fn send(&mut self, m: Outgoing<'_>) -> Result<String, SendError> {
        let mut data = self.0.borrow_mut();
        if data.fail_after.is_some_and(|n| data.posted.len() >= n) {
            return Err(SendError {
                code: "network".into(),
                resize: false,
            });
        }
        if data
            .fail_over
            .is_some_and(|n| m.markdown.chars().count() > n)
        {
            return Err(SendError {
                code: "too_long".into(),
                resize: true,
            });
        }
        let id = format!("bot-{}", data.posted.len());
        data.posted.push(Posted {
            channel: m.channel.into(),
            thread: m.thread.map(str::to_owned),
            text: m.markdown.into(),
            notify: m.notify,
            id: id.clone(),
        });
        Ok(id)
    }
    fn channel(&mut self, id: &str) -> core::Result<ChannelInfo> {
        self.0.borrow_mut().channel_reads.push(id.into());
        if self.0.borrow().channel_errors.contains(id) {
            return Err(core::Error::new(
                self.0
                    .borrow()
                    .adapter_error_code
                    .as_deref()
                    .unwrap_or("not_member"),
                "not a member",
            ));
        }
        if let Some(info) = self.0.borrow().channel_info.get(id) {
            return Ok(info.clone());
        }
        Ok(ChannelInfo {
            id: id.into(),
            name: Some(id.to_lowercase()),
            private: Some(true),
            problem: None,
        })
    }
    fn history(&mut self, channel: &str, after: &HistoryAfter) -> core::Result<Vec<Message>> {
        let mut data = self.0.borrow_mut();
        data.history_reads.push((channel.into(), after.clone()));
        if data.history_errors.contains(channel) {
            return Err(core::Error::new(
                data.adapter_error_code
                    .as_deref()
                    .unwrap_or("history_failed"),
                "cannot read history",
            ));
        }
        Ok(data
            .history
            .iter()
            .filter(|m| m.channel == channel && m.at > after.at)
            .cloned()
            .collect())
    }
    fn exists(&mut self, _: &str, _: Option<&str>, id: &str) -> core::Result<bool> {
        let mut data = self.0.borrow_mut();
        data.exists_reads.push(id.into());
        if data.exists_errors.contains(id) {
            return Err(core::Error::new(
                data.adapter_error_code.as_deref().unwrap_or("network"),
                "cannot check message",
            ));
        }
        Ok(!data.deleted.contains(id))
    }
}
// Test-only registration exercises the same Chat boundary as production.
struct MemoryChat;
impl Chat for MemoryChat {
    fn kind(&self) -> &'static str {
        "memory"
    }
    fn secret_keys(&self) -> &'static [&'static str] {
        &[]
    }
    fn read_config(&self, t: &toml::Table) -> Result<ChatConfig, Problem> {
        ChatConfig::parse(self.kind(), t)
    }
    fn connect(&self, _: Connect) -> core::Result<Box<dyn Connection>> {
        Ok(Box::new(MemoryConnection(Rc::default(), 10_000, true)))
    }
}
static TEST_CHATS: &[&dyn Chat] = &[&MemoryChat];

#[derive(Default)]
struct FakeCorral {
    agents: BTreeMap<String, Status>,
    replies: BTreeMap<String, Result<Reply, corral::Failure>>,
    sent: Vec<(String, String)>,
    status_reads: Vec<String>,
    outcomes: Vec<Result<Sent, corral::Failure>>,
    status_errors: BTreeMap<String, corral::Failure>,
}
impl Corral for FakeCorral {
    fn list(&mut self) -> corral::Result<Vec<Agent>> {
        Ok(self
            .agents
            .values()
            .map(|s| Agent {
                name: s.name.clone(),
                instance: Some(s.instance.clone()),
            })
            .collect())
    }
    fn status(&mut self, name: &str) -> corral::Result<Status> {
        self.status_reads.push(name.into());
        if let Some(error) = self.status_errors.get(name) {
            return Err(error.clone());
        }
        self.agents
            .get(name)
            .cloned()
            .ok_or_else(|| failure(2, "not_found"))
    }
    fn reply(&mut self, name: &str) -> corral::Result<Reply> {
        self.replies
            .get(name)
            .cloned()
            .unwrap_or_else(|| Err(failure(1, "no_reply")))
    }
    fn send(&mut self, name: &str, text: &str) -> corral::Result<Sent> {
        self.sent.push((name.into(), text.into()));
        if self.outcomes.is_empty() {
            Ok(Sent::default())
        } else {
            self.outcomes.remove(0)
        }
    }
}
fn failure(exit: i32, code: &str) -> corral::Failure {
    corral::Failure {
        exit: Some(exit),
        code: code.into(),
        message: "reason".into(),
        interrupted: false,
    }
}
fn status(name: &str, state: &str) -> Status {
    Status {
        name: name.into(),
        instance: "instance-1".into(),
        state: state.into(),
        state_started: Some(1.),
        turn_started: Some(1.),
        last_event: Some("Stop".into()),
        last_event_at: Some(1.),
        last_tool: None,
        title: None,
        paused: false,
        interrupted: false,
    }
}
struct Fixture {
    _dir: tempfile::TempDir,
    engine: Engine,
    corral: FakeCorral,
    memory: Rc<RefCell<Memory>>,
    events: Events,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let config = ChatConfig {
            kind: TEST_CHATS[0].kind().into(),
            owner: "owner".into(),
            channels: vec![
                Channel {
                    id: "C1".into(),
                    repo: "paddock".into(),
                },
                Channel {
                    id: "C2".into(),
                    repo: "ranch".into(),
                },
            ],
        };
        let events = store.events(&config.kind, &config.owner);
        let memory = Rc::new(RefCell::new(Memory::default()));
        let connections = BTreeMap::from([(
            "memory".into(),
            Box::new(MemoryConnection(memory.clone(), 10_000, true)) as Box<dyn Connection>,
        )]);
        let mut corral = FakeCorral::default();
        for name in ["paddock/main", "paddock/dev", "ranch/main", "private/main"] {
            corral.agents.insert(name.into(), status(name, "idle"));
        }
        let fixture = Self {
            _dir: dir,
            engine: Engine::new(store, vec![config], connections),
            corral,
            memory,
            events,
        };
        fixture.connected();
        fixture
    }
    fn step(&mut self, at: f64) {
        self.engine.step(&mut self.corral, at).unwrap();
    }
    fn message(&mut self, id: &str, thread: Option<&str>, text: &str, mentioned: bool) {
        self.events
            .emit(Event::Message(Message {
                channel: "C1".into(),
                id: id.into(),
                thread: thread.map(str::to_owned),
                sender: "owner".into(),
                mentioned,
                text: text.into(),
                attachments: false,
                at: 10.,
            }))
            .unwrap();
    }
    fn texts(&self) -> String {
        self.memory
            .borrow()
            .posted
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn binding() -> Self {
        use std::os::unix::fs::PermissionsExt;
        let mut f = Self::new();
        std::fs::set_permissions(f._dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        // Use the registered configuration parser with the in-memory transport only.
        let kind = CHATS[0].kind();
        std::fs::write(
            f._dir.path().join("config.toml"),
            format!("# keep this comment\n[{kind}]\nowner = 'UOWNER'\nchannels = [] # bindings\n"),
        )
        .unwrap();
        let (config, file) = core::config::load_with_source(f._dir.path())
            .unwrap()
            .unwrap();
        let store = Store::open(f._dir.path()).unwrap();
        f.events = store.events(kind, "UOWNER");
        f.engine = Engine::new(
            store,
            config.chats,
            BTreeMap::from([(
                kind.into(),
                Box::new(MemoryConnection(f.memory.clone(), 10_000, true)) as Box<dyn Connection>,
            )]),
        );
        f.engine.config_file = Some(file);
        f.connected();
        f
    }

    fn binding_message(&self, id: &str, text: &str) {
        self.events
            .emit(Event::Message(Message {
                channel: "CNEW".into(),
                id: id.into(),
                thread: None,
                sender: "UOWNER".into(),
                mentioned: true,
                text: text.into(),
                attachments: false,
                at: 10.,
            }))
            .unwrap();
    }

    fn reload_binding_config(&mut self) {
        let (config, file) = core::config::load_with_source(self._dir.path())
            .unwrap()
            .unwrap();
        self.engine.configs = config.chats;
        self.engine.config_file = Some(file);
    }

    fn binding_reply(&self, thread: &str, text: &str) {
        self.events
            .emit(Event::Message(Message {
                channel: "CNEW".into(),
                id: "followup".into(),
                thread: Some(thread.into()),
                sender: "UOWNER".into(),
                mentioned: false,
                text: text.into(),
                attachments: false,
                at: 12.,
            }))
            .unwrap();
    }
}

#[test]
fn binding_writes_config_then_reload_routes_the_next_message() {
    let mut f = Fixture::binding();
    f.step(2.);
    f.binding_message("bind", "绑 ranch");
    f.step(10.);
    assert_eq!(f.texts(), "已绑 ranch\n• main — 空闲");
    {
        let memory = f.memory.borrow();
        let posted = &memory.posted[0];
        assert_eq!(posted.channel, "CNEW");
        assert_eq!(posted.thread.as_deref(), Some("bind"));
    }
    assert!(f.corral.sent.is_empty());
    assert!(f.engine.configs[0].channels.is_empty());

    let config = core::config::load(f._dir.path()).unwrap().unwrap();
    assert_eq!(config.chats[0].channels[0].id, "CNEW");
    assert_eq!(config.chats[0].channels[0].repo, "ranch");
    f.engine.disconnect(CHATS[0].kind());
    f.reload_binding_config();
    f.connected();
    f.step(12.);
    f.binding_message("next", "请检查代码");
    f.step(14.);
    assert_eq!(
        f.corral.sent,
        vec![("ranch/main".into(), "请检查代码".into())]
    );
}

#[test]
fn binding_thread_followup_without_mention_reaches_main_and_reply_returns() {
    let mut f = Fixture::binding();
    f.binding_message("bind", "绑 ranch");
    f.step(10.);
    assert_eq!(f.texts(), "已绑 ranch\n• main — 空闲");
    f.reload_binding_config();
    f.binding_reply("bind", "回复 收到 即可");
    f.step(12.);
    assert_eq!(
        f.corral.sent,
        vec![("ranch/main".into(), "回复 收到 即可".into())]
    );
    assert_eq!(f.memory.borrow().posted[1].text, "→ ranch/main");

    let s = f.corral.agents.get_mut("ranch/main").unwrap();
    s.turn_started = Some(13.);
    s.last_event_at = Some(14.);
    f.corral.replies.insert(
        "ranch/main".into(),
        Ok(Reply {
            instance: "instance-1".into(),
            text: "收到".into(),
            at: 14.,
        }),
    );
    f.step(14.);
    f.step(16.);
    let memory = f.memory.borrow();
    assert_eq!(memory.posted.len(), 3);
    assert_eq!(memory.posted[2].text, "收到");
    assert!(memory.posted[2].notify);
    for posted in &memory.posted {
        assert_eq!(posted.channel, "CNEW");
        assert_eq!(posted.thread.as_deref(), Some("bind"));
    }
}

#[test]
fn binding_thread_without_main_handles_followup_before_or_after_main_starts() {
    for started in [true, false] {
        let mut f = Fixture::binding();
        f.corral.agents.remove("ranch/main");
        f.binding_message("bind", "绑 ranch");
        f.step(10.);
        assert_eq!(f.texts(), "已绑 ranch\nranch/main 没在跑");
        f.reload_binding_config();
        if started {
            f.corral
                .agents
                .insert("ranch/main".into(), status("ranch/main", "idle"));
        }
        f.binding_reply("bind", "回复 收到 即可");
        f.step(12.);
        if started {
            assert_eq!(
                f.corral.sent,
                vec![("ranch/main".into(), "回复 收到 即可".into())]
            );
        } else {
            assert!(f.corral.sent.is_empty());
        }
        let memory = f.memory.borrow();
        assert_eq!(memory.posted.len(), 2);
        assert_eq!(
            memory.posted[1].text,
            if started {
                "→ ranch/main"
            } else {
                "ranch/main 没在跑，没送"
            }
        );
        assert_eq!(memory.posted[1].channel, "CNEW");
        assert_eq!(memory.posted[1].thread.as_deref(), Some("bind"));
    }
}

#[test]
fn rejected_binding_thread_followup_without_mention_is_ignored() {
    let mut f = Fixture::binding();
    f.binding_message("refused", "绑 invalid/repo");
    f.step(8.);
    assert!(f.texts().contains("仓库名不合规则，没绑"));
    assert!(f.corral.sent.is_empty());
    // Bind successfully in another thread so an erroneous mapping would route the reply.
    f.binding_message("bind", "绑 ranch");
    f.step(10.);
    assert!(f.texts().contains("已绑 ranch"));
    f.reload_binding_config();
    let before = f.texts();
    f.binding_reply("refused", "回复 收到 即可");
    f.step(12.);
    assert!(f.corral.sent.is_empty());
    assert_eq!(f.texts(), before);
}

#[test]
fn binding_requires_a_confirmed_private_channel() {
    for (private, error, expected) in [
        (
            Some(false),
            false,
            "这是公开频道，没绑：主控的回复会带仓库代码和路径，整个工作区都看得到。请建私有频道再绑",
        ),
        (None, false, "查不到这个频道是不是私有的，没绑"),
        (Some(true), true, "查不到这个频道是不是私有的，没绑"),
    ] {
        let mut f = Fixture::binding();
        let path = f._dir.path().join("config.toml");
        let before = std::fs::read(&path).unwrap();
        f.memory.borrow_mut().channel_info.insert(
            "CNEW".into(),
            ChannelInfo {
                id: "CNEW".into(),
                name: Some("new".into()),
                private,
                problem: None,
            },
        );
        if error {
            f.memory.borrow_mut().channel_errors.insert("CNEW".into());
            f.memory.borrow_mut().adapter_error_code = Some("io".into());
        }
        f.binding_message("bind", "绑 ranch");
        f.step(10.);
        assert!(f.texts().contains(expected), "{}", f.texts());
        assert_eq!(std::fs::read(path).unwrap(), before);
        assert!(f.corral.sent.is_empty());
        assert!(f.corral.status_reads.is_empty());
        assert_eq!(f.memory.borrow().posted[0].thread.as_deref(), Some("bind"));
        assert!(f.engine.log.contains(&format!(
            "bind_failed {}",
            if error {
                "io"
            } else if private == Some(false) {
                "public_channel"
            } else {
                "channel_unknown"
            }
        )));
    }
}

#[test]
fn binding_failure_logs_only_one_sanitized_error_code() {
    for (code, expected) in [
        ("not_member", "not_member"),
        ("private diagnostic\ncredential-value", "adapter_error"),
    ] {
        let mut f = Fixture::binding();
        f.memory.borrow_mut().channel_errors.insert("CNEW".into());
        f.memory.borrow_mut().adapter_error_code = Some(code.into());
        f.binding_message("bind", "绑 ranch");
        f.step(10.);
        assert_eq!(
            f.engine
                .log
                .iter()
                .filter(|line| line.starts_with("bind_failed"))
                .cloned()
                .collect::<Vec<_>>(),
            vec![format!("bind_failed {expected}")]
        );
        assert!(!f.engine.log.join("\n").contains("credential-value"));
        assert!(!f.texts().contains(code));
    }
}

#[test]
fn binding_rejects_invalid_repository_names_without_writing() {
    for repo in [".hidden", "-repo", "repo/child", "仓库", "r!", "r🦄"] {
        let mut f = Fixture::binding();
        let path = f._dir.path().join("config.toml");
        let before = std::fs::read(&path).unwrap();
        f.binding_message("bind", &format!("绑 {repo}"));
        f.step(10.);
        assert!(f.texts().contains("仓库名不合规则"), "{}", f.texts());
        assert!(f.engine.log.contains(&"bind_failed invalid_repo".into()));
        assert_eq!(std::fs::read(path).unwrap(), before);
        assert!(f.corral.status_reads.is_empty());
    }
}

#[test]
fn binding_reports_the_existing_channel_name_or_id() {
    for lookup_fails in [false, true] {
        let mut f = Fixture::binding();
        let path = f._dir.path().join("config.toml");
        let source = std::fs::read_to_string(&path).unwrap().replace(
            "channels = []",
            "channels = [{id = 'COLD', repo = 'ranch'}]",
        );
        std::fs::write(&path, &source).unwrap();
        f.reload_binding_config();
        if lookup_fails {
            f.memory.borrow_mut().channel_errors.insert("COLD".into());
        }
        f.binding_message("bind", "绑 ranch");
        f.step(10.);
        assert_eq!(
            f.texts(),
            if lookup_fails {
                "ranch 已经绑在 #COLD，没绑"
            } else {
                "ranch 已经绑在 #cold，没绑"
            }
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), source);
        assert!(f.corral.sent.is_empty());
        assert!(f.engine.log.contains(&"bind_failed repo_bound".into()));
    }
}

#[test]
fn binding_usage_is_shown_for_other_unbound_mentions() {
    for text in ["?", "绑", "绑 ranch extra", "bind ranch", "绑ranch", ""] {
        let mut f = Fixture::binding();
        let path = f._dir.path().join("config.toml");
        let before = std::fs::read(&path).unwrap();
        f.binding_message("help", text);
        f.step(10.);
        assert_eq!(f.texts(), "这个频道没绑仓库。要绑，发 `@pony 绑 <仓库名>`");
        assert_eq!(std::fs::read(path).unwrap(), before);
        assert!(f.memory.borrow().channel_reads.is_empty());
        assert!(f.corral.status_reads.is_empty());
    }
}

#[test]
fn binding_refuses_invalid_or_changed_config_without_overwriting() {
    for (replacement, expected) in [
        ("owner = [", "配置现在有错"),
        ("# someone edited a comment\n", "配置刚被改过，请再说一次"),
    ] {
        let mut f = Fixture::binding();
        let path = f._dir.path().join("config.toml");
        let source = if replacement.starts_with('#') {
            format!("{replacement}{}", std::fs::read_to_string(&path).unwrap())
        } else {
            replacement.into()
        };
        std::fs::write(&path, &source).unwrap();
        f.binding_message("bind", "绑 ranch");
        f.step(10.);
        assert!(f.texts().contains(expected), "{}", f.texts());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
        assert!(f.engine.log.contains(&format!(
            "bind_failed {}",
            if replacement.starts_with('#') {
                "changed"
            } else {
                "config_invalid"
            }
        )));
        assert!(f.corral.status_reads.is_empty());
        assert_eq!(f.memory.borrow().posted[0].thread.as_deref(), Some("bind"));
        if replacement.starts_with('#') {
            f.reload_binding_config();
            f.binding_message("retry", "绑 ranch");
            f.step(12.);
            assert!(f.texts().contains("已绑 ranch"));
        }
    }
}

#[test]
fn binding_reports_non_utf8_config_as_invalid() {
    let mut f = Fixture::binding();
    let path = f._dir.path().join("config.toml");
    std::fs::write(&path, [0xff]).unwrap();
    f.binding_message("bind", "绑 ranch");
    f.step(10.);
    assert!(f.texts().contains("配置现在有错"), "{}", f.texts());
    assert_eq!(std::fs::read(path).unwrap(), vec![0xff]);
}

#[test]
fn binding_requests_config_repair_when_reload_has_disabled_writes() {
    let mut f = Fixture::binding();
    let path = f._dir.path().join("config.toml");
    let changed = format!(
        "proxy = 'http://localhost:9'\n{}",
        std::fs::read_to_string(&path).unwrap()
    );
    std::fs::write(&path, &changed).unwrap();
    f.engine.config_file = None;
    f.binding_message("bind", "绑 ranch");
    f.step(10.);
    assert_eq!(f.texts(), "配置现在有错，没绑；请先修好配置");
    assert_eq!(std::fs::read_to_string(path).unwrap(), changed);
    assert!(f.corral.status_reads.is_empty());
}

#[test]
fn binding_explains_unpreservable_format_without_claiming_a_disk_failure() {
    let mut f = Fixture::binding();
    let path = f._dir.path().join("config.toml");
    let source = format!("\u{feff}{}", std::fs::read_to_string(&path).unwrap());
    std::fs::write(&path, &source).unwrap();
    f.reload_binding_config();
    f.binding_message("bind", "绑 ranch");
    f.step(10.);
    assert_eq!(
        f.texts(),
        "配置文件的格式没法原样保留，没绑；请手动加这一行：\n{ id = \"CNEW\", repo = \"ranch\" }"
    );
    assert!(
        f.engine
            .log
            .contains(&"bind_failed format_not_preserved".into())
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), source);
}

#[test]
fn binding_preserves_linked_and_current_user_read_only_configs() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
    for case in ["symlink", "hardlink", "readonly", "owner-readonly"] {
        let mut f = Fixture::binding();
        let path = f._dir.path().join("config.toml");
        let target = f._dir.path().join("original.toml");
        let source = std::fs::read(&path).unwrap();
        match case {
            "symlink" => {
                std::fs::rename(&path, &target).unwrap();
                symlink(&target, &path).unwrap();
            }
            "hardlink" => std::fs::hard_link(&path, &target).unwrap(),
            _ => std::fs::set_permissions(
                &path,
                std::fs::Permissions::from_mode(if case == "readonly" { 0o444 } else { 0o466 }),
            )
            .unwrap(),
        }
        f.binding_message("bind", "绑 ranch");
        f.step(10.);
        assert_eq!(
            f.texts(),
            "配置文件是链接或只读的，没绑；请手动加这一行：\n{ id = \"CNEW\", repo = \"ranch\" }",
            "{case}"
        );
        assert!(
            f.engine
                .log
                .contains(&"bind_failed config_not_writable".into())
        );
        assert_eq!(std::fs::read(&path).unwrap(), source);
        if case == "symlink" {
            assert!(
                std::fs::symlink_metadata(&path)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            assert_eq!(std::fs::read(&target).unwrap(), source);
        } else if case == "hardlink" {
            assert_eq!(std::fs::metadata(&path).unwrap().nlink(), 2);
            assert_eq!(std::fs::read(&target).unwrap(), source);
        }
        assert!(f.corral.status_reads.is_empty());
    }
}

#[test]
fn binding_write_failure_reports_only_the_code_and_preserves_config() {
    use std::os::unix::fs::PermissionsExt;
    let mut f = Fixture::binding();
    // Keep the state store writable while making the config directory read-only.
    let home = f._dir.path().join("readonly-config");
    std::fs::create_dir(&home).unwrap();
    std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).unwrap();
    let source = std::fs::read(f._dir.path().join("config.toml")).unwrap();
    std::fs::write(home.join("config.toml"), &source).unwrap();
    f.engine.config_file = Some(core::config::load_with_source(&home).unwrap().unwrap().1);
    std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o500)).unwrap();
    f.binding_message("bind", "绑 ranch");
    let result = f.engine.step(&mut f.corral, 10.);
    std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).unwrap();
    result.unwrap();
    assert_eq!(f.texts(), "写配置失败，没绑：io");
    assert!(f.engine.log.contains(&"bind_failed io".into()));
    assert_eq!(std::fs::read(home.join("config.toml")).unwrap(), source);
    assert_eq!(std::fs::read_dir(&home).unwrap().count(), 1);
    assert!(f.corral.status_reads.is_empty());
}

#[test]
fn binding_is_owner_only_and_requires_a_mention() {
    for (sender, mentioned) in [("intruder", true), ("UOWNER", false)] {
        let mut f = Fixture::binding();
        let path = f._dir.path().join("config.toml");
        let before = std::fs::read(&path).unwrap();
        f.events
            .emit(Event::Message(Message {
                channel: "CNEW".into(),
                id: "bind".into(),
                thread: None,
                sender: sender.into(),
                mentioned,
                text: "绑 ranch".into(),
                attachments: false,
                at: 10.,
            }))
            .unwrap();
        f.step(10.);
        assert!(f.memory.borrow().posted.is_empty());
        assert!(f.memory.borrow().channel_reads.is_empty());
        assert!(f.corral.sent.is_empty());
        assert!(f.corral.status_reads.is_empty());
        assert_eq!(std::fs::read(path).unwrap(), before);
    }
}

#[test]
fn binding_in_an_already_bound_channel_keeps_normal_routing() {
    let mut f = Fixture::binding();
    f.binding_message("bind", "绑 ranch");
    f.step(10.);
    f.reload_binding_config();
    let path = f._dir.path().join("config.toml");
    let before = std::fs::read(&path).unwrap();
    f.binding_message("ordinary", "绑 paddock");
    f.step(12.);
    assert_eq!(
        f.corral.sent,
        vec![("ranch/main".into(), "绑 paddock".into())]
    );
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn binding_succeeds_without_a_main_and_reports_status_like_summary() {
    for (state, paused, expected) in [
        ("missing", false, "ranch/main 没在跑"),
        ("working", false, "• main — 干活中，2 分钟"),
        ("idle", true, "• main — 暂停中"),
        ("blocked", false, "• main — 等你（对话框）"),
        ("starting", false, "• main — 启动中"),
        ("unknown", false, "• main — 读不出"),
    ] {
        let mut f = Fixture::binding();
        if state == "missing" {
            f.corral.agents.remove("ranch/main");
        } else {
            let agent = f.corral.agents.get_mut("ranch/main").unwrap();
            agent.state = state.into();
            agent.paused = paused;
        }
        f.events
            .emit(Event::Message(Message {
                channel: "CNEW".into(),
                id: "bind".into(),
                thread: Some("parent".into()),
                sender: "UOWNER".into(),
                mentioned: true,
                text: "  绑\t ranch  ".into(),
                attachments: false,
                at: 10.,
            }))
            .unwrap();
        f.step(130.);
        assert_eq!(f.texts(), format!("已绑 ranch\n{expected}"));
        assert_eq!(
            f.memory.borrow().posted[0].thread.as_deref(),
            Some("parent")
        );
        assert_eq!(
            core::config::load(f._dir.path()).unwrap().unwrap().chats[0].channels[0].repo,
            "ranch"
        );
        assert_eq!(f.corral.status_reads, vec!["ranch/main"]);
    }
}
#[test]
fn owner_message_and_thread_followup_reach_agent_and_reply_returns_to_thread() {
    let mut f = Fixture::new();
    f.step(2.);
    f.message("m1", None, "请检查代码", true);
    f.step(10.);
    assert_eq!(
        f.corral.sent,
        vec![("paddock/main".into(), "请检查代码".into())]
    );
    assert!(f.texts().contains("→ paddock/main"));
    let s = f.corral.agents.get_mut("paddock/main").unwrap();
    s.turn_started = Some(11.);
    s.last_event_at = Some(12.);
    f.corral.replies.insert(
        "paddock/main".into(),
        Ok(Reply {
            instance: "instance-1".into(),
            text: "完成\n原样回复".into(),
            at: 12.,
        }),
    );
    f.step(12.);
    f.step(14.);
    let data = f.memory.borrow();
    let reply = data
        .posted
        .iter()
        .find(|p| p.text == "完成\n原样回复")
        .unwrap();
    assert_eq!(reply.thread.as_deref(), Some("m1"));
    assert!(reply.notify);
    drop(data);
    f.message("m2", Some("m1"), "dev 仍是原正文", false);
    f.step(16.);
    assert_eq!(
        f.corral.sent.last().unwrap(),
        &("paddock/main".into(), "dev 仍是原正文".into())
    );
}

#[test]
fn routing_table_privacy_and_status() {
    for (text, expected, body) in [
        ("ranch/main 看看", Some("ranch/main"), "看看"),
        ("dev 检查", Some("paddock/dev"), "检查"),
        ("--flag unchanged", Some("paddock/main"), "--flag unchanged"),
        ("private/main secret", None, ""),
        ("ranch/missing test", None, ""),
        ("dev", None, ""),
        ("   ", None, ""),
    ] {
        let mut f = Fixture::new();
        f.message("m1", None, text, true);
        f.step(10.);
        assert_eq!(
            f.corral.sent.first().map(|(n, _)| n.as_str()),
            expected,
            "{text}: {}",
            f.texts()
        );
        if expected.is_some() {
            assert_eq!(f.corral.sent[0].1, body)
        }
        assert!(
            !f.corral
                .status_reads
                .iter()
                .any(|n| n.starts_with("private/"))
        );
        if text.starts_with("private/") {
            assert!(!f.texts().contains("private"));
        }
    }
    let mut f = Fixture::new();
    f.events
        .emit(Event::Message(Message {
            channel: "C1".into(),
            id: "other".into(),
            thread: None,
            sender: "intruder".into(),
            mentioned: true,
            text: "private body".into(),
            attachments: false,
            at: 9.,
        }))
        .unwrap();
    assert!(
        !std::fs::read_to_string(f._dir.path().join("state.json"))
            .unwrap()
            .contains("private body")
    );
    f.message("note", None, "note", false);
    f.message("unknown", Some("unknown-thread"), "reply", false);
    f.step(10.);
    assert!(f.corral.sent.is_empty());
    assert!(f.memory.borrow().posted.is_empty());
    assert_eq!(
        f.engine
            .log
            .iter()
            .filter(|l| !l.starts_with("connection "))
            .collect::<Vec<_>>(),
        vec!["忽略了一条别人的消息"]
    );
    f.events
        .emit(Event::Message(Message {
            channel: "unbound".into(),
            id: "u".into(),
            thread: None,
            sender: "owner".into(),
            mentioned: true,
            text: "?".into(),
            attachments: false,
            at: 11.,
        }))
        .unwrap();
    f.step(12.);
    assert_eq!(f.texts(), "这个频道没绑仓库。要绑，发 `@pony 绑 <仓库名>`");
    f.message("s", None, "？", true);
    f.step(14.);
    assert!(f.corral.sent.is_empty());
    assert!(f.texts().contains("paddock"));
    assert!(f.texts().contains("空闲"));
    assert!(!f.texts().contains("private"));
    let mut f = Fixture::new();
    f.corral.agents.remove("paddock/main");
    f.message("m", None, "请看看", true);
    f.step(10.);
    assert!(f.texts().contains("paddock/main 没在跑，没送"));
    let mut f = Fixture::new();
    f.message("m", Some("external"), "dev hello", true);
    f.step(10.);
    assert_eq!(f.corral.sent[0].0, "paddock/dev");
}

#[test]
fn busy_states_queue_in_order_deletion_withdraws_and_exit_cancels() {
    for (state, paused, label) in [
        ("working", false, "正在干活"),
        ("starting", false, "正在干活"),
        ("blocked", false, "对话框"),
        ("idle", true, "暂停中"),
    ] {
        let mut f = Fixture::new();
        let s = f.corral.agents.get_mut("paddock/main").unwrap();
        s.state = state.into();
        s.paused = paused;
        f.message("a", None, "第一句", true);
        f.message("b", None, "第二句", true);
        f.step(10.);
        assert!(f.corral.sent.is_empty());
        assert!(f.texts().contains(label), "{}", f.texts());
        f.events
            .emit(Event::Deleted {
                channel: "C1".into(),
                id: "a".into(),
            })
            .unwrap();
        f.step(12.);
        assert!(f.texts().contains("撤回了"));
        let s = f.corral.agents.get_mut("paddock/main").unwrap();
        s.state = "idle".into();
        s.paused = false;
        s.turn_started = Some(11.);
        s.last_event_at = Some(13.);
        f.step(14.);
        f.step(16.);
        assert_eq!(
            f.corral.sent,
            vec![("paddock/main".into(), "第二句".into())]
        );
        assert!(f.texts().contains("已送给 paddock/main"));
        f.message("c", None, "第三句", true);
        f.step(18.);
        assert_eq!(f.corral.sent.len(), 1);
        f.corral.agents.remove("paddock/main");
        f.step(20.);
        assert!(f.texts().contains("退出了，没等到回复"));
        assert!(f.texts().contains("已经不在了，没送"));
        assert!(f.engine.state.pending.is_empty());
    }
}

#[test]
fn send_outcomes_do_not_force_or_duplicate_and_unknown_does_not_wait() {
    for (exit, code, label, retry) in [
        (7, "not_idle", "排", true),
        (8, "human_active", "30 秒", true),
        (10, "paused", "暂停中", true),
        (3, "not_delivered", "没重送", false),
        (2, "not_found", "不在了", false),
        (6, "other", "other: reason", false),
    ] {
        let mut f = Fixture::new();
        f.corral.outcomes.push(Err(failure(exit, code)));
        f.message("m", None, "hello", true);
        f.step(10.);
        assert!(f.texts().contains(label), "{code}: {}", f.texts());
        f.step(12.);
        if exit == 8 {
            assert_eq!(f.corral.sent.len(), 1);
            f.step(40.);
        }
        assert_eq!(f.corral.sent.len(), if retry { 2 } else { 1 }, "{code}");
    }
    let mut f = Fixture::new();
    f.corral.outcomes.push(Ok(Sent {
        merged_with_draft: true,
    }));
    f.message("m", None, "hi", true);
    f.step(10.);
    assert!(f.texts().contains("输入框里原有没提交的字"));
    let mut f = Fixture::new();
    f.corral.agents.get_mut("paddock/main").unwrap().state = "unknown".into();
    f.message("m", None, "hi", true);
    f.step(10.);
    assert!(f.texts().contains("等不到回复"));
    assert!(f.engine.state.pending.is_empty());
}

#[test]
fn notifications_baseline_two_polls_dedup_and_notification_threads() {
    let mut f = Fixture::new();
    f.corral.replies.insert(
        "paddock/main".into(),
        Ok(Reply {
            instance: "instance-1".into(),
            text: "旧回复".into(),
            at: 1.,
        }),
    );
    f.step(2.);
    f.step(4.);
    assert!(f.memory.borrow().posted.is_empty());
    let s = f.corral.agents.get_mut("paddock/dev").unwrap();
    s.state = "blocked".into();
    s.state_started = Some(5.);
    s.last_tool = Some("Bash".into());
    s.title = Some("等待权限".into());
    f.step(6.);
    assert!(f.memory.borrow().posted.is_empty());
    f.step(8.);
    let notice = f.memory.borrow().posted.last().unwrap().clone();
    assert_eq!(notice.channel, "C1");
    assert!(notice.thread.is_none());
    assert!(notice.notify);
    assert!(notice.text.contains("paddock/dev 在等你"));
    f.step(10.);
    assert_eq!(f.memory.borrow().posted.len(), 1);
    f.message("answer", Some(&notice.id), "同意继续", false);
    f.step(12.);
    assert!(f.corral.sent.is_empty());
    assert_eq!(f.engine.state.pending[0].agent, "paddock/dev");
    let s = f.corral.agents.get_mut("paddock/main").unwrap();
    s.turn_started = Some(13.);
    s.last_event_at = Some(14.);
    f.corral.replies.insert(
        "paddock/main".into(),
        Ok(Reply {
            instance: "instance-1".into(),
            text: "新回复".into(),
            at: 14.,
        }),
    );
    f.step(14.);
    f.step(16.);
    let notice = f.memory.borrow().posted.last().unwrap().clone();
    assert!(notice.text.contains("paddock/main 新回复\n新回复"));
    assert!(notice.thread.is_none());
    f.message("next", Some(&notice.id), "你好", false);
    f.step(18.);
    assert_eq!(f.corral.sent[0].0, "paddock/main");
    f.step(20.);
    assert_eq!(
        f.memory
            .borrow()
            .posted
            .iter()
            .filter(|p| p.text.contains("新回复\n新回复"))
            .count(),
        1
    );
}

#[test]
fn interrupted_turn_and_empty_reply_are_reported() {
    for interrupted in [false, true] {
        let mut f = Fixture::new();
        f.step(2.);
        f.message("m", None, "hello", true);
        f.step(10.);
        let s = f.corral.agents.get_mut("paddock/main").unwrap();
        s.turn_started = Some(11.);
        s.last_event_at = Some(12.);
        if interrupted {
            let mut e = failure(1, "no_reply");
            e.interrupted = true;
            f.corral.replies.insert("paddock/main".into(), Err(e));
        } else {
            f.corral.replies.insert(
                "paddock/main".into(),
                Ok(Reply {
                    instance: "instance-1".into(),
                    text: String::new(),
                    at: 12.,
                }),
            );
        }
        f.step(12.);
        f.step(14.);
        assert!(f.texts().contains(if interrupted {
            "paddock/main 这一轮被打断了，没有回复"
        } else {
            "（空回复）"
        }));
        assert!(f.engine.state.pending.is_empty());
    }
}

impl Fixture {
    fn restart(&mut self) {
        let store = Store::open(self._dir.path()).unwrap();
        self.events = store.events("memory", "owner");
        let connections = BTreeMap::from([(
            "memory".into(),
            Box::new(MemoryConnection(self.memory.clone(), 10_000, true)) as Box<dyn Connection>,
        )]);
        self.engine = Engine::new(store, self.engine.configs.clone(), connections);
        self.connected();
    }
    fn connected(&self) {
        self.events
            .emit(Event::Connection(ConnectionInfo {
                connection: "connected".into(),
                reason: None,
                message: None,
                workspace: Some("workspace".into()),
                bot: Some("pony".into()),
                other_connections: 0,
            }))
            .unwrap();
    }
}
#[test]
fn restart_restores_queue_waiting_dedup_and_ambiguous_send_is_never_retried() {
    use pony::core::state::Phase;
    use std::os::unix::fs::PermissionsExt;
    let mut f = Fixture::new();
    f.step(2.);
    f.message("a", None, "first", true);
    f.message("b", None, "second", true);
    f.step(10.);
    assert_eq!(f.corral.sent.len(), 1);
    f.restart();
    f.message("a", None, "first", true);
    let s = f.corral.agents.get_mut("paddock/main").unwrap();
    s.turn_started = Some(11.);
    s.last_event_at = Some(12.);
    f.corral.replies.insert(
        "paddock/main".into(),
        Ok(Reply {
            instance: "instance-1".into(),
            text: "done".into(),
            at: 12.,
        }),
    );
    f.step(14.);
    f.step(16.);
    assert_eq!(f.corral.sent.len(), 2);
    assert_eq!(f.corral.sent[1].1, "second");
    assert!(
        f.memory
            .borrow()
            .posted
            .iter()
            .any(|p| p.text == "done" && p.thread.as_deref() == Some("a"))
    );
    assert_eq!(
        std::fs::metadata(f._dir.path().join("state.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    f.engine.state.pending[0].phase = Phase::Sending;
    f.engine.store.save(&f.engine.state, None).unwrap();
    f.restart();
    f.step(18.);
    f.step(20.);
    assert_eq!(f.corral.sent.len(), 2);
    assert_eq!(
        f.memory
            .borrow()
            .posted
            .iter()
            .filter(|p| p.text == "上次发送结果不明，没重送，请确认后重发")
            .count(),
        1
    );
    assert!(f.engine.log.iter().any(|s| s.contains("send_uncertain")));
}
#[test]
fn reconnect_backfills_sorted_messages_with_age_limit_and_first_start_skips_history() {
    let mut f = Fixture::new();
    f.step(2.);
    assert!(f.memory.borrow().history_reads.is_empty());
    f.message("old", None, "请开始", true);
    f.step(10.);
    let mk = |id: &str, t: f64, thread: Option<&str>| Message {
        channel: "C1".into(),
        id: id.into(),
        thread: thread.map(str::to_owned),
        sender: "owner".into(),
        mentioned: true,
        text: format!("消息 {id}"),
        attachments: false,
        at: t,
    };
    f.memory.borrow_mut().history = vec![
        mk("newer", 49_000., None),
        mk("ancient", 11., None),
        mk("earlier", 48_000., Some("old")),
    ];
    f.restart();
    f.connected();
    f.step(50_000.);
    assert!(f.texts().contains("12 小时以前发的，没送"));
    assert!(f.texts().contains("离线期间发的"));
    let queue: Vec<_> = f
        .engine
        .state
        .pending
        .iter()
        .filter(|p| p.phase == pony::core::state::Phase::Queued)
        .collect();
    assert_eq!(queue.len(), 2);
    assert_eq!(queue[0].message, "earlier");
    assert_eq!(queue[1].message, "newer");
    assert!(
        queue[0]
            .text
            .contains("pony 当时离线，现在才送到）\n消息 earlier")
    );
    let before = f.engine.state.pending.len();
    f.message("newer", None, "消息 newer", true);
    f.step(50_002.);
    assert_eq!(f.engine.state.pending.len(), before);
    assert!(
        f.memory
            .borrow()
            .history_reads
            .iter()
            .any(|(channel, after)| channel == "C1" && after.id.as_deref() == Some("old"))
    );
}
#[test]
fn expired_threads_do_not_guess_and_reopened_instances_are_named() {
    let mut f = Fixture::new();
    f.message("root", None, "dev hello", true);
    f.step(10.);
    f.corral.agents.get_mut("paddock/dev").unwrap().instance = "instance-2".into();
    f.message("next", Some("root"), "hello again", false);
    f.step(12.);
    assert!(f.texts().contains("→ paddock/dev（重开过）"));
    f.message("late", Some("root"), "hello", false);
    f.step(31. * 86400.);
    assert!(
        f.texts()
            .contains("这个线程太久没用，对应关系已清掉，请在频道里重新 @ 我")
    );
}

#[test]
fn long_reply_retries_smaller_chunks_notifies_once_and_reports_final_failure() {
    let mut f = Fixture::new();
    f.step(2.);
    f.message("m", None, "hello", true);
    f.step(10.);
    let text = "字🙂".repeat(11_000);
    let s = f.corral.agents.get_mut("paddock/main").unwrap();
    s.turn_started = Some(11.);
    s.last_event_at = Some(12.);
    f.corral.replies.insert(
        "paddock/main".into(),
        Ok(Reply {
            instance: "instance-1".into(),
            text: text.clone(),
            at: 12.,
        }),
    );
    f.memory.borrow_mut().fail_over = Some(1800);
    f.step(12.);
    f.step(14.);
    let data = f.memory.borrow();
    let replies: Vec<_> = data
        .posted
        .iter()
        .filter(|p| p.text.contains('字'))
        .collect();
    assert!(replies.len() > 10);
    assert_eq!(replies.iter().filter(|p| p.notify).count(), 1);
    assert!(replies.iter().all(|p| p.thread.as_deref() == Some("m")));
    assert_eq!(
        replies
            .iter()
            .flat_map(|p| p.text.chars())
            .filter(|c| *c == '字' || *c == '🙂')
            .collect::<String>(),
        text
    );
    drop(data);
    f.message("next", Some("m"), "again", false);
    f.step(16.);
    let s = f.corral.agents.get_mut("paddock/main").unwrap();
    s.turn_started = Some(17.);
    s.last_event_at = Some(18.);
    f.corral.replies.insert(
        "paddock/main".into(),
        Ok(Reply {
            instance: "instance-1".into(),
            text: "x".repeat(3000),
            at: 18.,
        }),
    );
    f.memory.borrow_mut().fail_over = Some(50);
    f.step(18.);
    f.step(20.);
    assert!(f.texts().contains("段发不出去：too_long"));
    assert!(f.engine.log.iter().any(|l| l.contains("too_long")));
    assert_eq!(f.engine.reports["memory"].connection, "connected");
    assert!(f.engine.reports["memory"].reason.is_none());
}

#[test]
fn status_labels_queue_counts_and_current_repository_order() {
    let mut f = Fixture::new();
    for (name, state, paused, interrupted) in [
        ("paddock/main", "working", false, false),
        ("paddock/dev", "idle", false, true),
        ("ranch/main", "blocked", false, false),
        ("ranch/paused", "idle", true, false),
        ("ranch/start", "starting", false, false),
        ("ranch/raw", "unknown", false, false),
    ] {
        let mut s = status(name, state);
        s.paused = paused;
        s.interrupted = interrupted;
        f.corral.agents.insert(name.into(), s);
    }
    f.message("q", None, "hello", true);
    f.step(120.);
    f.message("s", None, "?", true);
    f.step(122.);
    let summary = f
        .memory
        .borrow()
        .posted
        .iter()
        .find(|p| p.text.starts_with("paddock（"))
        .unwrap()
        .text
        .clone();
    for expected in [
        "干活中，2 分钟",
        "空闲（上一轮被打断）",
        "等你（对话框）",
        "暂停中",
        "启动中",
        "读不出",
        "排着 1 句",
    ] {
        assert!(summary.contains(expected), "{summary}");
    }
    assert!(summary.find("paddock（").unwrap() < summary.find("ranch（").unwrap());
    assert!(!summary.contains("private"));
}
#[test]
fn offline_controller_reply_is_delivered_once_and_fresh_instances_start_at_baseline() {
    let mut f = Fixture::new();
    f.step(2.);
    f.step(4.);
    f.restart();
    let s = f.corral.agents.get_mut("ranch/main").unwrap();
    s.turn_started = Some(5.);
    s.last_event_at = Some(6.);
    f.corral.replies.insert(
        "ranch/main".into(),
        Ok(Reply {
            instance: "instance-1".into(),
            text: "离线完成".into(),
            at: 6.,
        }),
    );
    f.step(8.);
    f.step(10.);
    assert_eq!(
        f.memory
            .borrow()
            .posted
            .iter()
            .filter(|p| p.text.contains("（pony 不在线时）ranch/main 新回复"))
            .count(),
        1
    );
    f.restart();
    f.step(12.);
    f.step(14.);
    assert_eq!(
        f.memory
            .borrow()
            .posted
            .iter()
            .filter(|p| p.text.contains("离线完成"))
            .count(),
        1
    );
    f.corral.agents.get_mut("ranch/main").unwrap().instance = "new".into();
    f.corral.replies.insert(
        "ranch/main".into(),
        Ok(Reply {
            instance: "new".into(),
            text: "重开后的起点".into(),
            at: 15.,
        }),
    );
    f.step(16.);
    f.step(18.);
    assert!(!f.texts().contains("重开后的起点"));
}
#[test]
fn named_route_preserves_trailing_whitespace_and_edited_messages_do_not_replace_queue() {
    let mut f = Fixture::new();
    f.corral.agents.get_mut("paddock/dev").unwrap().state = "working".into();
    f.message("m", None, "dev --正文  \n ", true);
    f.step(10.);
    f.message("m", None, "dev changed", true);
    f.step(12.);
    assert_eq!(f.engine.state.pending.len(), 1);
    assert_eq!(f.engine.state.pending[0].text, "--正文  \n ");
}
#[test]
fn platforms_without_threads_require_mentions_and_explicit_names() {
    let mut f = Fixture::new();
    f.engine.connections.insert(
        "memory".into(),
        Box::new(MemoryConnection(f.memory.clone(), 10_000, false)),
    );
    f.message("a", None, "hello", true);
    f.step(10.);
    assert!(f.corral.sent.is_empty());
    f.message("b", None, "dev hello", true);
    f.step(12.);
    assert_eq!(f.corral.sent[0].0, "paddock/dev");
    let s = f.corral.agents.get_mut("paddock/dev").unwrap();
    s.turn_started = Some(13.);
    s.last_event_at = Some(14.);
    f.corral.replies.insert(
        "paddock/dev".into(),
        Ok(Reply {
            instance: "instance-1".into(),
            text: "完成".into(),
            at: 14.,
        }),
    );
    f.step(14.);
    f.step(16.);
    assert!(
        f.memory
            .borrow()
            .posted
            .iter()
            .any(|p| p.thread.is_none() && p.text == "paddock/dev: 完成")
    );
    f.message("c", Some("b"), "不应该送", false);
    f.step(18.);
    assert_eq!(f.corral.sent.len(), 1);
}

#[test]
fn human_active_backoff_keeps_later_messages_behind_the_head() {
    let mut f = Fixture::new();
    f.corral.outcomes.push(Err(failure(8, "human_active")));
    f.message("first", None, "first", true);
    f.message("second", None, "second", true);
    f.step(10.);
    f.step(12.);
    assert_eq!(f.corral.sent, vec![("paddock/main".into(), "first".into())]);
    f.step(40.);
    assert_eq!(f.corral.sent[1].1, "first");
    assert_eq!(f.corral.sent.len(), 2);
}

#[test]
fn connection_and_channel_metadata_flow_to_the_existing_status_schema() {
    use pony::core::{
        config::Config,
        status::{Snapshot, chats},
    };
    let mut f = Fixture::new();
    f.step(2.);
    let mut snapshot = Snapshot::new();
    snapshot.chats = chats(
        &Config {
            corral: None,
            proxy: None,
            chats: f.engine.configs.clone(),
        },
        "connecting",
        Some(1.),
    );
    snapshot.update(&f.engine, 2.);
    let value = serde_json::to_value(&snapshot).unwrap();
    assert_eq!(value["chats"][0]["connection"], "connected");
    assert_eq!(value["chats"][0]["channels"][0]["name"], "c1");
    assert_eq!(value["chats"][0]["channels"][0]["private"], true);
    assert!(!value.to_string().contains("owner"));
    f.events
        .emit(Event::Connection(ConnectionInfo {
            connection: "failed".into(),
            reason: Some("network".into()),
            message: Some("connection lost".into()),
            workspace: Some("workspace".into()),
            bot: Some("pony".into()),
            other_connections: 2,
        }))
        .unwrap();
    f.step(4.);
    snapshot.update(&f.engine, 4.);
    let value = serde_json::to_value(&snapshot).unwrap();
    assert_eq!(value["chats"][0]["since"], 4.);
    assert_eq!(value["chats"][0]["reason"], "network");
    assert_eq!(value["chats"][0]["other_connections"], 2);
}

#[test]
fn routing_acknowledgement_must_be_posted_before_corral_send_even_after_retry() {
    let mut f = Fixture::new();
    f.memory.borrow_mut().fail_over = Some(0);
    f.message("m", None, "hello", true);
    f.step(10.);
    assert!(f.corral.sent.is_empty());
    f.memory.borrow_mut().fail_over = None;
    f.step(12.);
    assert!(f.texts().contains("→ paddock/main"));
    assert_eq!(f.corral.sent.len(), 1);
}

fn incoming(channel: &str, id: &str, thread: Option<&str>, text: &str, at: f64) -> Message {
    Message {
        channel: channel.into(),
        id: id.into(),
        thread: thread.map(str::to_owned),
        sender: "owner".into(),
        mentioned: thread.is_none(),
        text: text.into(),
        attachments: false,
        at,
    }
}

#[test]
fn offline_new_thread_replies_are_backfilled_in_order_without_per_thread_reads() {
    let mut f = Fixture::new();
    f.message("100", None, "hello", true);
    f.step(10.);
    f.corral.agents.get_mut("paddock/dev").unwrap().state = "working".into();
    f.memory.borrow_mut().history = vec![
        incoming("C1", "201", Some("200"), "还有第二件", 21.),
        incoming("C1", "200", None, "dev 第一件事", 20.),
    ];
    f.restart();
    for at in [30., 32., 34., 36.] {
        f.step(at);
    }
    let pending: Vec<_> = f
        .engine
        .state
        .pending
        .iter()
        .filter(|p| p.agent == "paddock/dev")
        .collect();
    assert_eq!(
        pending
            .iter()
            .map(|p| p.message.as_str())
            .collect::<Vec<_>>(),
        ["200", "201"]
    );
    assert!(pending.iter().all(|p| p.text.contains("pony 当时离线")));
    assert_eq!(
        f.memory
            .borrow()
            .history_reads
            .iter()
            .filter(|(c, _)| c == "C1")
            .count(),
        1
    );
}

#[test]
fn notification_only_channel_backfills_from_previous_online_time() {
    let mut f = Fixture::new();
    f.step(2.);
    f.corral.agents.get_mut("paddock/dev").unwrap().state = "blocked".into();
    f.step(4.);
    f.step(6.);
    let root = f.memory.borrow().posted.last().unwrap().id.clone();
    assert!(f.engine.state.cursors.is_empty());
    f.memory
        .borrow_mut()
        .history
        .push(incoming("C1", "reply", Some(&root), "请继续", 10.));
    f.restart();
    f.step(20.);
    assert_eq!(f.engine.state.pending[0].agent, "paddock/dev");
    let data = f.memory.borrow();
    let after = &data
        .history_reads
        .iter()
        .find(|(c, _)| c == "C1")
        .unwrap()
        .1;
    assert!(after.id.is_none());
    assert_eq!(after.at, 2.);
}

#[test]
fn stop_failure_does_not_post_previous_turn_reply_and_next_message_is_sent() {
    let mut f = Fixture::new();
    f.corral.replies.insert(
        "paddock/main".into(),
        Ok(Reply {
            instance: "instance-1".into(),
            text: "上一轮的旧回复".into(),
            at: 1.,
        }),
    );
    f.step(2.);
    f.message("first", None, "first", true);
    f.message("second", None, "second", true);
    f.step(10.);
    let s = f.corral.agents.get_mut("paddock/main").unwrap();
    s.turn_started = Some(11.);
    s.last_event = Some("StopFailure".into());
    s.last_event_at = Some(12.);
    f.step(12.);
    f.step(14.);
    assert!(!f.texts().contains("上一轮的旧回复"));
    assert!(f.texts().contains("paddock/main 这一轮结束了，没有新回复"));
    assert_eq!(f.corral.sent.last().unwrap().1, "second");
}

#[test]
fn broken_watched_agent_does_not_block_other_repository() {
    let mut f = Fixture::new();
    f.corral
        .status_errors
        .insert("paddock/dev".into(), failure(9, "incompatible"));
    f.events
        .emit(Event::Message(incoming("C2", "good", None, "继续", 10.)))
        .unwrap();
    for at in [10., 12., 14., 16., 18.] {
        f.step(at);
    }
    assert_eq!(f.corral.sent, [("ranch/main".into(), "继续".into())]);
    assert!(f.engine.log.iter().any(|s| s.contains("incompatible")));
}

#[test]
fn broken_route_is_reported_and_dequeued_without_blocking_later_input() {
    let mut f = Fixture::new();
    f.corral
        .status_errors
        .insert("paddock/dev".into(), failure(9, "incompatible"));
    f.message("bad", None, "dev 请继续", true);
    f.events
        .emit(Event::Message(incoming("C2", "good", None, "继续", 10.)))
        .unwrap();
    for at in [10., 12., 14.] {
        f.step(at);
    }
    assert!(f.engine.store.inbox().is_empty());
    assert_eq!(f.texts().matches("incompatible: reason").count(), 1);
    assert_eq!(f.corral.sent, [("ranch/main".into(), "继续".into())]);
}

#[test]
fn broken_backfill_channel_does_not_hold_connection_event_or_other_inputs() {
    for channel_error in [true, false] {
        let mut f = Fixture::new();
        f.step(2.);
        if channel_error {
            f.memory.borrow_mut().channel_errors.insert("C1".into());
        } else {
            f.memory.borrow_mut().history_errors.insert("C1".into());
        }
        f.restart();
        f.events
            .emit(Event::Message(incoming("C2", "good", None, "继续", 10.)))
            .unwrap();
        for at in [10., 12., 14., 16.] {
            f.step(at);
        }
        assert!(f.engine.store.inbox().is_empty());
        assert_eq!(f.corral.sent, [("ranch/main".into(), "继续".into())]);
        assert!(f.engine.log.iter().any(|s| s.contains(if channel_error {
            "not_member"
        } else {
            "history_failed"
        })));
    }
}

#[test]
fn partial_reply_or_notice_failure_does_not_repost_successful_chunks_after_restart() {
    for waiting in [true, false] {
        let mut f = Fixture::new();
        f.engine.connections.insert(
            "memory".into(),
            Box::new(MemoryConnection(f.memory.clone(), 1000, true)),
        );
        f.step(2.);
        if waiting {
            f.message("request", None, "hi", true);
            f.step(10.);
        }
        let s = f.corral.agents.get_mut("paddock/main").unwrap();
        s.turn_started = Some(11.);
        s.last_event_at = Some(12.);
        f.corral.replies.insert(
            "paddock/main".into(),
            Ok(Reply {
                instance: "instance-1".into(),
                text: "片".repeat(3000),
                at: 12.,
            }),
        );
        // Waiting replies have an earlier arrow; notices split their title first.
        // In both cases, allow one body chunk through before rejecting the rest.
        f.memory.borrow_mut().fail_after = Some(2);
        f.step(12.);
        f.step(14.);
        f.step(16.);
        f.restart();
        f.step(18.);
        f.step(20.);
        assert_eq!(
            f.memory
                .borrow()
                .posted
                .iter()
                .filter(|p| p.text.contains('片'))
                .count(),
            1
        );
        assert!(f.engine.state.pending.is_empty());
    }
}

#[test]
fn delivery_failure_and_later_success_leave_connection_status_unchanged() {
    use pony::core::{
        config::Config,
        status::{Snapshot, chats},
    };
    let mut f = Fixture::new();
    f.step(2.);
    let mut snapshot = Snapshot::new();
    snapshot.chats = chats(
        &Config {
            corral: None,
            proxy: None,
            chats: f.engine.configs.clone(),
        },
        "connecting",
        Some(1.),
    );
    snapshot.update(&f.engine, 2.);
    f.memory.borrow_mut().fail_over = Some(0);
    f.message("failed", None, "hello", true);
    f.step(10.);
    snapshot.update(&f.engine, 10.);
    f.memory.borrow_mut().fail_over = None;
    f.step(12.);
    snapshot.update(&f.engine, 12.);
    let value = serde_json::to_value(snapshot).unwrap();
    assert_eq!(value["chats"][0]["connection"], "connected");
    assert_eq!(value["chats"][0]["since"], 2.);
    assert!(value["chats"][0]["reason"].is_null());
    assert_eq!(f.corral.sent.len(), 1);
}

#[test]
fn repeated_human_active_and_not_idle_rejections_are_reported_only_once() {
    for (exit, code, label) in [
        (8, "human_active", "人刚在窗口里操作过"),
        (7, "not_idle", "正在干活，放回队首排着"),
    ] {
        let mut f = Fixture::new();
        f.corral.outcomes = (0..4).map(|_| Err(failure(exit, code))).collect();
        f.message("first", None, "hello", true);
        f.step(10.);
        f.restart();
        for at in [40., 42., 72., 102.] {
            f.step(at);
        }
        assert_eq!(f.texts().matches(label).count(), 1);
        assert!(f.corral.sent.len() >= 4);
    }
}

#[test]
fn same_batch_idle_agent_queue_position_excludes_immediate_head() {
    let mut f = Fixture::new();
    f.message("first", None, "first", true);
    f.message("second", None, "second", true);
    f.step(10.);
    assert_eq!(f.corral.sent.len(), 1);
    let data = f.memory.borrow();
    let second = data
        .posted
        .iter()
        .find(|p| p.thread.as_deref() == Some("second"))
        .unwrap();
    assert!(second.text.contains("排在第 1 个"));
}

#[test]
fn queued_live_message_in_history_keeps_online_text() {
    let mut f = Fixture::new();
    f.step(2.);
    f.restart();
    let message = incoming("C1", "live", None, "现在发的", 10.);
    f.memory.borrow_mut().history.push(message.clone());
    f.events.emit(Event::Message(message)).unwrap();
    f.step(12.);
    assert_eq!(f.corral.sent, [("paddock/main".into(), "现在发的".into())]);
    assert!(!f.texts().contains("离线期间"));
}

#[test]
fn attachments_report_omission_after_arrow_and_attachment_only_input_explains() {
    let mut f = Fixture::new();
    for (id, text) in [("text", "文字"), ("only", "")] {
        let mut message = incoming("C1", id, None, text, 10.);
        message.attachments = true;
        f.events.emit(Event::Message(message)).unwrap();
    }
    f.step(10.);
    assert_eq!(f.corral.sent, [("paddock/main".into(), "文字".into())]);
    let text = f.texts();
    assert!(text.find("→ paddock/main").unwrap() < text.find("附件没送").unwrap());
    assert!(text.contains("附件没送，只收文字"));
    assert!(!text.contains("用法"));
}

#[test]
fn reconnect_withdraws_deleted_queue_and_defers_unverifiable_message_only() {
    let mut f = Fixture::new();
    f.corral.agents.get_mut("paddock/dev").unwrap().state = "working".into();
    f.message("deleted", None, "dev 删除我", true);
    f.message("uncertain", None, "dev 核对我", true);
    f.step(10.);
    f.memory.borrow_mut().deleted.insert("deleted".into());
    f.memory
        .borrow_mut()
        .exists_errors
        .insert("uncertain".into());
    f.corral.agents.get_mut("paddock/dev").unwrap().state = "idle".into();
    f.restart();
    f.events
        .emit(Event::Message(incoming("C2", "good", None, "继续", 12.)))
        .unwrap();
    f.step(12.);
    f.step(14.);
    assert_eq!(f.texts().matches("撤回了").count(), 1);
    assert_eq!(f.corral.sent, [("ranch/main".into(), "继续".into())]);
    assert!(f.memory.borrow().exists_reads.contains(&"deleted".into()));
    f.memory.borrow_mut().exists_errors.clear();
    f.step(16.);
    assert_eq!(f.corral.sent.len(), 1);
    f.step(18.);
    assert_eq!(f.corral.sent.last().unwrap().1, "核对我");
}

#[test]
fn cannot_execute_corral_is_definitely_unsent() {
    let mut f = Fixture::new();
    let mut error = failure(1, "corral_unavailable");
    error.exit = None;
    f.corral.outcomes.push(Err(error));
    f.message("m", None, "hello", true);
    f.step(10.);
    f.step(12.);
    assert!(f.texts().contains("没送"));
    assert!(!f.texts().contains("结果不明"));
    assert!(!f.engine.log.iter().any(|l| l == "send_uncertain"));
    assert!(f.engine.state.pending.is_empty());
}

#[test]
fn unchanged_polling_preserves_state_and_status_files_and_retention_is_bounded() {
    use pony::core::{
        state::{Address, Cursor, Expired, Seen},
        status::Snapshot,
    };
    use std::os::unix::fs::MetadataExt;
    let mut f = Fixture::new();
    f.step(2.);
    f.step(4.);
    let state_path = f._dir.path().join("state.json");
    let inode = std::fs::metadata(&state_path).unwrap().ino();
    for at in [6., 8., 10.] {
        f.step(at);
    }
    assert_eq!(std::fs::metadata(&state_path).unwrap().ino(), inode);
    std::fs::create_dir(f._dir.path().join("run")).unwrap();
    let snapshot = Snapshot::new();
    snapshot.save(f._dir.path()).unwrap();
    let path = f._dir.path().join("run/status.json");
    let inode = std::fs::metadata(&path).unwrap().ino();
    snapshot.save(f._dir.path()).unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().ino(), inode);
    let old = Address {
        kind: "memory".into(),
        channel: "C1".into(),
        thread: "old".into(),
    };
    f.engine.state.expired.push(Expired {
        address: old.clone(),
        at: 1.,
    });
    f.engine.state.cursors.push(Cursor {
        address: old,
        id: Some("old".into()),
        at: 1.,
    });
    f.engine.state.seen.push(Seen {
        kind: "memory".into(),
        channel: "C1".into(),
        id: "old".into(),
        at: 1.,
    });
    f.step(100. * 86400.);
    assert!(f.engine.state.seen.is_empty());
    assert!(f.engine.state.expired.is_empty());
    assert!(f.engine.state.cursors.is_empty());
}

#[test]
fn previous_untimed_state_remains_readable_with_queued_messages() {
    let mut f = Fixture::new();
    f.corral.agents.get_mut("paddock/dev").unwrap().state = "working".into();
    f.message("queued", None, "dev hello", true);
    f.step(10.);
    let path = f._dir.path().join("state.json");
    let mut document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    document["state"]["seen"] = serde_json::json!([["memory", "C1", "queued"]]);
    document["state"]["expired"] =
        serde_json::json!([{ "kind": "memory", "channel": "C1", "thread": "expired" }]);
    std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
    f.restart();
    assert_eq!(f.engine.state.pending[0].text, "hello");
    assert_eq!(f.engine.state.seen[0].id, "queued");
    assert_eq!(f.engine.state.expired[0].address.thread, "expired");
}

#[test]
fn failed_backfill_keeps_its_lower_bound_across_live_messages_and_restarts() {
    for has_cursor in [true, false] {
        for channel_error in [false, true] {
            let mut f = Fixture::new();
            f.step(2.);
            if has_cursor {
                f.events
                    .emit(Event::Message(incoming("C2", "seed", None, "?", 10.)))
                    .unwrap();
                f.step(10.);
            }
            let online = f.engine.state.online["memory"];
            f.corral.agents.get_mut("ranch/main").unwrap().state = "working".into();
            f.memory
                .borrow_mut()
                .history
                .push(incoming("C2", "offline", None, "离线那句", 100.));
            if channel_error {
                f.memory.borrow_mut().channel_errors.insert("C2".into());
            } else {
                f.memory.borrow_mut().history_errors.insert("C2".into());
            }
            f.restart();
            f.step(200.);
            let live = incoming("C2", "live", None, "在线那句", 201.);
            f.memory.borrow_mut().history.push(live.clone());
            f.events.emit(Event::Message(live)).unwrap();
            f.step(202.);
            assert_eq!(f.engine.state.online["memory"], online);
            let cursor = f
                .engine
                .state
                .cursors
                .iter()
                .find(|c| c.address.channel == "C2");
            assert_eq!(cursor.map(|c| c.at), has_cursor.then_some(10.));
            f.restart();
            f.memory.borrow_mut().channel_errors.clear();
            f.memory.borrow_mut().history_errors.clear();
            f.step(204.);
            let pending: Vec<_> = f
                .engine
                .state
                .pending
                .iter()
                .filter(|p| p.agent == "ranch/main")
                .collect();
            assert_eq!(pending.iter().filter(|p| p.message == "live").count(), 1);
            let offline = pending.iter().find(|p| p.message == "offline").unwrap();
            assert!(
                offline
                    .text
                    .contains("pony 当时离线，现在才送到）\n离线那句")
            );
            assert!(f.engine.store.state().backfills.is_empty());
            assert_eq!(
                f.memory
                    .borrow()
                    .history_reads
                    .iter()
                    .rev()
                    .find(|(channel, _)| channel == "C2")
                    .unwrap()
                    .1
                    .at,
                if has_cursor { 10. } else { online }
            );
            f.step(206.);
            assert_eq!(
                f.engine
                    .state
                    .pending
                    .iter()
                    .filter(|p| p.message == "offline")
                    .count(),
                1
            );
        }
    }
}

#[test]
fn backfill_after_a_day_does_not_claim_completed_live_message_was_unsent() {
    let mut f = Fixture::new();
    f.step(2.);
    f.memory.borrow_mut().history_errors.insert("C1".into());
    f.connected();
    f.step(100.);
    let live = incoming("C1", "200", None, "继续", 200.);
    f.memory.borrow_mut().history.push(live.clone());
    f.events.emit(Event::Message(live)).unwrap();
    f.step(200.);
    assert_eq!(f.corral.sent, [("paddock/main".into(), "继续".into())]);
    let s = f.corral.agents.get_mut("paddock/main").unwrap();
    s.turn_started = Some(201.);
    s.last_event_at = Some(202.);
    f.corral.replies.insert(
        "paddock/main".into(),
        Ok(Reply {
            instance: "instance-1".into(),
            text: "做完了".into(),
            at: 202.,
        }),
    );
    f.step(202.);
    f.step(204.);
    assert!(f.engine.state.pending.is_empty());
    assert!(f.texts().contains("做完了"));
    f.engine.disconnect("memory");
    f.step(300.);
    f.memory.borrow_mut().history_errors.clear();
    f.connected();
    f.step(300. + 25. * 3600.);
    assert!(!f.texts().contains("12 小时以前发的"));
    assert_eq!(f.corral.sent.len(), 1);
    assert!(f.engine.state.backfills.is_empty());
}

#[test]
fn sleep_without_disconnect_preserves_retry_deadlines() {
    let mut f = Fixture::new();
    f.step(2.);
    f.corral.agents.get_mut("paddock/dev").unwrap().state = "working".into();
    f.message("queued", None, "dev 核对我", true);
    f.step(10.);
    f.memory.borrow_mut().exists_errors.insert("queued".into());
    f.memory.borrow_mut().history_errors.insert("C2".into());
    f.connected();
    f.step(20.);
    // No disconnect/connected event: the core wakes before the socket worker.
    for hour in 1..=13 {
        f.step(20. + f64::from(hour) * 3600.);
        assert!(f.engine.state.pending.iter().any(|p| p.message == "queued"));
        assert!(
            f.engine
                .state
                .backfills
                .iter()
                .any(|b| b.address.channel == "C2")
        );
        assert!(!f.texts().contains("请重发"));
    }
    assert_eq!(f.memory.borrow().exists_reads.len(), 14);
    f.memory.borrow_mut().exists_errors.clear();
    f.memory.borrow_mut().history_errors.clear();
    f.corral.agents.get_mut("paddock/dev").unwrap().state = "idle".into();
    f.connected();
    f.step(46822.);
    f.step(46824.);
    assert_eq!(f.corral.sent, [("paddock/dev".into(), "核对我".into())]);
    assert!(f.engine.state.backfills.is_empty());
}

#[test]
fn backfill_retry_backs_off_across_restart_and_reconnect_retries_immediately() {
    let mut f = Fixture::new();
    f.step(2.);
    f.memory.borrow_mut().history_errors.insert("C2".into());
    f.connected();
    let reads = |f: &Fixture| {
        f.memory
            .borrow()
            .history_reads
            .iter()
            .filter(|(channel, _)| channel == "C2")
            .count()
    };
    let mut attempts = Vec::new();
    for at in (100..=160).step_by(2) {
        let before = reads(&f);
        f.step(at as f64);
        if reads(&f) > before {
            attempts.push(at);
        }
    }
    assert_eq!(attempts, [100, 102, 106, 114, 130]);
    f.restart();
    f.step(161.);
    assert_eq!(reads(&f), 6);
    let mut count = reads(&f);
    for at in [225., 353., 609., 909.] {
        f.step(at - 1.);
        assert_eq!(reads(&f), count);
        f.step(at);
        count += 1;
        assert_eq!(reads(&f), count);
        f.step(at + 1.);
        assert_eq!(reads(&f), count);
        f.step(at + 63.);
        assert_eq!(reads(&f), count);
    }
    f.connected();
    f.step(973.);
    assert_eq!(reads(&f), count + 1);
}

#[test]
fn backfill_retry_after_thirteen_hours_offline_recovers_recent_message() {
    let mut f = Fixture::new();
    f.step(2.);
    f.corral.agents.get_mut("ranch/main").unwrap().state = "working".into();
    f.memory.borrow_mut().history_errors.insert("C2".into());
    f.connected();
    f.step(100.);
    f.engine.disconnect("memory");
    let wake = 100. + 13. * 3600.;
    f.step(wake - 1.);
    f.memory.borrow_mut().history_errors.clear();
    f.memory.borrow_mut().history.push(incoming(
        "C2",
        "recent",
        None,
        "醒来前两小时那句",
        wake - 2. * 3600.,
    ));
    f.connected();
    f.step(wake);
    assert!(
        f.engine
            .store
            .state()
            .pending
            .iter()
            .any(|p| p.message == "recent" && p.text.contains("醒来前两小时那句"))
    );
    assert!(!f.texts().contains("请重发"));
    assert!(f.engine.store.state().backfills.is_empty());
}

#[test]
fn queue_retry_after_one_hour_offline_checks_before_sending() {
    let mut f = Fixture::new();
    f.corral.agents.get_mut("paddock/dev").unwrap().state = "working".into();
    f.message("queued", None, "dev 核对我", true);
    f.step(10.);
    f.memory.borrow_mut().exists_errors.insert("queued".into());
    f.connected();
    f.step(20.);
    assert_eq!(f.memory.borrow().exists_reads, ["queued"]);
    f.engine.disconnect("memory");
    f.step(3619.);
    f.memory.borrow_mut().exists_errors.clear();
    f.corral.agents.get_mut("paddock/dev").unwrap().state = "idle".into();
    f.connected();
    f.step(3620.);
    assert_eq!(f.memory.borrow().exists_reads, ["queued", "queued"]);
    assert!(f.corral.sent.is_empty());
    assert!(f.engine.store.state().pending.iter().any(|p| {
        p.message == "queued"
            && p.phase == pony::core::state::Phase::Queued
            && p.verification.is_none()
    }));
    // Queued messages still require two consecutive idle observations after reconnect.
    f.step(3622.);
    assert_eq!(f.corral.sent, [("paddock/dev".into(), "核对我".into())]);
    assert!(!f.texts().contains("请重发"));
}

#[test]
fn failed_backfill_retries_then_reports_and_clears_after_twelve_hours() {
    let mut f = Fixture::new();
    f.step(2.);
    f.memory.borrow_mut().history_errors.insert("C2".into());
    f.restart();
    f.step(100.);
    f.step(102.);
    f.step(106.);
    assert_eq!(
        f.memory
            .borrow()
            .history_reads
            .iter()
            .filter(|(c, _)| c == "C2")
            .count(),
        3
    );
    f.restart();
    let deadline = 100. + 12. * 3600.;
    f.step(deadline);
    assert!(!f.texts().contains("那段时间发的话请重发"));
    // The fourth failure schedules a 16-second retry; reconnect preserves the backoff.
    f.step(deadline + 14.);
    assert!(!f.texts().contains("那段时间发的话请重发"));
    f.step(deadline + 16.);
    let data = f.memory.borrow();
    assert_eq!(
        data.history_reads.iter().filter(|(c, _)| c == "C2").count(),
        5
    );
    let notice = data
        .posted
        .iter()
        .find(|p| p.text == "重连后没读到离线期间的消息（history_failed），那段时间发的话请重发")
        .unwrap();
    assert_eq!(notice.channel, "C2");
    assert!(notice.thread.is_none());
    drop(data);
    assert!(f.engine.store.state().backfills.is_empty());
    f.memory.borrow_mut().history_errors.clear();
    f.restart();
    f.step(deadline + 18.);
    assert_eq!(f.texts().matches("那段时间发的话请重发").count(), 1);
    assert_eq!(
        f.memory
            .borrow()
            .history_reads
            .iter()
            .rev()
            .find(|(c, _)| c == "C2")
            .unwrap()
            .1
            .at,
        deadline + 16.
    );
}

#[test]
fn status_summary_keeps_other_agents_when_one_status_is_unreadable() {
    let mut f = Fixture::new();
    f.corral
        .status_errors
        .insert("paddock/dev".into(), failure(9, "incompatible"));
    f.message("summary", None, "?", true);
    f.step(10.);
    let text = f.texts();
    assert!(text.contains("• dev — 读不出"));
    assert!(text.contains("• main — 空闲"));
    assert!(text.contains("ranch（"));
    assert!(!text.contains("incompatible: reason"));
    assert!(!text.contains("private"));
}

#[test]
fn unverifiable_queue_backs_off_and_reconnect_restarts_ten_minute_deadline() {
    let mut f = Fixture::new();
    f.corral.agents.get_mut("paddock/dev").unwrap().state = "working".into();
    f.message("uncertain", None, "dev 核对我", true);
    f.message("next", None, "dev 下一句", true);
    f.step(10.);
    f.memory
        .borrow_mut()
        .exists_errors
        .insert("uncertain".into());
    f.corral.agents.get_mut("paddock/dev").unwrap().state = "idle".into();
    let mut attempts = 0;
    for connected_at in [20., 600.] {
        f.restart();
        let mut previous = connected_at;
        for elapsed in [0., 2., 6., 14., 30., 62., 126., 254., 510.] {
            // Advance awake polling too; a gap over 60 seconds now means suspension.
            while previous + 2. < connected_at + elapsed {
                previous += 2.;
                f.step(previous);
            }
            f.step(connected_at + elapsed);
            attempts += 1;
            f.step(connected_at + elapsed + 1.);
            previous = connected_at + elapsed + 1.;
            assert_eq!(
                f.memory
                    .borrow()
                    .exists_reads
                    .iter()
                    .filter(|id| *id == "uncertain")
                    .count(),
                attempts
            );
        }
    }
    assert!(f.corral.sent.is_empty());
    let check = f
        .engine
        .state
        .pending
        .iter()
        .find(|p| p.message == "uncertain")
        .unwrap()
        .verification
        .as_ref()
        .unwrap();
    assert_eq!(check.started, 600.);
    assert_eq!(check.retry_at, 1410.);
    assert_eq!(check.delay, 300.);
    for tick in (1112..1200).step_by(2) {
        f.step(f64::from(tick));
    }
    f.step(1200.);
    assert!(!f.texts().contains("没法核对这句还在不在"));
    f.step(1202.);
    f.step(1204.);
    assert_eq!(
        f.texts()
            .matches("没法核对这句还在不在，先不送；要送请重发")
            .count(),
        1
    );
    assert_eq!(f.corral.sent, [("paddock/dev".into(), "下一句".into())]);
    assert!(
        !f.engine
            .store
            .state()
            .pending
            .iter()
            .any(|p| p.message == "uncertain")
    );
    assert_eq!(
        f.memory
            .borrow()
            .exists_reads
            .iter()
            .filter(|id| *id == "uncertain")
            .count(),
        18
    );
}

#[test]
fn adapter_reserved_error_codes_do_not_abort_the_core_but_state_io_still_does() {
    for code in ["io", "state_invalid", "secrets_too_open"] {
        for operation in ["channel", "history", "exists"] {
            let mut f = Fixture::new();
            f.corral.agents.get_mut("paddock/dev").unwrap().state = "working".into();
            f.message("queued", None, "dev hi", true);
            f.step(10.);
            f.memory.borrow_mut().adapter_error_code = Some(code.into());
            match operation {
                "channel" => {
                    f.memory.borrow_mut().channel_errors.insert("C1".into());
                }
                "history" => {
                    f.memory.borrow_mut().history_errors.insert("C1".into());
                }
                _ => {
                    f.memory.borrow_mut().exists_errors.insert("queued".into());
                }
            }
            f.restart();
            f.events
                .emit(Event::Message(incoming("C2", "good", None, "继续", 12.)))
                .unwrap();
            f.step(12.);
            assert_eq!(f.corral.sent, [("ranch/main".into(), "继续".into())]);
            assert!(f.engine.log.iter().any(|line| line.ends_with(code)));
        }
    }
    let mut f = Fixture::new();
    f.step(2.);
    f.message("queued", None, "hello", true);
    let path = f._dir.path().join("state.json");
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert_eq!(f.engine.step(&mut f.corral, 10.).unwrap_err().code, "io");
    assert!(f.corral.sent.is_empty());
}
