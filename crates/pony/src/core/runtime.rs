use super::{
    Error, Result,
    config::{self, Config},
    files,
    lock::Lock,
    log::Log,
    now, secrets,
    status::{self, Snapshot},
};
use super::{engine::Engine, state::Store};
use crate::{
    chats::{CHATS, Connect, Connection},
    corral::Process,
};
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
    sync::Arc,
    sync::atomic::{AtomicI32, Ordering},
    thread,
    time::Duration,
};

static STOP: AtomicI32 = AtomicI32::new(0);

extern "C" fn stop(signal: libc::c_int) {
    STOP.store(signal, Ordering::Relaxed);
}

fn signals() -> Result<()> {
    let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
    action.sa_sigaction = stop as *const () as usize;
    unsafe {
        libc::sigemptyset(&mut action.sa_mask);
    }
    for signal in [libc::SIGTERM, libc::SIGINT] {
        if unsafe { libc::sigaction(signal, &action, std::ptr::null_mut()) } < 0 {
            return Err(Error::new(
                "io",
                format!("signal handler: {}", io::Error::last_os_error()),
            ));
        }
    }
    Ok(())
}

pub fn run(home: &Path) -> Result<()> {
    files::directory(home, true)?;
    files::directory(&home.join("run"), true)?;
    let mut snapshot = Snapshot::new();
    signals()?;
    let _lock = Lock::acquire(home, snapshot.started)?;
    let log = Log::new(home)?;
    let result = serve(home, &log, &mut snapshot);
    if let Err(error) = &result {
        snapshot.exit(&error.code, &error.message);
        snapshot.config_error = Some(error.clone());
        log.event(&error.code)?;
    }
    snapshot.save(home)?;
    result
}

fn validate_credentials(home: &Path, config: &Config) -> Result<()> {
    for adapter in CHATS {
        if config.chats.iter().any(|chat| chat.kind == adapter.kind()) {
            // The shell validates but does not connect; R38e will consume these values.
            secrets::load(home, *adapter)?;
        }
    }
    Ok(())
}

fn missing(log: &Log, snapshot: &mut Snapshot) -> Result<()> {
    log.event("config_missing")?;
    snapshot.chats.clear();
    snapshot.exit("config_missing", "config.toml is missing");
    Ok(())
}

fn reload_config(
    home: &Path,
    active: &Config,
    had_error: bool,
    config_file: &mut Option<config::ConfigFile>,
) -> Result<Option<Config>> {
    let next = config::load_with_source(home).and_then(|loaded| {
        if let Some((next, _)) = &loaded
            && (next != active || had_error)
        {
            validate_credentials(home, next)?;
        }
        Ok(loaded)
    });
    match next {
        Ok(Some((next, file))) => {
            *config_file = Some(file);
            Ok(Some(next))
        }
        Ok(None) => Ok(None),
        Err(error) => {
            *config_file = None;
            Err(error)
        }
    }
}

fn serve(home: &Path, log: &Log, snapshot: &mut Snapshot) -> Result<()> {
    let Some((mut active, config_file)) = config::load_with_source(home)? else {
        return missing(log, snapshot);
    };
    snapshot.chats = status::chats(&active, "off", None);
    snapshot.save(home)?;
    validate_credentials(home, &active)?;
    let mut corral = open_corral(&active)?;
    let store = Store::open(home)?;
    let mut engine = Engine::new(
        store.clone(),
        active.chats.clone(),
        connect(home, &active, &store)?,
    );
    engine.config_file = Some(config_file);
    snapshot.chats = status::chats(&active, "connecting", Some(now()));
    snapshot.save(home)?;
    log.event("run_started")?;
    while STOP.load(Ordering::Relaxed) == 0 {
        thread::sleep(Duration::from_secs(
            if engine.state.interrupts.is_empty() && engine.state.approvals.is_empty() {
                2
            } else {
                1
            },
        ));
        if STOP.load(Ordering::Relaxed) != 0 {
            break;
        }
        let next = match reload_config(
            home,
            &active,
            snapshot.config_error.is_some(),
            &mut engine.config_file,
        ) {
            Ok(None) => return missing(log, snapshot),
            Ok(Some(next)) => Ok(next),
            Err(error) => Err(error),
        };
        match next {
            Ok(next) if next == active => {
                snapshot.config_error = None;
            }
            Ok(next) => {
                let updated_corral = if next.corral != active.corral {
                    Some(open_corral(&next)?)
                } else {
                    None
                };
                let mut connections = BTreeMap::new();
                for config in &next.chats {
                    if next.proxy == active.proxy && active.chats.iter().any(|old| old == config) {
                        if let Some(connection) = engine.connections.remove(&config.kind) {
                            connections.insert(config.kind.clone(), connection);
                        }
                    } else {
                        if let Some(mut connection) = engine.connections.remove(&config.kind) {
                            connection.close();
                        }
                        engine.disconnect(&config.kind);
                        let one = Config {
                            corral: next.corral.clone(),
                            proxy: next.proxy.clone(),
                            chats: vec![config.clone()],
                        };
                        connections.extend(connect(home, &one, &store)?);
                    }
                }
                for connection in engine.connections.values_mut() {
                    connection.close();
                }
                engine.connections = connections;
                engine.configs = next.chats.clone();
                engine.reports.retain(|kind, _| {
                    next.proxy == active.proxy
                        && next
                            .chats
                            .iter()
                            .any(|c| c.kind == *kind && active.chats.contains(c))
                });
                engine
                    .channels
                    .retain(|(kind, _), _| engine.reports.contains_key(kind));
                if let Some(updated) = updated_corral {
                    corral = updated;
                }
                let mut chats = status::chats(&next, "connecting", Some(now()));
                if next.proxy == active.proxy {
                    for chat in &mut chats {
                        let previous = active.chats.iter().find(|c| c.kind == chat.kind);
                        let updated = next.chats.iter().find(|c| c.kind == chat.kind);
                        if previous == updated
                            && let Some(old) = snapshot.chats.iter().find(|c| c.kind == chat.kind)
                        {
                            *chat = old.clone();
                        }
                    }
                }
                snapshot.chats = chats;
                snapshot.config_error = None;
                active = next;
                log.event("config_reloaded")?;
            }
            Err(error) => {
                if snapshot
                    .config_error
                    .as_ref()
                    .is_some_and(|old| old.code == error.code && old.message == error.message)
                {
                    // Keep serving the last valid configuration.
                } else {
                    log.event(&error.code)?;
                    snapshot.config_error = Some(error);
                }
            }
        }
        if let Err(error) = engine.step(&mut corral, now()) {
            if matches!(
                error.code.as_str(),
                "io" | "state_invalid" | "secrets_too_open"
            ) {
                return Err(error);
            }
            // Adapter/corral errors never put reply bodies or command stderr in logs.
            log.event(&super::engine::safe_code(&error.code))?;
        }
        for entry in engine.log.drain(..) {
            log.event(&entry)?;
        }
        snapshot.update(&engine, now());
        snapshot.save(home)?;
    }
    snapshot.exit(
        "stopped",
        &format!("stopped by signal {}", STOP.load(Ordering::Relaxed)),
    );
    for connection in engine.connections.values_mut() {
        connection.close();
    }
    log.event("stopped")?;
    Ok(())
}

fn open_corral(config: &Config) -> Result<Process> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| Error::new("corral_unavailable", "HOME is not set"))?;
    Process::discover(
        config.corral.as_deref(),
        std::env::var_os("PATH").as_deref(),
        &home,
    )
    .map_err(|e| Error::new(&e.code, e.message))
}

fn connect(
    home: &Path,
    config: &Config,
    store: &Store,
) -> Result<BTreeMap<String, Box<dyn Connection>>> {
    let mut connections = BTreeMap::new();
    for adapter in CHATS {
        let Some(chat) = config.chats.iter().find(|c| c.kind == adapter.kind()) else {
            continue;
        };
        let home = home.to_path_buf();
        let adapter = *adapter;
        let connection = adapter.connect(Connect {
            config: chat.clone(),
            proxy: config.proxy.clone(),
            events: store.events(&chat.kind, &chat.owner),
            credentials: Arc::new(move || secrets::load(&home, adapter)),
        })?;
        connections.insert(chat.kind.clone(), connection);
    }
    Ok(connections)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt};

    #[test]
    fn reload_error_disables_binding_until_credentials_are_valid() {
        let home = tempfile::tempdir().unwrap();
        fs::set_permissions(home.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let kind = CHATS[0].kind();
        let source = format!("[{kind}]\nowner = 'UOWNER'\nchannels = []\n");
        let path = home.path().join("config.toml");
        fs::write(&path, &source).unwrap();
        let (active, file) = config::load_with_source(home.path()).unwrap().unwrap();
        let mut config_file = Some(file);
        let changed = format!("proxy = 'http://localhost:9'\n{source}");
        fs::write(&path, &changed).unwrap();
        for had_error in [false, true] {
            let error = reload_config(home.path(), &active, had_error, &mut config_file)
                .err()
                .unwrap();
            assert_eq!(error.code, "secrets_missing");
            assert!(config_file.is_none());
            assert_eq!(fs::read_to_string(&path).unwrap(), changed);
        }
        let secrets = home.path().join("secrets");
        fs::create_dir(&secrets).unwrap();
        fs::set_permissions(&secrets, fs::Permissions::from_mode(0o700)).unwrap();
        let credentials = CHATS[0]
            .secret_keys()
            .iter()
            .map(|key| format!("{key} = 'test-only'\n"))
            .collect::<String>();
        let path = secrets.join(format!("{kind}.toml"));
        fs::write(&path, credentials).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
        let updated = reload_config(home.path(), &active, true, &mut config_file)
            .unwrap()
            .unwrap();
        assert_eq!(updated.proxy.as_deref(), Some("http://localhost:9"));
        config_file.unwrap().bind(kind, "CNEW", "new").unwrap();
        assert_eq!(
            config::load(home.path()).unwrap().unwrap().chats[0].channels[0].repo,
            "new"
        );
    }
}
