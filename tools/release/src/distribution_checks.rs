use super::*;

fn fixture() -> Work {
    Work::new(&std::env::temp_dir()).unwrap()
}

fn archive_files(directory: &Path) {
    fs::create_dir_all(directory).unwrap();
    fs::write(
        directory.join("esdieff"),
        b"\x7fELF\0retained executable\xff",
    )
    .unwrap();
    for name in DOCS {
        fs::write(
            directory.join(name),
            format!("Original {name}\n\nLicense and notice text.\n"),
        )
        .unwrap();
    }
}

fn packaged_files(extracted: &Path, packaged: &Path) {
    fs::create_dir_all(packaged.join("usr/bin")).unwrap();
    fs::create_dir_all(packaged.join("usr/share/doc/esdieff")).unwrap();
    fs::copy(extracted.join("esdieff"), packaged.join("usr/bin/esdieff")).unwrap();
    for name in DOCS {
        fs::copy(
            extracted.join(name),
            packaged.join("usr/share/doc/esdieff").join(name),
        )
        .unwrap();
    }
}

#[test]
fn packages_preserve_executable_and_every_shipped_document_byte() {
    let work = fixture();
    let extracted = work.0.join("archive");
    let packaged = work.0.join("package");
    archive_files(&extracted);
    packaged_files(&extracted, &packaged);
    compare_package(&extracted, &packaged).unwrap();
    let files = std::iter::once(("esdieff", packaged.join("usr/bin/esdieff"))).chain(
        DOCS.iter()
            .map(|name| (*name, packaged.join("usr/share/doc/esdieff").join(name))),
    );
    for (name, path) in files {
        fs::write(&path, b"changed bytes").unwrap();
        let error = compare_package(&extracted, &packaged)
            .unwrap_err()
            .to_string();
        assert!(error.contains("packaging changed"), "{error}");
        assert!(error.contains(name), "{error}");
        fs::copy(extracted.join(name), path).unwrap();
    }
    compare_package(&extracted, &packaged).unwrap();
    fs::remove_file(packaged.join("usr/share/doc/esdieff/LICENSE-APACHE")).unwrap();
    assert!(compare_package(&extracted, &packaged).is_err());
}

#[test]
fn retry_retains_identical_files_and_refuses_replacement_bytes() {
    let work = fixture();
    let incoming = work.0.join("incoming.deb");
    let retained = work.0.join("retained.deb");
    fs::write(&incoming, b"retained package bytes").unwrap();
    keep(&incoming, &retained).unwrap();
    keep(&incoming, &retained).unwrap();
    fs::write(&incoming, b"different release bytes").unwrap();
    assert!(keep(&incoming, &retained).is_err());
    assert_eq!(fs::read(retained).unwrap(), b"retained package bytes");
}

#[cfg(target_os = "linux")]
fn tar(root: &Path, entry: &str, archive: &Path) {
    let done = Command::new("tar")
        .arg("-cf")
        .arg(archive)
        .arg("-C")
        .arg(root)
        .arg(entry)
        .output()
        .unwrap();
    assert!(
        done.status.success(),
        "{}",
        String::from_utf8_lossy(&done.stderr)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn distribution_unpacks_the_prefixed_tar_without_changing_contents() {
    let work = fixture();
    let prefix = "esdieff-x86_64-unknown-linux-gnu";
    let source = work.0.join("source");
    archive_files(&source.join(prefix));
    let archive = work.0.join("archive.tar");
    tar(&source, prefix, &archive);
    let extracted = work.0.join("extracted");
    extract_archive(&archive, prefix, &extracted).unwrap();
    for name in std::iter::once("esdieff").chain(DOCS.iter().copied()) {
        assert_eq!(
            fs::read(extracted.join(name)).unwrap(),
            fs::read(source.join(prefix).join(name)).unwrap()
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn distribution_refuses_a_different_root_and_unexpected_or_missing_files() {
    for fault in ["wrong root", "extra file", "missing license"] {
        let work = fixture();
        let prefix = "esdieff-x86_64-unknown-linux-gnu";
        let source = work.0.join("source");
        let directory = if fault == "wrong root" {
            "other-project"
        } else {
            prefix
        };
        archive_files(&source.join(directory));
        if fault == "extra file" {
            fs::write(source.join(directory).join("unexpected.txt"), "extra").unwrap();
        }
        if fault == "missing license" {
            fs::remove_file(source.join(directory).join("LICENSE-MIT")).unwrap();
        }
        let archive = work.0.join("archive.tar");
        tar(&source, directory, &archive);
        assert!(
            extract_archive(&archive, prefix, &work.0.join("extracted")).is_err(),
            "accepted {fault}"
        );
    }
}
