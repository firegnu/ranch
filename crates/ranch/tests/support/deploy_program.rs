//! Synthetic versioned programs; no agent or service is started.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--version"] {
        println!(r#"{{"ok":true,"contract":"1"}}"#);
    } else if std::env::current_exe().unwrap().file_name().unwrap() == "corral" {
        let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap());
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
