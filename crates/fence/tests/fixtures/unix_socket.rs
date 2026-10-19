use std::{
    env,
    io::{Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    process::Command,
};

fn main() {
    let args: Vec<_> = env::args_os().collect();
    if args[1] == "client" {
        let mut stream = UnixStream::connect(&args[2]).unwrap();
        stream.write_all(b"socket works").unwrap();
        return;
    }
    let tmp = PathBuf::from(env::var_os("TMPDIR").unwrap());
    assert_eq!(tmp, PathBuf::from(&args[1]));
    let nested = tmp.join("nested");
    std::fs::create_dir(&nested).unwrap();
    let socket = nested.join("test.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    // A separate process must also be able to connect under the inherited sandbox.
    let status = Command::new(env::current_exe().unwrap())
        .arg("client")
        .arg(&socket)
        .status()
        .unwrap();
    assert!(status.success());
    listener.set_nonblocking(true).unwrap();
    let (mut stream, _) = listener.accept().unwrap();
    stream.set_nonblocking(true).unwrap();
    let mut bytes = String::new();
    stream.read_to_string(&mut bytes).unwrap();
    assert_eq!(bytes, "socket works");
    for path in &args[2..] {
        let error = UnixListener::bind(path).expect_err("socket outside TMPDIR was allowed");
        assert_eq!(
            error.kind(),
            std::io::ErrorKind::PermissionDenied,
            "{path:?}: {error}"
        );
    }
    println!("temporary socket bind/connect and outside denials verified");
}
