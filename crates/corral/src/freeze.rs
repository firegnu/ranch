//! Pausing an agent: SIGSTOP for its process group and every descendant, SIGCONT to continue.
//! Descendants that left the group (their own group or session) are found through the tree.
use std::collections::BTreeSet;

/// Stops the agent's group, then every descendant, until a pass finds none new. A stopped
/// parent cannot fork, so this ends. Returns the descendants that were signalled.
pub(crate) fn freeze(agent: i32) -> Vec<i32> {
    unsafe {
        libc::kill(-agent, libc::SIGSTOP);
    }
    let mut frozen = BTreeSet::new();
    loop {
        let fresh: Vec<_> = descendants(agent)
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
    frozen.into_iter().collect()
}

/// Continues what `freeze` stopped and whatever is in the tree now. SIGCONT to a running
/// process does nothing, so continuing too much is harmless; continuing too little is not.
pub(crate) fn thaw(agent: i32, frozen: &[i32]) {
    let mut all: BTreeSet<i32> = frozen.iter().copied().collect();
    all.extend(descendants(agent));
    for pid in all {
        unsafe {
            libc::kill(pid, libc::SIGCONT);
        }
    }
    unsafe {
        libc::kill(-agent, libc::SIGCONT);
    }
}

fn descendants(root: i32) -> Vec<i32> {
    let mut out = Vec::new();
    let mut queue = vec![root];
    while let Some(pid) = queue.pop() {
        for child in children(pid) {
            if child > 0 && child != root && !out.contains(&child) {
                out.push(child);
                queue.push(child);
            }
        }
    }
    out
}

#[cfg(target_os = "macos")]
fn children(pid: i32) -> Vec<i32> {
    let mut size = 256;
    loop {
        let mut buffer = vec![0 as libc::pid_t; size];
        let bytes = (size * std::mem::size_of::<libc::pid_t>()) as libc::c_int;
        let n = unsafe { libc::proc_listchildpids(pid, buffer.as_mut_ptr().cast(), bytes) };
        if n < 0 {
            return Vec::new();
        }
        let n = n as usize;
        if n < size {
            buffer.truncate(n);
            return buffer;
        }
        size *= 4;
    }
}

#[cfg(not(target_os = "macos"))]
fn children(pid: i32) -> Vec<i32> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.parse::<i32>().ok())
        .filter(|child| {
            std::fs::read_to_string(format!("/proc/{child}/stat"))
                .ok()
                .and_then(|stat| {
                    // The command name is in parentheses and may contain spaces.
                    let rest = &stat[stat.rfind(')')? + 2..];
                    rest.split(' ').nth(1)?.parse::<i32>().ok()
                })
                == Some(pid)
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
        let frozen = freeze(agent);
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
