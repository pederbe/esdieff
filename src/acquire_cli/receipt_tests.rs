use super::support::*;
use super::*;
use serde_json::{Value, json};
use std::{fs, path::Path};

fn named_args(document: &Path, report: &Path, digest: &str) -> Vec<OsString> {
    let mut arguments = args(document, digest);
    arguments.extend(["--report-output".into(), report.as_os_str().to_owned()]);
    arguments
}

#[test]
fn separate_named_json_receipt_preserves_verified_bytes_and_outcome() {
    let directory = Temp::new();
    let document = directory.path("snapshot.json");
    let report = directory.path("receipt.json");
    let mut fixture = Fixture::body(b"abc");
    let (code, stdout, stderr) = invoke(&named_args(&document, &report, ABC), &fixture.source);
    fixture.finish();
    assert_eq!((code, stderr), (0, String::new()));
    assert!(stdout.is_empty());
    assert_eq!(fs::read(&document).unwrap(), b"abc");
    let bytes = fs::read(&report).unwrap();
    assert!(bytes.len() <= RECEIPT_BYTES && bytes.ends_with(b"\n"));
    let receipt: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        receipt["result"],
        json!({"status":"acquired","complete":true,"document_committed":true,"actual_sha256":ABC,"byte_count":3,"failures":[]})
    );
    assert_eq!(
        receipt["document_output"]["native_bytes"],
        json!(document.as_os_str().as_encoded_bytes())
    );
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
}

#[cfg(unix)]
#[test]
fn ancestor_swap_after_final_receipt_check_cannot_redirect_the_write() {
    let directory = Temp::new();
    let document = directory.path("snapshot.json");
    let parent = directory.path("receipts");
    let retained = directory.path("original-parent");
    let redirected = directory.path("unrelated");
    fs::create_dir(&parent).unwrap();
    fs::create_dir(&redirected).unwrap();
    let report = parent.join("receipt.json");
    let mut fixture = Fixture::body(b"abc");
    let swap_parent = parent.clone();
    let swap_retained = retained.clone();
    let swap_redirected = redirected.clone();
    fixture.source.faults.after_receipt_check = Some(Box::new(move || {
        fs::rename(&swap_parent, &swap_retained).unwrap();
        std::os::unix::fs::symlink(&swap_redirected, &swap_parent).unwrap();
    }));
    let (code, stdout, stderr) = invoke(&named_args(&document, &report, ABC), &fixture.source);
    fixture.finish();
    assert!(
        !redirected.join("receipt.json").exists(),
        "receipt write was redirected through the new ancestor alias"
    );
    assert_eq!(code, 4);
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("receipt output failed") && stderr.contains("Verified document exists")
    );
    assert_eq!(fs::read(&document).unwrap(), b"abc");
}

#[test]
fn partial_named_receipt_failure_reports_actual_document_state_after_success_or_failure() {
    for (digest, committed) in [(ABC, true), (EMPTY, false)] {
        let directory = Temp::new();
        let document = directory.path("snapshot.json");
        let report = directory.path("receipt.json");
        let mut fixture = Fixture::body(b"abc");
        fixture.source.faults.receipt_write = true;
        let (code, stdout, stderr) =
            invoke(&named_args(&document, &report, digest), &fixture.source);
        fixture.finish();
        assert_eq!(code, 4);
        assert!(stdout.is_empty());
        assert!(
            stderr.contains("receipt output failed")
                && stderr.contains(if committed {
                    "Verified document exists"
                } else {
                    "Verified document was not committed"
                })
        );
        assert_eq!(document.exists(), committed);
        if committed {
            assert_eq!(fs::read(&document).unwrap(), b"abc");
        }
        let partial = fs::read(&report).unwrap();
        assert!(!partial.is_empty() && serde_json::from_slice::<Value>(&partial).is_err());
    }
}

#[test]
fn named_receipt_preflight_rejects_existing_colliding_and_invalid_destinations_without_requests() {
    let directory = Temp::new();
    let document = directory.path("snapshot.json");
    let report = directory.path("receipt.json");
    let source = ResponseSource {
        target: Some("http://127.0.0.1:9/".into()),
        ..ResponseSource::default()
    };
    fs::write(&report, b"existing receipt").unwrap();
    let mut rejected = vec![
        named_args(&document, &report, ABC),
        named_args(&document, &document, ABC),
        named_args(&document, &directory.path("missing/receipt.json"), ABC),
    ];
    let mut duplicate = named_args(&document, &directory.path("new.json"), ABC);
    duplicate.extend([
        "--report-output".into(),
        directory.path("another.json").into(),
    ]);
    rejected.push(duplicate);
    let mut absent_value = args(&document, ABC);
    absent_value.push("--report-output".into());
    rejected.push(absent_value);
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&directory.0, directory.path("alias-parent")).unwrap();
        rejected.push(named_args(
            &document,
            &directory.path("alias-parent/snapshot.json"),
            ABC,
        ));
        std::os::unix::fs::symlink(
            directory.path("not-created"),
            directory.path("dangling-receipt"),
        )
        .unwrap();
        rejected.push(named_args(
            &document,
            &directory.path("dangling-receipt"),
            ABC,
        ));
        fs::hard_link(&report, directory.path("receipt-hard-link")).unwrap();
        rejected.push(named_args(
            &document,
            &directory.path("receipt-hard-link"),
            ABC,
        ));
    }
    for arguments in rejected {
        let (code, stdout, stderr) = invoke(&arguments, &source);
        assert_eq!(code, 2, "{arguments:?}: {stderr}");
        assert!(!stderr.is_empty() && stdout.is_empty());
        assert_eq!(source.observed.get(), 0);
        assert!(!document.exists());
        assert_eq!(fs::read(&report).unwrap(), b"existing receipt");
    }
}

#[test]
fn named_human_and_json_receipts_express_success_and_content_failure() {
    for human in [false, true] {
        for (digest, success) in [(ABC, true), (EMPTY, false)] {
            let directory = Temp::new();
            let document = directory.path("snapshot.json");
            let report = directory.path("receipt");
            let mut arguments = named_args(&document, &report, digest);
            if human {
                arguments[12] = "human".into();
            }
            let mut fixture = Fixture::body(b"abc");
            let (code, stdout, stderr) = invoke(&arguments, &fixture.source);
            fixture.finish();
            assert_eq!(code, u8::from(!success));
            assert!(stdout.is_empty() && stderr.is_empty());
            let bytes = fs::read(&report).unwrap();
            assert!(bytes.len() <= RECEIPT_BYTES);
            if human {
                let text = String::from_utf8(bytes).unwrap();
                for expected in [
                    format!(
                        "Acquisition {}",
                        if success { "acquired" } else { "failed" }
                    ),
                    format!("Complete {success}"),
                    format!("Document committed {success}"),
                    format!("Actual SHA-256 {ABC}"),
                    "Bytes 3".into(),
                ] {
                    assert!(text.contains(&expected));
                }
                assert_eq!(text.contains("digest:"), !success);
            } else {
                let receipt: Value = serde_json::from_slice(&bytes).unwrap();
                assert_eq!(receipt["result"]["complete"], success);
                assert_eq!(receipt["result"]["document_committed"], success);
                assert_eq!(receipt["result"]["actual_sha256"], ABC);
                assert_eq!(receipt["result"]["byte_count"], 3);
                assert_eq!(
                    receipt["result"]["failures"].as_array().unwrap().len(),
                    usize::from(!success)
                );
                if !success {
                    assert_eq!(receipt["result"]["failures"][0]["phase"], "digest");
                }
            }
            assert_eq!(document.exists(), success);
        }
    }
}

#[test]
fn named_receipts_record_document_stage_commit_race_and_cleanup_outcomes_in_order() {
    for (stage, commit, cleanup, race, phases, committed) in [
        (true, false, false, false, vec!["stage"], false),
        (false, true, false, false, vec!["commit"], false),
        (false, false, false, true, vec!["commit"], false),
        (true, false, true, false, vec!["stage", "cleanup"], false),
        (false, true, true, false, vec!["commit", "cleanup"], false),
        (false, false, true, false, vec!["cleanup"], true),
    ] {
        let directory = Temp::new();
        let document = directory.path("snapshot.json");
        let report = directory.path("receipt.json");
        let mut fixture = Fixture::body(b"abc");
        fixture.source.faults = transport::TestFaults {
            stage,
            commit,
            cleanup,
            create_document: race,
            ..transport::TestFaults::default()
        };
        let (code, stdout, stderr) = invoke(&named_args(&document, &report, ABC), &fixture.source);
        fixture.finish();
        assert_eq!(code, 4);
        assert!(stdout.is_empty() && stderr.is_empty());
        let receipt: Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
        assert_eq!(receipt["result"]["document_committed"], committed);
        assert_eq!(receipt["result"]["actual_sha256"], ABC);
        assert_eq!(receipt["result"]["complete"], false);
        let actual_phases: Vec<_> = receipt["result"]["failures"]
            .as_array()
            .unwrap()
            .iter()
            .map(|failure| failure["phase"].as_str().unwrap())
            .collect();
        assert_eq!(actual_phases, phases);
        if cleanup {
            let native: Vec<u8> = receipt["staging_output"]["native_bytes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap() as u8)
                .collect();
            #[cfg(unix)]
            {
                use std::os::unix::ffi::OsStringExt;
                assert!(Path::new(&OsString::from_vec(native)).is_file());
            }
            #[cfg(not(unix))]
            let _ = native;
        } else {
            assert_eq!(receipt["staging_output"], Value::Null);
        }
        if committed {
            assert_eq!(fs::read(&document).unwrap(), b"abc");
        } else if race {
            assert_eq!(fs::read(&document).unwrap(), b"concurrent owner");
        } else {
            assert!(!document.exists());
        }
    }
}

#[test]
fn concurrent_receipt_creators_survive_both_before_delivery_and_after_final_recheck() {
    for during_fetch in [false, true] {
        let directory = Temp::new();
        let document = directory.path("snapshot.json");
        let report = directory.path("receipt.json");
        let mut fixture = Fixture::body(b"abc");
        let competing = report.clone();
        let change: Box<dyn Fn()> = Box::new(move || {
            fs::write(&competing, b"independent receipt").unwrap();
        });
        if during_fetch {
            fixture.source.faults.after_fetch = Some(change);
        } else {
            fixture.source.faults.after_receipt_check = Some(change);
        }
        let (code, stdout, stderr) = invoke(&named_args(&document, &report, ABC), &fixture.source);
        fixture.finish();
        assert_eq!(code, 4);
        assert!(
            stdout.is_empty()
                && stderr.contains("receipt output failed")
                && stderr.contains("Verified document exists")
        );
        assert_eq!(fs::read(&document).unwrap(), b"abc");
        assert_eq!(fs::read(&report).unwrap(), b"independent receipt");
    }
}

#[test]
fn already_absent_document_staging_is_not_reported_as_retained_cleanup_failure() {
    let directory = Temp::new();
    let document = directory.path("snapshot.json");
    let report = directory.path("receipt.json");
    let mut fixture = Fixture::body(b"abc");
    fixture.source.faults.staging_disappears = true;
    let (code, stdout, stderr) = invoke(&named_args(&document, &report, ABC), &fixture.source);
    fixture.finish();
    assert_eq!(code, 0);
    assert!(stdout.is_empty() && stderr.is_empty());
    let receipt: Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
    assert_eq!(receipt["staging_output"], Value::Null);
    assert_eq!(receipt["result"]["failures"], json!([]));
}

#[cfg(unix)]
#[test]
fn separate_native_receipt_and_document_paths_never_use_lossy_labels_for_io() {
    use std::os::unix::ffi::OsStringExt;
    let native_document = OsString::from_vec(b"snapshot-\xff.json".to_vec());
    let native_report = OsString::from_vec(b"receipt-\xfe.json".to_vec());
    for (document_name, report_name) in [
        ("snapshot-温度.json".into(), "receipt-測定.json".into()),
        (native_document.clone(), "receipt.json".into()),
        ("snapshot.json".into(), native_report.clone()),
        (native_document, native_report),
    ] {
        let directory = Temp::new();
        let document = directory.path(document_name);
        let report = directory.path(report_name);
        let document_rejection = super::tests::native_destination_rejection(&document);
        let report_rejection = super::tests::native_destination_rejection(&report);
        let document_supported = document_rejection.is_none();
        let report_supported = report_rejection.is_none();
        let mut fixture = Fixture::body(b"abc");
        let (code, stdout, stderr) = invoke(&named_args(&document, &report, ABC), &fixture.source);
        fixture.finish();
        assert_eq!(
            code,
            if document_supported && report_supported {
                0
            } else {
                4
            },
            "filesystem rejections: document={document_rejection:?}, receipt={report_rejection:?}; stdout: {}; {stderr}",
            String::from_utf8_lossy(&stdout)
        );
        assert!(stdout.is_empty());
        if document_supported {
            assert_eq!(fs::read(&document).unwrap(), b"abc");
        } else {
            assert!(!document.exists());
        }
        if report_supported {
            assert!(stderr.is_empty(), "{stderr}");
            let receipt: Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
            assert_eq!(
                receipt["document_output"]["native_bytes"],
                json!(document.as_os_str().as_encoded_bytes())
            );
            assert_eq!(receipt["result"]["document_committed"], document_supported);
        } else {
            assert!(!report.exists());
            assert!(stderr.contains("receipt output failed"), "{stderr}");
        }
        for path in [&document, &report] {
            if path.to_str().is_none() {
                assert!(!Path::new(&path.to_string_lossy().into_owned()).exists());
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn accepted_parent_aliases_preserve_separate_native_outputs() {
    let directory = Temp::new();
    let parent = directory.path("actual");
    let alias = directory.path("alias");
    fs::create_dir(&parent).unwrap();
    std::os::unix::fs::symlink(&parent, &alias).unwrap();
    let document = alias.join("snapshot.json");
    let report = parent.join("receipt.json");
    let mut fixture = Fixture::body(b"abc");
    let (code, stdout, stderr) = invoke(&named_args(&document, &report, ABC), &fixture.source);
    fixture.finish();
    assert_eq!(code, 0);
    assert!(stdout.is_empty() && stderr.is_empty());
    assert_eq!(fs::read(parent.join("snapshot.json")).unwrap(), b"abc");
    let receipt: Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
    assert_eq!(
        receipt["document_output"]["native_bytes"],
        json!(document.as_os_str().as_encoded_bytes())
    );
}

#[cfg(unix)]
#[test]
fn output_parent_changes_during_retrieval_preserve_each_independent_destination() {
    for changed_document in [false, true] {
        let directory = Temp::new();
        let parent = directory.path("changed");
        let retained = directory.path("retained");
        let unrelated = directory.path("unrelated");
        fs::create_dir(&parent).unwrap();
        fs::create_dir(&unrelated).unwrap();
        let document = if changed_document {
            parent.join("snapshot.json")
        } else {
            directory.path("snapshot.json")
        };
        let report = if changed_document {
            directory.path("receipt.json")
        } else {
            parent.join("receipt.json")
        };
        let competing = unrelated.join(if changed_document {
            "snapshot.json"
        } else {
            "receipt.json"
        });
        fs::write(&competing, b"independent owner").unwrap();
        let mut fixture = Fixture::body(b"abc");
        fixture.source.faults.after_fetch = Some(Box::new(move || {
            fs::rename(&parent, &retained).unwrap();
            std::os::unix::fs::symlink(&unrelated, &parent).unwrap();
        }));
        let (code, stdout, stderr) = invoke(&named_args(&document, &report, ABC), &fixture.source);
        fixture.finish();
        assert_eq!(code, 4);
        assert!(stdout.is_empty());
        assert_eq!(fs::read(&competing).unwrap(), b"independent owner");
        if changed_document {
            assert!(stderr.is_empty());
            let receipt: Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
            assert_eq!(receipt["result"]["document_committed"], false);
            assert_eq!(receipt["result"]["failures"][0]["phase"], "commit");
        } else {
            assert!(
                stderr.contains("receipt output failed")
                    && stderr.contains("Verified document exists")
            );
            assert_eq!(fs::read(&document).unwrap(), b"abc");
        }
    }
}

#[cfg(unix)]
#[test]
fn document_parent_swap_after_final_check_records_verified_commit_without_redirecting_bytes() {
    let directory = Temp::new();
    let parent = directory.path("documents");
    let retained = directory.path("retained");
    let unrelated = directory.path("unrelated");
    fs::create_dir(&parent).unwrap();
    fs::create_dir(&unrelated).unwrap();
    let document = parent.join("snapshot.json");
    let report = directory.path("receipt.json");
    let mut fixture = Fixture::body(b"abc");
    let saved = retained.clone();
    let redirected = unrelated.clone();
    fixture.source.faults.after_document_check = Some(Box::new(move || {
        fs::rename(&parent, &saved).unwrap();
        std::os::unix::fs::symlink(&redirected, &parent).unwrap();
    }));
    let (code, stdout, stderr) = invoke(&named_args(&document, &report, ABC), &fixture.source);
    fixture.finish();
    assert_eq!(code, 4);
    assert!(stdout.is_empty() && stderr.is_empty());
    assert!(!unrelated.join("snapshot.json").exists());
    assert_eq!(fs::read(retained.join("snapshot.json")).unwrap(), b"abc");
    assert_eq!(fs::read_dir(&retained).unwrap().count(), 1);
    let receipt: Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
    assert_eq!(receipt["result"]["document_committed"], true);
    assert_eq!(receipt["result"]["complete"], false);
    assert_eq!(receipt["result"]["status"], "failed");
    assert_eq!(receipt["result"]["failures"].as_array().unwrap().len(), 1);
    assert_eq!(receipt["result"]["failures"][0]["phase"], "commit");
    assert!(
        receipt["result"]["failures"][0]["message"]
            .as_str()
            .unwrap()
            .contains("committed")
    );
}

#[cfg(unix)]
#[test]
fn receipt_failure_after_a_moved_verified_commit_reports_unconfirmed_path_availability() {
    let directory = Temp::new();
    let parent = directory.path("documents");
    let retained = directory.path("retained");
    let unrelated = directory.path("unrelated");
    fs::create_dir(&parent).unwrap();
    fs::create_dir(&unrelated).unwrap();
    let document = parent.join("snapshot.json");
    let report = directory.path("receipt.json");
    let mut fixture = Fixture::body(b"abc");
    let saved = retained.clone();
    let redirected = unrelated.clone();
    fixture.source.faults.after_document_check = Some(Box::new(move || {
        fs::rename(&parent, &saved).unwrap();
        std::os::unix::fs::symlink(&redirected, &parent).unwrap();
    }));
    fixture.source.faults.receipt_write = true;
    let (code, stdout, stderr) = invoke(&named_args(&document, &report, ABC), &fixture.source);
    fixture.finish();
    assert_eq!(code, 4);
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("receipt output failed")
            && stderr.contains("was committed in the preflight directory")
            && stderr.contains("availability is unconfirmed")
    );
    assert!(!stderr.contains("was not committed"));
    assert!(!unrelated.join("snapshot.json").exists());
    assert_eq!(fs::read(retained.join("snapshot.json")).unwrap(), b"abc");
    assert!(serde_json::from_slice::<Value>(&fs::read(&report).unwrap()).is_err());
}

#[cfg(unix)]
#[test]
fn receipt_alias_created_after_final_check_cannot_modify_a_committed_document() {
    let directory = Temp::new();
    let document = directory.path("snapshot.json");
    let report = directory.path("receipt.json");
    let mut fixture = Fixture::body(b"abc");
    let competing = report.clone();
    let protected = document.clone();
    fixture.source.faults.after_receipt_check = Some(Box::new(move || {
        std::os::unix::fs::symlink(&protected, &competing).unwrap();
    }));
    let (code, stdout, stderr) = invoke(&named_args(&document, &report, ABC), &fixture.source);
    fixture.finish();
    assert_eq!(code, 4);
    assert!(
        stdout.is_empty()
            && stderr.contains("receipt output failed")
            && stderr.contains("Verified document exists")
    );
    assert_eq!(fs::read(&document).unwrap(), b"abc");
    assert!(
        fs::symlink_metadata(&report)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
fn named_receipt_is_attempted_for_http_and_incomplete_transport_failures() {
    for (wire, phase) in [
        (
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".as_slice(),
            "http",
        ),
        (
            b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nabc".as_slice(),
            "transport",
        ),
    ] {
        let directory = Temp::new();
        let document = directory.path("snapshot.json");
        let report = directory.path("receipt.json");
        let mut fixture = Fixture::wire(wire.to_vec());
        let (code, stdout, stderr) = invoke(&named_args(&document, &report, ABC), &fixture.source);
        fixture.finish();
        assert_eq!(code, 1);
        assert!(stdout.is_empty() && stderr.is_empty() && !document.exists());
        let receipt: Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
        assert_eq!(receipt["result"]["actual_sha256"], Value::Null);
        assert_eq!(receipt["result"]["byte_count"], Value::Null);
        assert_eq!(receipt["result"]["document_committed"], false);
        assert_eq!(receipt["result"]["failures"][0]["phase"], phase);
    }
}

#[test]
fn replaced_staging_inode_is_not_a_verified_commit_or_owned_cleanup_target() {
    let directory = Temp::new();
    let document = directory.path("snapshot.json");
    let report = directory.path("receipt.json");
    let saved = directory.path("saved-verified-stage");
    let original = std::sync::Arc::new(std::sync::Mutex::new(None));
    let observed = std::sync::Arc::clone(&original);
    let parent = directory.0.clone();
    let retained = saved.clone();
    let mut fixture = Fixture::body(b"abc");
    fixture.source.faults.after_document_check = Some(Box::new(move || {
        let stage = fs::read_dir(&parent)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(".esdieff-acquire-")
            })
            .unwrap();
        fs::rename(&stage, &retained).unwrap();
        fs::write(&stage, b"other").unwrap();
        *observed.lock().unwrap() = Some(stage);
    }));
    let (code, stdout, stderr) = invoke(&named_args(&document, &report, ABC), &fixture.source);
    fixture.finish();
    assert_eq!(code, 4);
    assert!(stdout.is_empty() && stderr.is_empty());
    let receipt: Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
    assert_eq!(receipt["result"]["document_committed"], false);
    assert_eq!(receipt["result"]["complete"], false);
    assert_eq!(receipt["result"]["actual_sha256"], ABC);
    assert_eq!(receipt["result"]["byte_count"], 3);
    let phases: Vec<_> = receipt["result"]["failures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|failure| failure["phase"].as_str().unwrap())
        .collect();
    assert_eq!(phases, ["commit", "cleanup"]);
    assert_eq!(fs::read(&saved).unwrap(), b"abc");
    let original = original.lock().unwrap();
    let original = original.as_ref().unwrap();
    assert_eq!(fs::read(original).unwrap(), b"other");
    if document.exists() {
        assert_eq!(fs::read(&document).unwrap(), b"other");
    }
    assert_eq!(
        receipt["staging_output"]["native_bytes"],
        json!(original.as_os_str().as_encoded_bytes())
    );
    let cleanup = receipt["result"]["failures"][1]["message"]
        .as_str()
        .unwrap();
    assert!(
        cleanup.contains("identity changed")
            && cleanup.contains("unconfirmed entry")
            && cleanup.contains("preserved")
    );
}

#[cfg(unix)]
#[test]
fn replaced_staging_symlinks_directories_and_fifos_are_preserved_without_stream_acquisition() {
    use std::os::unix::fs::FileTypeExt;
    for kind in ["symlink", "directory", "fifo"] {
        let directory = Temp::new();
        let document = directory.path("snapshot.json");
        let report = directory.path("receipt.json");
        let saved = directory.path("saved-verified-stage");
        let original = std::sync::Arc::new(std::sync::Mutex::new(None));
        let observed = std::sync::Arc::clone(&original);
        let parent = directory.0.clone();
        let retained = saved.clone();
        let mut fixture = Fixture::body(b"abc");
        fixture.source.faults.after_document_check = Some(Box::new(move || {
            let stage = fs::read_dir(&parent)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| {
                    path.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with(".esdieff-acquire-")
                })
                .unwrap();
            fs::rename(&stage, &retained).unwrap();
            match kind {
                "symlink" => std::os::unix::fs::symlink(&retained, &stage).unwrap(),
                "directory" => fs::create_dir(&stage).unwrap(),
                "fifo" => assert!(
                    std::process::Command::new("mkfifo")
                        .arg(&stage)
                        .status()
                        .unwrap()
                        .success()
                ),
                _ => unreachable!(),
            }
            *observed.lock().unwrap() = Some(stage);
        }));
        let started = std::time::Instant::now();
        let (code, stdout, stderr) = invoke(&named_args(&document, &report, ABC), &fixture.source);
        fixture.finish();
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "{kind} acquired an unrelated stream"
        );
        assert_eq!(code, 4);
        assert!(stdout.is_empty() && stderr.is_empty());
        let receipt: Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
        assert_eq!(receipt["result"]["document_committed"], false);
        assert_eq!(receipt["result"]["complete"], false);
        let phases: Vec<_> = receipt["result"]["failures"]
            .as_array()
            .unwrap()
            .iter()
            .map(|failure| failure["phase"].as_str().unwrap())
            .collect();
        assert_eq!(phases, ["commit", "cleanup"]);
        assert_eq!(fs::read(&saved).unwrap(), b"abc");
        let original = original.lock().unwrap();
        let original = original.as_ref().unwrap();
        let metadata = fs::symlink_metadata(original).unwrap();
        assert!(match kind {
            "symlink" => metadata.file_type().is_symlink(),
            "directory" => metadata.is_dir(),
            "fifo" => metadata.file_type().is_fifo(),
            _ => unreachable!(),
        });
        assert_eq!(
            receipt["staging_output"]["native_bytes"],
            json!(original.as_os_str().as_encoded_bytes())
        );
        assert!(
            receipt["result"]["failures"][1]["message"]
                .as_str()
                .unwrap()
                .contains("unconfirmed entry")
        );
    }
}
