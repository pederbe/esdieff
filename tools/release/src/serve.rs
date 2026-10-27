//! Loopback-only fixture for checking installation from held release archives.
use crate::Result;
use std::{
    fs,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    time::{Duration, Instant},
};

const HEADER_LIMIT: usize = 8 * 1024;

fn headers(stream: &mut TcpStream) -> io::Result<String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut bytes = Vec::new();
    let mut buffer = [0; 1024];
    while bytes.len() < HEADER_LIMIT {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "request header deadline exceeded",
            ));
        }
        stream.set_read_timeout(Some(remaining))?;
        let capacity = buffer.len().min(HEADER_LIMIT - bytes.len());
        let count = stream.read(&mut buffer[..capacity])?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "incomplete request",
            ));
        }
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(end) = bytes.windows(4).position(|value| value == b"\r\n\r\n") {
            bytes.truncate(end + 4);
            return String::from_utf8(bytes)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error));
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "request header is too large",
    ))
}

fn response(stream: &mut TcpStream, status: &str, body: &[u8], head: bool) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    if !head {
        stream.write_all(body)?;
    }
    Ok(())
}

fn filename(target: &str) -> Option<&str> {
    let name = target.strip_prefix('/')?.split('?').next()?;
    (!name.is_empty()
        && !matches!(name, "." | "..")
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte)))
    .then_some(name)
}

fn connection(root: &Path, stream: &mut TcpStream) -> io::Result<()> {
    stream.set_write_timeout(Some(Duration::from_secs(30)))?;
    let request = match headers(stream) {
        Ok(request) => request,
        Err(_) => return response(stream, "400 Bad Request", b"Invalid request\n", false),
    };
    let Some(line) = request.lines().next() else {
        return response(stream, "400 Bad Request", b"Invalid request\n", false);
    };
    let parts: Vec<_> = line.split_whitespace().collect();
    let [method, target, version] = parts.as_slice() else {
        return response(stream, "400 Bad Request", b"Invalid request\n", false);
    };
    let head = *method == "HEAD";
    if !matches!(*version, "HTTP/1.0" | "HTTP/1.1") {
        return response(stream, "400 Bad Request", b"Invalid request\n", head);
    }
    if !matches!(*method, "GET" | "HEAD") {
        return response(
            stream,
            "405 Method Not Allowed",
            b"Use GET or HEAD\n",
            false,
        );
    }
    let Some(name) = filename(target) else {
        return response(stream, "404 Not Found", b"Not found\n", head);
    };
    let path = root.join(name);
    if !fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_file()) {
        return response(stream, "404 Not Found", b"Not found\n", head);
    }
    let file = fs::File::open(path)?;
    let length = file.metadata()?.len();
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n"
    )?;
    if !head {
        let sent = io::copy(&mut file.take(length), stream)?;
        if sent != length {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "artifact changed during transfer",
            ));
        }
    }
    Ok(())
}

pub(crate) fn run(args: &[String]) -> Result<()> {
    let [directory, port_file] = args else {
        return Err("usage: esdieff-release serve-artifacts DIR PORT_FILE".into());
    };
    let root = fs::canonicalize(directory)?;
    if !root.is_dir() {
        return Err("artifact root is not a directory".into());
    }
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    fs::write(port_file, format!("{}\n", listener.local_addr()?.port()))?;
    for stream in listener.incoming() {
        let mut stream = stream?;
        if let Err(error) = connection(&root, &mut stream) {
            eprintln!("artifact fixture connection: {error}");
        }
    }
    Ok(())
}
