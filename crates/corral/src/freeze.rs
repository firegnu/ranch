//! Pausing an agent: SIGSTOP for everything it started, SIGCONT to continue.
//!
//! What it started is every descendant, plus every process in a session one of them is in.
//! The sessions catch what left the tree: a child whose parent exited is adopted by init but
//! stays in its session. Only a process that leaves for a session of its own and loses its
//! parent before it is seen (a daemon detaching at that very moment) can escape.
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

struct Proc {
    ppid: i32,
    sid: i32,
    /// Stopped, or already exiting; either way it does nothing more.
    still: bool,
    zombie: bool,
}

/// Whether the process has exited, even if nobody has reaped it yet.
pub(crate) fn exited(pid: i32) -> bool {
    table().get(&pid).is_none_or(|p| p.zombie)
}

/// Stops the agent's group, then what it started, until a fresh look finds nothing new.
/// Returns what it stopped once every one of them is confirmed stopped; otherwise continues
/// them all again and returns the ones that would not stop.
pub(crate) fn freeze(agent: i32) -> Result<Vec<i32>, Vec<i32>> {
    unsafe {
        libc::kill(-agent, libc::SIGSTOP);
    }
    let mut sessions = BTreeSet::from([agent]);
    let mut frozen = BTreeSet::new();
    loop {
        let fresh: Vec<_> = members(agent, &mut sessions, &table())
            .into_iter()
            .filter(|pid| frozen.insert(*pid))
            .collect();
        if fresh.is_empty() {
            break;
        }
        for pid in fresh {
            unsafe {
                libc::kill(pid, libc::SIGSTOP);
            }
        }
    }
    let frozen: Vec<_> = frozen.into_iter().collect();
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let table = table();
        let running: Vec<_> = std::iter::once(agent)
            .chain(frozen.iter().copied())
            .filter(|pid| table.get(pid).is_some_and(|p| !p.still))
            .collect();
        if running.is_empty() {
            return Ok(frozen);
        }
        if Instant::now() >= deadline {
            thaw(agent, &frozen);
            return Err(running);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Continues what `freeze` stopped and whatever the agent has started since. SIGCONT to a
/// running process does nothing, so continuing too much is harmless; too little is not.
pub(crate) fn thaw(agent: i32, frozen: &[i32]) {
    let table = table();
    let mut sessions: BTreeSet<i32> = frozen
        .iter()
        .filter_map(|pid| Some(table.get(pid)?.sid))
        .collect();
    sessions.insert(agent);
    let mut all: BTreeSet<i32> = frozen.iter().copied().collect();
    all.extend(members(agent, &mut sessions, &table));
    for pid in all {
        unsafe {
            libc::kill(pid, libc::SIGCONT);
        }
    }
    unsafe {
        libc::kill(-agent, libc::SIGCONT);
    }
}

/// The agent's descendants and everyone in their sessions, adding those sessions as it goes.
/// A session id is its leader's pid, which the system does not reuse while the session lives.
fn members(agent: i32, sessions: &mut BTreeSet<i32>, table: &BTreeMap<i32, Proc>) -> Vec<i32> {
    let below = |mut pid: i32| {
        for _ in 0..table.len() {
            match table.get(&pid) {
                Some(p) if p.ppid == agent => return true,
                Some(p) if p.ppid > 1 => pid = p.ppid,
                _ => return false,
            }
        }
        false
    };
    let mut out = BTreeSet::new();
    loop {
        let mut grew = false;
        for (&pid, p) in table {
            if pid > 1
                && pid != agent
                && !out.contains(&pid)
                && (sessions.contains(&p.sid) || below(pid))
            {
                out.insert(pid);
                sessions.insert(p.sid);
                grew = true;
            }
        }
        if !grew {
            return out.into_iter().collect();
        }
    }
}

#[cfg(target_os = "macos")]
fn table() -> BTreeMap<i32, Proc> {
    let mut size = 4096;
    let pids = loop {
        let mut pids = vec![0 as libc::pid_t; size];
        let bytes = (size * std::mem::size_of::<libc::pid_t>()) as libc::c_int;
        let n = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), bytes) };
        if n < 0 {
            return BTreeMap::new();
        }
        if (n as usize) < size {
            pids.truncate(n as usize);
            break pids;
        }
        size *= 4;
    };
    pids.into_iter()
        .filter_map(|pid| {
            let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
            let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
            let n = unsafe {
                libc::proc_pidinfo(
                    pid,
                    libc::PROC_PIDTBSDINFO,
                    0,
                    (&mut info as *mut libc::proc_bsdinfo).cast(),
                    size,
                )
            };
            let sid = unsafe { libc::getsid(pid) };
            (n == size && sid >= 0).then(|| {
                let proc = Proc {
                    ppid: info.pbi_ppid as i32,
                    sid,
                    still: matches!(info.pbi_status, libc::SSTOP | libc::SZOMB),
                    zombie: info.pbi_status == libc::SZOMB,
                };
                (pid, proc)
            })
        })
        .collect()
}

#[cfg(not(target_os = "macos"))]
fn table() -> BTreeMap<i32, Proc> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return BTreeMap::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let pid = e.file_name().to_str()?.parse::<i32>().ok()?;
            let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
            // The command name is in parentheses and may contain spaces.
            let mut fields = stat[stat.rfind(')')? + 2..].split(' ');
            let state = fields.next()?;
            let ppid = fields.next()?.parse().ok()?;
            let _pgrp = fields.next()?;
            let sid = fields.next()?.parse().ok()?;
            let still = matches!(state, "T" | "t" | "Z" | "X");
            let zombie = matches!(state, "Z" | "X");
            Some((
                pid,
                Proc {
                    ppid,
                    sid,
                    still,
                    zombie,
                },
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    fn eventually(check: impl Fn() -> bool) {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !check() {
            assert!(std::time::Instant::now() < end);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    fn stopped(pid: i32) -> bool {
        let out = Command::new("ps")
            .args(["-o", "stat=", "-p", &pid.to_string()])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().starts_with('T')
    }

    #[test]
    fn freezes_and_thaws_a_child_in_its_own_session() {
        // A shell in its own group starts a grandchild that leaves for its own session.
        let mut root = Command::new("/bin/sh")
            .args([
                "-c",
                "/usr/bin/perl -MPOSIX -e 'POSIX::setsid(); $|=1; print \"$$\\n\"; sleep 30' & wait",
            ])
            .stdout(Stdio::piped())
            .process_group(0)
            .spawn()
            .unwrap();
        let mut line = String::new();
        std::io::BufRead::read_line(
            &mut std::io::BufReader::new(root.stdout.as_mut().unwrap()),
            &mut line,
        )
        .unwrap();
        let grandchild: i32 = line.trim().parse().unwrap();
        let agent = root.id() as i32;
        let frozen = freeze(agent).unwrap();
        assert!(frozen.contains(&grandchild), "{frozen:?}");
        eventually(|| stopped(agent) && stopped(grandchild));
        thaw(agent, &frozen);
        eventually(|| !stopped(agent) && !stopped(grandchild));
        unsafe {
            libc::kill(grandchild, libc::SIGKILL);
            libc::kill(-agent, libc::SIGKILL);
        }
        let _ = root.wait();
    }
}
