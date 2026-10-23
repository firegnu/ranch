use super::{Error, Result, files, now};
use crate::{
    chats::{Action, Button, Choice, Event, Events, HistoryAfter},
    corral::{Questions, Status},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Address {
    pub kind: String,
    pub channel: String,
    pub thread: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Thread {
    pub address: Address,
    pub agent: String,
    pub instance: String,
    pub created: f64,
    pub used: f64,
    #[serde(default)]
    pub question: Option<QuestionContext>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct QuestionContext {
    pub instance: String,
    pub state_started: Option<f64>,
    pub handled: bool,
    #[serde(default)]
    pub questions: Option<Questions>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum Phase {
    Queued,
    Sending,
    Waiting,
    Uncertain,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Pending {
    #[serde(default)]
    pub button_answer: bool,
    #[serde(default)]
    pub interrupt_offered: bool,
    pub announcement: Option<String>,
    #[serde(default)]
    pub attachments: bool,
    #[serde(default)]
    pub rejection_reported: bool,
    #[serde(default)]
    pub verification: Option<Verification>,
    #[serde(default)]
    pub question_answer: bool,
    pub address: Address,
    pub message: String,
    pub agent: String,
    pub instance: String,
    pub text: String,
    pub at: f64,
    pub queued: bool,
    pub retry_at: f64,
    pub baseline: Option<f64>,
    pub phase: Phase,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Verification {
    pub started: f64,
    pub retry_at: f64,
    pub delay: f64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Watch {
    pub status: Status,
    pub turn: Option<f64>,
    pub reply: Option<f64>,
    pub blocked: Option<f64>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Interrupt {
    pub address: Address,
    pub agent: String,
    pub instance: String,
    pub turn: Option<f64>,
    pub blocked: bool,
    #[serde(default)]
    pub question_answer: bool,
    pub started: f64,
    pub poll_at: f64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ButtonMessage {
    pub address: Address,
    pub message: String,
    #[serde(default)]
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_value: Option<String>,
    // Flatten keeps active records written by the first R47a version readable.
    #[serde(flatten)]
    pub content: Option<ButtonContent>,
    pub created: f64,
    /// Set and persisted before keys, shared by all clicks on this message.
    pub outcome: Option<String>,
    pub used: bool,
    /// Update finished or abandoned; created is still needed for receipt retention.
    pub updated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_retry: Option<Verification>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ButtonContent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval: Option<ApprovalButton>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<QuestionForm>,
    pub text: String,
    pub notify: bool,
    pub button: Button,
    pub status: Status,
    pub screen: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ApprovalButton {
    pub button: Button,
    pub option: u8,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ApprovalCheck {
    pub address: Address,
    pub status: Status,
    pub screen: String,
    pub started: f64,
    pub poll_at: f64,
    pub sent: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct QuestionForm {
    pub dirty: bool,
    pub choices: Vec<Choice>,
    pub answered: Vec<bool>,
    pub action_at: Vec<f64>,
}

impl ButtonMessage {
    pub(super) fn matches(&self, value: &str) -> bool {
        self.value() == value
            || self.approval_value.as_deref() == Some(value)
            || self
                .content
                .as_ref()
                .and_then(|c| c.form.as_ref())
                .is_some_and(|form| {
                    form.choices.iter().any(|q| {
                        q.value == value
                            || q.submit.value == value
                            || q.options.iter().any(|o| o.value == value)
                    })
                })
    }
    pub(super) fn value(&self) -> &str {
        self.content
            .as_ref()
            .map_or(&self.value, |c| &c.button.value)
    }

    pub(super) fn finish_update(&mut self) {
        self.updated = true;
        self.update_retry = None;
        if let Some(content) = self.content.take() {
            self.value = content.button.value;
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Cursor {
    pub address: Address,
    pub id: Option<String>,
    pub at: f64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Backfill {
    pub address: Address,
    pub after: HistoryAfter,
    pub started: f64,
    pub error: String,
    #[serde(default)]
    pub retry_at: f64,
    #[serde(default = "initial_backfill_delay")]
    pub delay: f64,
}

fn initial_backfill_delay() -> f64 {
    2.
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Seen {
    pub kind: String,
    pub channel: String,
    pub id: String,
    pub at: f64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Expired {
    #[serde(flatten)]
    pub address: Address,
    #[serde(default = "now")]
    pub at: f64,
}

fn read_seen<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Vec<Seen>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Entry {
        Timed(Seen),
        Legacy((String, String, String)),
    }
    Ok(Vec::<Entry>::deserialize(deserializer)?
        .into_iter()
        .map(|entry| match entry {
            Entry::Timed(seen) => seen,
            Entry::Legacy((kind, channel, id)) => Seen {
                kind,
                channel,
                id,
                at: now(),
            },
        })
        .collect())
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub approvals: Vec<ApprovalCheck>,
    pub threads: Vec<Thread>,
    pub expired: Vec<Expired>,
    pub pending: Vec<Pending>,
    pub watches: BTreeMap<String, Watch>,
    #[serde(deserialize_with = "read_seen")]
    pub seen: Vec<Seen>,
    pub cursors: Vec<Cursor>,
    #[serde(default)]
    pub online: BTreeMap<String, f64>,
    #[serde(default)]
    pub backfills: Vec<Backfill>,
    #[serde(default)]
    pub interrupts: Vec<Interrupt>,
    #[serde(default)]
    pub buttons: Vec<ButtonMessage>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(from = "StoredReceived", into = "StoredReceived")]
pub struct Received {
    pub sequence: u64,
    pub kind: String,
    pub event: Event,
}

// Older pony versions can ignore a pending click instead of rejecting state.json.
// The optional field restores the action when read by a button-aware version.
#[derive(Serialize, Deserialize)]
struct StoredReceived {
    sequence: u64,
    kind: String,
    event: Event,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    action: Option<Action>,
}

impl From<Received> for StoredReceived {
    fn from(received: Received) -> Self {
        let (event, action) = match received.event {
            Event::Action(action) => (Event::Ignored, Some(action)),
            event => (event, None),
        };
        Self {
            sequence: received.sequence,
            kind: received.kind,
            event,
            action,
        }
    }
}

impl From<StoredReceived> for Received {
    fn from(stored: StoredReceived) -> Self {
        Self {
            sequence: stored.sequence,
            kind: stored.kind,
            event: stored.action.map(Event::Action).unwrap_or(stored.event),
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
struct Document {
    state: State,
    inbox: Vec<Received>,
    sequence: u64,
}

/// Shared only with adapter input workers. Core commits never overwrite their inbox.
#[derive(Clone)]
pub struct Store {
    home: PathBuf,
    data: Arc<Mutex<Document>>,
}

impl Store {
    /// Runtime calls this only after taking its process lock.
    pub fn open(home: &Path) -> Result<Self> {
        let path = home.join("state.json");
        let document = match OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&path)
        {
            Ok(mut file) => {
                files::check(&file, &path, false)?;
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes)
                    .map_err(|e| files::io_error(&path, e))?;
                serde_json::from_slice(&bytes).map_err(|_| {
                    Error::new(
                        "state_invalid",
                        "cannot parse state.json; preserved for recovery",
                    )
                })?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Document::default(),
            Err(e) => return Err(files::io_error(&path, e)),
        };
        Ok(Self {
            home: home.into(),
            data: Arc::new(Mutex::new(document)),
        })
    }
    fn write(&self, document: &Document) -> Result<()> {
        let path = self.home.join("state.json");
        let temp = self.home.join(format!(
            ".state.{}.{}.tmp",
            std::process::id(),
            now().to_bits()
        ));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temp)?;
            file.write_all(&serde_json::to_vec(document).expect("state is JSON-compatible"))?;
            file.sync_all()?;
            fs::rename(&temp, &path)?;
            fs::File::open(&self.home)?.sync_all()
        })()
        .map_err(|e| files::io_error(&path, e));
        if result.is_err() {
            let _ = fs::remove_file(temp);
        }
        result
    }
    pub fn state(&self) -> State {
        self.data.lock().unwrap().state.clone()
    }
    pub fn inbox(&self) -> Vec<Received> {
        self.data.lock().unwrap().inbox.clone()
    }
    pub fn save(&self, state: &State, finished: Option<u64>) -> Result<()> {
        let mut data = self.data.lock().unwrap();
        let mut next = data.clone();
        next.state = state.clone();
        if let Some(id) = finished {
            next.inbox.retain(|v| v.sequence != id);
        }
        if serde_json::to_vec(&next).expect("state is JSON-compatible")
            == serde_json::to_vec(&*data).expect("state is JSON-compatible")
        {
            return Ok(());
        }
        self.write(&next)?;
        *data = next;
        Ok(())
    }
    pub fn events(&self, kind: &str, owner: &str) -> Events {
        let store = self.clone();
        let kind = kind.to_owned();
        let owner = owner.to_owned();
        Events(Arc::new(move |event| {
            let event = match event {
                Event::Message(m) if m.sender != owner => Event::Ignored,
                Event::Action(mut action) if action.sender != owner => {
                    action.sender.clear();
                    Event::Action(action)
                }
                event => event,
            };
            let mut data = store.data.lock().unwrap();
            let mut next = data.clone();
            next.sequence += 1;
            let sequence = next.sequence;
            next.inbox.push(Received {
                sequence,
                kind: kind.clone(),
                event,
            });
            store.write(&next)?;
            *data = next;
            Ok(())
        }))
    }
}
