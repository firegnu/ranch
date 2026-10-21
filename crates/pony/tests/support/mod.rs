use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};
pub fn fake_corral(target: &Path) -> PathBuf {
    static BUILT: OnceLock<tempfile::TempDir> = OnceLock::new();
    let built = BUILT.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        let result = Command::new("rustc")
            .arg("--edition=2024")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/fake_corral.rs"))
            .arg("-o")
            .arg(dir.path().join("corral"))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        dir
    });
    let path = target.join("corral-test");
    std::fs::copy(built.path().join("corral"), &path).unwrap();
    path
}
