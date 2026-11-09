//! Guarded delivery is separate from legacy send/keys, including on old pens.
use super::{Chunk, Mode, Pen};
use crate::{Error, Result, agents, cli, events, now};
use serde_json::{Value, json};

#[derive(Clone)]
pub(super) struct Guard {
    client: (i32, u64),
    evidence: Value,
    force: bool,
    submit: bool,
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
        let st = cli::status_from_pen(self.metadata["name"].as_str().unwrap_or(""), self.status())?;
        match st.public["state"].as_str() {
            Some("idle") => {}
            Some("working")
                if agents::find(self.metadata["kind"].as_str().unwrap_or(""))
                    .is_some_and(|a| a.steer_key().is_some()) => {}
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
                }),
            });
        }
        self.clients.get_mut(&fd).unwrap().mode = Mode::Waiting;
        Ok(())
    }

    /// Runs immediately before the first byte of either guarded block is written.
    /// A cancelled submission leaves the editor alone and discards only this request's blocks.
    pub(super) fn check_steer(&mut self) -> bool {
        let Some(chunk) = self.input.front().filter(|c| !c.started) else {
            return false;
        };
        let Some(guard) = chunk.steer.clone() else {
            return false;
        };
        let failure = if chunk.failed {
            Some(Error::new(
                1,
                "write_failed",
                "PTY write failed; not submitting",
            ))
        } else {
            match self.steer_status(guard.force) {
                Err(e) => Some(e),
                Ok(st) if evidence(&st) != guard.evidence => Some(
                    Error::new(7, "not_idle", "state evidence changed before writing input")
                        .with("state", st.public["state"].clone()),
                ),
                Ok(_) => None,
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
