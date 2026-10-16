//! Shared by the integration tests.
use std::{path::Path, process::Command};

/// Copies an executable the test will run. `cp` writes it in a child process, so this
/// multi-threaded test process never holds the new file open for writing: on Linux a fork
/// from another test thread would inherit that descriptor and exec would fail with ETXTBSY.
pub fn copy_exe(from: impl AsRef<Path>, to: impl AsRef<Path>) {
    let status = Command::new("cp")
        .arg(from.as_ref())
        .arg(to.as_ref())
        .status()
        .unwrap();
    assert!(status.success(), "cp failed");
}
