mod support;
use pony::corral::{Corral, Process};
use std::{fs, path::PathBuf, process::Command};

#[test]
fn process_protocol_with_isolated_fake_program() {
    let temp = tempfile::tempdir().unwrap();
    support::fake_corral(temp.path());
    let result = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "exercise_protocol_in_child", "--nocapture"])
        .env_clear()
        .env("HOME", temp.path())
        .env("PONY_HOME", temp.path().join("pony"))
        .env("CORRAL_HOME", temp.path().join("corral-home"))
        .env("PONY_FAKE_TEST", temp.path())
        .env("FAKE_CORRAL_LOG", temp.path().join("calls"))
        .env("FAKE_CORRAL_RESPONSE", temp.path().join("response"))
        .env("PATH", temp.path())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn exercise_protocol_in_child() {
    let Some(root) = std::env::var_os("PONY_FAKE_TEST") else {
        return;
    };
    let root = PathBuf::from(root);
    let binary = root.join("corral-test");
    let response = root.join("response");
    let mut process = Process::discover(binary.to_str(), None, &root).unwrap();
    let body = "--force literal `$(nothing)`\n中文  ";
    process.send("repo/main", body).unwrap();
    process.escape("repo/main", false).unwrap();
    process.escape("repo/amp", true).unwrap();
    process.approve("repo/main", 1).unwrap();
    process.approve("repo/main", 2).unwrap();
    let calls = fs::read_to_string(root.join("calls")).unwrap();
    assert!(calls.contains("4:keys9:repo/main3:esc\n"));
    assert!(calls.contains("4:keys8:repo/amp3:esc3:esc\n"));
    assert!(calls.contains("4:keys9:repo/main6:text:1\n"));
    assert!(calls.contains("4:keys9:repo/main6:text:2\n"));
    assert!(
        !calls.contains("5:enter"),
        "numeric choices submit without Enter"
    );
    assert!(process.approve("repo/main", 0).is_err());
    assert!(process.approve("repo/main", 10).is_err());
    assert!(
        fs::read_to_string(root.join("calls"))
            .unwrap()
            .contains(&format!("4:send9:repo/main2:--{}:{body}\n", body.len()))
    );
    fs::write(&response,r#"{"ok":true,"agents":[{"name":"repo/main","instance":"i","future":1},{"name":"repo/starting","starting":true}]}"#).unwrap();
    let agents = process.list().unwrap();
    assert_eq!(agents.len(), 2);
    assert_eq!(agents[1].instance, None);
    fs::write(&response,r#"{"ok":true,"name":"repo/main","instance":"i","state":"idle","state_started":1.0,"turn_started":1.0,"last_event":"Stop","last_event_at":2.0,"last_tool":null,"title":null,"paused":false,"interrupted":true,"future":1}"#).unwrap();
    assert!(process.status("repo/main").unwrap().interrupted);
    fs::write(&response, r#"{"ok":true,"name":"repo/main","instance":"i","state":"blocked","state_started":1.0,"last_output":2.5,"last_human_input":0.5}"#).unwrap();
    assert_eq!(process.status("repo/main").unwrap().last_output, Some(2.5));
    fs::write(
        &response,
        r#"{"ok":true,"name":"repo/main","instance":"i","text":"原样\n","at":2.0}"#,
    )
    .unwrap();
    assert_eq!(process.reply("repo/main").unwrap().text, "原样\n");
    fs::write(&response, "permission?\n中文\n").unwrap();
    assert_eq!(process.read("repo/main").unwrap(), "permission?\n中文\n");
    assert!(
        fs::read_to_string(root.join("calls"))
            .unwrap()
            .contains("4:read9:repo/main7:--bytes4:4000\n")
    );
    fs::remove_file(&response).unwrap();
    assert_eq!(process.read("repo/main").unwrap_err().code, "not_found");
    fs::copy(&binary, root.join("corral")).unwrap();
    Process::discover(None, Some(root.as_os_str()), &root).unwrap();
    fs::remove_file(root.join("corral")).unwrap();
    fs::create_dir_all(root.join(".local/bin")).unwrap();
    fs::copy(&binary, root.join(".local/bin/corral")).unwrap();
    Process::discover(None, Some(root.as_os_str()), &root).unwrap();
    fs::write(&response, r#"{"ok":true,"contract":"2"}"#).unwrap();
    assert!(Process::discover(binary.to_str(), None, &root).is_err());
    // A configured path is authoritative; do not silently switch to another executable.
    assert!(
        Process::discover(root.join("missing").to_str(), Some(root.as_os_str()), &root).is_err()
    );
}
