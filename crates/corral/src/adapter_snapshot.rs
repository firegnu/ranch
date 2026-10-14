//! Per-agent tables pinned by snapshots taken before the adapter refactor (R3). The snapshot
//! files must not change; see `tests/adapters.rs`.
use serde_json::{Value, json};
use std::{fs, path::Path};

pub(crate) fn snapshot(name: &str, value: &Value) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(name);
    let text = serde_json::to_string_pretty(value).unwrap() + "\n";
    if std::env::var_os("CORRAL_WRITE_SNAPSHOTS").is_some() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, &text).unwrap();
        return;
    }
    let expected = fs::read_to_string(&path).unwrap();
    if expected != text {
        let actual = path.with_extension("actual.json");
        fs::write(&actual, &text).unwrap();
        panic!("{name} differs from its snapshot; see {}", actual.display());
    }
}

#[test]
fn kinds_quit_steps_and_files_are_unchanged() {
    let mut kinds = json!({});
    for kind in [
        "claude",
        "codex",
        "pi",
        "omp",
        "",
        "cat",
        "other",
        "Claude",
        "claude-code",
        "claude.exe",
        "/usr/bin/claude",
    ] {
        kinds[kind] = json!({
            "known": crate::hooks::known(kind),
            "quit_steps": crate::hooks::quit_steps(kind),
        });
    }
    snapshot(
        "adapters_tables.json",
        &json!({"kinds": kinds, "files": crate::state::FILES}),
    );
}
