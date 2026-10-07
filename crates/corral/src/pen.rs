// From Saddle `crates/corral-core/src/pen.rs` at commit `a31dea2`; ranch has changed it since.
use crate::{Error, Result, now, state};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
pub(crate) mod upgrade;
use std::{
    collections::{BTreeMap, VecDeque},
    fs::{self, File},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            fs::PermissionsExt,
            net::{UnixListener, UnixStream},
            process::{CommandExt, ExitStatusExt},
        },
    },
    path::PathBuf,
    process::{Command, Stdio},
};
use upgrade::{Custody, Handle, Upgrade};

pub fn start(cfg: Value) -> Result<Value> {
    let mut command = Command::new(crate::executable()?);
    command
        .arg("__pen")
        .env("CORRAL_HOME", std::path::absolute(state::home())?)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn()?;
    serde_json::to_writer(child.stdin.take().unwrap(), &cfg)?;
    let v = read_ready(
        child.stdout.take().unwrap(),
        std::time::Duration::from_secs(25),
    )?;
    if v["ok"] != true {
        let _ = child.wait();
        return Err(Error {
            code: v["code"].as_i64().unwrap_or(1) as i32,
            value: v,
        });
    }
    Ok(v)
}
fn read_ready(mut output: impl Read + AsRawFd, timeout: std::time::Duration) -> Result<Value> {
    let deadline = std::time::Instant::now() + timeout;
    let mut ready = Vec::new();
    while let Some(remaining) = deadline.checked_duration_since(std::time::Instant::now()) {
        let mut fd = libc::pollfd {
            fd: output.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let rc = unsafe {
            libc::poll(
                &mut fd,
                1,
                remaining.as_millis().max(1).min(i32::MAX as u128) as i32,
            )
        };
        if rc == 0 {
            break;
        }
        if rc < 0 {
            if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(std::io::Error::last_os_error().into());
        }
        let mut bytes = [0; 4096];
        let n = match output.read(&mut bytes) {
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        };
        if n == 0 {
            break;
        }
        ready.extend_from_slice(&bytes[..n]);
        if let Some(end) = ready.iter().position(|b| *b == b'\n') {
            return serde_json::from_slice(&ready[..end])
                .map_err(|_| Error::new(1, "pen_failed", "pen did not report readiness"));
        }
        if ready.len() > 1024 * 1024 {
            break;
        }
    }
    Err(Error::new(
        1,
        "pen_failed",
        "pen did not report readiness before the deadline",
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readiness_deadline_includes_a_partial_line() {
        let (reader, mut writer) = UnixStream::pair().unwrap();
        let worker = std::thread::spawn(move || {
            writer.write_all(b"{\"ok\":").unwrap();
            std::thread::sleep(std::time::Duration::from_millis(350));
        });
        let before = std::time::Instant::now();
        let result = read_ready(reader, std::time::Duration::from_millis(50));
        let elapsed = before.elapsed();
        worker.join().unwrap();
        assert!(result.is_err());
        assert!(
            elapsed < std::time::Duration::from_millis(250),
            "waited {elapsed:?}"
        );
    }
}
#[derive(PartialEq, Serialize, Deserialize)]

enum Mode {
    Handshake,
    Waiting,
    Closing,
    Attached,
}
#[derive(Serialize, Deserialize)]
struct Client {
    sock: Handle<UnixStream>,
    input: Vec<u8>,
    output: VecDeque<u8>,
    mode: Mode,
    rows: u16,
    cols: u16,
    order: u64,
}
impl Client {
    fn reply(&mut self, mut value: Value) {
        value["proto"] = json!(1);
        self.output.extend(serde_json::to_vec(&value).unwrap());
        self.output.push_back(b'\n');
        self.mode = Mode::Closing;
    }
}
#[derive(Serialize, Deserialize)]
struct Chunk {
    bytes: VecDeque<u8>,
    delay: f64,
    reply: Option<(i32, u64)>,
    request_id: Option<String>,
    failed: bool,
}
#[derive(Serialize, Deserialize)]
struct Pen {
    agent: u32,
    master: Handle<File>,
    listener: Handle<UnixListener>,
    lock: Handle<File>,
    custody: Custody,
    observer: bool,
    upgrade: Upgrade,
    #[serde(skip)]
    owns: bool,
    #[serde(skip)]
    pending_upgrade: Option<PathBuf>,
    dir: PathBuf,
    metadata: Value,
    labels: Value,
    instance: String,
    started: f64,
    clients: BTreeMap<i32, Client>,
    ring: VecDeque<u8>,
    last_output: Option<f64>,
    input: VecDeque<Chunk>,
    next_input: f64,
    stopping: bool,
    stop_at: Option<f64>,
    stopped_by: Value,
    exit: Option<i32>,
    exit_at: f64,
    pty_open: bool,
    writer: Option<i32>,
    next_order: u64,
    size: (u16, u16),
    resize_at: Option<f64>,
    term: crate::terminal::Terminal,
    human: Option<f64>,
    recent: Vec<Value>,
    stop_steps: VecDeque<Value>,
    // Pens before pausing existed leave these out of their snapshots.
    #[serde(default)]
    paused: Option<f64>,
    #[serde(default)]
    frozen: Vec<i32>,
    #[serde(default)]
    resumed_at: Option<f64>,
}
impl Drop for Pen {
    fn drop(&mut self) {
        if !self.owns || self.custody == Custody::Handoff {
            return;
        }
        if self.exit.is_none() && self.custody == Custody::Owner {
            // SIGKILL ends the stopped group; descendants that left it must not stay frozen.
            self.resume();
            unsafe {
                libc::kill(-(self.agent as i32), libc::SIGKILL);
                libc::waitpid(self.agent as i32, std::ptr::null_mut(), 0);
            }
        }
        let _ = fs::remove_file(self.dir.join("sock"));
    }
}
impl Pen {
    fn resume(&mut self) {
        if self.paused.take().is_some() {
            crate::freeze::thaw(self.agent as i32, &self.frozen);
            self.frozen.clear();
            self.resumed_at = Some(now());
        }
    }
    fn stop_step(&mut self) {
        if let Some(step) = self.stop_steps.pop_front() {
            if let Some(keys) = step["keys"].as_str() {
                if let Ok(bytes) = B64.decode(keys) {
                    self.input.push_back(Chunk {
                        bytes: bytes.into(),
                        delay: 0.0,
                        reply: None,
                        request_id: None,
                        failed: false,
                    });
                }
                self.stopped_by = json!("keys");
            } else {
                let sig = match step["signal"].as_str() {
                    Some("HUP") => libc::SIGHUP,
                    Some("INT") => libc::SIGINT,
                    Some("TERM") => libc::SIGTERM,
                    _ => libc::SIGKILL,
                };
                unsafe {
                    libc::kill(-(self.agent as i32), sig);
                }
                self.stopped_by =
                    json!(format!("SIG{}", step["signal"].as_str().unwrap_or("KILL")));
            }
            self.stop_at = if self.stop_steps.is_empty() {
                None
            } else {
                Some(now() + step["wait"].as_f64().unwrap_or(3.0).clamp(0.0, 60.0))
            };
        }
    }
    fn status(&self) -> Value {
        let mut status = json!({"ok":true,"proto":1,"instance":self.instance,"agent_pid":self.agent,"pen_pid":std::process::id(),"started":self.started,"attached":self.clients.values().filter(|c|c.mode==Mode::Attached).count(),"writer_attached":self.writer.is_some(),"last_output":self.last_output,"title":self.term.title,"last_human_input":self.human,"size":[self.size.0,self.size.1],"bracketed_paste":self.term.paste(),"recent_sends":self.recent,"paused":self.paused.is_some(),"paused_at":self.paused,"resumed_at":self.resumed_at});
        status["exe"] = json!(crate::executable().ok());
        status["custody"] = json!(self.custody);
        status["capabilities"] =
            json!({"upgrade":1,"recover":1,"snapshot":upgrade::SCHEMA,"pause":1});
        status["upgrade"] = self.upgrade.public();
        status
    }
    fn resize(&mut self, rows: u16, cols: u16, redraw: bool) {
        self.size = (rows.max(1), cols.max(1));
        let s = libc::winsize {
            ws_row: self.size.0,
            ws_col: if redraw {
                self.size.1.saturating_sub(1).max(1)
            } else {
                self.size.1
            },
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        unsafe {
            libc::ioctl(self.master.as_raw_fd(), libc::TIOCSWINSZ as _, &s);
        }
        if redraw {
            self.resize_at = Some(now() + 0.15);
        }
    }
    fn drop_client(&mut self, fd: i32) {
        self.clients.remove(&fd);
        if self.writer == Some(fd) {
            self.writer = self
                .clients
                .iter()
                .filter(|(_, c)| c.mode == Mode::Attached)
                .min_by_key(|(_, c)| c.order)
                .map(|(fd, _)| *fd);
            if let Some(c) = self.writer.and_then(|fd| self.clients.get(&fd)) {
                self.resize(c.rows, c.cols, true);
            }
        }
    }
    fn frames(&mut self, fd: i32) {
        loop {
            let Some(c) = self.clients.get_mut(&fd) else {
                return;
            };
            if c.input.len() < 5 {
                return;
            }
            let n = u32::from_be_bytes(c.input[1..5].try_into().unwrap()) as usize;
            if n > 16 * 1024 * 1024 {
                self.drop_client(fd);
                return;
            }
            if c.input.len() < 5 + n {
                return;
            }
            let typ = c.input[0];
            let bytes = c.input[5..5 + n].to_vec();
            c.input.drain(..5 + n);
            if self.writer != Some(fd) {
                continue;
            }
            if typ == b'i' && self.paused.is_some() {
                // Typing into a paused agent would pile up in the terminal until it continues.
                continue;
            }
            if typ == b'i' {
                if crate::terminal::human(&bytes) {
                    self.human = Some(now());
                }
                self.input.push_back(Chunk {
                    bytes: bytes.into(),
                    delay: 0.0,
                    reply: None,
                    request_id: None,
                    failed: false,
                });
            } else if typ == b'r' && bytes.len() == 4 {
                let rows = u16::from_be_bytes(bytes[..2].try_into().unwrap()).max(1);
                let cols = u16::from_be_bytes(bytes[2..].try_into().unwrap()).max(1);
                let c = self.clients.get_mut(&fd).unwrap();
                c.rows = rows;
                c.cols = cols;
                self.resize(rows, cols, false);
            }
        }
    }
    fn request(&mut self, fd: i32, req: Value) {
        if let Some(instance) = req["instance"].as_str()
            && instance != self.instance
        {
            self.clients
                .get_mut(&fd)
                .unwrap()
                .reply(json!({"ok":false,"error":"restarted","message":"instance changed"}));
            return;
        }
        let reply = match req["op"].as_str() {
            Some("status") => self.status(),
            Some("upgrade") => self.prepare_upgrade(&req),
            Some("recover") => {
                json!({"ok":false,"error":"not_hold","message":"recover requires Hold"})
            }
            Some("attach") => {
                let readonly = self.writer.is_some();
                let c = self.clients.get_mut(&fd).unwrap();
                c.rows = req["rows"].as_u64().unwrap_or(40).clamp(1, 10000) as u16;
                c.cols = req["cols"].as_u64().unwrap_or(120).clamp(1, 10000) as u16;
                c.reply(json!({"ok":true,"instance":self.instance,"readonly":readonly}));
                c.mode = Mode::Attached;
                c.output.extend(self.term.replay());
                let (rows, cols) = (c.rows, c.cols);
                if !readonly {
                    self.writer = Some(fd);
                    self.resize(rows, cols, true);
                }
                self.frames(fd);
                return;
            }
            Some("read") => {
                let n = req["bytes"].as_u64().unwrap_or(16000) as usize;
                json!({"ok":true,"data":B64.encode(self.ring.iter().skip(self.ring.len().saturating_sub(n)).copied().collect::<Vec<_>>())})
            }
            Some("pause") => {
                if self.exit.is_some() || self.stopping {
                    json!({"ok":false,"error":"stopping","message":"the agent is stopping"})
                } else {
                    if self.paused.is_none() {
                        self.frozen = crate::freeze::freeze(self.agent as i32);
                        self.paused = Some(now());
                    }
                    json!({"ok":true,"instance":self.instance,"paused":true,"paused_at":self.paused})
                }
            }
            Some("resume") => {
                self.resume();
                json!({"ok":true,"instance":self.instance,"paused":false,"paused_at":null})
            }
            Some("send" | "keys") if self.paused.is_some() => {
                json!({"ok":false,"error":"paused","message":"the agent is paused; resume it first"})
            }
            Some("send" | "keys") => {
                if req["op"] == "send"
                    && req["force"] != true
                    && self.human.is_some_and(|t| now() - t < 30.0)
                {
                    self.clients.get_mut(&fd).unwrap().reply(json!({"ok":false,"error":"human_active","last_human_input":self.human,"message":"someone typed in an attached window recently"}));
                    return;
                }
                let parsed = (|| -> Option<Vec<Chunk>> {
                    let values = req["chunks"].as_array()?;
                    let mut out = Vec::new();
                    for (i, v) in values.iter().enumerate() {
                        out.push(Chunk {
                            bytes: B64.decode(v["data"].as_str()?).ok()?.into(),
                            delay: v["delay"].as_f64().unwrap_or(0.0),
                            reply: (i + 1 == values.len()).then_some((fd, self.clients[&fd].order)),
                            request_id: req["request_id"].as_str().map(str::to_owned),
                            failed: false,
                        });
                    }
                    Some(out)
                })();
                match parsed {
                    Some(chunks) if !chunks.is_empty() => {
                        if req["op"] == "send" {
                            self.recent.push(
                                json!({"t":now(),"digest":req["digest"].as_str().unwrap_or(""),"request_id":req["request_id"],"state":"accepted"}),
                            );
                            if self.recent.len() > 10 {
                                self.recent.remove(0);
                            }
                        }
                        self.input.extend(chunks);
                        self.clients.get_mut(&fd).unwrap().mode = Mode::Waiting;
                        return;
                    }
                    Some(_) => json!({"ok":true}),
                    None => json!({"ok":false,"error":"bad_request","message":"bad input chunks"}),
                }
            }
            Some("stop") => {
                if !self.stopping {
                    let steps = req["steps"].as_array().cloned().unwrap_or_default();
                    if steps.iter().any(|s| {
                        if let Some(k) = s["keys"].as_str() {
                            B64.decode(k).is_err()
                        } else {
                            !matches!(s["signal"].as_str(), Some("HUP" | "INT" | "TERM" | "KILL"))
                        }
                    }) {
                        self.clients.get_mut(&fd).unwrap().reply(
                            json!({"ok":false,"error":"bad_request","message":"bad stop steps"}),
                        );
                        return;
                    }
                    // A stopped process acts on HUP or TERM only once it continues.
                    self.resume();
                    self.stopping = true;
                    self.stop_steps = steps.into();
                    self.stop_steps.push_back(json!({"signal":"KILL","wait":0}));
                    self.stop_step();
                }
                json!({"ok":true})
            }
            _ => json!({"ok":false,"error":"bad_op","message":"unknown operation"}),
        };
        self.clients.get_mut(&fd).unwrap().reply(reply);
    }
    fn io_client(&mut self, fd: i32, flags: i16) {
        let mut request = None;
        let mut remove = false;
        if let Some(c) = self.clients.get_mut(&fd) {
            if flags & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0 {
                let mut data = [0; 65536];
                match c.sock.read(&mut data) {
                    Ok(0) => remove = true,
                    Ok(n) => {
                        c.input.extend_from_slice(&data[..n]);
                        if c.mode == Mode::Handshake {
                            if let Some(i) = c.input.iter().position(|b| *b == b'\n') {
                                request = serde_json::from_slice::<Value>(&c.input[..i]).ok();
                                if request.is_none() {
                                    remove = true
                                }
                                c.input.drain(..=i);
                            } else if c.input.len() > 16 * 1024 * 1024 {
                                remove = true
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(_) => remove = true,
                }
            }
            if flags & libc::POLLOUT != 0 && !c.output.is_empty() {
                match c.sock.write(c.output.make_contiguous()) {
                    Ok(n) => {
                        c.output.drain(..n);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(_) => remove = true,
                }
            }
            if c.mode == Mode::Closing && c.output.is_empty() {
                remove = true
            }
        }
        if remove {
            self.drop_client(fd);
        } else if let Some(req) = request {
            self.request(fd, req);
        } else if self
            .clients
            .get(&fd)
            .is_some_and(|c| c.mode == Mode::Attached)
        {
            self.frames(fd);
        }
    }
    fn run(&mut self) -> Result<()> {
        loop {
            if let Some(target) = self.pending_upgrade.take() {
                self.handoff(target);
            }
            self.finish_upgrade();
            self.observe_exit()?;
            if self.exit.is_some() && (!self.pty_open || now() - self.exit_at > 0.3) {
                break;
            }
            if self.resize_at.is_some_and(|t| now() >= t) {
                self.resize_at = None;
                self.resize(self.size.0, self.size.1, false);
            }
            if self.stop_at.is_some_and(|t| now() >= t) {
                self.stop_at = None;
                self.stop_step();
            }
            let mut poll = vec![libc::pollfd {
                fd: self.listener.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            }];
            if self.pty_open {
                poll.push(libc::pollfd {
                    fd: self.master.as_raw_fd(),
                    events: libc::POLLIN
                        | if !self.input.is_empty() && now() >= self.next_input {
                            libc::POLLOUT
                        } else {
                            0
                        },
                    revents: 0,
                });
            }
            for (&fd, c) in &self.clients {
                poll.push(libc::pollfd {
                    fd,
                    events: libc::POLLIN
                        | if c.output.is_empty() {
                            0
                        } else {
                            libc::POLLOUT
                        },
                    revents: 0,
                });
            }
            let timeout = if !self.input.is_empty() && self.next_input > now() {
                ((self.next_input - now()) * 1000.0).min(50.0) as i32
            } else {
                50
            };
            let rc = unsafe { libc::poll(poll.as_mut_ptr(), poll.len() as _, timeout) };
            if rc < 0 {
                if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(std::io::Error::last_os_error().into());
            }
            for item in poll {
                if item.revents == 0 {
                    continue;
                }
                if item.fd == self.listener.as_raw_fd() {
                    if let Ok((s, _)) = self.listener.accept() {
                        s.set_nonblocking(true)?;
                        self.next_order += 1;
                        self.clients.insert(
                            s.as_raw_fd(),
                            Client {
                                sock: Handle::new(s),
                                input: Vec::new(),
                                output: VecDeque::new(),
                                mode: Mode::Handshake,
                                rows: 40,
                                cols: 120,
                                order: self.next_order,
                            },
                        );
                    }
                } else if item.fd == self.master.as_raw_fd() {
                    if item.revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0 {
                        let mut data = [0; 65536];
                        match self.master.read(&mut data) {
                            Ok(0) => self.pty_open = false,
                            Ok(n) => {
                                self.last_output = Some(now());
                                self.term.feed(&data[..n]);
                                self.ring.extend(&data[..n]);
                                if self.ring.len() > 512 * 1024 {
                                    self.ring.drain(..self.ring.len() - 512 * 1024);
                                }
                                let mut slow = Vec::new();
                                for (&fd, c) in &mut self.clients {
                                    if c.mode == Mode::Attached {
                                        c.output.extend(&data[..n]);
                                        if c.output.len() > 8 * 1024 * 1024 {
                                            slow.push(fd);
                                        }
                                    }
                                }
                                for fd in slow {
                                    self.drop_client(fd);
                                }
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                            Err(_) => self.pty_open = false,
                        }
                    }
                    if item.revents & libc::POLLOUT != 0
                        && let Some(c) = self.input.front_mut()
                    {
                        match self.master.write(c.bytes.make_contiguous()) {
                            Ok(n) => {
                                c.bytes.drain(..n);
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                            Err(_) => {
                                c.failed = true;
                                if let Some(id) = &c.request_id
                                    && let Some(r) =
                                        self.recent.iter_mut().find(|r| r["request_id"] == *id)
                                {
                                    r["state"] = json!("write_failed");
                                }
                                c.bytes.clear();
                            }
                        }
                        if c.bytes.is_empty() {
                            let chunk = self.input.pop_front().unwrap();
                            self.next_input = now() + chunk.delay;
                            if chunk.failed && chunk.request_id.is_some() {
                                for pending in &mut self.input {
                                    if pending.request_id == chunk.request_id {
                                        pending.failed = true;
                                    }
                                }
                            }
                            let mut written = !chunk.failed;
                            if chunk.reply.is_some()
                                && let Some(id) = &chunk.request_id
                                && let Some(r) =
                                    self.recent.iter_mut().find(|r| r["request_id"] == *id)
                            {
                                written &= r["state"] != "write_failed";
                                if written {
                                    r["state"] = json!("written");
                                    r["written_at"] = json!(now());
                                }
                            }
                            if let Some(c) = chunk.reply.and_then(|(fd, order)| {
                                self.clients.get_mut(&fd).filter(|c| c.order == order)
                            }) {
                                c.reply(if written {json!({"ok":true,"request_id":chunk.request_id,"state":"written"})} else {json!({"ok":false,"error":"write_failed","message":"PTY write failed","request_id":chunk.request_id})});
                            }
                        }
                    }
                } else {
                    self.io_client(item.fd, item.revents);
                }
            }
        }
        // The agent is gone; nothing it froze may stay frozen.
        self.resume();
        // Match v1 shutdown: bounded flushing lets attached readers receive the last PTY bytes.
        for client in self.clients.values_mut() {
            let _ = client.sock.set_nonblocking(false);
            let _ = client
                .sock
                .set_write_timeout(Some(std::time::Duration::from_millis(500)));
            let _ = client.sock.write_all(client.output.make_contiguous());
        }
        let code = self.exit.and_then(|e| {
            if self.observer {
                None
            } else {
                let e = std::process::ExitStatus::from_raw(e);
                e.code().or_else(|| e.signal().map(|n| -n))
            }
        });
        state::write(
            &self.dir.join("exit.json"),
            &json!({"instance":self.instance,"code":code,"t":now(),"stop_step":self.stopped_by}),
        )?;
        Ok(())
    }
}
pub fn worker() -> i32 {
    let result = (|| -> Result<()> {
        let cfg: Value = serde_json::from_reader(std::io::stdin())?;
        let name = cfg["name"]
            .as_str()
            .ok_or_else(|| Error::new(1, "usage", "missing name"))?;
        let (name, lock) = state::acquire(name, cfg["unique"] == true)?;
        let dir = state::dir(&name)?;
        for file in state::FILES {
            if *file != "lock" {
                let _ = fs::remove_file(dir.join(file));
            }
        }
        use std::os::unix::fs::OpenOptionsExt;
        let log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(dir.join("pen.log"))?;
        if unsafe { libc::dup2(log.as_raw_fd(), 2) } < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let instance = uuid::Uuid::new_v4().simple().to_string()[..12].to_owned();
        let raw: Vec<String> = cfg["argv"]
            .as_array()
            .ok_or_else(|| Error::new(1, "usage", "missing argv"))?
            .iter()
            .map(|v| v.as_str().unwrap_or("").to_owned())
            .collect();
        let argv = crate::hooks::argv(&raw, &dir, cfg["prompt"].as_str())?;
        let (environment, warnings) =
            crate::environment::build(&name, &instance, &dir.join("events"), &cfg["env"]);
        fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(dir.join("events"))?;
        state::write(
            &dir.join("labels.json"),
            &json!({"instance":instance,"labels":cfg["labels"].as_object().cloned().unwrap_or_default()}),
        )?;
        let mut master = -1;
        let mut slave = -1;
        let mut size = libc::winsize {
            ws_row: 40,
            ws_col: 120,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        if unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut size,
            )
        } < 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        let master = unsafe { File::from_raw_fd(master) };
        let slave = unsafe { File::from_raw_fd(slave) };
        unsafe {
            libc::fcntl(master.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC);
            libc::fcntl(slave.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC);
        }
        let program = argv[0].as_str();
        let socket = state::socket(&name)?;
        let listener = UnixListener::bind(&socket)?;
        listener.set_nonblocking(true)?;
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))?;
        let mut command = Command::new(program);
        command
            .args(&argv[1..])
            .env_clear()
            .envs(environment)
            .current_dir(cfg["cwd"].as_str().unwrap())
            .stdin(slave.try_clone()?)
            .stdout(slave.try_clone()?)
            .stderr(slave.try_clone()?);
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                for sig in 1..32 {
                    if sig != libc::SIGKILL && sig != libc::SIGSTOP {
                        libc::signal(sig, libc::SIG_DFL);
                    }
                }
                let mut mask = std::mem::zeroed();
                libc::sigemptyset(&mut mask);
                libc::sigprocmask(libc::SIG_SETMASK, &mask, std::ptr::null_mut());
                Ok(())
            });
        }
        let agent = command
            .spawn()
            .map_err(|e| Error::new(1, "exec_failed", e))?;
        drop(command);
        drop(slave);
        let started = now();
        let kind = std::path::Path::new(program)
            .file_name()
            .unwrap()
            .to_string_lossy();
        let mut pen = Pen {
            agent: agent.id(),
            master: Handle::new(master),
            listener: Handle::new(listener),
            lock: Handle::new(lock),
            custody: Custody::Owner,
            observer: false,
            owns: true,
            pending_upgrade: None,
            upgrade: Upgrade::default(),
            dir,
            metadata: Value::Null,
            labels: json!({"instance":instance,"labels":cfg["labels"].as_object().cloned().unwrap_or_default()}),
            instance,
            started,
            clients: BTreeMap::new(),
            ring: VecDeque::new(),
            last_output: None,
            input: VecDeque::new(),
            next_input: 0.0,
            stopping: false,
            stop_at: None,
            stopped_by: Value::Null,
            exit: None,
            exit_at: 0.0,
            pty_open: true,
            writer: None,
            next_order: 0,
            size: (40, 120),
            resize_at: None,
            term: crate::terminal::Terminal::default(),
            human: None,
            recent: Vec::new(),
            stop_steps: VecDeque::new(),
            paused: None,
            frozen: Vec::new(),
            resumed_at: None,
        };
        if let Some(prompt) = cfg["prompt"].as_str() {
            pen.recent
                .push(json!({"t":started,"digest":crate::events::digest(prompt)}));
        }
        pen.metadata = json!({"name":name,"instance":pen.instance,"proto":1,"kind":kind,"cwd":cfg["cwd"],"argv":argv,"started":started,"pen_pid":std::process::id(),"agent_pid":pen.agent,"version":env!("CARGO_PKG_VERSION"),"has_prompt":cfg["prompt"].is_string(),"exe":crate::executable()?,"upgrade_capable":true});
        state::write(&pen.dir.join("meta.json"), &pen.metadata)?;
        // Ready is the only stdout record. Closing its writer lets the short-lived start command exit.
        let mut ready = json!({"ok":true,"name":name,"instance":pen.instance,"kind":kind});
        if !warnings.is_empty() {
            ready["warnings"] = json!(warnings)
        }
        writeln!(std::io::stdout(), "{ready}")?;
        std::io::stdout().flush()?;
        let null = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/null")?;
        unsafe {
            libc::dup2(null.as_raw_fd(), 0);
            libc::dup2(null.as_raw_fd(), 1);
            libc::signal(libc::SIGHUP, libc::SIG_IGN);
            libc::fcntl(pen.master.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK);
        }
        pen.run()
    })();
    if let Err(e) = result {
        let mut v = e.value;
        v["code"] = json!(e.code);
        println!("{v}");
        return e.code;
    }
    0
}
