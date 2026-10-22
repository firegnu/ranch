//! Synthetic versioned programs; no agent or service is started.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--hold-pipe"] {
        // Finite-lived descendant: the parent exits first, leaving stdout open.
        std::thread::sleep(std::time::Duration::from_secs(7));
        std::fs::write(
            std::env::var_os("DEPLOY_HELD_PIPE_DONE").unwrap(),
            "finished",
        )
        .unwrap();
    } else if args == ["--version"] {
        if std::env::current_exe().unwrap().file_name().unwrap() == "fence"
            && std::env::var_os("DEPLOY_HELD_PIPE_DONE").is_some()
        {
            std::process::Command::new(std::env::current_exe().unwrap())
                .arg("--hold-pipe")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap();
        }
        println!(r#"{{"ok":true,"contract":"1"}}"#);
    } else if std::env::current_exe().unwrap().file_name().unwrap() == "pony" {
        let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap());
        if args == ["status"] {
            if home.join("pony-kickstarted").exists() {
                for n in 0..3 {
                    let path = home.join(format!("pony-poll-{n}.json"));
                    if let Ok(status) = std::fs::read_to_string(&path) {
                        std::fs::remove_file(path).unwrap();
                        println!("{status}");
                        return;
                    }
                }
            }
            let status = std::fs::read_to_string(home.join("pony-status.json"))
                .unwrap_or_else(|_| r#"{"ok":true,"service":"not_installed","running":false,"pid":null,"chats":[]}"#.into());
            println!("{status}");
        } else if args == ["service", "install", "--dry-run"] {
            if home.join("pony-no-service").exists() {
                println!(r#"{{"ok":false,"error":"usage"}}"#);
                std::process::exit(2);
            }
            let state = if home.join("pony-replace").exists() { "replace" } else { "same" };
            println!(r#"{{"ok":true,"status":"{state}"}}"#);
        } else if args == ["service", "install", "--yes"] {
            assert!(home.join("pony-replace").exists());
            std::fs::rename(home.join("pony-status-next.json"), home.join("pony-status.json")).unwrap();
            std::fs::remove_file(home.join("pony-replace")).unwrap();
            std::fs::write(home.join("pony-installed"), "yes").unwrap();
            println!(r#"{{"ok":true,"status":"replace","applied":true}}"#);
        } else {
            panic!("unexpected pony invocation: {args:?}");
        }
    } else if std::env::current_exe().unwrap().file_name().unwrap() == "corral" {
        let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap());
        if args == ["integration", "status"] && std::env::var_os("DEPLOY_SLOW_STATUS").is_some() {
            std::thread::sleep(std::time::Duration::from_secs(6));
        }
        if args == ["install-skills", "--yes"] && home.join("fail-install").exists() {
            println!(r#"{{"ok":false,"error":"io","message":"synthetic install failure"}}"#);
            std::process::exit(1);
        }
        let status = std::process::Command::new(std::env::var_os("DEPLOY_REAL_CORRAL").unwrap())
            .args(args)
            .status()
            .unwrap();
        std::process::exit(status.code().unwrap_or(1));
    } else {
        eprintln!("unexpected synthetic program invocation: {args:?}");
        std::process::exit(2);
    }
}
