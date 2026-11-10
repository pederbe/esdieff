//! User documentation describes the current release, so it carries no release numbers.

use std::fs;
use std::path::{Path, PathBuf};

/// Exact phrases a reader needs: another product's version or sample data.
const ALLOWED: &[&str] = &[
    "SARIF 2.1.0",
    // The `version` field of the example models.
    "\"1.0.0\"",
    "`1.0.0`",
];

/// Text that introduces a specification section number rather than a version.
const SECTION_PREFIXES: &[&str] = &["Section ", "Sections ", "section-"];

/// Pages for contributors and maintainers, and pages whose purpose is measurement.
const SKIPPED_PAGES: &[&str] = &["device-catalogue.md", "profiling.md", "project.md"];

/// Decision records and working material.
const SKIPPED_DIRECTORIES: &[&str] = &["adr", "agents", "profiling"];

fn user_pages(directory: &Path, pages: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        if path.is_dir() {
            if !SKIPPED_DIRECTORIES.contains(&name.as_str()) {
                user_pages(&path, pages);
            }
        // The formal contracts and the inventory cite specification clauses throughout.
        } else if name.ends_with(".md")
            && !name.ends_with("-contract.md")
            && !name.ends_with("-inventory.md")
            && !SKIPPED_PAGES.contains(&name.as_str())
        {
            pages.push(path);
        }
    }
}

/// Finds tokens shaped like `1.2.3` or `v1.2.3` that are not section numbers.
fn release_numbers(line: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut rest = 0;
    while let Some(offset) = line[rest..].find(|c: char| c.is_ascii_digit()) {
        let start = rest + offset;
        let length = line[start..]
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(line.len() - start);
        let token = line[start..start + length].trim_end_matches('.');
        rest = start + length;
        let parts: Vec<&str> = token.split('.').collect();
        let before = &line[..start];
        if parts.len() == 3
            && parts.iter().all(|part| !part.is_empty())
            && !SECTION_PREFIXES
                .iter()
                .any(|prefix| before.ends_with(prefix))
            // The second number of "Sections 2.3.1 and 2.3.3".
            && !(before.contains("Sections ") && before.ends_with(" and "))
        {
            found.push(token);
        }
    }
    found
}

#[test]
fn user_documentation_carries_no_release_numbers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = vec![
        root.join("README.md"),
        root.join("GLOSSARY.md"),
        root.join("scripts/install.sh"),
    ];
    user_pages(&root.join("docs"), &mut files);
    files.sort();

    let mut failures = Vec::new();
    for file in &files {
        let text = fs::read_to_string(file).unwrap();
        for (index, line) in text.lines().enumerate() {
            let mut checked = line.to_owned();
            for phrase in ALLOWED {
                checked = checked.replace(phrase, "");
            }
            for number in release_numbers(&checked) {
                failures.push(format!(
                    "{}:{}: {number}",
                    file.strip_prefix(&root).unwrap().display(),
                    index + 1
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "User documentation describes the current release. Remove each number below, or add \
         its exact phrase to ALLOWED in tools/terminal-html/tests/user_docs.rs when a reader \
         needs it, such as another product's version or sample data.\n{}",
        failures.join("\n")
    );
}

#[test]
fn release_numbers_are_told_apart_from_other_numbers() {
    assert_eq!(
        release_numbers("install v1.2.3 or 10.20.30."),
        ["1.2.3", "10.20.30"]
    );
    assert_eq!(release_numbers("esdieff-0.4.0.tar.gz"), ["0.4.0"]);
    assert_eq!(release_numbers("0.3.0 and 0.4.0"), ["0.3.0", "0.4.0"]);
    assert!(release_numbers("Sections 2.3.1 and 2.3.3").is_empty());
    assert!(release_numbers("Section 2.2.2, #section-4.7.1, 127.0.0.1, 1.5 and 3.").is_empty());
}
