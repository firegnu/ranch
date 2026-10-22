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
