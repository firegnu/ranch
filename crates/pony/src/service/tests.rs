use super::*;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};

#[test]
fn install_and_uninstall_are_idempotent_and_preserve_user_data() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home & <test> \"' 中文");
    fs::create_dir(&home).unwrap();
    let mut loaded = false;
    let mut calls = Vec::new();
    let mut launch = |args: &[String]| {
        // The plist and log must already exist when launchd loads the job.
        assert!(plist(&home).is_file());
        assert!(home.join(".pony/logs/launchd.log").is_file());
        calls.push(args.to_vec());
        match args[0].as_str() {
            "bootstrap" if loaded => Ok(Some(libc::EIO)),
            "bootstrap" => {
                loaded = true;
                Ok(Some(0))
            }
            "print" => Ok(Some(if loaded { 0 } else { 113 })),
            "bootout" => {
                loaded = false;
                Ok(Some(0))
            }
            _ => panic!("unexpected launchctl command"),
        }
    };
    let installed = manage(&home, false, false, &mut launch).unwrap();
    assert_eq!(installed["status"], "create");
    assert_eq!(installed["applied"], true);
    let bytes = read_owned(&home).unwrap().unwrap();
    let body = String::from_utf8(bytes.clone()).unwrap();
    assert!(body.contains(
        "home &amp; &lt;test&gt; &quot;&apos; 中文/.local/bin/pony</string><string>run</string>"
    ));
    assert!(body.contains("<key>RunAtLoad</key><true/>"));
    assert!(body.contains("<key>SuccessfulExit</key><false/>"));
    assert!(body.contains("<key>ThrottleInterval</key><integer>10</integer>"));
    assert!(body.contains("<key>StandardOutPath</key>"));
    assert!(body.contains("<key>StandardErrorPath</key>"));
    assert_eq!(body.matches("/.pony/logs/launchd.log</string>").count(), 2);
    assert!(!body.contains("EnvironmentVariables"));
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("/usr/bin/plutil")
            .args(["-convert", "json", "-o", "-"])
            .arg(plist(&home))
            .env_clear()
            .env("HOME", &home)
            .env("PONY_HOME", home.join(".pony"))
            .env("CORRAL_HOME", home.join(".corral"))
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let parsed: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            parsed,
            json!({
                "Label": "local.ranch.pony",
                "ProgramArguments": [home.join(".local/bin/pony"), "run"],
                "RunAtLoad": true, "KeepAlive": {"SuccessfulExit":false}, "ThrottleInterval":10,
                "StandardOutPath":home.join(".pony/logs/launchd.log"),
                "StandardErrorPath":home.join(".pony/logs/launchd.log"),
            })
        );
    }
    let inode = fs::metadata(plist(&home)).unwrap().ino();
    assert_eq!(
        manage(&home, false, false, &mut launch).unwrap()["status"],
        "same"
    );
    assert_eq!(fs::metadata(plist(&home)).unwrap().ino(), inode);
    for (path, mode) in [
        (".pony", 0o700),
        (".pony/logs", 0o700),
        (".pony/logs/launchd.log", 0o600),
    ] {
        assert_eq!(
            fs::metadata(home.join(path)).unwrap().permissions().mode() & 0o777,
            mode
        );
    }
    let config = home.join(".pony/config.toml");
    fs::write(&config, "# local configuration").unwrap();
    let log = home.join(".pony/logs/launchd.log");
    fs::write(&log, "retained diagnostics").unwrap();
    assert_eq!(
        manage(&home, true, false, &mut launch).unwrap()["status"],
        "remove"
    );
    assert_eq!(read_owned(&home).unwrap(), None);
    assert_eq!(
        manage(&home, true, false, &mut launch).unwrap()["status"],
        "absent"
    );
    assert_eq!(fs::read_to_string(config).unwrap(), "# local configuration");
    assert_eq!(fs::read_to_string(log).unwrap(), "retained diagnostics");
    let domain = format!("gui/{}", unsafe { libc::geteuid() });
    let target = format!("{domain}/local.ranch.pony");
    assert_eq!(
        calls,
        vec![
            vec![
                "bootstrap".into(),
                domain.clone(),
                plist(&home).to_str().unwrap().into()
            ],
            vec![
                "bootstrap".into(),
                domain,
                plist(&home).to_str().unwrap().into()
            ],
            vec!["print".into(), target.clone()],
            vec!["bootout".into(), target],
        ]
    );
    assert_eq!(
        fs::read_dir(plist(&home).parent().unwrap())
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn foreign_files_and_symlinks_are_never_changed_or_loaded() {
    for kind in ["file", "directory", "symlink", "dangling", "parent"] {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path();
        let path = plist(home);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let other = home.join("other");
        fs::write(&other, content(home).unwrap()).unwrap();
        match kind {
            "file" => fs::write(&path, "foreign content").unwrap(),
            "directory" => fs::create_dir(&path).unwrap(),
            "symlink" => symlink(&other, &path).unwrap(),
            "dangling" => symlink(home.join("absent"), &path).unwrap(),
            "parent" => {
                fs::remove_dir(path.parent().unwrap()).unwrap();
                fs::create_dir(home.join("elsewhere")).unwrap();
                symlink(home.join("elsewhere"), path.parent().unwrap()).unwrap();
            }
            _ => unreachable!(),
        }
        for remove in [false, true] {
            for dry in [false, true] {
                let error = manage(home, remove, dry, &mut |_| {
                    panic!("must not call launchctl")
                })
                .unwrap_err();
                assert_eq!(error.code, "foreign", "{kind}");
            }
        }
        assert_eq!(fs::read_to_string(&other).unwrap(), content(home).unwrap());
        if kind == "file" {
            assert_eq!(fs::read_to_string(&path).unwrap(), "foreign content");
        }
        assert!(!home.join(".pony").exists());
    }
}

#[test]
fn launchctl_errors_leave_the_plist_for_retry_and_unloaded_jobs_can_be_removed() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();
    let mut fail = |_: &[String]| Ok(Some(libc::EIO));
    assert_eq!(
        manage(home, false, false, &mut fail).unwrap_err().code,
        "launchctl_failed"
    );
    let original = read_owned(home).unwrap();
    assert!(original.is_some());
    assert_eq!(
        manage(home, true, false, &mut fail).unwrap_err().code,
        "launchctl_failed"
    );
    assert_eq!(read_owned(home).unwrap(), original);
    assert_eq!(
        manage(home, true, false, &mut |_| Ok(Some(libc::ESRCH))).unwrap()["applied"],
        true
    );
    assert_eq!(read_owned(home).unwrap(), None);
}

#[test]
fn owned_plist_updates_atomically_and_changed_files_are_left_alone() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();
    let path = plist(home);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let old = format!("{MARKER}\nold pony plist");
    fs::write(&path, &old).unwrap();
    let old_inode = fs::metadata(&path).unwrap().ino();
    assert_eq!(
        manage(home, false, false, &mut |_| Ok(Some(0))).unwrap()["status"],
        "replace"
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), content(home).unwrap());
    assert_ne!(fs::metadata(&path).unwrap().ino(), old_inode);
    assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);

    let previous = read_owned(home).unwrap();
    fs::write(&path, &old).unwrap();
    assert_eq!(
        write_plist(home, &content(home).unwrap(), &previous)
            .unwrap_err()
            .code,
        "changed"
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), old);
    fs::write(&path, "someone else's new file").unwrap();
    assert_eq!(
        write_plist(home, &content(home).unwrap(), &None)
            .unwrap_err()
            .code,
        "foreign"
    );
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "someone else's new file"
    );

    fs::write(&path, &old).unwrap();
    let error = manage(home, true, false, &mut |_| {
        fs::write(&path, "changed during bootout").unwrap();
        Ok(Some(0))
    })
    .unwrap_err();
    assert_eq!(error.code, "foreign");
    assert_eq!(fs::read_to_string(&path).unwrap(), "changed during bootout");
}

#[test]
fn previews_never_launch_or_write_and_existing_logs_are_not_truncated() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();
    for remove in [false, true] {
        assert_eq!(
            manage(home, remove, true, &mut |_| panic!(
                "preview must not launch"
            ))
            .unwrap()["applied"],
            false
        );
    }
    assert_eq!(fs::read_dir(home).unwrap().count(), 0);
    manage(home, false, false, &mut |_| Ok(Some(0))).unwrap();
    let log = home.join(".pony/logs/launchd.log");
    fs::write(&log, "keep this").unwrap();
    manage(home, false, false, &mut |_| Ok(Some(0))).unwrap();
    assert_eq!(fs::read_to_string(&log).unwrap(), "keep this");
    fs::remove_file(&log).unwrap();
    let target = home.join("unrelated");
    fs::write(&target, "leave alone").unwrap();
    symlink(&target, &log).unwrap();
    assert!(
        manage(home, false, false, &mut |_| panic!(
            "unsafe log must not launch"
        ))
        .is_err()
    );
    assert_eq!(fs::read_to_string(&target).unwrap(), "leave alone");
}
