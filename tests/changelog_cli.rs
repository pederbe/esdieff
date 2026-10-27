//! `diff --format markdown` writes a Markdown changelog of a Revision comparison. Follows
//! ADR 0043: SARIF and Markdown report projections.
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

const LOCK: &str = include_str!("../examples/door-lock-check/rich.sdf.json");

struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-changelog-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Compares `old` and `new` lock revisions in a fresh directory.
fn diff(old: &str, new: &str, acknowledged: Option<&str>, format: &str) -> Output {
    let tree = Tree::new();
    tree.write("old/lock.sdf.json", old);
    tree.write("new/lock.sdf.json", new);
    let mut args = vec!["diff", "--old", "old", "--new", "new", "--format", format];
    if let Some(acknowledged) = acknowledged {
        tree.write("acknowledged.json", acknowledged);
        args.extend(["--acknowledged", "acknowledged.json"]);
    }
    Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(args)
        .current_dir(tree.path())
        .output()
        .unwrap()
}

/// The changelog, after checking that its exit code equals the JSON report's.
fn changelog(old: &str, new: &str, acknowledged: Option<&str>, exit: i32) -> String {
    let json = diff(old, new, acknowledged, "json");
    assert_eq!(json.status.code(), Some(exit));
    let output = diff(old, new, acknowledged, "markdown");
    assert_eq!(
        output.status.code(),
        Some(exit),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap()
}

/// A breaking narrower credential, an undetermined added unit, a compatible description and
/// a renamed event, which removes one event and adds another.
fn revised_lock() -> String {
    LOCK.replacen("\"maxLength\": 8", "\"maxLength\": 6", 1)
        .replace(
            "An authored demonstration token with Unicode scalar length constraints",
            "A demonstration token",
        )
        .replacen(
            "\"type\": \"string\",",
            "\"type\": \"string\", \"unit\": \"Cel\",",
            1,
        )
        .replace("\"stateChanged\": {", "\"changed\": {")
}

const ACKNOWLEDGED: &str = r#"{"schema":"esdieff.acknowledged-differences","version":"1","differences":[
    {"kind":"changed","subject":"quality","quality":"maxLength","classification":"breaking",
     "earlier":{"document":"lock.sdf.json","pointer":"/sdfObject/Lock/sdfProperty/credential/maxLength"},
     "later":{"document":"lock.sdf.json","pointer":"/sdfObject/Lock/sdfProperty/credential/maxLength"}}]}"#;

#[test]
fn changelogs_list_differences_by_classification() {
    assert_eq!(
        changelog(LOCK, &revised_lock(), Some(ACKNOWLEDGED), 1),
        "# Model changes

Disposition: failed, with 2 breaking, 1 undetermined and 2 compatible differences. 1 breaking and 0 undetermined differences are acknowledged.

## Breaking differences

- Removed `sdfEvent` in `lock.sdf.json` at `/sdfObject/Lock/sdfEvent/stateChanged`

## Undetermined differences

- Changed quality `unit` in `lock.sdf.json` at `/sdfObject/Lock/sdfProperty/state/unit`: unit is added, which changes the stated meaning of values (`comparison.unit_added`)

## Compatible differences

- Added `sdfEvent` in `lock.sdf.json` at `/sdfObject/Lock/sdfEvent/changed`
- Changed quality `description` in `lock.sdf.json` at `/sdfObject/Lock/sdfProperty/credential/description`

## Acknowledged differences

- Breaking: changed quality `maxLength` in `lock.sdf.json` at `/sdfObject/Lock/sdfProperty/credential/maxLength` (witness `\"aaaaaaa\"` for client-supplied data, earlier admitted, later rejected)
"
    );
}

#[test]
fn changelogs_leave_out_empty_sections() {
    let described = LOCK.replace(
        "An authored demonstration token with Unicode scalar length constraints",
        "A demonstration token",
    );
    let text = changelog(LOCK, &described, None, 0);
    assert_eq!(
        text,
        "# Model changes

Disposition: passed, with 0 breaking, 0 undetermined and 1 compatible differences.

## Compatible differences

- Changed quality `description` in `lock.sdf.json` at `/sdfObject/Lock/sdfProperty/credential/description`
"
    );
    let unit = LOCK.replacen(
        "\"type\": \"string\",",
        "\"type\": \"string\", \"unit\": \"Cel\",",
        1,
    );
    let text = changelog(LOCK, &unit, None, 3);
    assert!(text.contains("\n## Undetermined differences\n"), "{text}");
    assert!(!text.contains("## Breaking") && !text.contains("## Compatible"));
}

#[test]
fn changelogs_render_given_names_inertly() {
    // Renaming the credential Property removes it and adds the hostile name.
    let hostile = LOCK.replacen("\"credential\": {", r#""`*[x](y)*`<b>\n# heading": {"#, 1);
    let text = changelog(LOCK, &hostile, None, 1);
    assert!(
        text.contains(
            "\n- Added `sdfProperty` in `lock.sdf.json` at ``/sdfObject/Lock/sdfProperty/`*[x](y)*`<b>\\u000A# heading``\n"
        ),
        "{text}"
    );
    assert!(!text.lines().any(|line| line.starts_with("# heading")));
}

#[test]
fn markdown_applies_only_to_diff() {
    let tree = Tree::new();
    tree.write("lock.sdf.json", LOCK);
    let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(["check", "--format", "markdown", "lock.sdf.json"])
        .current_dir(tree.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}
