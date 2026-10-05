//! Public orchestration uses exactly the same pen protocol as any other client.
use crate::{Result, now, state};
use serde_json::{Value, json};
use std::{path::Path, thread, time::Duration};

pub(crate) fn run(name: Option<&str>, target: &Path, recover: bool) -> Result<Value> {
    let target = crate::pen::upgrade::probe_target(target)?;
    let helper = crate::hooks::helper(&target, true)?;
    if let Some(name) = name {
        return Ok(with_after(name, &target, one(name, &target, recover)));
    }
    let live: Vec<String> = state::list()?
        .iter()
        .filter_map(|s| s["name"].as_str().map(str::to_owned))
        .collect();
    let mut names: std::collections::BTreeSet<String> = live.iter().cloned().collect();
    names.extend(crate::after::names()?);
    let results: Vec<_> = names
        .iter()
        .map(|name| {
            let pen = if live.contains(name) {
                one(name, &target, recover)
            } else {
                json!({"ok":true,"name":name,"result":"complete","pen":"not_running"})
            };
            with_after(name, &target, pen)
        })
        .collect();
    Ok(
        json!({"ok":results.iter().all(|v|v["result"]=="complete" || v["result"]=="already_current"),"helper":{"result":"complete","path":helper,"exe":target},"legacy_after":"unrecorded workers cannot be discovered or migrated","results":results}),
    )
}

fn with_after(name: &str, target: &Path, mut result: Value) -> Value {
    if result["result"] == "needs_restart" {
        result["legacy_after"] = json!({"result":"unsupported","message":"legacy in-memory workers cannot be migrated; recorded workers are checked separately"});
    }
    let reminders = match crate::after::upgrade(name, target) {
        Ok(records) => records,
        Err(e) => vec![json!({"result":"failed","error":e.value})],
    };
    if let Some(failed) = reminders.iter().find(|r| r["result"] != "complete") {
        result["pen_result"] = result["result"].clone();
        if result["result"] == "complete" || result["result"] == "already_current" {
            result["result"] = failed["result"].clone();
        }
        result["ok"] = json!(false);
    }
    result["after"] = json!(reminders);
    result
}

fn one(name: &str, target: &Path, recover: bool) -> Value {
    let initial = match state::request(name, json!({"op":"status"})) {
        Ok(s) => s,
        Err(e) => return json!({"ok":false,"name":name,"result":"unknown","error":e.value}),
    };
    if initial["capabilities"][if recover { "recover" } else { "upgrade" }] != 1 {
        return json!({"ok":false,"name":name,"instance":initial["instance"],"result":"needs_restart","message":"this pen has no upgrade protocol; no restart attempted"});
    }
    let mut epoch = if recover {
        initial["upgrade"]["epoch"].clone()
    } else {
        json!(uuid::Uuid::new_v4().to_string())
    };
    let mut attempt = if recover {
        initial["upgrade"]["attempt"].as_u64().unwrap_or(0) + 1
    } else {
        1
    };
    let request = state::request(
        name,
        json!({"op":if recover {"recover"} else {"upgrade"},"exe":target,"epoch":epoch,"attempt":attempt,"instance":initial["instance"]}),
    );
    if let Ok(reply) = &request {
        if reply["result"] == "already_current" {
            return json!({"ok":true,"name":name,"instance":initial["instance"],"result":"already_current","exe":target,"upgrade":reply["upgrade"]});
        }
        if reply["ok"] != true {
            return json!({"ok":false,"name":name,"instance":initial["instance"],"result":"failed","error":reply,"epoch":epoch,"attempt":attempt});
        }
        if reply["result"] == "accepted" {
            epoch = reply["upgrade"]["epoch"].clone();
            attempt = reply["upgrade"]["attempt"].as_u64().unwrap_or(attempt);
        }
    }
    // Receipt loss is queried, never retried. An exe match alone is insufficient.
    let deadline = now() + 5.0;
    let mut observed = None;
    loop {
        if let Ok(st) =
            state::request_timeout(name, json!({"op":"status"}), Duration::from_millis(500))
        {
            if st["instance"] != initial["instance"] {
                break;
            }
            let u = &st["upgrade"];
            if u["epoch"] == epoch && u["attempt"] == attempt && u["target"] == json!(target) {
                let result = if u["state"] == "none"
                    && u["result"] == "complete"
                    && st["exe"] == json!(target)
                {
                    "complete"
                } else if u["result"] == "failed" {
                    "failed"
                } else if matches!(u["state"].as_str(), Some("hold" | "hold_unprotected")) {
                    u["state"].as_str().unwrap()
                } else {
                    "pending"
                };
                observed = Some(
                    json!({"ok":result=="complete"||result=="pending","name":name,"instance":st["instance"],"result":result,"exe":st["exe"],"custody":st["custody"],"upgrade":u}),
                );
                if result != "pending" {
                    return observed.unwrap();
                }
            }
        }
        if now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    observed.unwrap_or_else(||json!({"ok":false,"name":name,"instance":initial["instance"],"result":"unknown","epoch":epoch,"attempt":attempt,"target":target}))
}
