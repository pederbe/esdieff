//! Explicit caller-supplied metadata for an independently pinned local snapshot.
//! Follows ADR0004: Supplied catalogues and checking scope, and
//! ADR0035: Explicit pinned document acquisition.

use esdieff::{Catalogue, SourceInput};
use sha2::{Digest, Sha256};
use std::{error::Error, fs::File, io::Read, path::PathBuf};

const URI: &str = "https://raw.githubusercontent.com/one-data-model/playground/41be1e0e564ad39f4d5a1c41fcccef435e388ee9/sdfObject/sdfobject-switch_binary.sdf.json";
const COMMIT: &str = "41be1e0e564ad39f4d5a1c41fcccef435e388ee9";
const DIGEST: &str = "6fe5a1538da0b5f48898e367f0f03b6b70fa38b6ebaa67117e408ab87fb0b61a";
const BYTE_COUNT: usize = 787;

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("examples/pinned-acquisition/original.sdf.json"));
    let mut bytes = Vec::with_capacity(BYTE_COUNT + 1);
    File::open(path)?
        .take(BYTE_COUNT as u64 + 1)
        .read_to_end(&mut bytes)?;
    let digest: String = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if bytes.len() != BYTE_COUNT || digest != DIGEST {
        return Err("local bytes differ from the independently pinned digest".into());
    }
    // Metadata describes reviewed local content. SourceInput does not retrieve a URI.
    let catalogue = Catalogue::take_inputs(vec![
        SourceInput::owned("switch", bytes)
            .acquisition_uri(URI)
            .version_token(COMMIT)
            .digest(DIGEST),
    ])?;
    let source = catalogue
        .sources()
        .next()
        .ok_or("missing supplied source")?;
    println!(
        "{} bytes from {} at {} with SHA-256 {}",
        source.raw_bytes().len(),
        source.acquisition_uri().ok_or("missing URI")?,
        source.version_token().ok_or("missing version token")?,
        source.digest().ok_or("missing digest")?
    );
    Ok(())
}
