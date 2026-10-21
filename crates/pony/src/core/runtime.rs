use super::{
    Error, Result,
    config::{self, Config},
    files,
    lock::Lock,
    log::Log,
    now, secrets,
    status::{self, Snapshot},
};
use crate::chats::CHATS;
use std::{
    io,
    path::Path,
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
    let _lock = Lock::acquire(home, snapshot.started)?;
    let log = Log::new(home)?;
    let result = signals().and_then(|()| serve(home, &log, &mut snapshot));
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

fn serve(home: &Path, log: &Log, snapshot: &mut Snapshot) -> Result<()> {
    let Some(mut active) = config::load(home)? else {
        return missing(log, snapshot);
    };
    snapshot.chats = status::chats(&active, "off", snapshot.started);
    validate_credentials(home, &active)?;
    snapshot.chats = status::chats(&active, "connecting", now());
    snapshot.save(home)?;
    log.event("run_started")?;
    while STOP.load(Ordering::Relaxed) == 0 {
        thread::sleep(Duration::from_secs(2));
        if STOP.load(Ordering::Relaxed) != 0 {
            break;
        }
        let next = match config::load(home) {
            Ok(None) => return missing(log, snapshot),
            Ok(Some(next)) if next == active && snapshot.config_error.is_none() => continue,
            Ok(Some(next)) => validate_credentials(home, &next).map(|()| next),
            Err(error) => Err(error),
        };
        match next {
            Ok(next) => {
                let mut chats = status::chats(&next, "connecting", now());
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
                    continue;
                }
                log.event(&error.code)?;
                snapshot.config_error = Some(error);
            }
        }
        snapshot.save(home)?;
    }
    snapshot.exit(
        "stopped",
        &format!("stopped by signal {}", STOP.load(Ordering::Relaxed)),
    );
    log.event("stopped")?;
    Ok(())
}
