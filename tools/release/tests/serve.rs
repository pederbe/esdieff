use std::{
    fs,
    io::{Read, Write},
    net::TcpStream,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Server {
    child: Child,
    root: PathBuf,
    port: u16,
}

impl Server {
    fn start() -> Self {
        let root = loop {
            let root = std::env::temp_dir().join(format!(
                "esdieff-artifact-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&root) {
                Ok(()) => break root,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create fixture: {error}"),
            }
        };
        let artifacts = root.join("artifacts");
        fs::create_dir(&artifacts).unwrap();
        fs::write(
            artifacts.join("esdieff-test.zip"),
            b"held\0artifact\xffbytes",
        )
        .unwrap();
        fs::write(root.join("outside.zip"), "must stay outside the fixture").unwrap();
        let port_file = root.join("port");
        let child = Command::new(env!("CARGO_BIN_EXE_esdieff-release"))
            .arg("serve-artifacts")
            .arg(&artifacts)
            .arg(&port_file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut server = Self {
            child,
            root,
            port: 0,
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if let Ok(text) = fs::read_to_string(&port_file)
                && let Ok(port) = text.trim().parse()
            {
                server.port = port;
                return server;
            }
            assert!(
                server.child.try_wait().unwrap().is_none(),
                "artifact fixture exited before writing its port"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("artifact fixture did not start");
    }

    fn request(&self, request: &str) -> Vec<u8> {
        let mut stream = TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, self.port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        response
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn parts(response: &[u8]) -> (&str, &[u8]) {
    let split = response
        .windows(4)
        .position(|bytes| bytes == b"\r\n\r\n")
        .unwrap()
        + 4;
    (
        std::str::from_utf8(&response[..split]).unwrap(),
        &response[split..],
    )
}

#[test]
fn loopback_get_and_head_serve_exact_held_bytes() {
    let server = Server::start();
    let response = server.request("GET /esdieff-test.zip HTTP/1.1\r\nHost: localhost\r\n\r\n");
    let (headers, body) = parts(&response);
    assert!(headers.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(headers.contains(&format!("Content-Length: {}\r\n", body.len())));
    assert_eq!(body, b"held\0artifact\xffbytes");
    let response = server.request("HEAD /esdieff-test.zip HTTP/1.1\r\nHost: localhost\r\n\r\n");
    let (headers, body) = parts(&response);
    assert!(headers.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(headers.contains("Content-Length: 19\r\n"));
    assert!(body.is_empty());
}

#[test]
fn parent_paths_encoded_names_and_missing_files_are_refused() {
    let server = Server::start();
    for path in [
        "/../outside.zip",
        "/%2e%2e/outside.zip",
        "/missing.zip",
        "/",
        "/..",
        "/nested/file.zip",
        "/.\\outside.zip",
    ] {
        let response = server.request(&format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n"));
        assert!(
            parts(&response).0.starts_with("HTTP/1.1 404 Not Found\r\n"),
            "accepted {path}"
        );
    }
    let response = server.request("POST /esdieff-test.zip HTTP/1.1\r\nHost: localhost\r\n\r\n");
    assert!(
        parts(&response)
            .0
            .starts_with("HTTP/1.1 405 Method Not Allowed\r\n")
    );
}

#[cfg(unix)]
#[test]
fn symbolic_links_cannot_expose_files_outside_artifacts() {
    let server = Server::start();
    std::os::unix::fs::symlink(
        server.root.join("outside.zip"),
        server.root.join("artifacts/link.zip"),
    )
    .unwrap();
    let response = server.request("GET /link.zip HTTP/1.1\r\nHost: localhost\r\n\r\n");
    assert!(parts(&response).0.starts_with("HTTP/1.1 404 Not Found\r\n"));
}

#[test]
fn oversized_headers_are_refused_without_stopping_the_fixture() {
    let server = Server::start();
    let request = format!(
        "GET /esdieff-test.zip HTTP/1.1\r\nX-Test: {}",
        "x".repeat(8 * 1024)
    );
    // Stop at the limit without a terminator, so a close does not leave unread
    // client bytes that the operating system would answer with a TCP reset.
    let response = server.request(&request[..8 * 1024]);
    assert!(
        parts(&response)
            .0
            .starts_with("HTTP/1.1 400 Bad Request\r\n")
    );
    let response = server.request("GET /esdieff-test.zip HTTP/1.1\r\nHost: localhost\r\n\r\n");
    assert!(parts(&response).0.starts_with("HTTP/1.1 200 OK\r\n"));
}
