//! Verified snapshots replay through the ordinary supplied-Catalogue boundary.
//! Follows ADR0004: Supplied catalogues and checking scope, ADR0006: Scoped
//! reference failures, and ADR0035: Explicit pinned document acquisition.

use super::support::{EMPTY, Fixture, MALFORMED, Temp, args, invoke};
use esdieff::{Acquisition, Catalogue, CheckRequest, Disposition, SourceInput};
use serde_json::{Value, json};
use std::{fs, path::Path};

const PINNED: &[u8] = include_bytes!("../../examples/pinned-acquisition/original.sdf.json");
const PINNED_SHA256: &str = "6fe5a1538da0b5f48898e367f0f03b6b70fa38b6ebaa67117e408ab87fb0b61a";
const PINNED_COMMIT: &str = "41be1e0e564ad39f4d5a1c41fcccef435e388ee9";
const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
const REFRESH_COMMIT: &str = "1111111111111111111111111111111111111111";

// Expected digests were independently recorded with sha256sum before transport.
const UNSUPPLIED: &[u8] = br##"{"info":{},"namespace":{"own":"urn:example:acquired","remote":"https://example.org/missing"},"defaultNamespace":"own","sdfData":{"Reading":{"sdfRef":"remote:#/sdfData/Reading"}}}"##;
const UNSUPPLIED_SHA256: &str = "461c93f2a8b4bdcd03b3bd3d843dd4ae6556add5f74da5776a05082dde164bf5";
const ORIGINAL: &[u8] = br##"{"info":{},"namespace":{"d":"urn:example:ambiguous"},"defaultNamespace":"d","sdfData":{"Value":{"type":"integer","maximum":10}}}"##;
const ORIGINAL_SHA256: &str = "d12eac37c8b8a600d9e61c0f01aba3e93247fa418490c0a2685be52f65fe5d60";
const REFRESHED: &[u8] = br##"{"info":{},"namespace":{"d":"urn:example:ambiguous"},"defaultNamespace":"d","sdfData":{"Value":{"type":"integer","maximum":9}}}"##;
const REFRESHED_SHA256: &str = "edae86ddd1a759bd4854d8efb51b6113fe203353668e155c082d5a0fcd02868b";
const CONSUMER: &[u8] = br##"{"info":{},"namespace":{"d":"urn:example:ambiguous"},"sdfData":{"Use":{"sdfRef":"d:#/sdfData/Value"}}}"##;

fn acquire(document: &Path, bytes: &[u8], digest: &str, commit: &str) -> Value {
    let mut arguments = args(document, digest);
    arguments[4] = commit.into();
    let mut fixture = Fixture::body(bytes);
    let (code, stdout, stderr) = invoke(&arguments, &fixture.source);
    fixture.finish();
    assert_eq!((code, stderr), (0, String::new()));
    assert_eq!(fixture.source.observed.get(), 1);
    assert_eq!(fs::read(document).unwrap(), bytes);
    let receipt: Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(
        receipt["result"],
        json!({"status":"acquired","complete":true,"document_committed":true,
            "actual_sha256":digest,"byte_count":bytes.len(),"failures":[]})
    );
    receipt
}

fn check_file(document: &Path) -> Value {
    let catalogue = Catalogue::from_inputs(&[SourceInput::file("snapshot", document)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    serde_json::from_str(&report.to_json()).unwrap()
}

#[test]
fn independently_pinned_one_dm_bytes_replay_as_an_ordinary_local_catalogue() {
    let directory = Temp::new();
    let document = directory.path("switch.sdf.json");
    let mut arguments = args(&document, PINNED_SHA256);
    arguments[2] = "one-data-model/playground".into();
    arguments[4] = PINNED_COMMIT.into();
    arguments[6] = "sdfObject/sdfobject-switch_binary.sdf.json".into();
    let mut fixture = Fixture::body(PINNED);
    let (code, stdout, stderr) = invoke(&arguments, &fixture.source);
    fixture.finish();
    assert_eq!((code, stderr), (0, String::new()));
    assert_eq!(fixture.source.observed.get(), 1);
    let receipt: Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(receipt["schema"], "esdieff.acquisition");
    assert_eq!(receipt["version"], "1.0");
    assert_eq!(
        receipt["source"],
        json!({"repository":"one-data-model/playground","commit":PINNED_COMMIT,
            "path":"sdfObject/sdfobject-switch_binary.sdf.json",
            "uri":"https://raw.githubusercontent.com/one-data-model/playground/41be1e0e564ad39f4d5a1c41fcccef435e388ee9/sdfObject/sdfobject-switch_binary.sdf.json",
            "expected_sha256":PINNED_SHA256})
    );
    assert_eq!(
        receipt["result"],
        json!({"status":"acquired","complete":true,"document_committed":true,
            "actual_sha256":PINNED_SHA256,"byte_count":787,"failures":[]})
    );
    assert_eq!(fs::read(&document).unwrap(), PINNED);
    drop(fixture); // The controlled response source is closed before local replay.

    let catalogue = Catalogue::from_inputs(&[SourceInput::file("switch", &document)]).unwrap();
    let source = catalogue.sources().next().unwrap();
    assert_eq!(source.acquisition(), Acquisition::Acquired);
    assert_eq!(source.raw_bytes(), PINNED);
    assert_eq!(
        source.raw_path_identity(),
        Some(document.as_os_str().as_encoded_bytes())
    );
    assert!(source.acquisition_uri().is_none());
    assert!(source.version_token().is_none());
    assert!(source.digest().is_none());
    let first = catalogue.check(CheckRequest::new()).unwrap().to_json();
    let second = catalogue.check(CheckRequest::new()).unwrap().to_json();
    assert_eq!(first, second);
    let report: Value = serde_json::from_str(&first).unwrap();
    assert_eq!(report["request"]["scope"]["kind"], "catalogue");
    assert_eq!(report["request"]["scope"]["roots"], json!([]));
    assert_eq!(report["sources"][0]["acquisition"], "acquired");
    assert_eq!(fs::read(&document).unwrap(), PINNED);
}

#[test]
fn digest_matching_malformed_and_empty_snapshots_fail_only_during_ordinary_checking() {
    for (bytes, digest) in [(b"{".as_slice(), MALFORMED), (b"".as_slice(), EMPTY)] {
        let directory = Temp::new();
        let document = directory.path("malformed.json");
        acquire(&document, bytes, digest, COMMIT);
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::file("snapshot", &document)]).unwrap();
        let source = catalogue.sources().next().unwrap();
        assert_eq!(source.acquisition(), Acquisition::Acquired);
        assert!(source.content_complete());
        assert_eq!(source.raw_bytes(), bytes);
        assert!(source.json().present().is_none());
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().disposition(), Disposition::Failed);
        assert!(!report.outcome().complete());
        assert_eq!(report.outcome().errors(), 1);
        assert!(
            report
                .diagnostics()
                .conditions()
                .any(|condition| condition.code() == "json.syntax")
        );
    }
}

#[test]
fn namespace_and_unsupplied_references_trigger_no_retrieval_or_implied_root() {
    let directory = Temp::new();
    let document = directory.path("dependent.json");
    let mut fixture = Fixture::body(UNSUPPLIED);
    let (code, stdout, stderr) = invoke(&args(&document, UNSUPPLIED_SHA256), &fixture.source);
    fixture.finish();
    assert_eq!((code, stderr), (0, String::new()));
    assert_eq!(fixture.source.observed.get(), 1);
    let receipt: Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(receipt["result"]["actual_sha256"], UNSUPPLIED_SHA256);
    assert_ne!(receipt["source"]["uri"], "urn:example:acquired");
    assert_ne!(receipt["source"]["uri"], "https://example.org/missing");
    let catalogue = Catalogue::from_inputs(&[SourceInput::file("dependent", &document)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 0);
    assert_eq!(report.outcome().disposition(), Disposition::Undetermined);
    assert!(!report.outcome().complete());
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["request"]["scope"]["kind"], "catalogue");
    assert_eq!(wire["request"]["scope"]["roots"], json!([]));
    assert_eq!(wire["sources"].as_array().unwrap().len(), 1);
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "reference.unavailable")
    );
    assert_eq!(fixture.source.observed.get(), 1);
    assert_eq!(fs::read(&document).unwrap(), UNSUPPLIED);
}

#[test]
fn repeated_and_overlapping_acquired_contributors_preserve_ambiguity_in_both_orders() {
    let directory = Temp::new();
    let first = directory.path("first.json");
    let overlap = directory.path("overlap.json");
    acquire(&first, ORIGINAL, ORIGINAL_SHA256, COMMIT);
    acquire(&overlap, REFRESHED, REFRESHED_SHA256, REFRESH_COMMIT);
    for second in [&first, &overlap] {
        for reverse in [false, true] {
            let mut inputs = [
                SourceInput::file("first", &first),
                SourceInput::file("second", second),
                SourceInput::supplied("consumer", CONSUMER),
            ];
            if reverse {
                inputs.reverse();
            }
            let catalogue = Catalogue::from_inputs(&inputs).unwrap();
            assert_eq!(catalogue.sources().len(), 3);
            let report = catalogue.check(CheckRequest::new()).unwrap();
            assert_eq!(report.outcome().errors(), 0);
            assert_eq!(report.outcome().disposition(), Disposition::Undetermined);
            assert!(!report.outcome().complete());
            let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
            let ambiguous = wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|condition| condition["code"] == "reference.ambiguous")
                .unwrap();
            let related = ambiguous["related"].as_array().unwrap();
            for key in ["first", "second"] {
                let id = catalogue.source_id(key).unwrap().get();
                assert!(related.iter().any(|location| location["source_id"] == id
                    && location["pointer"] == "/sdfData/Value"));
            }
        }
    }
}

#[test]
fn deliberate_refresh_changes_pins_and_destination_while_old_replay_is_identical() {
    let directory = Temp::new();
    let original = directory.path("original.json");
    let refreshed = directory.path("refreshed.json");
    let first = acquire(&original, ORIGINAL, ORIGINAL_SHA256, COMMIT);
    let before = check_file(&original);
    let second = acquire(&refreshed, REFRESHED, REFRESHED_SHA256, REFRESH_COMMIT);
    assert_eq!(first["source"]["commit"], COMMIT);
    assert_eq!(second["source"]["commit"], REFRESH_COMMIT);
    assert_eq!(first["source"]["expected_sha256"], ORIGINAL_SHA256);
    assert_eq!(second["source"]["expected_sha256"], REFRESHED_SHA256);
    assert_ne!(first["document_output"], second["document_output"]);
    assert_eq!(fs::read(&original).unwrap(), ORIGINAL);
    assert_eq!(fs::read(&refreshed).unwrap(), REFRESHED);
    assert_eq!(before, check_file(&original));
}
