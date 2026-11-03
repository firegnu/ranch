//! Optional adapter-owned observation thread. No agent protocol belongs in the pen.
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub(crate) trait Watcher: Sync {
    fn run(&self, context: Context);
}
pub(crate) struct Context {
    pub dir: PathBuf,
    pub instance: String,
    pub cwd: String,
    pub started: f64,
    pub shared: Arc<Mutex<State>>,
    pub stop: Arc<AtomicBool>,
    pub exiting: Arc<AtomicBool>,
}
pub(crate) struct State {
    pub cursor: Value,
    pub last_contact: f64,
    pub error: Option<String>,
    pub finished: bool,
    pub remaining: Vec<String>,
}
pub(super) struct Running {
    shared: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    exiting: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    pub restart_on_exit: bool,
}
pub(crate) fn lock(shared: &Mutex<State>) -> MutexGuard<'_, State> {
    shared.lock().unwrap_or_else(|e| e.into_inner())
}
impl Running {
    pub fn start(
        watcher: &'static dyn Watcher,
        dir: PathBuf,
        instance: String,
        cwd: String,
        started: f64,
        cursor: Value,
    ) -> Self {
        let shared = Arc::new(Mutex::new(State {
            cursor,
            last_contact: crate::now(),
            error: None,
            finished: false,
            remaining: vec![],
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let exiting = Arc::new(AtomicBool::new(false));
        let context = Context {
            dir,
            instance,
            cwd,
            started,
            shared: shared.clone(),
            stop: stop.clone(),
            exiting: exiting.clone(),
        };
        let thread = thread::spawn(move || watcher.run(context));
        Self {
            shared,
            stop,
            exiting,
            thread: Some(thread),
            restart_on_exit: false,
        }
    }
    pub fn status(&self) -> Value {
        let s = lock(&self.shared);
        json!({"running":self.thread.as_ref().is_some_and(|h| !h.is_finished()),"last_contact":s.last_contact,"error":s.error})
    }
    pub fn finished(&self) -> bool {
        lock(&self.shared).finished || self.thread.as_ref().is_none_or(|h| h.is_finished())
    }
    pub fn finish(&self) {
        self.exiting.store(true, Ordering::SeqCst);
    }
    pub fn remaining(&self) -> Vec<String> {
        lock(&self.shared).remaining.clone()
    }
    pub fn cursor(&self) -> Value {
        lock(&self.shared).cursor.clone()
    }
    /// Never fork until JoinHandle confirms that the observer is gone.
    pub fn stop(&mut self, timeout: Duration) -> bool {
        self.stop.store(true, Ordering::SeqCst);
        let deadline = Instant::now() + timeout;
        while self.thread.as_ref().is_some_and(|h| !h.is_finished()) {
            if Instant::now() >= deadline {
                self.stop.store(false, Ordering::SeqCst);
                self.restart_on_exit = true;
                return false;
            }
            thread::sleep(Duration::from_millis(10));
        }
        self.reap();
        true
    }
    pub fn reap(&mut self) {
        if self.thread.as_ref().is_some_and(|h| h.is_finished())
            && self.thread.take().unwrap().join().is_err()
        {
            eprintln!("watcher: observer thread panicked");
            lock(&self.shared).error = Some("observer thread panicked".into());
        }
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Stubborn;
    static RELEASE: AtomicBool = AtomicBool::new(false);
    impl Watcher for Stubborn {
        fn run(&self, _: Context) {
            while !RELEASE.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
    #[test]
    fn a_thread_that_does_not_stop_cancels_the_upgrade_without_forking() {
        use crate::pen::{Pen, upgrade::Upgrade};
        use std::{
            fs::{self, File},
            os::{fd::AsRawFd, unix::net::UnixListener},
        };
        let root = tempfile::tempdir_in("/tmp").unwrap();
        let listener = UnixListener::bind(root.path().join("sock")).unwrap();
        listener.set_nonblocking(true).unwrap();
        let master = File::open("/dev/null").unwrap();
        let lock = crate::state::lock(&root.path().join("lock"), true)
            .unwrap()
            .unwrap();
        let mut pen: Pen = serde_json::from_value(json!({
            "agent":1,"master":master.as_raw_fd(),"listener":listener.as_raw_fd(),"lock":lock.as_raw_fd(),
            "custody":"owner","observer":true,"upgrade":Upgrade::default(),"dir":root.path(),
            "metadata":{"kind":"canter","cwd":root.path()},"labels":{},"instance":"stubborn","started":crate::now(),
            "clients":{},"ring":[],"last_output":null,"input":[],"next_input":0.0,"stopping":false,
            "stop_at":null,"stopped_by":null,"exit":0,"exit_at":crate::now(),"pty_open":false,
            "writer":null,"next_order":0,"size":[40,120],"resize_at":null,
            "term":crate::terminal::Terminal::default(),"human":null,"recent":[],"stop_steps":[]
        })).unwrap();
        pen.watcher = Some(Running::start(
            &Stubborn,
            root.path().into(),
            "stubborn".into(),
            root.path().display().to_string(),
            crate::now(),
            json!({}),
        ));
        pen.pending_upgrade = Some(root.path().join("must-not-execute"));
        let release = thread::spawn(|| {
            thread::sleep(Duration::from_millis(3300));
            RELEASE.store(true, Ordering::SeqCst);
        });
        pen.run().unwrap();
        release.join().unwrap();
        assert_eq!(pen.upgrade.public()["result"], "failed");
        assert!(
            pen.upgrade.public()["last_error"]
                .as_str()
                .unwrap()
                .contains("observer thread did not stop")
        );
        assert!(!root.path().join("upgrade.json").exists());
        assert!(fs::metadata(root.path().join("exit.json")).is_ok());
    }
}
