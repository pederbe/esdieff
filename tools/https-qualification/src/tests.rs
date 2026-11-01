use super::*;
use rustls::{
    ServerConfig, ServerConnection, StreamOwned,
    pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime, pem::PemObject},
};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener},
    thread,
    time::Instant,
};
use ureq::{
    tls::{Certificate, RootCerts},
    unversioned::{
        resolver::{ResolvedSocketAddrs, Resolver},
        transport::{DefaultConnector, NextTimeout},
    },
};

const CA: &[u8] = include_bytes!("../fixtures/ca.pem");
const EC: &[u8] = include_bytes!("../fixtures/ecdsa.pem");
const RSA: &[u8] = include_bytes!("../fixtures/rsa.pem");
const EC_KEY: &[u8] = include_bytes!("../fixtures/ecdsa.key");
const RSA_KEY: &[u8] = include_bytes!("../fixtures/rsa.key");
fn digest(bytes: &[u8]) -> String {
    sha256(bytes)
}
fn trusted() -> TlsConfig {
    TlsConfig::builder()
        .unversioned_rustls_crypto_provider(Arc::new(rustls_provider_rustcrypto_pq::provider()))
        .root_certs(RootCerts::new_with_certs(&[
            Certificate::from_pem(CA).unwrap()
        ]))
        .build()
}

struct Server {
    uri: String,
    address: SocketAddr,
    join: thread::JoinHandle<Vec<u8>>,
}
impl Server {
    fn start(rsa: bool, handshake_delay: Duration, chunks: Vec<(Duration, Vec<u8>)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let provider = Arc::new(rustls_provider_rustcrypto_pq::provider());
        let cfg = ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from_pem_slice(if rsa { RSA } else { EC }).unwrap()],
                PrivateKeyDer::from_pem_slice(if rsa { RSA_KEY } else { EC_KEY }).unwrap(),
            )
            .unwrap();
        let join = thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            thread::sleep(handshake_delay);
            let mut stream =
                StreamOwned::new(ServerConnection::new(Arc::new(cfg)).unwrap(), socket);
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") && request.len() < HEADER_BYTES {
                let mut byte = [0];
                if stream.read_exact(&mut byte).is_err() {
                    return request;
                }
                request.push(byte[0]);
            }
            for (delay, bytes) in chunks {
                thread::sleep(delay);
                if stream
                    .write_all(&bytes)
                    .and_then(|()| stream.flush())
                    .is_err()
                {
                    break;
                }
            }
            stream.conn.send_close_notify();
            let _ = stream.flush();
            request
        });
        Self {
            uri: format!("https://localhost:{}/file", address.port()),
            address,
            join,
        }
    }
    fn response(bytes: Vec<u8>) -> Self {
        Self::start(false, Duration::ZERO, vec![(Duration::ZERO, bytes)])
    }
    fn finish(self) -> Vec<u8> {
        self.join.join().unwrap()
    }
}
fn call(server: &Server, expected: &[u8]) -> Result<Vec<u8>, Failure> {
    receive(
        &config(trusted(), Duration::from_secs(1), Duration::from_secs(2)).new_agent(),
        &server.uri,
        &digest(expected),
    )
}
fn response(body: &[u8]) -> Vec<u8> {
    let mut bytes =
        format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len()).into_bytes();
    bytes.extend_from_slice(body);
    bytes
}

#[test]
fn rsa_and_ecdsa_handshakes_preserve_bytes_and_anonymous_identity_request() {
    let bytes = b"\0\xff\r\n{invalid json}";
    for rsa in [false, true] {
        let server = Server::start(rsa, Duration::ZERO, vec![(Duration::ZERO, response(bytes))]);
        assert_eq!(call(&server, bytes).unwrap(), bytes);
        let request = String::from_utf8(server.finish())
            .unwrap()
            .to_ascii_lowercase();
        assert!(request.contains("accept-encoding: identity\r\n"));
        for name in ["authorization:", "proxy-authorization:", "cookie:"] {
            assert!(!request.contains(name));
        }
    }
}

#[test]
fn untrusted_issuer_and_wrong_hostname_fail_before_http() {
    let server = Server::response(response(b"x"));
    let agent = config(tls_config(), Duration::from_secs(1), Duration::from_secs(2)).new_agent();
    assert!(matches!(
        receive(&agent, &server.uri, &digest(b"x")),
        Err(Failure::Transport(_))
    ));
    assert!(server.finish().is_empty());
    let server = Server::response(response(b"x"));
    let wrong = server.uri.replace("localhost", "127.0.0.1");
    let agent = config(trusted(), Duration::from_secs(1), Duration::from_secs(2)).new_agent();
    assert!(matches!(
        receive(&agent, &wrong, &digest(b"x")),
        Err(Failure::Transport(_))
    ));
    assert!(server.finish().is_empty());
}

#[test]
fn expired_certificate_is_rejected_by_the_selected_verifier() {
    use rustls::client::danger::ServerCertVerifier;
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(CertificateDer::from_pem_slice(CA).unwrap())
        .unwrap();
    let verifier = rustls::client::WebPkiServerVerifier::builder_with_provider(
        Arc::new(roots),
        Arc::new(rustls_provider_rustcrypto_pq::provider()),
    )
    .build()
    .unwrap();
    let result = verifier.verify_server_cert(
        &CertificateDer::from_pem_slice(EC).unwrap(),
        &[],
        &ServerName::try_from("localhost").unwrap(),
        &[],
        UnixTime::since_unix_epoch(Duration::from_secs(8_000_000_000)),
    );
    assert!(matches!(
        result,
        Err(rustls::Error::InvalidCertificate(
            rustls::CertificateError::ExpiredContext { .. }
        ))
    ));
}

#[test]
fn status_encoding_digest_and_incomplete_failures() {
    for status in [201, 204, 301, 302, 401, 404, 500] {
        let server = Server::response(format!("HTTP/1.1 {status} No\r\nContent-Length: 0\r\nLocation: https://example.org/\r\n\r\n").into_bytes());
        assert!(matches!(call(&server, b""), Err(Failure::Status(s)) if s == status));
        server.finish();
    }
    let server = Server::response(
        b"HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: 0\r\n\r\n".to_vec(),
    );
    assert!(matches!(call(&server, b""), Err(Failure::Encoding)));
    server.finish();
    let server = Server::response(response(b"x"));
    assert!(matches!(call(&server, b"y"), Err(Failure::Digest)));
    server.finish();
    let server = Server::response(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nx".to_vec());
    assert!(matches!(call(&server, b"x"), Err(Failure::Body(_))));
    server.finish();
}

#[test]
fn fixed_chunked_and_close_delimited_bodies_obey_the_body_ceiling() {
    let body = vec![b'x'; BODY_BYTES];
    let server = Server::response(response(&body));
    assert_eq!(call(&server, &body).unwrap().len(), BODY_BYTES);
    server.finish();
    let server = Server::response(response(&vec![b'x'; BODY_BYTES + 1]));
    assert!(matches!(call(&server, b""), Err(Failure::BodyLimit)));
    server.finish();
    let mut close = b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".to_vec();
    close.extend_from_slice(&vec![b'x'; BODY_BYTES + 1]);
    let server = Server::response(close);
    assert!(matches!(call(&server, b""), Err(Failure::BodyLimit)));
    server.finish();
    let mut chunked = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec();
    chunked.extend_from_slice(format!("{:x}\r\n", BODY_BYTES + 1).as_bytes());
    chunked.extend_from_slice(&vec![b'x'; BODY_BYTES + 1]);
    chunked.extend_from_slice(b"\r\n0\r\n\r\n");
    let server = Server::response(chunked);
    assert!(matches!(call(&server, b""), Err(Failure::BodyLimit)));
    server.finish();
}

#[test]
fn single_large_header_is_rejected_but_cumulative_informational_headers_are_not() {
    let mut large = b"HTTP/1.1 200 OK\r\nX-Large: ".to_vec();
    large.extend_from_slice(&vec![b'x'; HEADER_BYTES]);
    large.extend_from_slice(b"\r\nContent-Length: 0\r\n\r\n");
    let server = Server::response(large);
    assert!(matches!(call(&server, b""), Err(Failure::Transport(_))));
    server.finish();
    let mut info = b"HTTP/1.1 103 Early Hints\r\nX-Padding: ".to_vec();
    info.extend_from_slice(&vec![b'x'; 4096]);
    info.extend_from_slice(b"\r\n\r\n");
    let mut chunks = vec![(Duration::ZERO, info); 20];
    chunks.push((Duration::ZERO, response(b"x")));
    let server = Server::start(false, Duration::ZERO, chunks);
    // A negative control: this acceptance violates the inherited cumulative cap.
    assert_eq!(call(&server, b"x").unwrap(), b"x");
    server.finish();
}

#[derive(Debug)]
struct SlowResolver {
    address: SocketAddr,
    delay: Duration,
}
impl Resolver for SlowResolver {
    fn resolve(
        &self,
        _: &ureq::http::Uri,
        _: &ureq::config::Config,
        _: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        thread::sleep(self.delay);
        let mut result = self.empty();
        result.push(self.address);
        Ok(result)
    }
}

#[test]
fn resolution_and_handshake_receive_separate_allowances() {
    let server = Server::start(
        false,
        Duration::from_millis(150),
        vec![(Duration::ZERO, response(b"x"))],
    );
    let resolver = SlowResolver {
        address: server.address,
        delay: Duration::from_millis(150),
    };
    let agent = Agent::with_parts(
        config(
            trusted(),
            Duration::from_millis(250),
            Duration::from_secs(2),
        ),
        DefaultConnector::default(),
        resolver,
    );
    let started = Instant::now();
    assert_eq!(receive(&agent, &server.uri, &digest(b"x")).unwrap(), b"x");
    assert!(started.elapsed() > Duration::from_millis(250));
    server.finish();
}

#[test]
fn handshake_headers_and_trickling_body_exhaust_deadlines() {
    let server = Server::start(
        false,
        Duration::from_millis(500),
        vec![(Duration::ZERO, response(b"x"))],
    );
    let agent = config(
        trusted(),
        Duration::from_millis(100),
        Duration::from_secs(2),
    )
    .new_agent();
    assert!(matches!(
        receive(&agent, &server.uri, &digest(b"x")),
        Err(Failure::Transport(ureq::Error::Timeout(
            ureq::Timeout::Connect
        )))
    ));
    server.finish();
    let server = Server::start(
        false,
        Duration::ZERO,
        vec![(Duration::from_millis(500), response(b"x"))],
    );
    let agent = config(
        trusted(),
        Duration::from_secs(1),
        Duration::from_millis(200),
    )
    .new_agent();
    assert!(matches!(
        receive(&agent, &server.uri, &digest(b"x")),
        Err(Failure::Transport(ureq::Error::Timeout(
            ureq::Timeout::Global
        )))
    ));
    server.finish();
    let mut chunks = vec![(
        Duration::ZERO,
        b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n".to_vec(),
    )];
    chunks.extend(vec![(Duration::from_millis(80), b"x".to_vec()); 10]);
    let server = Server::start(false, Duration::ZERO, chunks);
    let agent = config(
        trusted(),
        Duration::from_secs(1),
        Duration::from_millis(250),
    )
    .new_agent();
    assert!(matches!(
        receive(&agent, &server.uri, &digest(b"xxxxxxxxxx")),
        Err(Failure::Body(error)) if error.get_ref()
            .and_then(|source| source.downcast_ref::<ureq::Error>())
            .is_some_and(|error| matches!(error, ureq::Error::Timeout(ureq::Timeout::Global)))
    ));
    server.finish();
}

#[test]
fn other_origins_are_refused() {
    for uri in [
        "http://raw.githubusercontent.com/x",
        "https://raw.githubusercontent.com.evil/x",
        "https://user:secret@raw.githubusercontent.com/x",
    ] {
        assert!(matches!(fetch(uri, ""), Err(Failure::Origin)));
    }
}

#[test]
fn complete_empty_and_chunked_bodies_keep_exact_bytes() {
    let server = Server::response(response(b""));
    assert!(call(&server, b"").unwrap().is_empty());
    server.finish();
    let server = Server::response(b"HTTP/1.1 200 OK\r\nContent-Encoding: identity\r\nTransfer-Encoding: chunked\r\n\r\n2\r\na\n\r\n1\r\nb\r\n0\r\n\r\n".to_vec());
    assert_eq!(call(&server, b"a\nb").unwrap(), b"a\nb");
    server.finish();
}

#[test]
fn delayed_resolution_exhausts_the_allowance_it_receives() {
    use std::sync::{Mutex, mpsc};
    #[derive(Debug)]
    struct BlockedResolver(Mutex<mpsc::Receiver<()>>);
    impl Resolver for BlockedResolver {
        fn resolve(
            &self,
            _: &ureq::http::Uri,
            _: &ureq::config::Config,
            timeout: NextTimeout,
        ) -> Result<ResolvedSocketAddrs, ureq::Error> {
            // Same bounded channel wait as ureq's DefaultResolver, with a
            // deterministic slow lookup in place of the host's getaddrinfo.
            let result = self.0.lock().unwrap().recv_timeout(*timeout.after);
            assert!(result.is_err());
            Err(ureq::Error::Timeout(timeout.reason))
        }
    }
    let (sender, receiver) = mpsc::sync_channel(1);
    let (release, released) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
        // Lookup cannot finish until the caller explicitly releases it.
        released.recv_timeout(Duration::from_secs(2)).unwrap();
        let _ = sender.send(());
    });
    let agent = Agent::with_parts(
        config(trusted(), Duration::from_millis(50), Duration::from_secs(1)),
        DefaultConnector::default(),
        BlockedResolver(Mutex::new(receiver)),
    );
    let started = Instant::now();
    assert!(matches!(
        receive(&agent, "https://localhost/file", &digest(b"")),
        Err(Failure::Transport(ureq::Error::Timeout(
            ureq::Timeout::Resolve
        )))
    ));
    let elapsed = started.elapsed();
    release.send(()).unwrap();
    worker.join().unwrap();
    assert!(elapsed >= Duration::from_millis(50));
    assert!(elapsed < Duration::from_secs(1));
}
