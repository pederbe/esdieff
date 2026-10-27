use super::{Failure, Receipt, ResponseSource};
use crate::document_cli::NativeOutput;
use cap_std::fs::OpenOptions;
#[cfg(test)]
use std::fs;
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);
pub(super) fn deliver_receipt(
    bytes: &[u8],
    output: &NativeOutput,
    other: &NativeOutput,
    stdout: &mut dyn Write,
    source: &ResponseSource,
    receipt: &Receipt<'_>,
) -> std::io::Result<()> {
    if output.path().is_none() {
        return stdout.write_all(bytes).and_then(|_| stdout.flush());
    }
    output
        .recheck(other)
        .map_err(|(_, message)| std::io::Error::other(message))?;
    let _ = source;
    #[cfg(test)]
    if let Some(change) = &source.faults.after_receipt_check {
        change();
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = output
        .parent()
        .open_with(output.basename(), &options)?
        .into_std();
    #[cfg(test)]
    if source.faults.receipt_write {
        file.write_all(&bytes[..bytes.len().min(16)])?;
        return Err(std::io::Error::other("injected receipt write failure"));
    }
    bytes
        .chunks(16 * 1024)
        .try_for_each(|chunk| file.write_all(chunk))?;
    file.sync_all()?;
    if receipt.failures.is_empty()
        && let Some(file) = &receipt.verified_file
    {
        other
            .confirm(file)
            .map_err(|(_, message)| std::io::Error::other(message))?;
    }
    output
        .confirm(&file)
        .map_err(|(_, message)| std::io::Error::other(message))
}
struct Staging {
    path: PathBuf,
    file: File,
}
impl Staging {
    fn new(output: &NativeOutput) -> Result<Self, Failure> {
        let target = output.path().expect("named document preflight");
        let parent = target
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        for _ in 0..100 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let name = format!(".esdieff-acquire-{}-{sequence}", std::process::id());
            let path = parent.join(&name);
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use cap_std::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match output.parent().open_with(name, &options) {
                Ok(file) => {
                    return Ok(Self {
                        path,
                        file: file.into_std(),
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(Failure::new(
                        "stage",
                        format!("cannot create private staging: {error}"),
                    ));
                }
            }
        }
        Err(Failure::new(
            "stage",
            "bounded staging-name attempts exhausted",
        ))
    }
}
pub(super) fn deliver(
    bytes: &[u8],
    output: &NativeOutput,
    other: &NativeOutput,
    receipt: &mut Receipt<'_>,
    source: &ResponseSource,
) {
    let _ = source;
    if let Err((_, message)) = output.recheck(other) {
        receipt.failures.push(Failure::new("commit", message));
        return;
    }
    let staging = match Staging::new(output) {
        Ok(staging) => staging,
        Err(failure) => {
            receipt.failures.push(failure);
            return;
        }
    };
    let Staging { path, mut file } = staging;
    let mut write_bytes = || {
        bytes
            .chunks(16 * 1024)
            .try_for_each(|chunk| file.write_all(chunk))
            .and_then(|_| file.sync_all())
    };
    let staged = {
        #[cfg(test)]
        if source.faults.stage {
            Err(std::io::Error::other("injected staging write failure"))
        } else {
            write_bytes()
        }
        #[cfg(not(test))]
        write_bytes()
    };
    if let Err(error) = staged {
        receipt.failures.push(Failure::new(
            "stage",
            format!("cannot stage verified bytes: {error}"),
        ));
    }
    if receipt.failures.is_empty() {
        let commit = output
            .recheck(other)
            .map_err(|(_, reason)| Failure::new("commit", reason))
            .and_then(|_| {
                output.confirm_parent_entry(path.file_name().unwrap(), &file)
                    .map_err(|error| Failure::new("commit", format!("staging identity changed before commit: {error}")))?;
                #[cfg(test)]
                if let Some(change) = &source.faults.after_document_check {
                    change();
                }
                #[cfg(test)]
                if source.faults.create_document {
                    // Create the competing name after the final recheck. The
                    // atomic link must still refuse to replace its contents.
                    fs::write(output.path().unwrap(), b"concurrent owner").unwrap();
                }
                #[cfg(test)]
                if source.faults.commit {
                    return Err(Failure::new("commit", "injected commit failure"));
                }
                output.parent().hard_link(path.file_name().unwrap(), output.parent(), output.basename()).map_err(|error| {
                    Failure::new("commit", format!("cannot commit new document: {error}"))
                })?;
                output.confirm_parent_entry(output.basename(), &file)
                    .map_err(|error| Failure::new("commit", format!("linked destination is not the verified staging inode: {error}")))?;
                receipt.committed = true;
                output.confirm(&file).map_err(|(_, message)| Failure::new("commit", format!("verified document committed in preflight directory; requested path availability is unconfirmed: {message}")))
            });
        match commit {
            Ok(()) => receipt.committed = true,
            Err(failure) => receipt.failures.push(failure),
        }
    }
    #[cfg(test)]
    if source.faults.staging_disappears {
        output
            .parent()
            .remove_file(path.file_name().unwrap())
            .unwrap();
    }
    let staging_identity = output.confirm_parent_entry(path.file_name().unwrap(), &file);
    let cleanup_refused = staging_identity.is_err();
    let cleanup = staging_identity.and_then(|_| {
        #[cfg(test)]
        if source.faults.cleanup {
            Err(std::io::Error::other("injected cleanup failure"))
        } else {
            output.parent().remove_file(path.file_name().unwrap())
        }
        #[cfg(not(test))]
        output.parent().remove_file(path.file_name().unwrap())
    });
    if let Err(error) = cleanup
        && error.kind() != std::io::ErrorKind::NotFound
    {
        receipt.staging = Some(path);
        let message = if cleanup_refused {
            format!(
                "staging identity changed or could not be confirmed: {error}. The unconfirmed entry at the reported native staging path was preserved"
            )
        } else {
            format!("cannot remove task-created staging: {error}")
        };
        receipt.failures.push(Failure::new("cleanup", message));
    }
    if receipt.committed {
        receipt.verified_file = Some(file);
    }
}
