use serde_json::{Value, json};
use std::{fs, process::Command};
use tempfile::TempDir;

fn run(home: &TempDir, args: &[&str], code: i32) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_ranch"))
        .args(["dispatch"])
        .args(args)
        .env("HOME", home.path())
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn local(home: &TempDir, data: &str) {
    fs::create_dir_all(home.path().join(".ranch")).unwrap();
    fs::write(home.path().join(".ranch/dispatch.toml"), data).unwrap();
}

#[test]
fn default_cells_preserve_commands_labels_and_review_assignments() {
    let home = TempDir::new().unwrap();
    let table = run(&home, &["table"], 0);
    assert_eq!(table["cells"].as_object().unwrap().len(), 12);
    let cases = [
        (
            "core-design",
            "claude",
            "opus[1m]",
            "high",
            "claude --dangerously-skip-permissions --model 'opus[1m]' --effort high",
        ),
        (
            "core-code",
            "codex",
            "gpt-6-astra",
            "high",
            "codex --yolo --disable memories -m gpt-6-astra -c 'model_reasoning_effort=\"high\"'",
        ),
        (
            "design",
            "claude",
            "opus[1m]",
            "high",
            "claude --dangerously-skip-permissions --model 'opus[1m]' --effort high",
        ),
        (
            "ui-trial",
            "claude",
            "sonnet",
            "high",
            "claude --dangerously-skip-permissions --model sonnet --effort high",
        ),
        (
            "implementation",
            "codex",
            "gpt-6-astra",
            "medium",
            "codex --yolo --disable memories -m gpt-6-astra -c 'model_reasoning_effort=\"medium\"'",
        ),
        (
            "implementation-hard",
            "codex",
            "gpt-6-astra",
            "high",
            "codex --yolo --disable memories -m gpt-6-astra -c 'model_reasoning_effort=\"high\"'",
        ),
        (
            "review",
            "codex",
            "gpt-6-astra",
            "xhigh",
            "codex --yolo --disable memories -m gpt-6-astra -c 'model_reasoning_effort=\"xhigh\"'",
        ),
        (
            "claude-medium",
            "claude",
            "opus[1m]",
            "medium",
            "claude --dangerously-skip-permissions --model 'opus[1m]' --effort medium",
        ),
    ];
    for (name, agent, model, effort, command) in cases {
        let cell = run(&home, &["cell", name], 0)["cell"].clone();
        assert_eq!(cell, table["cells"][name]);
        assert_eq!(cell["source"], "builtin");
        assert_eq!(cell["agents"], json!([agent]));
        assert_eq!(cell["model"], model);
        assert_eq!(cell["effort"], effort);
        assert_eq!(cell["choices"][0]["command"], command);
        assert_eq!(
            cell["choices"][0]["labels"],
            json!([format!("effort={effort}"), format!("model={model}")])
        );
        let argv = if agent == "claude" {
            json!([
                "claude",
                "--dangerously-skip-permissions",
                "--model",
                model,
                "--effort",
                effort
            ])
        } else {
            json!([
                "codex",
                "--yolo",
                "--disable",
                "memories",
                "-m",
                model,
                "-c",
                format!("model_reasoning_effort=\"{effort}\"")
            ])
        };
        assert_eq!(cell["choices"][0]["argv"], argv);
        assert!(cell["rework"].is_null());
        if name.starts_with("core-") {
            assert_eq!(cell["cross_review"], "review");
            assert_eq!(
                cell["review"]["choices"],
                table["cells"]["review"]["choices"]
            );
        } else {
            assert!(cell["cross_review"].is_null());
            assert!(cell["review"].is_null());
        }
    }
    for name in ["chores", "ui-clear", "amp", "agy"] {
        let cell = run(&home, &["cell", name], 0)["cell"].clone();
        assert_eq!(cell, table["cells"][name]);
        let expected = match name {
            "amp" => vec![("amp", json!(["amp"]), "amp")],
            "agy" => vec![(
                "agy",
                json!(["agy", "--dangerously-skip-permissions"]),
                "agy --dangerously-skip-permissions",
            )],
            _ => vec![
                ("pi", json!(["pi"]), "pi"),
                (
                    "omp",
                    json!(["omp", "--approval-mode", "yolo"]),
                    "omp --approval-mode yolo",
                ),
            ],
        };
        assert_eq!(cell["choices"].as_array().unwrap().len(), expected.len());
        for (choice, (agent, argv, command)) in
            cell["choices"].as_array().unwrap().iter().zip(expected)
        {
            assert_eq!(choice["agent"], agent);
            assert_eq!(choice["argv"], argv);
            assert_eq!(choice["command"], command);
            assert_eq!(choice["labels"], json!([]));
        }
        assert!(cell["model"].is_null());
        assert!(cell["effort"].is_null());
        assert!(cell["cross_review"].is_null());
        assert!(cell["rework"].is_null());
    }
}

#[test]
fn local_cell_patch_changes_only_that_cell_and_is_reread() {
    let home = TempDir::new().unwrap();
    let before = run(&home, &["table"], 0);
    local(&home, "[cells.implementation]\nmodel = 'local-model'\n");
    let after = run(&home, &["table"], 0);
    for (name, cell) in before["cells"].as_object().unwrap() {
        if name != "implementation" {
            assert_eq!(&after["cells"][name], cell, "{name}");
        }
    }
    assert_eq!(after["agents"], before["agents"]);
    let cell = &after["cells"]["implementation"];
    assert_eq!(cell["source"], "user");
    assert_eq!(cell["effort"], "medium");
    assert_eq!(cell["choices"][0]["argv"][5], "local-model");
    assert_eq!(
        cell["choices"][0]["labels"],
        json!(["effort=medium", "model=local-model"])
    );
    local(&home, "[cells.implementation]\nmodel = 'next-model'\n");
    assert_eq!(
        run(&home, &["cell", "implementation"], 0)["cell"]["model"],
        "next-model"
    );
}

#[test]
fn local_agent_patch_keeps_other_fields_and_other_agents() {
    let home = TempDir::new().unwrap();
    let before = run(&home, &["table"], 0);
    local(&home, "[agents.omp]\nargv = ['omp', '--local-option']\n");
    let after = run(&home, &["table"], 0);
    for name in ["claude", "codex", "pi", "amp", "agy"] {
        assert_eq!(after["agents"][name], before["agents"][name]);
    }
    assert_eq!(after["agents"]["omp"]["source"], "user");
    for name in ["chores", "ui-clear"] {
        let cell = &after["cells"][name];
        assert_eq!(cell["source"], "builtin");
        assert_eq!(cell["choices"][1]["source"], "user");
        assert_eq!(cell["choices"][1]["command"], "omp --local-option");
        assert_eq!(cell["choices"][1]["labels"], json!([]));
        assert_eq!(cell["choices"][0], before["cells"][name]["choices"][0]);
    }
}

#[test]
fn new_names_and_links_are_data_and_optional_links_can_be_cleared() {
    let home = TempDir::new().unwrap();
    local(
        &home,
        r#"
[agents.custom]
argv = ["custom", "--model", "${model}"]
labels = ["model=${model}"]
[cells."light.docs"]
agents = ["custom"]
model = "local"
cross_review = "review"
rework = "implementation-hard"
[cells.core-code]
cross_review = ""
"#,
    );
    let cell = run(&home, &["cell", "light.docs"], 0)["cell"].clone();
    assert_eq!(cell["source"], "user");
    assert_eq!(cell["choices"][0]["command"], "custom --model local");
    assert_eq!(cell["choices"][0]["labels"], json!(["model=local"]));
    assert_eq!(cell["review"]["name"], "review");
    assert_eq!(
        cell["review"]["choices"][0]["labels"],
        json!(["effort=xhigh", "model=gpt-6-astra"])
    );
    assert_eq!(cell["rework"], "implementation-hard");
    assert!(run(&home, &["cell", "core-code"], 0)["cell"]["review"].is_null());
    assert_eq!(run(&home, &["table"], 0)["cells"]["light.docs"], cell);
}

#[test]
fn invalid_local_tables_fail_with_location_instead_of_falling_back() {
    let home = TempDir::new().unwrap();
    let cases = [
        ("[cells", "line"),
        ("typo = true", "typo"),
        ("[cells.chores]\ntyop = 1", "cells.chores"),
        ("[agents.pi]\ntyop = 1", "agents.pi"),
        (
            "[cells.chores]\nagents = ['missing']",
            "cells.chores.agents",
        ),
        (
            "[cells.chores]\ncross_review = 'missing'",
            "cells.chores.cross_review",
        ),
        ("[cells.chores]\nrework = 'missing'", "cells.chores.rework"),
        ("[cells.chores]\nrework = 'chores'", "cycle"),
        (
            "[cells.chores]\nrework = 'ui-clear'\n[cells.ui-clear]\nrework = 'chores'",
            "cycle",
        ),
        (
            "[cells.chores]\ncross_review = 'ui-clear'\n[cells.ui-clear]\ncross_review = 'chores'",
            "cycle",
        ),
        (
            "[cells.chores]\ncross_review = 'ui-clear'\n[cells.ui-clear]\nrework = 'chores'",
            "cycle",
        ),
    ];
    for (data, location) in cases {
        local(&home, data);
        // Even reading an unrelated cell must reject a broken table.
        for args in [&["cell", "implementation"][..], &["table"][..]] {
            let error = run(&home, args, 1);
            assert_eq!(error["ok"], false);
            assert_eq!(error["error"], "invalid_data");
            let message = error["message"].as_str().unwrap();
            assert!(message.contains("dispatch.toml"), "{message}");
            assert!(message.contains(location), "{data}: {message}");
        }
    }
}
