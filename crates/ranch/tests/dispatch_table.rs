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
            "middle-design",
            "claude",
            "opus[1m]",
            "high",
            "claude --dangerously-skip-permissions --model 'opus[1m]' --effort high",
        ),
        (
            "middle-other",
            "codex",
            "gpt-6-astra",
            "high",
            "codex --yolo --disable memories -m gpt-6-astra -c 'model_reasoning_effort=\"high\"'",
        ),
        (
            "high-design",
            "claude",
            "opus[1m]",
            "xhigh",
            "claude --dangerously-skip-permissions --model 'opus[1m]' --effort xhigh",
        ),
        (
            "high-other",
            "codex",
            "gpt-6-astra",
            "xhigh",
            "codex --yolo --disable memories -m gpt-6-astra -c 'model_reasoning_effort=\"xhigh\"'",
        ),
        (
            "critical-design",
            "claude",
            "opus[1m]",
            "xhigh",
            "claude --dangerously-skip-permissions --model 'opus[1m]' --effort xhigh",
        ),
        (
            "critical-other",
            "codex",
            "gpt-6-astra",
            "xhigh",
            "codex --yolo --disable memories -m gpt-6-astra -c 'model_reasoning_effort=\"xhigh\"'",
        ),
        (
            "review-codex",
            "codex",
            "gpt-6-astra",
            "xhigh",
            "codex --yolo --disable memories -m gpt-6-astra -c 'model_reasoning_effort=\"xhigh\"'",
        ),
        (
            "review-claude",
            "claude",
            "opus[1m]",
            "xhigh",
            "claude --dangerously-skip-permissions --model 'opus[1m]' --effort xhigh",
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
        let rework = match name {
            "middle-design" => json!("high-design"),
            "middle-other" => json!("high-other"),
            _ => Value::Null,
        };
        assert_eq!(cell["rework"], rework);
        if name.starts_with("critical-") {
            let review = if agent == "claude" {
                "review-codex"
            } else {
                "review-claude"
            };
            assert_eq!(cell["cross_review"], review);
            assert_eq!(cell["review"]["name"], review);
            assert_eq!(cell["review"]["model"], table["cells"][review]["model"]);
            assert_eq!(cell["review"]["effort"], "xhigh");
            assert_eq!(cell["review"]["choices"], table["cells"][review]["choices"]);
        } else {
            assert!(cell["cross_review"].is_null());
            assert!(cell["review"].is_null());
        }
    }
    for name in ["low-other", "low-design", "amp", "agy"] {
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
        assert!(cell["review"].is_null());
        let rework = match name {
            "low-design" => json!("middle-design"),
            "low-other" => json!("middle-other"),
            _ => Value::Null,
        };
        assert_eq!(cell["rework"], rework);
    }
}

#[test]
fn local_cell_patch_changes_only_that_cell_and_is_reread() {
    let home = TempDir::new().unwrap();
    let before = run(&home, &["table"], 0);
    local(&home, "[cells.middle-other]\nmodel = 'local-model'\n");
    let after = run(&home, &["table"], 0);
    for (name, cell) in before["cells"].as_object().unwrap() {
        if name != "middle-other" {
            assert_eq!(&after["cells"][name], cell, "{name}");
        }
    }
    assert_eq!(after["agents"], before["agents"]);
    let cell = &after["cells"]["middle-other"];
    assert_eq!(cell["source"], "user");
    assert_eq!(cell["effort"], "high");
    assert_eq!(cell["choices"][0]["argv"][5], "local-model");
    assert_eq!(
        cell["choices"][0]["labels"],
        json!(["effort=high", "model=local-model"])
    );
    local(&home, "[cells.middle-other]\nmodel = 'next-model'\n");
    assert_eq!(
        run(&home, &["cell", "middle-other"], 0)["cell"]["model"],
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
    for name in ["low-other", "low-design"] {
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
cross_review = "review-codex"
rework = "high-other"
[cells.critical-other]
cross_review = ""
"#,
    );
    let cell = run(&home, &["cell", "light.docs"], 0)["cell"].clone();
    assert_eq!(cell["source"], "user");
    assert_eq!(cell["choices"][0]["command"], "custom --model local");
    assert_eq!(cell["choices"][0]["labels"], json!(["model=local"]));
    assert_eq!(cell["review"]["name"], "review-codex");
    assert_eq!(
        cell["review"]["choices"][0]["labels"],
        json!(["effort=xhigh", "model=gpt-6-astra"])
    );
    assert_eq!(cell["rework"], "high-other");
    assert!(run(&home, &["cell", "critical-other"], 0)["cell"]["review"].is_null());
    assert_eq!(run(&home, &["table"], 0)["cells"]["light.docs"], cell);
}

#[test]
fn invalid_local_tables_fail_with_location_instead_of_falling_back() {
    let home = TempDir::new().unwrap();
    let cases = [
        ("[cells", "line"),
        ("typo = true", "typo"),
        ("[cells.low-other]\ntyop = 1", "cells.low-other"),
        ("[agents.pi]\ntyop = 1", "agents.pi"),
        (
            "[cells.low-other]\nagents = ['missing']",
            "cells.low-other.agents",
        ),
        (
            "[cells.low-other]\ncross_review = 'missing'",
            "cells.low-other.cross_review",
        ),
        (
            "[cells.low-other]\nrework = 'missing'",
            "cells.low-other.rework",
        ),
        ("[cells.low-other]\nrework = 'low-other'", "cycle"),
        (
            "[cells.low-other]\nrework = 'low-design'\n[cells.low-design]\nrework = 'low-other'",
            "cycle",
        ),
        (
            "[cells.low-other]\ncross_review = 'low-design'\n[cells.low-design]\ncross_review = 'low-other'",
            "cycle",
        ),
        (
            "[cells.low-other]\ncross_review = 'low-design'\n[cells.low-design]\nrework = 'low-other'",
            "cycle",
        ),
    ];
    for (data, location) in cases {
        local(&home, data);
        // Even reading an unrelated cell must reject a broken table.
        for args in [&["cell", "middle-other"][..], &["table"][..]] {
            let error = run(&home, args, 1);
            assert_eq!(error["ok"], false);
            assert_eq!(error["error"], "invalid_data");
            let message = error["message"].as_str().unwrap();
            assert!(message.contains("dispatch.toml"), "{message}");
            assert!(message.contains(location), "{data}: {message}");
        }
    }
}

#[test]
fn descriptions_and_agent_options_are_data_in_read_output() {
    let home = TempDir::new().unwrap();
    let table = run(&home, &["table"], 0);
    for cell in table["cells"].as_object().unwrap().values() {
        assert!(!cell["description"].as_str().unwrap_or("").is_empty());
    }
    for name in ["claude", "codex"] {
        assert_eq!(table["agents"][name]["efforts"], json!(["high", "xhigh"]));
    }
    assert_eq!(table["agents"]["claude"]["models"], json!(["opus[1m]"]));
    assert_eq!(table["agents"]["codex"]["models"], json!(["gpt-6-astra"]));
    for name in ["pi", "omp", "amp", "agy"] {
        assert_eq!(table["agents"][name]["efforts"], json!([]));
        assert_eq!(table["agents"][name]["models"], json!([]));
    }
    local(
        &home,
        "[cells.middle-other]\ndescription = '本机说明'\n\
         [agents.codex]\nefforts = ['high', 'xhigh', 'custom']\nmodels = ['suggested']\n",
    );
    let table = run(&home, &["table"], 0);
    assert_eq!(table["cells"]["middle-other"]["description"], "本机说明");
    assert_eq!(
        run(&home, &["cell", "middle-other"], 0)["cell"]["description"],
        "本机说明"
    );
    assert_eq!(table["agents"]["codex"]["efforts"][2], "custom");
    assert_eq!(table["agents"]["codex"]["models"], json!(["suggested"]));
}

#[test]
fn edit_preserves_other_cells_templates_order_and_comments() {
    let home = TempDir::new().unwrap();
    let original = "# user header\n[agents.pi] # keep template\nargv = ['pi', '--local']\n\n\
                    [cells.middle-other] # target\nmodel = 'old' # model note\n\
                    # keep description note\ndescription = 'old description'\n\n\
                    # keep next cell\n[cells.low-other]\nagents = [ 'omp', 'pi' ] # order\n";
    local(&home, original);
    let before = run(&home, &["table"], 0);
    let after = run(
        &home,
        &[
            "edit",
            "middle-other",
            "--agents",
            r#"["claude","codex"]"#,
            "--model",
            "unlisted-model",
            "--effort",
            "high",
            "--cross-review",
            "review-codex",
            "--rework",
            "high-other",
            "--description",
            "新的说明",
        ],
        0,
    );
    let saved = fs::read_to_string(home.path().join(".ranch/dispatch.toml")).unwrap();
    assert!(saved.starts_with(original.split("[cells.middle-other]").next().unwrap()));
    assert!(
        saved.ends_with("# keep next cell\n[cells.low-other]\nagents = [ 'omp', 'pi' ] # order\n")
    );
    assert!(saved.contains("# target"));
    assert!(saved.contains("# model note"));
    assert!(saved.contains("# keep description note"));
    assert_eq!(after["agents"], before["agents"]);
    for (name, cell) in before["cells"].as_object().unwrap() {
        if name != "middle-other" {
            assert_eq!(&after["cells"][name], cell);
        }
    }
    let cell = &after["cells"]["middle-other"];
    assert_eq!(cell["agents"], json!(["claude", "codex"]));
    assert_eq!(cell["model"], "unlisted-model");
    assert_eq!(cell["effort"], "high");
    assert_eq!(cell["cross_review"], "review-codex");
    assert_eq!(cell["review"]["name"], "review-codex");
    assert_eq!(cell["rework"], "high-other");
    assert_eq!(cell["description"], "新的说明");
    assert_eq!(&run(&home, &["cell", "middle-other"], 0)["cell"], cell);
}

#[test]
fn edit_creates_missing_file_clears_fields_and_reset_restores_defaults() {
    let home = TempDir::new().unwrap();
    let before = run(&home, &["table"], 0);
    let edited = run(
        &home,
        &[
            "edit",
            "critical-other",
            "--model",
            "",
            "--effort",
            "",
            "--cross-review",
            "",
            "--rework",
            "",
            "--description",
            "",
        ],
        0,
    );
    assert!(home.path().join(".ranch/dispatch.toml").is_file());
    let cell = &edited["cells"]["critical-other"];
    for key in ["model", "effort", "cross_review", "rework", "review"] {
        assert!(cell[key].is_null(), "{key}");
    }
    assert_eq!(cell["description"], "");
    assert_eq!(
        cell["choices"][0]["argv"],
        json!(["codex", "--yolo", "--disable", "memories"])
    );
    assert_eq!(cell["choices"][0]["labels"], json!([]));
    assert_eq!(edited["agents"], before["agents"]);
    let empty = run(&home, &["edit", "critical-other", "--agents", "[]"], 0);
    assert_eq!(empty["cells"]["critical-other"]["agents"], json!([]));
    assert_eq!(empty["cells"]["critical-other"]["choices"], json!([]));
    let restored = run(&home, &["reset", "critical-other"], 0);
    assert_eq!(restored["cells"], before["cells"]);
    assert_eq!(restored["agents"], before["agents"]);
    assert_eq!(
        run(&home, &["cell", "critical-other"], 0)["cell"],
        before["cells"]["critical-other"]
    );
    assert!(
        !fs::read_to_string(home.path().join(".ranch/dispatch.toml"))
            .unwrap()
            .contains("critical-other")
    );
}

#[test]
fn invalid_edits_report_field_and_leave_file_unchanged() {
    let home = TempDir::new().unwrap();
    let original = "# preserve exactly\n[cells.middle-other]\nmodel = 'custom'\n\
                    [cells.low-design]\nrework = 'middle-other'\n";
    local(&home, original);
    for (args, location) in [
        (
            vec!["--agents", r#"["missing"]"#],
            "cells.middle-other.agents",
        ),
        (vec!["--effort", "unknown"], "cells.middle-other.effort"),
        (vec!["--agents", r#"["pi"]"#, "--effort", "high"], "pi"),
        (
            vec!["--cross-review", "missing"],
            "cells.middle-other.cross_review",
        ),
        (vec!["--rework", "missing"], "cells.middle-other.rework"),
        (vec!["--rework", "low-design"], "cycle"),
    ] {
        let mut command = vec!["edit", "middle-other"];
        command.extend(args);
        let error = run(&home, &command, 1);
        assert_eq!(error["error"], "invalid_data");
        assert!(
            error["message"].as_str().unwrap().contains(location),
            "{error}"
        );
        assert_eq!(
            fs::read_to_string(home.path().join(".ranch/dispatch.toml")).unwrap(),
            original
        );
    }
    // The same table validation also applies when reading a hand-edited file.
    local(&home, "[cells.middle-other]\neffort = 'unknown'\n");
    assert_eq!(run(&home, &["table"], 1)["error"], "invalid_data");
    // Options overridden in data, including non-suggested models, are honored by writes.
    local(
        &home,
        "[agents.codex]\nefforts = ['high', 'xhigh', 'custom']\n",
    );
    let after = run(
        &home,
        &[
            "edit",
            "middle-other",
            "--effort",
            "custom",
            "--model",
            "free-form",
        ],
        0,
    );
    assert_eq!(after["cells"]["middle-other"]["effort"], "custom");
    assert_eq!(after["cells"]["middle-other"]["model"], "free-form");
}

#[test]
fn version_checks_detect_external_changes_without_writing() {
    let home = TempDir::new().unwrap();
    let table = run(&home, &["table"], 0);
    let missing = table["local"]["version"]
        .as_str()
        .expect("missing-file version");
    let first = run(
        &home,
        &[
            "edit",
            "middle-other",
            "--model",
            "first",
            "--if-version",
            missing,
        ],
        0,
    );
    let version = first["local"]["version"].as_str().unwrap();
    assert_ne!(version, missing);
    assert_eq!(run(&home, &["table"], 0)["local"]["version"], version);
    let path = home.path().join(".ranch/dispatch.toml");
    let saved = fs::read_to_string(&path).unwrap();
    let changed = format!("{saved}# another editor\n");
    fs::write(&path, &changed).unwrap();
    for args in [
        vec![
            "edit",
            "middle-other",
            "--model",
            "stale",
            "--if-version",
            version,
        ],
        vec!["reset", "middle-other", "--if-version", version],
    ] {
        let error = run(&home, &args, 3);
        assert_eq!(error["error"], "conflict");
        assert_eq!(error["expected_version"], version);
        assert_ne!(error["actual_version"], version);
        assert_eq!(fs::read_to_string(&path).unwrap(), changed);
    }
    // Staleness takes precedence even when the external edit breaks TOML.
    local(&home, "[invalid");
    assert_eq!(
        run(
            &home,
            &["reset", "middle-other", "--if-version", version],
            3
        )["error"],
        "conflict"
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "[invalid");
    local(&home, &changed);
    let current = run(&home, &["table"], 0);
    let reset = run(
        &home,
        &[
            "reset",
            "middle-other",
            "--if-version",
            current["local"]["version"].as_str().unwrap(),
        ],
        0,
    );
    assert_eq!(reset["cells"]["middle-other"]["model"], "gpt-6-astra");
    fs::remove_file(&path).unwrap();
    assert_eq!(
        run(
            &home,
            &[
                "edit",
                "middle-other",
                "--model",
                "stale",
                "--if-version",
                version
            ],
            3
        )["error"],
        "conflict"
    );
    assert!(!path.exists());
}

#[test]
fn edits_refuse_dotted_keys_that_cannot_preserve_original_order() {
    let home = TempDir::new().unwrap();
    let original = "# interleaved dotted keys\n\
                    cells.middle-other.model = 'first' # first\n\
                    cells.low-other.agents = ['pi'] # middle\n\
                    cells.middle-other.effort = 'high' # last\n";
    local(&home, original);
    run(&home, &["table"], 0);
    for args in [
        vec!["edit", "middle-other", "--model", "second"],
        vec!["reset", "middle-other"],
    ] {
        let error = run(&home, &args, 1);
        assert_eq!(error["error"], "format_not_preserved");
        assert!(error["message"].as_str().unwrap().contains("dotted"));
        assert_eq!(
            fs::read_to_string(home.path().join(".ranch/dispatch.toml")).unwrap(),
            original
        );
    }
}
