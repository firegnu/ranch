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
    path::{Path, PathBuf},
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

    #[test]
    #[ignore = "isolated CLI subprocess for the stop deadline test"]
    fn stop_cli_reports_remaining_processes() {
        let Ok(expected) = std::env::var("CORRAL_TEST_STOP_EXPECTED") else {
            return;
        };
        let expected: Value = serde_json::from_str(&expected).unwrap();
        let result = crate::cli::run(&["stop".into(), "test".into()])
            .unwrap()
            .unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn final_kill_wait_is_bounded_and_records_remaining_processes() {
        use std::time::{Duration, Instant};
        for parent_exited in [true, false] {
            let home = tempfile::tempdir_in("/tmp").unwrap();
            let dir = home.path().join("test");
            fs::create_dir(&dir).unwrap();
            let listener = UnixListener::bind(dir.join("sock")).unwrap();
            listener.set_nonblocking(true).unwrap();
            let master = File::open("/dev/null").unwrap();
            let lock = state::lock(&dir.join("lock"), true).unwrap().unwrap();
            let mut child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
            let pid = child.id() as i32;
            // A live process at the post-KILL boundary models a signal that could not stop it.
            // The snapshot exercises the actual loop without requiring root or an unkillable process.
            let before = Instant::now();
            let mut pen: Pen = serde_json::from_value(json!({
                "agent":pid,"master":master.as_raw_fd(),"listener":listener.as_raw_fd(),
                "lock":lock.as_raw_fd(),"custody":"owner","observer":false,
                "upgrade":Upgrade::default(),"dir":dir,"metadata":{"kind":"kimi"},
                "labels":{},"instance":"stop-deadline-test","started":now(),
                "clients":{},"ring":[],"last_output":null,"input":[],"next_input":0.0,
                "stopping":true,"stop_at":null,"stopped_by":"SIGKILL",
                "exit":if parent_exited { Some(0) } else { None },"exit_at":0.0,
                "pty_open":false,"writer":null,"next_order":0,"size":[40,120],
                "resize_at":null,"term":crate::terminal::Terminal::default(),
                "human":null,"recent":[],"stop_steps":[],
                "stop_tree":crate::freeze::StopTree::capture(pid),"stop_deadline":now()+5.0
            }))
            .unwrap();
            // Always clean up our exact child, including on a regression or assertion panic.
            let (done, wait) = std::sync::mpsc::channel();
            let cleanup = std::thread::spawn(move || {
                let _ = wait.recv_timeout(Duration::from_secs(10));
                let _ = child.kill();
                let _ = child.wait();
            });
            state::write(&dir.join("meta.json"), &json!({"kind":"kimi"})).unwrap();
            let cli = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "pen::tests::stop_cli_reports_remaining_processes",
                ])
                .env("HOME", home.path())
                .env("CORRAL_HOME", home.path())
                .env("KIMI_CODE_HOME", home.path().join("kimi-home"))
                .env(
                    "CORRAL_TEST_STOP_EXPECTED",
                    json!({"ok":true,"name":"test","instance":"stop-deadline-test",
                        "exit_code":if parent_exited { Some(0) } else { None },
                        "stopped_by":"SIGKILL","remaining_pids":[pid]})
                    .to_string(),
                )
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let result = pen.run();
            let elapsed = before.elapsed();
            let still_running = unsafe { libc::kill(pid, 0) == 0 };
            let _ = done.send(());
            cleanup.join().unwrap();
            drop(listener);
            fs::remove_file(dir.join("sock")).unwrap();
            drop(lock);
            let cli = cli.wait_with_output().unwrap();
            result.unwrap();
            assert!(
                still_running,
                "pen waited until the cleanup guard killed the residual process"
            );
            assert!(
                elapsed >= Duration::from_secs(5) && elapsed < Duration::from_secs(7),
                "{elapsed:?}"
            );
            assert!(
                cli.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&cli.stdout),
                String::from_utf8_lossy(&cli.stderr)
            );
            let exit = state::read(dir.join("exit.json"));
            assert_eq!(exit["remaining_pids"], json!([pid]));
            assert_eq!(exit["stop_step"], "SIGKILL");
            assert_eq!(
                exit["code"],
                if parent_exited { json!(0) } else { Value::Null }
            );
            let log = fs::read_to_string(dir.join("pen.log")).unwrap();
            assert!(
                log.contains("SIGKILL") && log.contains(&pid.to_string()),
                "{log}"
            );
        }
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
    #[serde(default)]
    pending_prompt: Option<String>,
    #[serde(default)]
    stop_tree: Option<crate::freeze::StopTree>,
    #[serde(default)]
    stop_deadline: Option<f64>,
    // Pens before pausing existed leave these out of their snapshots.
    #[serde(default)]
    paused: Option<f64>,
    #[serde(default)]
    frozen: Vec<i32>,
    #[serde(default)]
    resumed_at: Option<f64>,
    /// Only for an agent with screen rules; snapshots leave it out and it is rebuilt from `ring`.
    #[serde(skip)]
    screen: Option<crate::screen::Screen>,
    /// The HOME whose trust list may hold the agent's directory (`trust`), to clean on exit.
    #[serde(default)]
    trust: Option<PathBuf>,
}
/// Takes out what `trust::add` put in for this instance; a failure only goes to the log.
fn untrust(home: Option<&Path>, instance: &str) {
    if let Some(home) = home
        && let Err(e) = crate::trust::remove(home, instance)
    {
        eprintln!("trust: {}", e.value);
    }
}
impl Drop for Pen {
    fn drop(&mut self) {
        if !self.owns || self.custody == Custody::Handoff {
            return;
        }
        if let Some(tree) = &mut self.stop_tree {
            tree.signal(libc::SIGKILL);
        }
        if self.exit.is_none() && self.custody == Custody::Owner {
            // SIGKILL ends the stopped group; descendants that left it must not stay frozen.
            self.resume();
            unsafe {
                libc::kill(-(self.agent as i32), libc::SIGKILL);
                // A failed KILL must not turn the bounded shutdown into an unbounded Drop.
                let flags = if self.stop_deadline.is_some_and(|t| now() >= t) {
                    libc::WNOHANG
                } else {
                    0
                };
                libc::waitpid(self.agent as i32, std::ptr::null_mut(), flags);
            }
        }
        let _ = fs::remove_file(self.dir.join("sock"));
    }
}
impl Pen {
    fn first_prompt(&mut self) {
        if self.pending_prompt.is_none() || self.stopping || self.paused.is_some() {
            return;
        }
        let ready =
            crate::agents::find(self.metadata["kind"].as_str().unwrap_or("")).is_some_and(|a| {
                self.screen
                    .as_ref()
                    .and_then(|s| s.text())
                    .is_some_and(|s| a.input_ready(&s))
            });
        if ready {
            let text = self.pending_prompt.take().unwrap();
            let bytes = if self.term.paste() {
                [b"\x1b[200~".as_slice(), text.as_bytes(), b"\x1b[201~"].concat()
            } else {
                text.into_bytes()
            };
            for (bytes, delay) in [(bytes, 0.3), (vec![b'\r'], 0.0)] {
                self.input.push_back(Chunk {
                    bytes: bytes.into(),
                    delay,
                    reply: None,
                    request_id: None,
                    failed: false,
                });
            }
        }
    }
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
                if let Some(tree) = &mut self.stop_tree {
                    tree.signal(sig);
                }
                if self.stop_tree.is_none() || self.exit.is_none() {
                    unsafe {
                        libc::kill(-(self.agent as i32), sig);
                    }
                }
                self.stopped_by =
                    json!(format!("SIG{}", step["signal"].as_str().unwrap_or("KILL")));
            }
            self.stop_at = if self.stop_steps.is_empty() {
                if self.stop_tree.is_some() {
                    self.stop_deadline = Some(now() + 5.0);
                }
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
        if let Some(text) = self.screen.as_ref().and_then(|s| s.text()) {
            status["screen"] = json!(text);
        }
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
        if let Some(screen) = &mut self.screen {
            screen.resize((s.ws_row, s.ws_col));
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
            // The snapshot an upgrade hands over must not change under it, nor reach a pen that
            // cannot keep the agent paused: one at a time.
            Some("pause" | "resume") if self.pending_upgrade.is_some() || self.upgrade.busy() => {
                json!({"ok":false,"error":"upgrade_busy","message":"an upgrade is in progress; try again when it is done","upgrade":self.upgrade.public()})
            }
            Some("pause") if self.exit.is_some() || self.stopping => {
                json!({"ok":false,"error":"stopping","message":"the agent is stopping"})
            }
            Some("pause") => {
                if self.paused.is_none() {
                    match crate::freeze::freeze(self.agent as i32) {
                        Ok(frozen) => {
                            self.frozen = frozen;
                            self.paused = Some(now());
                        }
                        Err(running) => {
                            self.clients.get_mut(&fd).unwrap().reply(json!({"ok":false,"error":"freeze_failed","message":"some processes did not stop; nothing is left paused","running":running}));
                            return;
                        }
                    }
                }
                json!({"ok":true,"instance":self.instance,"paused":true,"paused_at":self.paused})
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
                    if crate::agents::find(self.metadata["kind"].as_str().unwrap_or(""))
                        .is_some_and(|a| a.stop_descendants())
                    {
                        self.stop_tree = Some(crate::freeze::StopTree::capture(self.agent as i32));
                    }
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
        if self.screen.is_none()
            && crate::agents::reads_screen(self.metadata["kind"].as_str().unwrap_or(""))
        {
            let (a, b) = self.ring.as_slices();
            let mut screen = crate::screen::Screen::new(self.size, a);
            screen.feed(b);
            self.screen = Some(screen);
        }
        let mut remaining_pids = Vec::new();
        loop {
            if let Some(target) = self.pending_upgrade.take() {
                self.handoff(target);
            }
            self.finish_upgrade();
            self.first_prompt();
            let was_running = self.exit.is_none();
            self.observe_exit()?;
            if was_running && self.exit.is_some() && self.metadata["sandbox"].is_object() {
                // The runner owns the sandbox proxies. Nothing in its group may outlive it.
                unsafe {
                    libc::kill(-(self.agent as i32), libc::SIGKILL);
                }
            }
            if self.stop_deadline.is_some_and(|t| now() >= t) {
                if let Some(tree) = &mut self.stop_tree {
                    remaining_pids = tree.remaining();
                }
                break;
            }
            if self.exit.is_some()
                && (!self.pty_open || now() - self.exit_at > 0.3)
                && self.stop_tree.as_mut().is_none_or(|tree| tree.is_empty())
            {
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
                                if let Some(s) = &mut self.screen {
                                    s.feed(&data[..n]);
                                }
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
        // Nothing this agent froze may stay frozen, even if it could not be killed.
        self.resume();
        untrust(self.trust.as_deref(), &self.instance);
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
        let mut exit =
            json!({"instance":self.instance,"code":code,"t":now(),"stop_step":self.stopped_by});
        if !remaining_pids.is_empty() {
            use std::os::unix::fs::OpenOptionsExt;
            let mut log = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .mode(0o600)
                .open(self.dir.join("pen.log"))?;
            writeln!(
                log,
                "stop: processes still running 5 seconds after final SIGKILL: {remaining_pids:?}"
            )?;
            exit["remaining_pids"] = json!(remaining_pids);
        }
        state::write(&self.dir.join("exit.json"), &exit)?;
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
        let argv = crate::agents::argv(&raw, &dir, cfg["prompt"].as_str())?;
        let (mut environment, warnings) =
            crate::environment::build(&name, &instance, &dir.join("events"), &cfg["env"]);
        environment.extend(crate::agents::env(
            &raw[0],
            cfg["cwd"].as_str().unwrap_or(""),
        ));
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
                // A raw pointer: macOS takes `*mut winsize`, Linux `*const winsize`.
                &raw mut size,
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
        let program = raw[0].as_str();
        let socket = state::socket(&name)?;
        let listener = UnixListener::bind(&socket)?;
        listener.set_nonblocking(true)?;
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))?;
        let mut command = if cfg["sandbox"].is_object() {
            crate::sandbox::command(
                &cfg["sandbox"],
                &crate::agents::kind(program),
                &name,
                &instance,
                &dir,
                &argv,
            )?
        } else {
            let mut command = Command::new(&argv[0]);
            command.args(&argv[1..]);
            command
        };
        command
            .env_clear()
            .envs(&environment)
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
        // An agent that trusts only listed directories need not ask about one under a trusted one;
        // the list is the one under the HOME the agent gets.
        let home = environment
            .get("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()));
        let trust = crate::trust::add(
            &crate::agents::kind(program),
            &home,
            cfg["cwd"].as_str().unwrap_or(""),
            &name,
            &instance,
        )
        .unwrap_or_else(|e| {
            eprintln!("trust: {}", e.value);
            Some(home)
        });
        let agent = match command.spawn() {
            Ok(agent) => agent,
            Err(e) => {
                untrust(trust.as_deref(), &instance);
                return Err(Error::new(1, "exec_failed", e));
            }
        };
        drop(command);
        drop(slave);
        let started = now();
        let kind = crate::agents::kind(program);
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
            pending_prompt: crate::agents::find(&kind)
                .filter(|a| a.prompt_via_terminal())
                .and_then(|_| cfg["prompt"].as_str().map(str::to_owned)),
            stop_tree: None,
            stop_deadline: None,
            paused: None,
            frozen: Vec::new(),
            resumed_at: None,
            screen: None,
            trust,
        };
        if let Some(prompt) = cfg["prompt"].as_str() {
            pen.recent
                .push(json!({"t":started,"digest":crate::events::digest(prompt)}));
        }
        pen.metadata = json!({"name":name,"instance":pen.instance,"proto":1,"kind":kind,"cwd":cfg["cwd"],"argv":argv,"started":started,"pen_pid":std::process::id(),"agent_pid":pen.agent,"version":env!("CARGO_PKG_VERSION"),"has_prompt":cfg["prompt"].is_string(),"exe":crate::executable()?,"upgrade_capable":true});
        pen.metadata["sandbox"] = cfg["sandbox"].clone();
        state::write(&pen.dir.join("meta.json"), &pen.metadata)?;
        // Ready is the only stdout record. Closing its writer lets the short-lived start command exit.
        let mut ready = json!({"ok":true,"name":name,"instance":pen.instance,"kind":kind});
        ready["sandbox"] = crate::sandbox::public(&pen.metadata);
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
