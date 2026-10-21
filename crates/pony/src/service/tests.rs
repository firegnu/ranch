use super::*;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};

#[test]
fn revision_two_log_is_prepared_before_stop_and_permissions_only_narrow() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();
    let path = plist(home);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let old = format!("{MARKER}\nold definition");
    fs::write(&path, &old).unwrap();
    let pony = home.join(".pony");
    let logs = pony.join("logs");
    fs::create_dir_all(&logs).unwrap();
    fs::set_permissions(&pony, fs::Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(&logs, fs::Permissions::from_mode(0o755)).unwrap();
    let error = manage(home, false, false, &mut |_| {
        panic!("invalid logs must not stop the service")
    })
    .unwrap_err();
    assert_eq!(error.code, "unsafe_path");
    assert_eq!(fs::read_to_string(&path).unwrap(), old);
    assert_eq!(fs::metadata(&logs).unwrap().mode() & 0o7777, 0o755);

    fs::set_permissions(&logs, fs::Permissions::from_mode(0o700)).unwrap();
    let log = logs.join("launchd.log");
    fs::write(&log, "retained log").unwrap();
    for expected in ["replace", "same"] {
        fs::set_permissions(&log, fs::Permissions::from_mode(0o644)).unwrap();
        manage(home, false, true, &mut |_| panic!("preview")).unwrap();
        assert_eq!(fs::metadata(&log).unwrap().mode() & 0o7777, 0o644);
        let result = manage(home, false, false, &mut |args: &[String]| {
            assert_eq!(
                fs::metadata(&log).unwrap().mode() & 0o7777,
                0o600,
                "repair log before any launchctl command"
            );
            assert_eq!(fs::read_to_string(&log).unwrap(), "retained log");
            Ok(Some(if args[0] == "bootout" { libc::ESRCH } else { 0 }))
        })
        .unwrap();
        assert_eq!(result["status"], expected);
    }
    fs::write(&path, &old).unwrap();
    fs::set_permissions(&log, fs::Permissions::from_mode(0o200)).unwrap();
    let error = manage(home, false, false, &mut |_| {
        panic!("do not widen permissions or stop service")
    })
    .unwrap_err();
    assert_eq!(error.code, "unsafe_path");
    assert_eq!(fs::metadata(&log).unwrap().mode() & 0o7777, 0o200);
    assert_eq!(fs::read_to_string(path).unwrap(), old);
}

#[test]
fn revision_two_successful_bootout_is_confirmed_before_completion() {
    use std::collections::VecDeque;
    let mut prints = VecDeque::from([0, 0, 113]);
    let mut waits = 0;
    bootout(
        "gui/501/local.ranch.pony",
        &mut |args: &[String]| match args[0].as_str() {
            "bootout" => Ok(Some(0)),
            "print" => Ok(Some(prints.pop_front().unwrap())),
            _ => panic!("unexpected command"),
        },
        &mut || {
            waits += 1;
            true
        },
    )
    .unwrap();
    assert!(
        prints.is_empty(),
        "bootout success alone does not confirm removal"
    );
    assert_eq!(waits, 2);
}

#[test]
fn revision_replace_loads_new_definition_and_create_rejects_loaded_name() {
    use std::cell::RefCell;
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();
    let path = plist(home);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let old = content(home)
        .unwrap()
        .replace("<integer>10</integer>", "<integer>20</integer>");
    fs::write(&path, &old).unwrap();
    let preview = manage(home, false, true, &mut |_| {
        panic!("preview must not launch")
    })
    .unwrap();
    assert_eq!(preview["commands"][0]["argv"][1], "bootout");
    let failure = manage(home, false, false, &mut |_| Ok(Some(libc::EIO))).unwrap_err();
    assert_eq!(failure.code, "launchctl_failed");
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        old,
        "failed bootout preserves the old definition"
    );
    let loaded = RefCell::new(Some(old.clone()));
    let mut launch = |args: &[String]| match args[0].as_str() {
        "bootstrap" if loaded.borrow().is_some() => Ok(Some(libc::EIO)),
        "bootstrap" => {
            *loaded.borrow_mut() = Some(fs::read_to_string(&path).unwrap());
            Ok(Some(0))
        }
        "print" => Ok(Some(if loaded.borrow().is_some() { 0 } else { 113 })),
        "bootout" => {
            assert_eq!(
                fs::read_to_string(&path).unwrap(),
                old,
                "unload before replacing the plist"
            );
            *loaded.borrow_mut() = None;
            Ok(Some(0))
        }
        _ => panic!("unexpected launchctl command"),
    };
    let result = manage(home, false, false, &mut launch).unwrap();
    assert_eq!(result["status"], "replace");
    assert_eq!(
        loaded.borrow().as_deref(),
        Some(content(home).unwrap().as_str()),
        "replace must change the definition held by launchd"
    );
    let preview = manage(home, false, true, &mut |_| panic!("preview")).unwrap();
    assert_eq!(preview["status"], "same");

    fs::remove_file(&path).unwrap();
    for _ in 0..2 {
        let error = manage(
            home,
            false,
            false,
            &mut |args: &[String]| match args[0].as_str() {
                "bootstrap" => Ok(Some(libc::EIO)),
                "print" => Ok(Some(0)),
                _ => panic!("must not unload a definition from elsewhere"),
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "already_loaded");
        assert!(error.message.contains("bootout"));
        assert!(
            !path.exists(),
            "a retry must not mistake the conflict for a same install"
        );
    }
}

#[test]
fn revision_bootout_waits_for_removal_and_reports_partial_failures() {
    use std::collections::VecDeque;
    // Polling uses a fake wait so success, timeout and errors need no real sleeps.
    for (prints, wait_limit, expected) in [
        (vec![0, 0, 113], 3, None),
        (vec![0, 0], 1, Some("stop_pending")),
        (vec![0, 5], 3, Some("launchctl_failed")),
    ] {
        let mut prints = VecDeque::from(prints);
        let mut waits = 0;
        let result = bootout(
            "gui/501/local.ranch.pony",
            &mut |args: &[String]| match args[0].as_str() {
                "bootout" => Ok(Some(libc::EINPROGRESS)),
                "print" => Ok(Some(prints.pop_front().unwrap())),
                _ => panic!("unexpected command"),
            },
            &mut || {
                waits += 1;
                waits <= wait_limit
            },
        );
        if let Some(code) = expected {
            let error = result.unwrap_err();
            assert_eq!(error.code, code);
            assert!(error.message.contains("retry this command"));
            assert!(error.message.contains("plist left"));
            assert!(!error.message.contains("Some("));
            if code == "launchctl_failed" {
                assert!(error.message.contains("exit 5"));
            }
        } else {
            result.unwrap();
            assert_eq!(waits, 2);
        }
        assert!(prints.is_empty());
    }

    for remove in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path();
        let path = plist(home);
        let old = format!("{MARKER}\nold definition");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, &old).unwrap();
        let mut gone = false;
        manage(
            home,
            remove,
            false,
            &mut |args: &[String]| match args[0].as_str() {
                "bootout" => Ok(Some(libc::EINPROGRESS)),
                "print" => {
                    assert_eq!(
                        fs::read_to_string(&path).unwrap(),
                        old,
                        "do not change plist before removal completes"
                    );
                    gone = true;
                    Ok(Some(113))
                }
                "bootstrap" => {
                    assert!(gone);
                    Ok(Some(0))
                }
                _ => panic!("unexpected command"),
            },
        )
        .unwrap();
        assert!(gone);
        assert_eq!(path.exists(), !remove);

        fs::write(&path, &old).unwrap();
        let error = manage(
            home,
            remove,
            false,
            &mut |args: &[String]| match args[0].as_str() {
                "bootout" => {
                    fs::write(&path, "changed while stopping").unwrap();
                    Ok(Some(0))
                }
                "print" => Ok(Some(113)),
                _ => panic!("unexpected command"),
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "foreign");
        assert!(error.message.contains("service was unloaded"));
        assert!(error.message.contains("rerun pony service"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "changed while stopping");
    }

    let temp = tempfile::tempdir().unwrap();
    let path = plist(temp.path());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, format!("{MARKER}\nold definition")).unwrap();
    let error = manage(
        temp.path(),
        false,
        false,
        &mut |args: &[String]| match args[0].as_str() {
            "bootout" => Ok(Some(0)),
            "print" => Ok(Some(113)),
            "bootstrap" => Ok(Some(libc::EIO)),
            _ => panic!("unexpected command"),
        },
    )
    .unwrap_err();
    assert!(error.message.contains("previous service was unloaded"));
    assert!(error.message.contains("exit 5"));
    assert_eq!(error.message.matches("plist left in place").count(), 1);
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        content(temp.path()).unwrap()
    );
    assert_eq!(
        manage(temp.path(), false, false, &mut |_| Ok(Some(0))).unwrap()["status"],
        "same"
    );
}

#[test]
fn revision_apply_requires_nonroot_account_home_but_accepts_directory_aliases() {
    let temp = tempfile::tempdir().unwrap();
    let account = temp.path().join("account");
    let other = temp.path().join("other");
    fs::create_dir(&account).unwrap();
    fs::create_dir(&other).unwrap();
    assert_eq!(
        check_context(&account, 0, |_| panic!("root rejected before lookup"))
            .unwrap_err()
            .code,
        "unsafe_context"
    );
    for home in [&other, &temp.path().join("missing")] {
        assert_eq!(
            check_context(home, 501, |_| Ok(account.clone()))
                .unwrap_err()
                .code,
            "unsafe_context"
        );
    }
    check_context(&account, 501, |uid| {
        assert_eq!(uid, 501);
        Ok(account.clone())
    })
    .unwrap();
    let alias = temp.path().join("alias");
    symlink(&account, &alias).unwrap();
    check_context(&alias, 501, |_| Ok(account.clone())).unwrap();
    assert_eq!(fs::read_dir(&account).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&other).unwrap().count(), 0);
}

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
            vec!["bootout".into(), target.clone()],
            vec!["print".into(), target],
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
        manage(
            home,
            false,
            false,
            &mut |args: &[String]| match args[0].as_str() {
                "bootout" | "bootstrap" => Ok(Some(0)),
                "print" => Ok(Some(113)),
                _ => panic!("unexpected command"),
            }
        )
        .unwrap()["status"],
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
    let error = manage(
        home,
        true,
        false,
        &mut |args: &[String]| match args[0].as_str() {
            "bootout" => {
                fs::write(&path, "changed during bootout").unwrap();
                Ok(Some(0))
            }
            "print" => Ok(Some(113)),
            _ => panic!("unexpected command"),
        },
    )
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
