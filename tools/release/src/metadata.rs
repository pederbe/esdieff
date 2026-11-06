//! Package versions and release notes. release-plz writes the dated changelog
//! sections, and the Release workflow publishes the section for its tag.
use crate::Result;
use std::fs;

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

pub(crate) fn notes(args: &[String]) -> Result<()> {
    let [tag, output] = args else {
        return Err("usage: esdieff-release release-notes TAG OUTPUT".into());
    };
    version(tag)?;
    let version = tag.strip_prefix('v').ok_or("expected version tag")?;
    let changelog = fs::read_to_string("CHANGELOG.md")?.replace("\r\n", "\n");
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
