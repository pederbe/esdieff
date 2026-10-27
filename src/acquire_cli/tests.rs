use super::support::*;
use super::*;
use serde_json::{Value, json};
use std::{
    fs, io,
    time::{Duration, Instant},
};

#[test]
fn acquisition_help_is_available_without_a_request() {
    let mut stdout = Vec::new();
    assert_eq!(
        execute(
            &["acquire".into(), "--help".into()],
            &ResponseSource::default(),
            &mut stdout
        ),
        Ok(0)
    );
    assert!(String::from_utf8(stdout).unwrap().contains("--sha256"));
}

#[test]
fn pinned_bytes_empty_malformed_and_ordinary_have_exact_success_receipts() {
    for (bytes, digest) in [
        (b"".as_slice(), EMPTY),
        (b"{", MALFORMED),
        (b"{}", OBJECT),
        (b"abc", ABC),
    ] {
        let directory = Temp::new();
        let document = directory.path("saved.json");
        let mut fixture = Fixture::body(bytes);
        let (code, stdout, stderr) = invoke(&args(&document, digest), &fixture.source);
        fixture.finish();
        assert_eq!((code, stderr), (0, String::new()));
        assert_eq!(fs::read(&document).unwrap(), bytes);
        let receipt: Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(receipt["schema"], "esdieff.acquisition");
        assert_eq!(receipt["version"], "1.0");
        assert_eq!(
            receipt["result"],
            json!({"status":"acquired","complete":true,"document_committed":true,"actual_sha256":digest,"byte_count":bytes.len(),"failures":[]})
        );
        assert_eq!(receipt["source"]["repository"], "Owner/models");
        assert_eq!(
            receipt["source"]["commit"],
            "0123456789abcdef0123456789abcdef01234567"
        );
        assert_eq!(receipt["source"]["path"], "models/document.json");
        assert_eq!(receipt["source"]["expected_sha256"], digest);
        assert_eq!(
            receipt["source"]["uri"],
            "https://raw.githubusercontent.com/Owner/models/0123456789abcdef0123456789abcdef01234567/models/document.json"
        );
        assert_eq!(
            receipt["document_output"]["native_bytes"],
            json!(document.as_os_str().as_encoded_bytes())
        );
        assert_eq!(receipt["staging_output"], Value::Null);
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&document).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let request = String::from_utf8(fixture.request())
            .unwrap()
            .to_ascii_lowercase();
        assert!(request.contains("accept-encoding: identity\r\n"));
        assert!(!request.contains("authorization:") && !request.contains("cookie:"));
    }
}

#[test]
fn repository_paths_are_encoded_with_a_fixed_production_origin() {
    let directory = Temp::new();
    let document = directory.path("output");
    let mut arguments = args(&document, ABC);
    arguments[6] = "space here/é%2e.json".into();
    let mut fixture = Fixture::body(b"abc");
    let (code, stdout, _) = invoke(&arguments, &fixture.source);
    fixture.finish();
    assert_eq!(code, 0);
    let receipt: Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(receipt["source"]["path"], "space here/é%2e.json");
    assert_eq!(
        receipt["source"]["uri"],
        "https://raw.githubusercontent.com/Owner/models/0123456789abcdef0123456789abcdef01234567/space%20here/%C3%A9%252e.json"
    );
    assert!(String::from_utf8(fixture.request()).unwrap().starts_with("GET /Owner/models/0123456789abcdef0123456789abcdef01234567/space%20here/%C3%A9%252e.json HTTP/1.1\r\n"));
}

#[test]
fn invalid_requests_and_destinations_never_reach_the_response_source() {
    let directory = Temp::new();
    let document = directory.path("output");
    let valid = args(&document, ABC);
    let source = ResponseSource {
        target: Some("http://127.0.0.1:9/".into()),
        ..ResponseSource::default()
    };
    let mut rejected = Vec::new();
    for (index, values) in [
        (2, vec!["https://github.com/a/b", "a/b/c", "a/.."]),
        (
            4,
            vec!["main", "01234", "gggggggggggggggggggggggggggggggggggggggg"],
        ),
        (
            6,
            vec![
                "/absolute",
                "a//b",
                "a/../b",
                "a/./b",
                "a\\b",
                "a?b",
                "a#b",
                "a\nb",
            ],
        ),
        (
            8,
            vec![
                "",
                "123",
                "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
            ],
        ),
        (12, vec!["yaml"]),
    ] {
        for value in values {
            let mut arguments = valid.clone();
            arguments[index] = value.into();
            rejected.push(arguments);
        }
    }
    for index in [1, 3, 5, 7, 9, 11] {
        let mut arguments = valid.clone();
        arguments.drain(index..=index + 1);
        rejected.push(arguments);
        let mut arguments = valid.clone();
        arguments.extend([valid[index].clone(), valid[index + 1].clone()]);
        rejected.push(arguments);
    }
    // Format is optional. Only its duplicate and unknown value are rejected.
    rejected.retain(|arguments| {
        arguments.len() != valid.len() - 2 || arguments.iter().any(|value| value == "--format")
    });
    for extra in [
        vec!["--url", "https://example.org"],
        vec!["--insecure"],
        vec!["--proxy", "localhost"],
        vec!["--retry", "2"],
        vec!["model.json"],
        vec!["--replace-document"],
    ] {
        let mut arguments = valid.clone();
        arguments.extend(extra.into_iter().map(OsString::from));
        rejected.push(arguments);
    }
    let mut enormous = valid.clone();
    enormous[6] = "x".repeat(64 * 1024).into();
    rejected.push(enormous);
    let mut excessive_uri = valid.clone();
    excessive_uri[6] = "x".repeat(2048).into();
    rejected.push(excessive_uri);
    let mut excessive_reserve = valid.clone();
    excessive_reserve[10] = "x".repeat(1500).into();
    rejected.push(excessive_reserve);
    let mut missing_parent = valid.clone();
    missing_parent[10] = directory.path("missing/output").into();
    rejected.push(missing_parent);
    let mut standard_output = valid.clone();
    standard_output[10] = "-".into();
    rejected.push(standard_output);
    for arguments in rejected {
        let (code, stdout, stderr) = invoke(&arguments, &source);
        assert_eq!(code, 2, "{arguments:?}: {stderr}");
        assert!(stdout.is_empty() && !stderr.is_empty());
        assert_eq!(source.observed.get(), 0);
        assert!(!document.exists());
    }
    fs::write(&document, b"existing owner").unwrap();
    assert_eq!(invoke(&valid, &source).0, 2);
    assert_eq!(fs::read(&document).unwrap(), b"existing owner");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&document, directory.path("alias")).unwrap();
        let mut arguments = valid.clone();
        arguments[10] = directory.path("alias").into();
        assert_eq!(invoke(&arguments, &source).0, 2);
        fs::hard_link(&document, directory.path("hardlink")).unwrap();
        arguments[10] = directory.path("hardlink").into();
        assert_eq!(invoke(&arguments, &source).0, 2);
    }
    assert_eq!(source.observed.get(), 0);
}

#[test]
fn http_redirect_encoding_and_truncated_transport_are_distinct_failures() {
    for (wire, phase) in [
        (
            b"HTTP/1.1 404 Missing\r\nContent-Length: 0\r\n\r\n".as_slice(),
            "http",
        ),
        (
            b"HTTP/1.1 302 Found\r\nLocation: https://example.org/\r\nContent-Length: 0\r\n\r\n",
            "http",
        ),
        (
            b"HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: 3\r\n\r\nabc",
            "http",
        ),
        (
            b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\nabc",
            "transport",
        ),
        (
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n",
            "transport",
        ),
    ] {
        let directory = Temp::new();
        let document = directory.path("output");
        let mut fixture = Fixture::wire(wire.to_vec());
        let (code, stdout, stderr) = invoke(&args(&document, ABC), &fixture.source);
        fixture.finish();
        assert_eq!((code, stderr), (1, String::new()));
        assert!(!document.exists());
        let receipt: Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(receipt["result"]["failures"][0]["phase"], phase);
        assert_eq!(receipt["result"]["actual_sha256"], Value::Null);
        assert_eq!(receipt["result"]["byte_count"], Value::Null);
        assert_eq!(fixture.source.observed.get(), 1);
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
    }
}

#[test]
fn complete_digest_mismatch_retains_actual_identity_without_document_output() {
    let directory = Temp::new();
    let document = directory.path("output");
    let mut fixture = Fixture::body(b"abc");
    let (code, stdout, _) = invoke(&args(&document, EMPTY), &fixture.source);
    fixture.finish();
    assert_eq!(code, 1);
    assert!(!document.exists());
    let receipt: Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(receipt["result"]["actual_sha256"], ABC);
    assert_eq!(receipt["result"]["byte_count"], 3);
    assert_eq!(receipt["result"]["failures"][0]["phase"], "digest");
}

#[test]
fn bodies_without_lengths_and_chunked_bodies_preserve_complete_bytes() {
    for wire in [
        b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\nabc".as_slice(),
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1\r\na\r\n2\r\nbc\r\n0\r\n\r\n",
    ] {
        let directory = Temp::new();
        let document = directory.path("output");
        let mut fixture = Fixture::wire(wire.to_vec());
        assert_eq!(invoke(&args(&document, ABC), &fixture.source).0, 0);
        fixture.finish();
        assert_eq!(fs::read(&document).unwrap(), b"abc");
    }
}

#[test]
fn exactly_four_mebibytes_are_admitted_and_streamed_overflow_is_refused() {
    // Digests independently recorded with the system sha256sum utility.
    for (size, digest, length, expected) in [
        (
            BODY_BYTES,
            "299285fc41a44cdb038b9fdaf494c76ca9d0c866672b2b266c1a0c17dda60a05",
            true,
            0,
        ),
        (
            BODY_BYTES + 1,
            "acd560a1e1d523c090ab93aed616d154b7b5e8206a153cced729d83f2c7dcfc3",
            true,
            1,
        ),
        (
            BODY_BYTES + 1,
            "acd560a1e1d523c090ab93aed616d154b7b5e8206a153cced729d83f2c7dcfc3",
            false,
            1,
        ),
    ] {
        let directory = Temp::new();
        let document = directory.path("output");
        let mut wire = if length {
            format!("HTTP/1.1 200 OK\r\nContent-Length: {size}\r\n\r\n").into_bytes()
        } else {
            b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".to_vec()
        };
        wire.extend(std::iter::repeat_n(b'a', size));
        let mut fixture = Fixture::wire(wire);
        let (code, stdout, _) = invoke(&args(&document, digest), &fixture.source);
        fixture.finish();
        assert_eq!(code, expected);
        let receipt: Value = serde_json::from_slice(&stdout).unwrap();
        if expected == 0 {
            assert_eq!(fs::read(&document).unwrap(), vec![b'a'; BODY_BYTES]);
        } else {
            assert!(!document.exists());
            assert_eq!(receipt["result"]["failures"][0]["phase"], "limits");
            assert_eq!(receipt["result"]["actual_sha256"], Value::Null);
        }
    }
}

#[test]
fn header_work_is_bounded_for_one_long_header_and_many_headers() {
    for single in [true, false] {
        let directory = Temp::new();
        let document = directory.path("output");
        let mut wire = b"HTTP/1.1 200 OK\r\n".to_vec();
        if single {
            wire.extend_from_slice(b"X-Long: ");
            wire.extend(std::iter::repeat_n(b'x', HEADER_BYTES));
            wire.extend_from_slice(b"\r\n");
        } else {
            for _ in 0..70 {
                wire.extend_from_slice(b"X-Part: ");
                wire.extend(std::iter::repeat_n(b'x', 1000));
                wire.extend_from_slice(b"\r\n");
            }
        }
        wire.extend_from_slice(b"Content-Length: 3\r\n\r\nabc");
        let mut fixture = Fixture::wire(wire);
        let (code, stdout, _) = invoke(&args(&document, ABC), &fixture.source);
        fixture.finish();
        assert_eq!(code, 1);
        assert!(!document.exists());
        assert_eq!(
            serde_json::from_slice::<Value>(&stdout).unwrap()["result"]["failures"][0]["phase"],
            "limits"
        );
    }
}

#[test]
fn connection_handshake_and_whole_transfer_deadlines_stop_real_transport() {
    for tls in [true, false] {
        let directory = Temp::new();
        let document = directory.path("output");
        let parts = if tls {
            vec![(Duration::from_millis(250), Vec::new())]
        } else {
            vec![
                (
                    Duration::ZERO,
                    b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\na".to_vec(),
                ),
                (Duration::from_millis(250), b"bc".to_vec()),
            ]
        };
        let mut fixture = Fixture::script(parts, tls);
        fixture.source.connect = Duration::from_millis(60);
        fixture.source.whole = Duration::from_millis(if tls { 180 } else { 80 });
        let start = Instant::now();
        let (code, stdout, _) = invoke(&args(&document, ABC), &fixture.source);
        assert!(start.elapsed() < Duration::from_millis(220));
        fixture.finish();
        assert_eq!(code, 1);
        assert!(!document.exists());
        let receipt: Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(receipt["result"]["failures"][0]["phase"], "limits");
        assert_eq!(receipt["result"]["actual_sha256"], Value::Null);
    }
}

#[test]
fn stage_commit_race_and_cleanup_failures_preserve_authoritative_outcomes() {
    for (stage, commit, cleanup, race, expected_phases, committed) in [
        (true, false, false, false, vec!["stage"], false),
        (false, true, false, false, vec!["commit"], false),
        (false, false, false, true, vec!["commit"], false),
        (false, true, true, false, vec!["commit", "cleanup"], false),
        (false, false, true, false, vec!["cleanup"], true),
    ] {
        let directory = Temp::new();
        let document = directory.path("output");
        let mut fixture = Fixture::body(b"abc");
        fixture.source.faults = transport::TestFaults {
            stage,
            commit,
            cleanup,
            create_document: race,
            ..transport::TestFaults::default()
        };
        let (code, stdout, _) = invoke(&args(&document, ABC), &fixture.source);
        fixture.finish();
        assert_eq!(code, 4);
        let receipt: Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(receipt["result"]["document_committed"], committed);
        assert_eq!(receipt["result"]["actual_sha256"], ABC);
        let phases: Vec<_> = receipt["result"]["failures"]
            .as_array()
            .unwrap()
            .iter()
            .map(|failure| failure["phase"].as_str().unwrap())
            .collect();
        assert_eq!(phases, expected_phases);
        if committed {
            assert_eq!(fs::read(&document).unwrap(), b"abc");
        } else if race {
            assert_eq!(fs::read(&document).unwrap(), b"concurrent owner");
        } else {
            assert!(!document.exists());
        }
        if cleanup {
            assert!(receipt["staging_output"]["native_bytes"].is_array());
            assert_eq!(
                fs::read_dir(&directory.0).unwrap().count(),
                1 + usize::from(committed)
            );
        } else {
            assert_eq!(receipt["staging_output"], Value::Null);
            assert_eq!(
                fs::read_dir(&directory.0).unwrap().count(),
                usize::from(race)
            );
        }
    }
}

#[test]
fn failed_staging_write_retains_its_original_failure_before_cleanup_failure() {
    let directory = Temp::new();
    let document = directory.path("output");
    let mut fixture = Fixture::body(b"abc");
    fixture.source.faults.stage = true;
    fixture.source.faults.cleanup = true;
    let (code, stdout, _) = invoke(&args(&document, ABC), &fixture.source);
    fixture.finish();
    assert_eq!(code, 4);
    let receipt: Value = serde_json::from_slice(&stdout).unwrap();
    let phases: Vec<_> = receipt["result"]["failures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|failure| failure["phase"].as_str().unwrap())
        .collect();
    assert_eq!(phases, ["stage", "cleanup"]);
    assert!(receipt["staging_output"]["native_bytes"].is_array());
    assert!(!document.exists());
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

struct FailedStdout;
impl Write for FailedStdout {
    fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed stdout"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn stdout_failure_discloses_that_verified_document_remains_committed() {
    let directory = Temp::new();
    let document = directory.path("output");
    let mut fixture = Fixture::body(b"abc");
    let (code, message) =
        execute(&args(&document, ABC), &fixture.source, &mut FailedStdout).unwrap_err();
    fixture.finish();
    assert_eq!(code, 4);
    assert!(message.contains("Verified document exists"));
    assert!(message.contains(&document.to_string_lossy().into_owned()));
    assert_eq!(fs::read(&document).unwrap(), b"abc");
}

#[test]
fn human_stdout_retains_the_same_authoritative_outcome() {
    let directory = Temp::new();
    let document = directory.path("output");
    let mut arguments = args(&document, ABC);
    arguments.truncate(11);
    let mut fixture = Fixture::body(b"abc");
    let (code, stdout, _) = invoke(&arguments, &fixture.source);
    fixture.finish();
    assert_eq!(code, 0);
    let stdout = String::from_utf8(stdout).unwrap();
    for expected in [
        "Acquisition acquired",
        "Complete true",
        "Document committed true",
        "Bytes 3",
        ABC,
    ] {
        assert!(stdout.contains(expected));
    }
}

#[cfg(unix)]
pub(super) fn native_destination_rejection(path: &std::path::Path) -> Option<io::Error> {
    // Unix paths retain arbitrary bytes, but the filesystem need not accept
    // every name. APFS, for example, requires valid UTF-8 when creating files.
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(file) => {
            drop(file);
            fs::remove_file(path).unwrap();
            None
        }
        Err(error) => {
            assert!(
                path.as_os_str().to_str().is_none(),
                "valid native destination was rejected: {error}"
            );
            assert!(!path.exists());
            Some(error)
        }
    }
}

#[cfg(unix)]
#[test]
fn native_document_destinations_match_filesystem_support_and_retain_identity() {
    use std::os::unix::ffi::OsStringExt;
    for name in [
        OsString::from("native-温度.json"),
        OsString::from_vec(b"native-\xff.json".to_vec()),
    ] {
        let directory = Temp::new();
        let document = directory.path(name);
        let rejection = native_destination_rejection(&document);
        let supported = rejection.is_none();
        let mut fixture = Fixture::body(b"abc");
        let (code, stdout, stderr) = invoke(&args(&document, ABC), &fixture.source);
        fixture.finish();
        assert_eq!(
            code,
            if supported { 0 } else { 4 },
            "filesystem rejection: {rejection:?}; receipt: {}; {stderr}",
            String::from_utf8_lossy(&stdout)
        );
        assert!(stderr.is_empty(), "{stderr}");
        let receipt: Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(
            receipt["document_output"]["native_bytes"],
            json!(document.as_os_str().as_encoded_bytes())
        );
        assert_eq!(receipt["result"]["document_committed"], supported);
        if supported {
            assert_eq!(fs::read(&document).unwrap(), b"abc");
        } else {
            assert!(!document.exists());
            assert_eq!(receipt["result"]["failures"][0]["phase"], "commit");
        }
        if document.to_str().is_none() {
            assert!(!std::path::Path::new(&document.to_string_lossy().into_owned()).exists());
        }
    }
}
