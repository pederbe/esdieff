use super::*;
use std::{
    fs,
    io::Read,
    net::TcpListener,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub(super) const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
pub(super) const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
pub(super) const OBJECT: &str = "44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a";
pub(super) const MALFORMED: &str =
    "021fb596db81e6d02bf3d2586ee3981fe519f275c0ac9ca76bbcf2ebb4097d96";
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
pub(super) struct Temp(pub(super) PathBuf);
impl Temp {
    pub(super) fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "esdieff-acquire-test-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    pub(super) fn path(&self, name: impl AsRef<Path>) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

pub(super) fn args(destination: &Path, digest: &str) -> Vec<OsString> {
    vec![
        "acquire".into(),
        "--github".into(),
        "Owner/models".into(),
        "--commit".into(),
        "0123456789abcdef0123456789abcdef01234567".into(),
        "--path".into(),
        "models/document.json".into(),
        "--sha256".into(),
        digest.into(),
        "--document-output".into(),
        destination.as_os_str().to_owned(),
        "--format".into(),
        "json".into(),
    ]
}
/// Acquire with default controls and redirected stdout, as the plain receipt tests expect.
pub(super) fn execute(
    arguments: &[OsString],
    source: &ResponseSource,
    stdout: &mut dyn std::io::Write,
) -> Result<u8, Error> {
    acquire(arguments, &mut ColorMode::default(), false, source, stdout)
}
pub(super) fn invoke(arguments: &[OsString], source: &ResponseSource) -> (u8, Vec<u8>, String) {
    let mut stdout = Vec::new();
    match execute(arguments, source, &mut stdout) {
        Ok(code) => (code, stdout, String::new()),
        Err((code, message)) => (code, stdout, format!("esdieff: {message}\n")),
    }
}
pub(super) struct Fixture {
    pub(super) source: ResponseSource,
    request: Arc<Mutex<Vec<u8>>>,
    worker: Option<JoinHandle<()>>,
}
impl Fixture {
    pub(super) fn body(bytes: &[u8]) -> Self {
        let mut wire = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            bytes.len()
        )
        .into_bytes();
        wire.extend_from_slice(bytes);
        Self::wire(wire)
    }
    pub(super) fn wire(bytes: Vec<u8>) -> Self {
        Self::script(vec![(Duration::ZERO, bytes)], false)
    }
    pub(super) fn script(parts: Vec<(Duration, Vec<u8>)>, tls: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let target = format!(
            "{}://{}/",
            if tls { "https" } else { "http" },
            listener.local_addr().unwrap()
        );
        let request = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&request);
        let worker = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0; 1024];
            while let Ok(size) = socket.read(&mut chunk) {
                if size == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..size]);
                if tls
                    || bytes.windows(4).any(|value| value == b"\r\n\r\n")
                    || bytes.len() >= 16 * 1024
                {
                    break;
                }
            }
            *captured.lock().unwrap() = bytes;
            for (delay, part) in parts {
                thread::sleep(delay);
                if socket.write_all(&part).is_err() {
                    break;
                }
            }
        });
        Self {
            source: ResponseSource {
                target: Some(target),
                ..ResponseSource::default()
            },
            request,
            worker: Some(worker),
        }
    }
    pub(super) fn request(&self) -> Vec<u8> {
        self.request.lock().unwrap().clone()
    }
    pub(super) fn finish(&mut self) {
        if let Some(worker) = self.worker.take() {
            if self.source.observed.get() == 0 {
                let address = self
                    .source
                    .target
                    .as_ref()
                    .unwrap()
                    .split_once("://")
                    .unwrap()
                    .1
                    .trim_end_matches('/');
                if let Ok(socket) = std::net::TcpStream::connect(address) {
                    let _ = socket.shutdown(std::net::Shutdown::Both);
                }
            }
            worker.join().unwrap();
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.finish();
    }
}
