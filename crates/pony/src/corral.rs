//! Only the public command protocol; no runtime files or Rust dependency on corral.
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Agent {
    pub name: String,
    #[serde(default)]
    pub instance: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Status {
    pub name: String,
    pub instance: String,
    pub state: String,
    pub state_started: Option<f64>,
    pub turn_started: Option<f64>,
    pub last_event: Option<String>,
    pub last_event_at: Option<f64>,
    pub last_tool: Option<String>,
    pub title: Option<String>,
    pub kind: Option<String>,
    pub questions: Option<Questions>,
    pub paused_at: Option<f64>,
    pub last_human_input: Option<f64>,
    #[serde(default)]
    pub paused: bool,
    #[serde(default)]
    pub interrupted: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Questions {
    pub items: Vec<Question>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Question {
    pub header: String,
    pub question: String,
    pub multi_select: bool,
    pub options: Vec<QuestionOption>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct QuestionOption {
    pub label: String,
    pub description: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Reply {
    pub instance: String,
    pub text: String,
    pub at: f64,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Sent {
    #[serde(default)]
    pub merged_with_draft: bool,
}

#[derive(Clone, Debug)]
pub struct Failure {
    pub exit: Option<i32>,
    pub code: String,
    pub message: String,
    pub interrupted: bool,
}

impl Failure {
    fn protocol() -> Self {
        Self {
            exit: None,
            code: "corral_protocol".into(),
            message: "invalid corral response".into(),
            interrupted: false,
        }
    }
}

pub type Result<T> = std::result::Result<T, Failure>;

pub trait Corral {
    fn list(&mut self) -> Result<Vec<Agent>>;
    fn status(&mut self, name: &str) -> Result<Status>;
    fn reply(&mut self, name: &str) -> Result<Reply>;
    fn read(&mut self, name: &str) -> Result<String>;
    fn send(&mut self, name: &str, text: &str) -> Result<Sent>;
    fn escape(&mut self, name: &str, twice: bool) -> Result<()>;
}

pub struct Process {
    path: PathBuf,
}

impl Process {
    pub fn discover(
        configured: Option<&str>,
        path: Option<&std::ffi::OsStr>,
        home: &Path,
    ) -> Result<Self> {
        use std::os::unix::fs::PermissionsExt;
        let executable = |p: &Path| {
            p.is_file()
                && p.metadata()
                    .is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
        };
        let candidate = configured
            .map(PathBuf::from)
            .or_else(|| {
                path.into_iter()
                    .flat_map(std::env::split_paths)
                    .map(|p| p.join("corral"))
                    .find(|p| executable(p))
            })
            .unwrap_or_else(|| home.join(".local/bin/corral"));
        let process = Self { path: candidate };
        let version: serde_json::Value = process.call(&["--version"])?;
        if version["contract"] != "1" {
            return Err(Failure::protocol());
        }
        Ok(process)
    }

    fn call<T: serde::de::DeserializeOwned>(&self, args: &[&str]) -> Result<T> {
        let output = self.output(args)?;
        decode(output.status.code(), &output.stdout)
    }

    fn output(&self, args: &[&str]) -> Result<std::process::Output> {
        Command::new(&self.path)
            .args(args)
            .output()
            .map_err(|_| Failure {
                exit: None,
                code: "corral_unavailable".into(),
                message: "cannot execute corral".into(),
                interrupted: false,
            })
    }
}

fn decode<T: serde::de::DeserializeOwned>(exit: Option<i32>, bytes: &[u8]) -> Result<T> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| Failure::protocol())?;
    if exit == Some(0) && value["ok"] == true {
        return serde_json::from_value(value).map_err(|_| Failure::protocol());
    }
    if exit.is_some_and(|e| e != 0) && value["ok"] == false {
        return Err(Failure {
            exit,
            code: value["error"]
                .as_str()
                .ok_or_else(Failure::protocol)?
                .into(),
            message: value["message"]
                .as_str()
                .unwrap_or("corral command failed")
                .into(),
            interrupted: value["interrupted"] == true,
        });
    }
    Err(Failure::protocol())
}

impl Corral for Process {
    fn list(&mut self) -> Result<Vec<Agent>> {
        #[derive(Deserialize)]
        struct Listing {
            agents: Vec<Agent>,
        }
        self.call::<Listing>(&["ls"]).map(|v| v.agents)
    }
    fn status(&mut self, name: &str) -> Result<Status> {
        self.call(&["status", name])
    }
    fn reply(&mut self, name: &str) -> Result<Reply> {
        self.call(&["reply", name])
    }
    fn read(&mut self, name: &str) -> Result<String> {
        let output = self.output(&["read", name, "--bytes", "4000"])?;
        if output.status.success() {
            String::from_utf8(output.stdout).map_err(|_| Failure::protocol())
        } else {
            decode(output.status.code(), &output.stdout)
        }
    }
    fn send(&mut self, name: &str, text: &str) -> Result<Sent> {
        self.call(&["send", name, "--", text])
    }
    fn escape(&mut self, name: &str, twice: bool) -> Result<()> {
        let args = if twice {
            vec!["keys", name, "esc", "esc"]
        } else {
            vec!["keys", name, "esc"]
        };
        self.call::<serde_json::Value>(&args).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_json_and_exit_codes_are_preserved() {
        let sent: Sent = decode(
            Some(0),
            br#"{"ok":true,"merged_with_draft":true,"future":1}"#,
        )
        .unwrap();
        assert!(sent.merged_with_draft);
        for (exit, code) in [
            (7, "not_idle"),
            (8, "human_active"),
            (10, "paused"),
            (3, "not_delivered"),
            (2, "not_found"),
            (1, "no_reply"),
            (6, "io"),
        ] {
            let bytes = serde_json::to_vec(
                &serde_json::json!({"ok":false,"error":code,"message":"reason","interrupted":true}),
            )
            .unwrap();
            let error = decode::<Sent>(Some(exit), &bytes).unwrap_err();
            assert_eq!(
                (
                    error.exit,
                    error.code.as_str(),
                    error.message.as_str(),
                    error.interrupted
                ),
                (Some(exit), code, "reason", true)
            );
        }
        for (exit, bytes) in [
            (Some(0), b"garbage".as_slice()),
            (Some(0), br#"{"ok":false}"#),
            (Some(1), br#"{"ok":true}"#),
            (None, b"{}"),
        ] {
            assert!(decode::<Sent>(exit, bytes).is_err());
        }
    }
}
