//! Caller-supplied source identities describe local bytes without retrieving URIs.
//! Follows ADR0004: Supplied catalogues and checking scope.
use esdieff::{Acquisition, Catalogue, SourceInput};

const ORIGINAL: &[u8] = br#"{"namespace":{"model":"urn:example:metadata"},"defaultNamespace":"model","sdfData":{"Reading":{"type":"number"}}}"#;
const URI: &str = "file:///models/reading.sdf.json";
const VERSION: &str = "revision-a";
const DIGEST: &str = "caller-provided-content-digest";

#[test]
fn supplied_metadata_preserves_original_content_and_document_namespace() {
    let catalogue = Catalogue::take_inputs(vec![
        SourceInput::owned("reading", ORIGINAL.to_vec())
            .acquisition_uri(URI)
            .version_token(VERSION)
            .digest(DIGEST),
    ])
    .unwrap();
    let source = catalogue.sources().next().unwrap();
    assert_eq!(source.acquisition(), Acquisition::Supplied);
    assert_eq!(source.acquisition_uri(), Some(URI));
    assert_eq!(source.version_token(), Some(VERSION));
    assert_eq!(source.digest(), Some(DIGEST));
    assert_eq!(source.raw_bytes(), ORIGINAL);
    assert!(source.raw_path_identity().is_none());
    assert_eq!(
        source
            .json()
            .require()
            .unwrap()
            .get("namespace")
            .require()
            .unwrap()
            .get("model")
            .require()
            .unwrap()
            .text()
            .require()
            .unwrap(),
        "urn:example:metadata"
    );
}
