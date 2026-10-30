//! Release metadata lives on immutable tags above checked development snapshots.
use crate::Result;
use std::{fs, path::Path, process::Command};

const REPOSITORY: &str = "https://github.com/pederbe/esdieff";
const RELEASE_SUBJECT: &str = "Prepare automated release v";

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let done = Command::new("git").current_dir(root).args(args).output()?;
    if !done.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&done.stderr).trim()
        )
        .into());
    }
    Ok(String::from_utf8(done.stdout)?)
}

fn read(path: impl AsRef<Path>) -> Result<String> {
    Ok(fs::read_to_string(path)?.replace("\r\n", "\n"))
}

pub(crate) fn version(text: &str) -> Result<[u64; 3]> {
    let mut parts = text.strip_prefix('v').unwrap_or(text).split('.');
    let mut parsed = [0; 3];
    for part in &mut parsed {
        let text = parts.next().ok_or("expected a stable x.y.z version")?;
        if text.is_empty()
            || !text.bytes().all(|byte| byte.is_ascii_digit())
            || (text.len() > 1 && text.starts_with('0'))
        {
            return Err("expected a stable x.y.z version".into());
        }
        *part = text.parse()?;
    }
    if parts.next().is_some() {
        return Err("expected a stable x.y.z version".into());
    }
    Ok(parsed)
}

fn field(line: &str, key: &str) -> Option<String> {
    let (name, value) = line.split_once('=')?;
    (name.trim() == key).then_some(())?;
    let value = value.trim().strip_prefix('"')?;
    Some(value.split_once('"')?.0.to_owned())
}

pub(crate) fn crate_version(manifest: &str) -> Result<String> {
    let mut package = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            package = line == "[package]";
        } else if package && let Some(value) = field(line, "version") {
            version(&value)?;
            return Ok(value);
        }
    }
    Err("Cargo.toml has no package version".into())
}

fn replace_manifest_version(manifest: &str, next: &str) -> Result<String> {
    let mut package = false;
    let mut replaced = false;
    let mut result = String::new();
    for line in manifest.lines() {
        if line.trim().starts_with('[') {
            package = line.trim() == "[package]";
        }
        if package && field(line, "version").is_some() {
            if replaced {
                return Err("duplicate package version".into());
            }
            result.push_str(&format!("version = \"{next}\"\n"));
            replaced = true;
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }
    if !replaced {
        return Err("missing package version".into());
    }
    Ok(result)
}

fn replace_lock_version(lock: &str, next: &str) -> Result<String> {
    let mut result = String::new();
    let mut replaced = 0;
    for (index, block) in lock.split("[[package]]").enumerate() {
        if index > 0 {
            result.push_str("[[package]]");
        }
        if !block
            .lines()
            .any(|line| field(line, "name").as_deref() == Some("esdieff"))
        {
            result.push_str(block);
            continue;
        }
        for line in block.lines() {
            if field(line, "version").is_some() {
                result.push_str(&format!("version = \"{next}\"\n"));
                replaced += 1;
            } else {
                result.push_str(line);
                result.push('\n');
            }
        }
    }
    if replaced != 1 {
        return Err("Cargo.lock must contain exactly one esdieff package".into());
    }
    Ok(result)
}

fn meaningful(path: &str) -> bool {
    !path.ends_with(".md")
        && !["docs/", "site/", "tools/site/", ".github/ISSUE_TEMPLATE/"]
            .iter()
            .any(|prefix| path.starts_with(prefix))
        && !matches!(
            path,
            "book.toml"
                | "wrangler.jsonc"
                | "scripts/build-site.sh"
                | "scripts/ansi-to-html.sh"
                | ".github/FUNDING.yml"
                | ".github/workflows/docs.yml"
                | ".github/workflows/website.yml"
                | ".github/workflows/website-deploy.yml"
        )
}

fn bump(mut old: [u64; 3], breaking: bool) -> Result<[u64; 3]> {
    let index = if breaking {
        usize::from(old[0] == 0)
    } else {
        2
    };
    old[index] = old[index].checked_add(1).ok_or("version overflow")?;
    for part in &mut old[index + 1..] {
        *part = 0;
    }
    Ok(old)
}

fn release_date(date: &str) -> bool {
    date.len() == 10
        && date.bytes().enumerate().all(|(index, byte)| {
            if index == 4 || index == 7 {
                byte == b'-'
            } else {
                byte.is_ascii_digit()
            }
        })
}

// Finalized changelogs need no placeholder for work that has not happened yet.
// Older tags may still have one, so keep authored entries separate from history.
fn changelog_sections(changelog: &str) -> Result<(&str, &str, &str)> {
    let (intro, first) = changelog
        .split_once("## [")
        .ok_or("missing changelog release section")?;
    let (authored, history) = if let Some(rest) = first.strip_prefix("Unreleased]\n") {
        rest.split_once("\n## [")
            .ok_or("missing previous changelog section")?
    } else {
        ("", first)
    };
    let (released, date) = history
        .lines()
        .next()
        .and_then(|heading| heading.split_once("] - "))
        .ok_or("missing dated changelog release section")?;
    version(released)?;
    if !release_date(date) || history.contains("\n## [Unreleased]") {
        return Err("invalid changelog release history".into());
    }
    Ok((intro, authored, history))
}

// Keep authored bullets together with their continuation lines.
fn authored_records(text: &str) -> Vec<(String, String)> {
    let mut records = Vec::new();
    let mut heading = String::new();
    let mut entry = String::new();
    for line in text.lines().chain(std::iter::once("### ")) {
        if line.starts_with("### ") || line.starts_with("- ") || line.is_empty() {
            if !entry.trim().is_empty() {
                records.push((heading.clone(), entry.trim().to_owned()));
                entry.clear();
            }
            if line.starts_with("### ") {
                heading = line.to_owned();
                continue;
            }
        }
        if !line.is_empty() {
            entry.push_str(line);
            entry.push('\n');
        }
    }
    records
}

fn authored_delta(current: &str, previous: &str) -> String {
    let old = authored_records(previous);
    let mut output = String::new();
    let mut last_heading = String::new();
    for (heading, entry) in authored_records(current) {
        if old.contains(&(heading.clone(), entry.clone())) {
            continue;
        }
        if heading != last_heading && !heading.is_empty() {
            output.push_str(&format!("{heading}\n\n"));
            last_heading = heading;
        }
        output.push_str(&format!("{entry}\n\n"));
    }
    output.trim().to_owned()
}

pub(crate) fn prepare(args: &[String]) -> Result<()> {
    let [previous, date, mode, output, rest @ ..] = args else {
        return Err("usage: esdieff-release prepare-weekly PREVIOUS_TAG YYYY-MM-DD (auto|minor|major) OUTPUT [MINIMUM_TAG]".into());
    };
    if rest.len() > 1 {
        return Err("expected at most one minimum version tag".into());
    }
    let root = std::env::current_dir()?;
    let old = version(previous)?;
    if !release_date(date) {
        return Err("expected release date YYYY-MM-DD".into());
    }
    if !["auto", "minor", "major"].contains(&mode.as_str()) {
        return Err("invalid version increment".into());
    }
    if !git(&root, &["status", "--porcelain"])?.is_empty() {
        return Err("release preparation needs a clean checkout".into());
    }
    let source = git(&root, &["rev-parse", "HEAD"])?;
    let subject = git(&root, &["show", "-s", "--format=%s", previous])?;
    let base = if subject.starts_with(RELEASE_SUBJECT) {
        format!("{previous}^")
    } else {
        previous.to_owned()
    };
    git(&root, &["merge-base", "--is-ancestor", &base, "HEAD"])?;
    let paths = git(&root, &["diff", "--name-only", &base, "HEAD"])?;
    let previous_commit = git(&root, &["rev-parse", &format!("{previous}^{{commit}}")])?;
    if source.trim() == previous_commit.trim() || !paths.lines().any(meaningful) {
        fs::write(output, format!("changed=false\nsource={}\n", source.trim()))?;
        return Ok(());
    }
    let log = git(
        &root,
        &[
            "log",
            "--reverse",
            "--format=%s%x00%b%x00%H%x00",
            &format!("{base}..HEAD"),
        ],
    )?;
    let records: Vec<_> = log.split('\0').collect();
    let mut groups = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    let mut breaking = false;
    let mut feature = false;
    for record in records.chunks(3) {
        let [subject, body, hash] = record else {
            continue;
        };
        let subject = subject.trim();
        if subject.is_empty() {
            continue;
        }
        let is_breaking = subject
            .split_once(':')
            .is_some_and(|(prefix, _)| prefix.ends_with('!'))
            || body.contains("BREAKING CHANGE:")
            || body.contains("BREAKING-CHANGE:");
        breaking |= is_breaking;
        feature |= subject.starts_with("feat:")
            || subject.starts_with("feat(")
            || subject.starts_with("feat!");
        let group = if is_breaking {
            0
        } else if subject.starts_with("feat") {
            1
        } else if subject.starts_with("fix") {
            2
        } else {
            3
        };
        let hash = hash.trim();
        let details = body
            .lines()
            .map(|line| format!("  {line}\n"))
            .collect::<String>();
        groups[group].push(format!(
            "- {subject} ([{}]({REPOSITORY}/commit/{hash}))\n{details}",
            &hash[..7]
        ));
    }
    let mut next = bump(old, breaking)?;
    if mode == "minor" || (feature && old[0] > 0) {
        next = next.max([old[0], old[1].checked_add(1).ok_or("version overflow")?, 0]);
    }
    if mode == "major" {
        next = [old[0].checked_add(1).ok_or("version overflow")?, 0, 0];
    }
    let manifest = read(root.join("Cargo.toml"))?;
    next = next.max(version(&crate_version(&manifest)?)?);
    if let Some(minimum) = rest.first() {
        let floor = version(minimum)?;
        if next <= floor {
            next = bump(floor, false)?;
        }
    }
    let next = format!("{}.{}.{}", next[0], next[1], next[2]);
    let manifest = replace_manifest_version(&manifest, &next)?;
    let lock = replace_lock_version(&read(root.join("Cargo.lock"))?, &next)?;
    let changelog = read(root.join("CHANGELOG.md"))?;
    let (intro, authored, _) = changelog_sections(&changelog)?;
    let previous_changelog =
        git(&root, &["show", &format!("{previous}:CHANGELOG.md")])?.replace("\r\n", "\n");
    let (_, _, history) = changelog_sections(&previous_changelog)?;
    let prior_source =
        git(&root, &["show", &format!("{base}:CHANGELOG.md")])?.replace("\r\n", "\n");
    let (_, prior_authored, _) = changelog_sections(&prior_source)?;
    let mut notes = authored_delta(authored, prior_authored);
    notes.push_str("\n\n");
    for (heading, entries) in ["Breaking changes", "Added", "Fixed", "Other changes"]
        .into_iter()
        .zip(groups)
    {
        if !entries.is_empty() {
            notes.push_str(&format!("### {heading}\n\n{}\n", entries.concat()));
        }
    }
    notes.push_str(&format!(
        "[Full comparison]({REPOSITORY}/compare/{previous}...v{next})\n"
    ));
    let changelog = format!(
        "{intro}## [{next}] - {date}\n\n{}\n\n## [{history}",
        notes.trim()
    );
    let changelog = changelog
        .lines()
        .map(|line| {
            if line.starts_with("[Unreleased]:") {
                format!("[Unreleased]: {REPOSITORY}/compare/v{next}...HEAD")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(root.join("Cargo.toml"), manifest)?;
    fs::write(root.join("Cargo.lock"), lock)?;
    fs::write(root.join("CHANGELOG.md"), changelog)?;
    fs::write(
        output,
        format!("changed=true\ntag=v{next}\nsource={}\n", source.trim()),
    )?;
    Ok(())
}

pub(crate) fn notes(args: &[String]) -> Result<()> {
    let [tag, output] = args else {
        return Err("usage: esdieff-release release-notes TAG OUTPUT".into());
    };
    version(tag)?;
    let version = tag.strip_prefix('v').ok_or("expected version tag")?;
    let changelog = read("CHANGELOG.md")?;
    let section = changelog
        .split_once(&format!("## [{version}] - "))
        .ok_or("missing dated release changelog section")?
        .1;
    let (_, section) = section.split_once('\n').ok_or("empty changelog section")?;
    let notes = section.split("\n## [").next().unwrap_or("").trim();
    if notes.is_empty() {
        return Err("empty release notes".into());
    }
    fs::write(output, format!("{notes}\n"))?;
    Ok(())
}
