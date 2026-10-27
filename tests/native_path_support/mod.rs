use std::{fs, io, path::Path};

/// Keep native-path tests meaningful on filesystems such as APFS, which refuse
/// invalid UTF-8 names even though Unix argument strings can represent them.
pub fn write(path: &Path, bytes: &[u8]) -> Option<io::Error> {
    match fs::write(path, bytes) {
        Ok(()) => None,
        Err(error) => {
            assert!(path.to_str().is_none(), "valid filename rejected: {error}");
            assert!(!path.exists(), "failed fixture write created a file");
            Some(error)
        }
    }
}
