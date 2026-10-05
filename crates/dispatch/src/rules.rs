// From Saddle `plugins/dispatch/src/rules.rs` at commit `c21674a`, telemetry capture removed.
//! Request constants ported verbatim from Corral 6923da1 route.py (see README).
use serde::Serialize;
pub(super) const MODEL: &str = "jev-1.13.0";
pub(super) const TIERS: [&str; 3] = ["轻", "常规", "重"];
pub(super) const CONFIDENT: f64 = 0.8;
pub(super) const CROSS_YES: f64 = 0.8;
pub(super) const CROSS_NO: f64 = 0.2;
pub(super) const VISIBLE_YES: f64 = 0.8;
pub(super) const VISIBLE_NO: f64 = 0.2;
#[derive(Serialize)]
struct ScoreQuestion {
    r#type: &'static str,
    instructions: &'static str,
    criteria: [&'static str; 3],
}
#[derive(Serialize)]
struct NoulQuestion {
    r#type: &'static str,
    instructions: &'static str,
}
#[derive(Serialize)]
struct Questions {
    tier: ScoreQuestion,
    cross_data_model: NoulQuestion,
    cross_concurrency: NoulQuestion,
    cross_security_privacy: NoulQuestion,
    cross_core_rules: NoulQuestion,
    visible: NoulQuestion,
    doc_only: NoulQuestion,
}
fn questions() -> Questions {
    Questions {
        tier: ScoreQuestion {
            r#type: "score",
            instructions: "How much reasoning capacity does a coding agent need to do the task in `task_summary` well?",
            criteria: [
                "Light: looking things up, mechanical edits, editing text or data by an explicit checklist, or running commands and reporting results. Little design judgment.",
                "Regular: implementing a feature or fixing an ordinary bug by following a clear task description. Normal design judgment within one module.",
                "Heavy: designing a data model, concurrency or transactions, cross-module design, hard-to-find bugs, or careful review of someone else's code. Mistakes are costly or subtle.",
            ],
        },
        cross_data_model: NoulQuestion {
            r#type: "noul",
            instructions: "Does the task in `task_summary` create or change database table structure, such as adding or altering tables, columns, constraints or migrations? Only reading or writing rows does not count.",
        },
        cross_concurrency: NoulQuestion {
            r#type: "noul",
            instructions: "Does the task in `task_summary` involve concurrency, background workers, locking, retries, or database transactions?",
        },
        cross_security_privacy: NoulQuestion {
            r#type: "noul",
            instructions: "Does the task in `task_summary` handle security or private data, such as permissions, file system isolation, logging of personal content, or exposing data to others?",
        },
        cross_core_rules: NoulQuestion {
            r#type: "noul",
            instructions: "Does the task in `task_summary` implement or change the product's core business rules, the central decision logic the product depends on?",
        },
        visible: NoulQuestion {
            r#type: "noul",
            instructions: "Is the task in `task_summary` limited to presentation (visual layout, colors, text, documentation) or read-only research, with no new features, no input handling changes and no logic changes?",
        },
        doc_only: NoulQuestion {
            r#type: "noul",
            instructions: "Does the task in `task_summary` only produce a written proposal, research notes or a design document, without changing any program code?",
        },
    }
}
pub(super) fn request(summary: &str) -> Vec<u8> {
    #[derive(Serialize)]
    struct State<'a> {
        task_summary: &'a str,
    }
    #[derive(Serialize)]
    struct Request<'a> {
        state: State<'a>,
        model: &'static str,
        questions: Questions,
    }
    serde_json::to_vec(&Request {
        state: State {
            task_summary: summary,
        },
        model: MODEL,
        questions: questions(),
    })
    .expect("request constants")
}
#[cfg(test)]
pub(super) fn fingerprint() -> String {
    use sha2::Digest;
    // Canonical compact UTF-8 JSON, object keys recursively sorted by serde_json's
    // default BTreeMap; arrays retain order. No preserve_order feature in this build.
    let rules = serde_json::json!({"model":MODEL,"tiers":TIERS,"questions":questions(),
        "thresholds":{"confident":CONFIDENT,"cross_yes":CROSS_YES,"cross_no":CROSS_NO,"visible_yes":VISIBLE_YES,"visible_no":VISIBLE_NO}});
    format!(
        "sha256:{:x}",
        sha2::Sha256::digest(serde_json::to_vec(&rules).expect("rule constants"))
    )
}
