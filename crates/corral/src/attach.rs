use crate::{Error, Result, state, terminal::Terminal};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    os::{fd::AsRawFd, unix::net::UnixStream},
    thread,
    time::Duration,
};
fn size() -> (u16, u16) {
    let mut s: libc::winsize = unsafe { std::mem::zeroed() };
    if unsafe { libc::ioctl(1, libc::TIOCGWINSZ as _, &mut s) } < 0 {
        (40, 120)
    } else {
        (s.ws_row.max(1), s.ws_col.max(2))
    }
}
fn frame(s: &mut UnixStream, typ: u8, data: &[u8]) -> std::io::Result<()> {
    s.write_all(&[typ])?;
    s.write_all(&(data.len() as u32).to_be_bytes())?;
    s.write_all(data)
}
struct Restore {
    old: libc::termios,
    term: Terminal,
}
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = std::io::stdout().write_all(&self.term.restore());
        let _ = std::io::stdout().write_all(b"\x1b[23;0t");
        let _ = std::io::stdout().flush();
        unsafe {
            libc::tcsetattr(0, libc::TCSADRAIN, &self.old);
        }
    }
}
static INTERRUPTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
extern "C" fn interrupt(_: i32) {
    INTERRUPTED.store(true, std::sync::atomic::Ordering::Relaxed);
}
struct Interrupt(libc::sighandler_t);
impl Drop for Interrupt {
    fn drop(&mut self) {
        unsafe {
            libc::signal(libc::SIGINT, self.0);
        }
    }
}
pub fn run(name: &str, wait: bool) -> Result<()> {
    if !wait {
        state::require(name, json!({"op":"status"}))?;
    }
    let _interrupt = wait.then(|| {
        INTERRUPTED.store(false, std::sync::atomic::Ordering::Relaxed);
        Interrupt(unsafe {
            libc::signal(libc::SIGINT, interrupt as *const () as libc::sighandler_t)
        })
    });

    if unsafe { libc::isatty(0) == 0 || libc::isatty(1) == 0 } {
        return Err(Error::new(
            1,
            "not_a_tty",
            "attach needs a terminal on stdin and stdout",
        ));
    }
    loop {
        let mut announced = false;
        let mut socket = loop {
            if wait && INTERRUPTED.load(std::sync::atomic::Ordering::Relaxed) {
                return Ok(());
            }
            match UnixStream::connect(state::socket(name)?) {
                Ok(s) => break s,
                Err(_) if wait => {
                    if !announced {
                        eprintln!("\x1b]2;waiting for {name}\x07waiting for {name} ...\r");
                        announced = true;
                    }
                    thread::sleep(Duration::from_millis(100));
                }

                Err(_) => return Err(state::not_found(name)),
            }
        };
        let mut dimensions = size();
        let (r, c) = dimensions;
        writeln!(
            socket,
            "{}",
            json!({"proto":1,"op":"attach","rows":r,"cols":c})
        )?;
        let mut line = Vec::new();
        let mut b = [0];
        while socket.read(&mut b)? != 0 {
            line.push(b[0]);
            if b[0] == b'\n' {
                break;
            }
            if line.len() > 16 * 1024 * 1024 {
                return Err(Error::new(1, "bad_reply", "attach reply too long"));
            }
        }
        let hello: Value = serde_json::from_slice(&line)?;
        state::check_proto(&hello["proto"])?;
        if hello["ok"] != true {
            return Err(Error::new(1, "attach_failed", "pen rejected attach"));
        }
        let mut old = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(0, &mut old) } < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let mut raw = old;
        unsafe {
            libc::cfmakeraw(&mut raw);
            if libc::tcsetattr(0, libc::TCSANOW, &raw) < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        let mut restore = Restore {
            old,
            term: Terminal::default(),
        };
        print!("\x1b[22;0t\x1b]2;{name}\x07");
        std::io::stdout().flush()?;
        let mut detached = false;
        loop {
            let mut p = [
                libc::pollfd {
                    fd: 0,
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd: socket.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                },
            ];
            if unsafe { libc::poll(p.as_mut_ptr(), 2, 100) } < 0 {
                if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(std::io::Error::last_os_error().into());
            }
            if p[0].revents & (libc::POLLIN | libc::POLLHUP) != 0 {
                let mut data = [0; 65536];
                let n = std::io::stdin().read(&mut data)?;
                if n == 0 {
                    detached = true;
                    break;
                }
                if let Some(i) = crate::terminal::detach(&data[..n]) {
                    if i > 0 {
                        frame(&mut socket, b'i', &data[..i])?
                    }
                    detached = true;
                    break;
                }
                frame(&mut socket, b'i', &data[..n])?;
            }
            if p[1].revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0 {
                let mut data = [0; 65536];
                let n = socket.read(&mut data).unwrap_or(0);
                if n == 0 {
                    break;
                }
                restore.term.feed(&data[..n]);
                std::io::stdout().write_all(&data[..n])?;
                std::io::stdout().flush()?;
            }
            let new_size = size();
            if new_size != dimensions {
                dimensions = new_size;
                let (r, c) = dimensions;
                frame(
                    &mut socket,
                    b'r',
                    &[r.to_be_bytes(), c.to_be_bytes()].concat(),
                )?;
            }
        }
        drop(restore);
        if detached {
            eprintln!("\r\n[corral] detached from {name}; it keeps running");
            return Ok(());
        }
        if !wait {
            eprintln!("\r\n[corral] {name} exited\r");
            return Err(Error {
                code: 2,
                value: Value::Null,
            });
        }
    }
}
