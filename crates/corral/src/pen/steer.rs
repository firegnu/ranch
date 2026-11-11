//! Guarded delivery is separate from legacy send/keys, including on old pens.
use super::{Chunk, Mode, Pen};
use crate::{Error, Result, agents, cli, events, now};
use serde_json::{Value, json};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const QUIET_GAP: Duration = Duration::from_millis(300);
const GAP_WAIT_LIMIT: Duration = Duration::from_secs(3);

pub(super) struct ApiRequest {
    client: (i32, u64),
    request_id: Option<String>,
    thread: JoinHandle<Result<Value>>,
}

#[derive(Clone)]
struct Gap {
    deadline: Instant,
    quiet_since: Instant,
}

#[derive(Clone)]
pub(super) struct Guard {
    client: (i32, u64),
    evidence: Value,
    force: bool,
    submit: bool,
    working: bool,
    gap: Option<Gap>,
}

fn evidence(st: &cli::Status) -> Value {
    json!([
        st.public["instance"],
        st.public["state"],
        st.public["state_started"],
        st.public["turn_started"],
        st.public["last_event"],
        st.public["last_event_at"],
        st.snapshot.as_ref().map(|s| &s["main_session"]),
    ])
}

impl Pen {
    fn steer_status(&self, force: bool) -> Result<cli::Status> {
        if self.stopping || self.exit.is_some() {
            return Err(Error::new(7, "not_idle", "the agent is stopping"));
        }
        if self.paused.is_some() {
            return Err(Error::new(
                10,
                "paused",
                "the agent is paused; resume it first",
            ));
        }
        let st = catch_unwind(AssertUnwindSafe(|| {
            cli::status_from_pen(self.metadata["name"].as_str().unwrap_or(""), self.status())
        }))
        .map_err(|_| {
            Error::new(
                1,
                "state_check_failed",
                "status calculation panicked; input withheld",
            )
        })??;
        match st.public["state"].as_str() {
            Some("idle") => {}
            Some("working")
                if agents::find(self.metadata["kind"].as_str().unwrap_or(""))
                    .is_some_and(|a| a.steer_key().is_some() || a.steer_api().is_some()) => {}
            Some("working" | "unknown") => {
                return Err(Error::new(
                    9,
                    "unsupported_steer",
                    "this agent does not support working input; wait for idle and use ordinary send",
                ));
            }
            _ => {
                return Err(Error::new(
                    7,
                    "not_idle",
                    "the agent cannot accept input in this state",
                )
                .with("state", st.public["state"].clone()));
            }
        }
        if !force && self.human.is_some_and(|t| now() - t < 30.0) {
            return Err(Error::new(
                8,
                "human_active",
                "someone typed in an attached window recently",
            )
            .with("last_human_input", self.human));
        }
        Ok(st)
    }

    pub(super) fn steer(&mut self, fd: i32, req: &Value) -> Result<()> {
        if self.pending_upgrade.is_some() || self.upgrade.busy() {
            return Err(Error::new(
                1,
                "upgrade_busy",
                "an upgrade is in progress; no input written",
            ));
        }
        let st = self.steer_status(req["force"] == true)?;
        let text = req["text"]
            .as_str()
            .ok_or_else(|| Error::new(1, "bad_request", "missing text"))?;
        let key = if st.public["state"] == "working" {
            if text.trim_start().starts_with(['/', '!'])
                || text
                    .chars()
                    .any(|c| c.is_control() && c != '\n' && c != '\t')
            {
                return Err(Error::new(
                    1,
                    "usage",
                    "working input must be ordinary text, without command prefixes or control keys",
                ));
            }
            if let Some(api) = agents::find(self.metadata["kind"].as_str().unwrap_or(""))
                .and_then(|a| a.steer_api())
            {
                if req["api_receipt"] != true {
                    return Err(Error::new(
                        9,
                        "unsupported_steer",
                        "this client cannot read API receipts; wait for idle and use ordinary send",
                    )
                    .with("written", false)
                    .with("submitted", false)
                    .with("confirmed", false));
                }
                let dir = self.dir.clone();
                let session = st
                    .snapshot
                    .as_ref()
                    .and_then(|s| s["main_session"].as_str())
                    .unwrap_or("")
                    .to_owned();
                let input = text.to_owned();
                let accepted_at = now();
                let thread = thread::Builder::new()
                    .name("steer".into())
                    .spawn(move || api(&dir, &session, &input))
                    .map_err(|e| {
                        Error::new(
                            7,
                            "steer_failed",
                            format!("could not start input request: {e}"),
                        )
                        .with("written", false)
                        .with("submitted", false)
                        .with("confirmed", false)
                    })?;
                self.api_steers.push(ApiRequest {
                    client: (fd, self.clients[&fd].order),
                    request_id: req["request_id"].as_str().map(str::to_owned),
                    thread,
                });
                self.recent.push(json!({"t":accepted_at,"digest":events::digest(text),"request_id":req["request_id"],"state":"accepted"}));
                if self.recent.len() > 10 {
                    self.recent.remove(0);
                }
                self.clients.get_mut(&fd).unwrap().mode = Mode::Waiting;
                return Ok(());
            }
            if !self.term.paste() {
                return Err(Error::new(
                    9,
                    "unsupported",
                    "working input requires bracketed paste; no input written",
                ));
            }
            agents::find(self.metadata["kind"].as_str().unwrap_or(""))
                .unwrap()
                .steer_key()
                .unwrap()
        } else {
            b"\r"
        };
        let body = if self.term.paste() {
            [
                b"\x1b[200~".as_slice(),
                text.as_bytes(),
                b"\x1b[201~".as_slice(),
            ]
            .concat()
        } else {
            text.as_bytes().to_vec()
        };
        self.recent.push(json!({"t":now(),"digest":events::digest(text),"request_id":req["request_id"],"state":"accepted"}));
        if self.recent.len() > 10 {
            self.recent.remove(0);
        }
        for (bytes, delay, reply) in [
            (body, 0.3, None),
            (key.to_vec(), 0.0, Some((fd, self.clients[&fd].order))),
        ] {
            self.input.push_back(Chunk {
                bytes: bytes.into(),
                delay,
                reply,
                request_id: req["request_id"].as_str().map(str::to_owned),
                failed: false,
                started: false,
                steer: Some(Guard {
                    client: (fd, self.clients[&fd].order),
                    evidence: evidence(&st),
                    force: req["force"] == true,
                    submit: reply.is_some(),
                    working: st.public["state"] == "working",
                    gap: None,
                }),
            });
        }
        self.clients.get_mut(&fd).unwrap().mode = Mode::Waiting;
        Ok(())
    }

    pub(super) fn finish_api_steers(&mut self) {
        let mut i = 0;
        while i < self.api_steers.len() {
            if !self.api_steers[i].thread.is_finished() {
                i += 1;
                continue;
            }
            let pending = self.api_steers.remove(i);
            let mut result = match pending.thread.join() {
                Ok(Ok(receipt)) => receipt,
                Ok(Err(error)) => error.value,
                Err(_) => {
                    json!({"ok":false,"error":"not_delivered","message":"input request panicked; receipt unknown, do not resend","written":null,"submitted":null,"confirmed":false})
                }
            };
            result["request_id"] = json!(pending.request_id);
            if let Some(id) = &pending.request_id
                && let Some(record) = self
                    .recent
                    .iter_mut()
                    .rev()
                    .find(|r| r["request_id"] == *id)
            {
                record["state"] = json!(if result["ok"] == true {
                    "written"
                } else if result["written"] == false {
                    "rejected"
                } else {
                    "unknown"
                });
                for key in ["written", "submitted", "confirmed", "queued_for_next_turn"] {
                    if let Some(value) = result.get(key) {
                        record[key] = value.clone();
                    }
                }
                if result["ok"] != true {
                    record["reason"] = result["error"].clone();
                }
            }
            if let Some(client) = self
                .clients
                .get_mut(&pending.client.0)
                .filter(|c| c.order == pending.client.1)
            {
                client.reply(result);
            }
        }
    }

    /// Runs immediately before the first byte of either guarded block is written.
    /// A cancelled submission leaves the editor alone and discards only this request's blocks.
    /// Returns true when writing is withheld, either to wait for a gap or to cancel.
    pub(super) fn check_steer(&mut self) -> bool {
        let Some(chunk) = self.input.front().filter(|c| !c.started) else {
            return false;
        };
        let Some(mut guard) = chunk.steer.clone() else {
            return false;
        };
        let failure = if chunk.failed {
            Some(Error::new(
                1,
                "write_failed",
                "PTY write failed; not submitting",
            ))
        } else if guard.gap.is_some()
            && self
                .input
                .iter()
                .skip(1)
                .any(|c| c.steer.as_ref().is_none_or(|g| g.client != guard.client))
        {
            Some(Error::new(
                1,
                "input_queued",
                "other input is queued; not submitting",
            ))
        } else if guard.gap.is_some()
            && self
                .clients
                .get(&guard.client.0)
                .is_none_or(|c| c.order != guard.client.1)
        {
            Some(Error::new(
                1,
                "client_gone",
                "the requesting client disconnected; not submitting",
            ))
        } else {
            match self.steer_status(guard.force) {
                Err(e) => Some(e),
                Ok(st) => {
                    let current = evidence(&st);
                    let changed = current != guard.evidence;
                    if guard.submit
                        && guard.working
                        && st.public["state"] == "working"
                        && (changed || guard.gap.is_some())
                    {
                        let at = Instant::now();
                        let gap = guard.gap.get_or_insert(Gap {
                            deadline: at + GAP_WAIT_LIMIT,
                            quiet_since: at,
                        });
                        if changed {
                            guard.evidence = current;
                            gap.quiet_since = at;
                        }
                        if at < gap.deadline {
                            if at.duration_since(gap.quiet_since) >= QUIET_GAP {
                                return false;
                            }
                            // Keep servicing clients and terminal output while input waits.
                            self.next_input = now() + 0.05;
                            self.input.front_mut().unwrap().steer = Some(guard);
                            return true;
                        }
                    }
                    if changed || guard.gap.is_some() {
                        Some(
                            Error::new(
                                7,
                                "state_changed",
                                "state evidence changed before writing input",
                            )
                            .with("state", st.public["state"].clone()),
                        )
                    } else {
                        None
                    }
                }
            }
        };
        let Some(error) = failure else { return false };
        let request_id = chunk.request_id.clone();
        let mut result = if guard.submit && !chunk.failed {
            json!({"ok":false,"error":"draft_not_submitted","message":"text was written but the submit key was withheld; inspect the draft, do not resend","reason":error.value["error"],"state":error.value["state"],"written":true})
        } else {
            let mut result = error.value;
            result["written"] = if chunk.failed {
                Value::Null
            } else {
                json!(false)
            };
            result
        };
        result["submitted"] = json!(false);
        result["confirmed"] = json!(false);
        result["request_id"] = json!(request_id);
        self.input
            .retain(|c| c.steer.as_ref().is_none_or(|g| g.client != guard.client));
        if let Some(id) = request_id
            && let Some(record) = self.recent.iter_mut().rev().find(|r| r["request_id"] == id)
        {
            record["state"] = json!(if result["error"] == "draft_not_submitted" {
                "draft"
            } else {
                "rejected"
            });
            record["written"] = result["written"].clone();
            record["submitted"] = json!(false);
            record["reason"] = result["reason"]
                .as_str()
                .map_or_else(|| result["error"].clone(), |s| json!(s));
        }
        if let Some(client) = self
            .clients
            .get_mut(&guard.client.0)
            .filter(|c| c.order == guard.client.1)
        {
            client.reply(result);
        }
        true
    }
}
