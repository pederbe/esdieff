//! Bounded local source selection for direct checks and `diff`, before ordinary acquisition.
//! Follows ADR0004: Supplied catalogues and checking scope, and
//! ADR0009: Bounded processing and reporting.
use std::{
    ffi::OsStr,
    fs, io,
    path::{Component, Path, PathBuf},
};

type Error = (u8, String);
const MAX_ENTRIES: usize = 10_000;
const MAX_SOURCES: usize = 256;
const MAX_METADATA: usize = 1024 * 1024;
const MAX_DEPTH: usize = 64;
// Native read_dir entries contain one component, bounded by the host directory
// record. This slot covers a Unix dirent and Windows encoded-name expansion.
const NATIVE_COMPONENT_SLOT: usize = 128 * 1024;

struct Budget {
    retained: usize,
    entries: usize,
}
impl Budget {
    fn transient(bytes: usize) -> Result<(), Error> {
        if bytes > MAX_METADATA {
            return Err(crate::usage(
                "directory transient path handling exceeds 1 MiB",
            ));
        }
        Ok(())
    }
    fn charge(&mut self, bytes: usize) -> Result<(), Error> {
        self.retained = self
            .retained
            .checked_add(bytes)
            .ok_or_else(|| crate::usage("directory metadata overflow"))?;
        if self.retained > MAX_METADATA {
            return Err(crate::usage(
                "directory path and option metadata exceeds 1 MiB",
            ));
        }
        Ok(())
    }
    fn entry(&mut self) -> Result<(), Error> {
        self.entries = self
            .entries
            .checked_add(1)
            .ok_or_else(|| crate::usage("directory entry count overflow"))?;
        if self.entries > MAX_ENTRIES {
            return Err(crate::usage(
                "directory discovery exceeds 10,000 observed entries",
            ));
        }
        Ok(())
    }
}
fn failure(path: &Path, reason: impl std::fmt::Display) -> Error {
    // Do not allocate a lossy copy of an arbitrarily long path for a diagnostic.
    let path = path.as_os_str();
    let label = if path.as_encoded_bytes().len() <= 256 {
        path.to_string_lossy()
    } else {
        "[path label omitted]".into()
    };
    (
        2,
        format!("cannot prepare directory input {label:?}: {reason}"),
    )
}

#[cfg(unix)]
type Identity = (u64, u64);
#[cfg(unix)]
fn identity(_: &Path, metadata: &fs::Metadata) -> io::Result<Identity> {
    use std::os::unix::fs::MetadataExt;
    Ok((metadata.dev(), metadata.ino()))
}
#[cfg(windows)]
type Identity = same_file::Handle;
#[cfg(windows)]
fn identity(path: &Path, _: &fs::Metadata) -> io::Result<Identity> {
    use std::os::windows::fs::OpenOptionsExt;
    // Zero desired access requests metadata identity without content-read permission.
    // BACKUP_SEMANTICS also allows a handle if a concurrent change makes this a directory.
    let file = fs::OpenOptions::new()
        .access_mode(0)
        .custom_flags(0x02000000)
        .open(path)?;
    same_file::Handle::from_file(file)
}
#[cfg(not(any(unix, windows)))]
type Identity = ();
#[cfg(not(any(unix, windows)))]
fn identity(_: &Path, _: &fs::Metadata) -> io::Result<Identity> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "native filesystem identity is unavailable",
    ))
}
fn overlap(inputs: &[&OsStr], discovered: &[(PathBuf, PathBuf)]) -> Result<(), Error> {
    if discovered.len() == 1 && inputs.iter().all(|path| *path == OsStr::new("-")) {
        return Ok(());
    }
    let mut explicit = Vec::with_capacity(inputs.len());
    for path in inputs.iter().filter(|path| **path != OsStr::new("-")) {
        let path = Path::new(path);
        match fs::metadata(path) {
            Ok(metadata) if metadata.is_file() => {
                explicit.push(identity(path, &metadata).map_err(|error| failure(path, error))?)
            }
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) => {}
            Err(error) => return Err(failure(path, error)),
        }
    }
    if discovered.len() == 1 && explicit.is_empty() {
        return Ok(());
    }
    let mut automatic = Vec::with_capacity(discovered.len());
    for (_, path) in discovered {
        let metadata = fs::metadata(path).map_err(|error| failure(path, error))?;
        let id = identity(path, &metadata).map_err(|error| failure(path, error))?;
        // At most 256 supplied attempts bounds both retained identity storage and
        // these comparisons. Explicit duplicates intentionally remain independent.
        if explicit
            .iter()
            .chain(&automatic)
            .any(|previous| *previous == id)
        {
            return Err(failure(
                path,
                "discovered model overlaps another supplied file",
            ));
        }
        automatic.push(id);
    }
    Ok(())
}

pub(crate) fn prepare(
    inputs: &[&OsStr],
    directories: &[&OsStr],
    roots: &[&OsStr],
    recursive: bool,
    exclusions: &[&OsStr],
) -> Result<Vec<PathBuf>, Error> {
    let discovered = discover(inputs, directories, roots, recursive, exclusions)?;
    Ok(discovered.into_iter().map(|(_, joined)| joined).collect())
}

/// Selects model files as `prepare` does, keeping each file's path relative to its supplied
/// directory beside the joined path. Discovery already charges both to its metadata budget.
pub(crate) fn discover(
    inputs: &[&OsStr],
    directories: &[&OsStr],
    roots: &[&OsStr],
    recursive: bool,
    exclusions: &[&OsStr],
) -> Result<Vec<(PathBuf, PathBuf)>, Error> {
    if directories.is_empty() {
        return Ok(Vec::new());
    }
    let mut budget = Budget {
        retained: 0,
        entries: 0,
    };
    for path in inputs
        .iter()
        .chain(directories)
        .chain(roots)
        .chain(exclusions)
    {
        budget.charge(path.as_encoded_bytes().len())?;
    }
    for exclusion in exclusions {
        validate_exclusion(exclusion)?;
    }
    // Both lists have a fixed ceiling before allocation, including sorting storage.
    let mut discovered = Vec::with_capacity(MAX_SOURCES - inputs.len());
    for directory in directories {
        let directory = Path::new(directory);
        let mut selected = Vec::with_capacity(MAX_SOURCES - inputs.len() - discovered.len());
        Discovery {
            budget: &mut budget,
            recursive,
            exclusions,
            previous_sources: inputs.len() + discovered.len(),
        }
        .walk(directory, Path::new(""), 0, &mut selected)?;
        if selected.is_empty() {
            return Err(failure(directory, "no regular .sdf.json files selected"));
        }
        selected.sort_unstable_by(|left, right| {
            left.0
                .as_os_str()
                .as_encoded_bytes()
                .cmp(right.0.as_os_str().as_encoded_bytes())
        });
        discovered.extend(selected);
    }
    overlap(inputs, &discovered)?;
    Ok(discovered)
}

struct Discovery<'a> {
    budget: &'a mut Budget,
    recursive: bool,
    exclusions: &'a [&'a OsStr],
    previous_sources: usize,
}
impl Discovery<'_> {
    fn walk(
        &mut self,
        directory: &Path,
        relative: &Path,
        depth: usize,
        selected: &mut Vec<(PathBuf, PathBuf)>,
    ) -> Result<(), Error> {
        // read_dir retains the native directory path alongside its open handle.
        self.budget
            .charge(directory.as_os_str().as_encoded_bytes().len())?;
        let entries = fs::read_dir(directory).map_err(|error| failure(directory, error))?;
        for entry in entries {
            self.budget.entry()?;
            let entry = entry.map_err(|error| failure(directory, error))?;
            // Admit the native component before requesting its owned copy. Match
            // literal exclusions before any required entry type or child access.
            Budget::transient(NATIVE_COMPONENT_SLOT)?;
            let name = entry.file_name();
            if name.as_encoded_bytes().len() > NATIVE_COMPONENT_SLOT {
                return Err(failure(
                    directory,
                    "native directory component exceeds its slot",
                ));
            }
            if self.exclusions.iter().any(|excluded| {
                Path::new(excluded)
                    .components()
                    .eq(relative.components().chain([Component::Normal(&name)]))
            }) {
                continue;
            }
            let kind = entry
                .file_type()
                .map_err(|error| failure(directory, error))?;
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if entry
                    .metadata()
                    .map_err(|error| failure(directory, error))?
                    .file_attributes()
                    & 0x400
                    != 0
                {
                    continue;
                }
            }
            drop(entry);
            if !(kind.is_file() || self.recursive && kind.is_dir()) {
                continue;
            }
            if kind.is_file() && !name.as_encoded_bytes().ends_with(b".sdf.json") {
                continue;
            }
            if kind.is_dir() && depth >= MAX_DEPTH {
                return Err(failure(directory, "recursive directory depth exceeds 64"));
            }
            if kind.is_file() && self.previous_sources + selected.len() >= MAX_SOURCES {
                return Err(crate::usage(
                    "directory discovery exceeds 256 assembled source attempts",
                ));
            }
            let joined = joined_size(directory, &name)?;
            let relative_size = joined_size(relative, &name)?;
            self.budget.charge(relative_size)?;
            self.budget.charge(joined)?;
            Budget::transient(
                joined
                    .checked_add(relative_size)
                    .and_then(|size| size.checked_add(NATIVE_COMPONENT_SLOT))
                    .ok_or_else(|| crate::usage("directory path size overflow"))?,
            )?;
            let path = directory.join(&name);
            let relative_path = relative.join(&name);
            drop(name);
            if kind.is_dir() {
                self.walk(&path, &relative_path, depth + 1, selected)?;
            } else {
                selected.push((relative_path, path));
            }
        }
        Ok(())
    }
}
fn validate_exclusion(value: &OsStr) -> Result<(), Error> {
    let bytes = value.as_encoded_bytes();
    // Path::components normalizes interior current-directory components. Check
    // their original spelling as well so `models/./one.sdf.json` is refused.
    let dot = bytes
        .split(|byte| *byte == b'/' || cfg!(windows) && *byte == b'\\')
        .any(|component| component == b"." || component == b"..");
    if bytes.is_empty()
        || dot
        || Path::new(value)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(crate::usage(
            "exclusion must be a nonempty relative path without root, parent or current components",
        ));
    }
    Ok(())
}
fn joined_size(parent: &Path, name: &OsStr) -> Result<usize, Error> {
    parent
        .as_os_str()
        .as_encoded_bytes()
        .len()
        .checked_add(name.as_encoded_bytes().len())
        .and_then(|size| size.checked_add(1))
        .ok_or_else(|| crate::usage("directory path size overflow"))
}
