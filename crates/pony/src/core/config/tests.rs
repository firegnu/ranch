use super::*;
use std::os::unix::fs::PermissionsExt;

fn fixture(source: &str) -> (tempfile::TempDir, ConfigFile) {
    let home = tempfile::tempdir().unwrap();
    fs::set_permissions(home.path(), fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(home.path().join("config.toml"), source).unwrap();
    let (_, file) = load_with_source(home.path()).unwrap().unwrap();
    (home, file)
}

#[test]
fn binding_preserves_inline_array_comments_order_and_permissions() {
    let kind = CHATS[0].kind();
    let source = format!(
        "# header\ncorral = '/tmp/fake-corral'\nproxy = 'http://localhost:9' # proxy\n\
         [{kind}] # adapter\nchannels = [{{ repo = 'old', id = 'COLD' }}] # bindings\n\
         owner = 'UOWNER' # owner\n"
    );
    for mode in [0o600, 0o640, 0o644] {
        let (home, file) = fixture(&source);
        fs::set_permissions(&file.path, fs::Permissions::from_mode(mode)).unwrap();
        file.bind(kind, "CNEW", "new.repo-1_2").unwrap();
        let written = fs::read_to_string(&file.path).unwrap();
        assert_eq!(
            written,
            format!(
                "# header\ncorral = '/tmp/fake-corral'\nproxy = 'http://localhost:9' # proxy\n\
             [{kind}] # adapter\nchannels = [{{ repo = 'old', id = 'COLD' }}, {{ id = \"CNEW\", repo = \"new.repo-1_2\" }}] # bindings\n\
             owner = 'UOWNER' # owner\n"
            )
        );
        assert_eq!(
            fs::metadata(&file.path).unwrap().permissions().mode() & 0o7777,
            mode
        );
        let config = load(home.path()).unwrap().unwrap();
        assert_eq!(config.chats[0].channels.len(), 2);
        assert_eq!(config.chats[0].channels[1].repo, "new.repo-1_2");
        assert_eq!(fs::read_dir(home.path()).unwrap().count(), 1);
    }
}

#[test]
fn binding_preserves_array_of_tables_comments_and_order() {
    let kind = CHATS[0].kind();
    let source = format!(
        "# header\nproxy = 'http://localhost:9'\n[{kind}]\nowner = 'UOWNER'\n\
         # first binding\n[[{kind}.channels]]\nrepo = 'old' # repository\nid = 'COLD'\n"
    );
    let (home, file) = fixture(&source);
    file.bind(kind, "CNEW", "new").unwrap();
    assert_eq!(
        fs::read_to_string(&file.path).unwrap(),
        format!("{source}\n[[{kind}.channels]]\nid = \"CNEW\"\nrepo = \"new\"\n")
    );
    let config = load(home.path()).unwrap().unwrap();
    assert_eq!(config.chats[0].channels[0].repo, "old");
    assert_eq!(config.chats[0].channels[1].id, "CNEW");
}

#[test]
fn binding_accepts_missing_final_newline_in_both_channel_formats() {
    let kind = CHATS[0].kind();
    for channels in [
        "channels = [{id = 'COLD', repo = 'old'}] # keep".to_owned(),
        format!("[[{kind}.channels]]\nid = 'COLD'\nrepo = 'old' # keep"),
    ] {
        let source = format!("[{kind}]\nowner = 'UOWNER'\n{channels}");
        let (home, file) = fixture(&source);
        file.bind(kind, "CNEW", "new").unwrap();
        let written = fs::read_to_string(&file.path).unwrap();
        assert!(written.ends_with('\n'));
        assert!(written.contains("# keep"));
        let config = load(home.path()).unwrap().unwrap();
        assert_eq!(config.chats[0].channels.len(), 2);
        assert_eq!(config.chats[0].channels[0].repo, "old");
        assert_eq!(config.chats[0].channels[1].id, "CNEW");
    }
}

#[test]
fn binding_rechecks_disk_after_staging_and_discards_the_temporary_file() {
    let kind = CHATS[0].kind();
    let source = format!("[{kind}]\nowner = 'UOWNER'\nchannels = []\n");
    let (home, file) = fixture(&source);
    let binding = file.prepare_binding(kind, "CNEW", "new").unwrap();
    let edited = format!("{source}# changed just before rename\n");
    fs::write(&file.path, &edited).unwrap();
    assert_eq!(binding.commit().unwrap_err().code, "changed");
    assert_eq!(fs::read_to_string(&file.path).unwrap(), edited);
    assert_eq!(fs::read_dir(home.path()).unwrap().count(), 1);
}

#[test]
fn binding_rechecks_links_read_only_access_and_removal_before_rename() {
    use std::os::unix::fs::{MetadataExt, symlink};
    let kind = CHATS[0].kind();
    let source = format!("[{kind}]\nowner = 'UOWNER'\nchannels = []\n");
    for case in ["removed", "symlink", "hardlink", "readonly"] {
        let (home, file) = fixture(&source);
        let binding = file.prepare_binding(kind, "CNEW", "new").unwrap();
        let target = home.path().join("original.toml");
        if case == "readonly" {
            fs::set_permissions(&file.path, fs::Permissions::from_mode(0o444)).unwrap();
        } else {
            fs::rename(&file.path, &target).unwrap();
            if case == "symlink" {
                symlink(&target, &file.path).unwrap();
            } else if case == "hardlink" {
                fs::hard_link(&target, &file.path).unwrap();
            }
        }
        assert_eq!(
            binding.commit().unwrap_err().code,
            if case == "removed" {
                "changed"
            } else {
                "config_not_writable"
            }
        );
        if case == "removed" {
            assert!(!file.path.exists());
        } else if case == "symlink" {
            assert!(
                fs::symlink_metadata(&file.path)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
        } else if case == "hardlink" {
            assert_eq!(fs::metadata(&file.path).unwrap().nlink(), 2);
        }
        assert_eq!(
            fs::read_to_string(if case == "readonly" {
                &file.path
            } else {
                &target
            })
            .unwrap(),
            source
        );
        assert_eq!(
            fs::read_dir(home.path()).unwrap().count(),
            if matches!(case, "removed" | "readonly") {
                1
            } else {
                2
            }
        );
    }
}

#[test]
fn binding_failed_rename_leaves_the_original_file_unchanged() {
    let kind = CHATS[0].kind();
    let source = format!("[{kind}]\nowner = 'UOWNER'\nchannels = []\n");
    let (home, file) = fixture(&source);
    let binding = file.prepare_binding(kind, "CNEW", "new").unwrap();
    // Deterministic filesystem failure after the candidate has been written.
    fs::remove_file(binding.temporary.path()).unwrap();
    assert_eq!(binding.commit().unwrap_err().code, "io");
    assert_eq!(fs::read_to_string(&file.path).unwrap(), source);
    assert_eq!(fs::read_dir(home.path()).unwrap().count(), 1);
}

#[test]
fn binding_validates_the_candidate_before_writing() {
    let kind = CHATS[0].kind();
    let source =
        format!("[{kind}]\nowner = 'UOWNER'\nchannels = [{{id = 'COLD', repo = 'old'}}]\n");
    let (home, file) = fixture(&source);
    for (id, repo) in [
        ("CNEW", "old"),
        ("COLD", "new"),
        ("bad-id", "new"),
        ("CNEW", "bad/repo"),
    ] {
        assert_eq!(
            file.bind(kind, id, repo).unwrap_err().code,
            "config_invalid"
        );
        assert_eq!(fs::read_to_string(&file.path).unwrap(), source);
        assert_eq!(fs::read_dir(home.path()).unwrap().count(), 1);
    }
}
