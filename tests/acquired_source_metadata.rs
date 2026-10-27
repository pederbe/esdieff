//! Acquired bytes remain ordinary inputs with explicit caller-supplied identities.
//! Follows ADR0004: Supplied catalogues and checking scope, and
//! ADR0035: Explicit pinned document acquisition.

use esdieff::{Acquisition, Catalogue, SourceInput};
use sha2::{Digest, Sha256};

const ORIGINAL: &[u8] = include_bytes!("../examples/pinned-acquisition/original.sdf.json");
const URI: &str = "https://raw.githubusercontent.com/one-data-model/playground/41be1e0e564ad39f4d5a1c41fcccef435e388ee9/sdfObject/sdfobject-switch_binary.sdf.json";
const COMMIT: &str = "41be1e0e564ad39f4d5a1c41fcccef435e388ee9";
const DIGEST: &str = "6fe5a1538da0b5f48898e367f0f03b6b70fa38b6ebaa67117e408ab87fb0b61a";

#[test]
fn reviewed_retrieval_metadata_preserves_original_content_and_document_namespace() {
    let digest: String = Sha256::digest(ORIGINAL)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(digest, DIGEST);
    let catalogue = Catalogue::take_inputs(vec![
        SourceInput::owned("switch", ORIGINAL.to_vec())
            .acquisition_uri(URI)
            .version_token(COMMIT)
            .digest(DIGEST),
    ])
    .unwrap();
    let source = catalogue.sources().next().unwrap();
    assert_eq!(source.acquisition(), Acquisition::Supplied);
    assert_eq!(source.acquisition_uri(), Some(URI));
    assert_eq!(source.version_token(), Some(COMMIT));
    assert_eq!(source.digest(), Some(DIGEST));
    assert_eq!(source.raw_bytes(), ORIGINAL);
    assert_eq!(source.raw_bytes().len(), 787);
    assert!(source.raw_path_identity().is_none());
    assert_eq!(
        source
            .json()
            .require()
            .unwrap()
            .get("namespace")
            .require()
            .unwrap()
            .get("ocf")
            .require()
            .unwrap()
            .text()
            .require()
            .unwrap(),
        "https://onedm.org/ecosystem/ocf"
    );
}
