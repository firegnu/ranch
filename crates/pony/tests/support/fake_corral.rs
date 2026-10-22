use std::{env, fs, io::Write};
fn main() {
    let args: Vec<_> = env::args().skip(1).collect();
    if let Ok(path) = env::var("FAKE_CORRAL_LOG") {
        let mut log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        for arg in &args {
            write!(log, "{}:{arg}", arg.len()).unwrap();
        }
        writeln!(log).unwrap();
    }
    if let Ok(path) = env::var("FAKE_CORRAL_RESPONSE")
        && std::path::Path::new(&path).exists()
    {
        print!("{}", fs::read_to_string(path).unwrap());
        std::process::exit(
            env::var("FAKE_CORRAL_EXIT")
                .unwrap_or("0".into())
                .parse()
                .unwrap(),
        );
    }
    match args.first().map(String::as_str) {
        Some("--version") => println!(r#"{{"ok":true,"contract":"1","version":"test"}}"#),
        Some("ls") => println!(r#"{{"ok":true,"agents":[]}}"#),
        Some("send") => println!(r#"{{"ok":true,"confirmed":true,"merged_with_draft":false}}"#),
        Some("keys") => println!(r#"{{"ok":true}}"#),
        _ => {
            println!(r#"{{"ok":false,"error":"not_found","message":"not found"}}"#);
            std::process::exit(2);
        }
    }
}
