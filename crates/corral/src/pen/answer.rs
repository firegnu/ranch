//! Request-addressed answers through an adapter's public API; never terminal keys.
use super::{Mode, Pen};
use crate::{Error, Result, agents, cli, now};
use serde_json::{Value, json};
use std::thread::{self, JoinHandle};

pub(super) struct Pending {
    client: (i32, u64),
    request_id: String,
    thread: JoinHandle<Result<Value>>,
}

impl Pen {
    pub(super) fn answer(&mut self, fd: i32, req: &Value) -> Result<()> {
        let api = agents::find(self.metadata["kind"].as_str().unwrap_or(""))
            .and_then(|a| a.answer_api())
            .ok_or_else(|| Error::new(9, "unsupported", "this agent has no answer API"))?;
        if self.pending_upgrade.is_some() || self.upgrade.busy() || self.api_answer.is_some() {
            return Err(Error::new(
                7,
                "answer_busy",
                "another answer or upgrade is in progress",
            ));
        }
        if self.paused.is_some() {
            return Err(Error::new(10, "paused", "the agent is paused"));
        }
        if self.human.is_some_and(|t| now() - t < 30.) {
            return Err(Error::new(
                8,
                "human_active",
                "someone typed in an attached window recently",
            ));
        }
        let st = cli::status_from_pen(self.metadata["name"].as_str().unwrap_or(""), self.status())?;
        if matches!(st.public["state"].as_str(), Some("starting" | "unknown")) {
            return Err(Error::new(
                9,
                "unsupported",
                "no verifiable session for answering",
            ));
        }
        if self.stopping || self.exit.is_some() || st.public["state"] != "blocked" {
            return Err(Error::new(
                7,
                "request_changed",
                "the request is no longer open",
            ));
        }
        let request = st.public["interaction"]["request"].clone();
        let id = req["request_id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| Error::new(1, "bad_request", "missing request id"))?;
        if request["id"].as_str().is_none() {
            return Err(Error::new(
                9,
                "unsupported",
                "the front request cannot be verified",
            ));
        }
        if request["id"] != id {
            return Err(Error::new(
                7,
                "request_changed",
                "a different request is at the front",
            ));
        }
        let dir = self.dir.clone();
        let session = st
            .snapshot
            .as_ref()
            .and_then(|s| s["main_session"].as_str())
            .unwrap_or("")
            .to_owned();
        let answer = req["answer"].clone();
        let thread = thread::Builder::new()
            .name("answer".into())
            .spawn(move || api(&dir, &session, &request, &answer))
            .map_err(|e| {
                Error::new(
                    7,
                    "answer_failed",
                    format!("could not start answer request: {e}"),
                )
            })?;
        self.api_answer = Some(Pending {
            client: (fd, self.clients[&fd].order),
            request_id: id.into(),
            thread,
        });
        self.clients.get_mut(&fd).unwrap().mode = Mode::Waiting;
        Ok(())
    }

    pub(super) fn finish_api_answer(&mut self) {
        if !self
            .api_answer
            .as_ref()
            .is_some_and(|p| p.thread.is_finished())
        {
            return;
        }
        let pending = self.api_answer.take().unwrap();
        let mut result = match pending.thread.join() {
            Ok(Ok(receipt)) => receipt,
            Ok(Err(error)) => error.value,
            Err(_) => {
                json!({"ok":false,"error":"not_delivered","message":"answer receipt unknown; do not retry","written":null,"submitted":null,"confirmed":false})
            }
        };
        result["request_id"] = json!(pending.request_id);
        if let Some(client) = self
            .clients
            .get_mut(&pending.client.0)
            .filter(|c| c.order == pending.client.1)
        {
            client.reply(result);
        }
    }
}
