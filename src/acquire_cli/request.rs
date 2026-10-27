use super::{ORIGIN, RECEIPT_BYTES};
use crate::{
    cli_style::ColorMode,
    document_cli::{Error, usage},
};
use std::{
    ffi::{OsStr, OsString},
    path::Path,
};

pub(super) struct Request<'a> {
    pub(super) repository: &'a str,
    pub(super) commit: &'a str,
    pub(super) path: &'a str,
    pub(super) expected: &'a str,
    pub(super) document: &'a OsStr,
    pub(super) report: &'a OsStr,
    pub(super) uri: String,
    pub(super) json: bool,
}
fn text<'a>(value: &'a OsStr, flag: &str) -> Result<&'a str, Error> {
    value
        .to_str()
        .ok_or_else(|| usage(format!("{flag} requires UTF-8 metadata")))
}
fn hex(value: &str, size: usize) -> bool {
    value.len() == size && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(&mut encoded, "%{byte:02X}").expect("String write");
        }
    }
    encoded
}
pub(super) fn parse<'a>(
    args: &'a [OsString],
    color: &mut ColorMode,
) -> Result<Option<Request<'a>>, Error> {
    let size = args
        .iter()
        .try_fold(0usize, |total, value| {
            total.checked_add(value.as_encoded_bytes().len())
        })
        .ok_or_else(|| usage("acquisition request size overflow"))?;
    if size > 64 * 1024 {
        return Err(usage("acquisition request exceeds 64 KiB"));
    }
    let mut values = [None; 7];
    let mut index = 1;
    let mut help = false;
    while let Some(flag) = args.get(index) {
        index += 1;
        if color.consume(flag, args, &mut index)? {
            continue;
        }
        if flag == "--help" || flag == "-h" {
            if help {
                return Err(usage("repeated acquisition help option"));
            }
            help = true;
            continue;
        }
        let slot = match flag.to_str() {
            Some("--github") => 0,
            Some("--commit") => 1,
            Some("--path") => 2,
            Some("--sha256") => 3,
            Some("--document-output") => 4,
            Some("--format") => 5,
            Some("--report-output") => 6,
            _ => return Err(usage("unknown acquisition option or positional input")),
        };
        if values[slot].is_some() {
            return Err(usage("repeated acquisition option"));
        }
        values[slot] = Some(
            args.get(index)
                .ok_or_else(|| usage("acquisition option requires a value"))?
                .as_os_str(),
        );
        index += 1;
    }
    if help {
        return Ok(None);
    }
    let repository = text(
        values[0].ok_or_else(|| usage("acquisition requires --github"))?,
        "--github",
    )?;
    let commit = text(
        values[1].ok_or_else(|| usage("acquisition requires --commit"))?,
        "--commit",
    )?;
    let path = text(
        values[2].ok_or_else(|| usage("acquisition requires --path"))?,
        "--path",
    )?;
    let expected = text(
        values[3].ok_or_else(|| usage("acquisition requires --sha256"))?,
        "--sha256",
    )?;
    let document = values[4].ok_or_else(|| usage("acquisition requires --document-output"))?;
    let report = values[6].unwrap_or_else(|| OsStr::new("-"));
    let json = match values[5].map(|value| text(value, "--format")).transpose()? {
        None | Some("human") => false,
        Some("json") => true,
        _ => return Err(usage("acquisition format must be human or json")),
    };
    let repository_parts: Vec<_> = repository.split('/').collect();
    if repository_parts.len() != 2
        || repository_parts.iter().any(|part| {
            part.is_empty()
                || matches!(*part, "." | "..")
                || !part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-._".contains(&byte))
        })
    {
        return Err(usage("--github must name one public OWNER/REPOSITORY"));
    }
    if !hex(commit, 40) {
        return Err(usage("commit must be a complete 40-hex identity"));
    }
    if !hex(expected, 64) {
        return Err(usage("expected SHA-256 must be a complete 64-hex digest"));
    }
    if path
        .chars()
        .any(|character| character.is_control() || matches!(character, '\\' | '?' | '#'))
        || path
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
        || Path::new(path).is_absolute()
    {
        return Err(usage(
            "repository path must contain relative, nonempty ordinary segments",
        ));
    }
    let encoded_path_bytes = path
        .bytes()
        .map(|byte| {
            if byte == b'/' || byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                1
            } else {
                3
            }
        })
        .sum::<usize>();
    if ORIGIN.len() + repository.len() + commit.len() + 2 + encoded_path_bytes > 2048 {
        return Err(usage(
            "acquisition source metadata exceeds its byte ceiling",
        ));
    }
    let uri = format!(
        "{ORIGIN}{repository}/{commit}/{}",
        path.split('/').map(encode).collect::<Vec<_>>().join("/")
    );
    if uri.len() > 2048 || commit.len() > 128 || expected.len() > 256 {
        return Err(usage(
            "acquisition source metadata exceeds its byte ceiling",
        ));
    }
    // Six escape bytes per source byte and 22 per native byte retain both
    // labels and native arrays. The staging basename adds at most 96 bytes.
    let source = repository.len() + commit.len() + path.len() + expected.len() + uri.len();
    let native = document.as_encoded_bytes().len();
    let reserve = source
        .checked_mul(6)
        .and_then(|value| {
            native
                .checked_add(96)
                .and_then(|native| native.checked_mul(44))
                .and_then(|native| value.checked_add(native))
        })
        .and_then(|value| value.checked_add(12 * 1024))
        .ok_or_else(|| usage("acquisition receipt reserve overflow"))?;
    if reserve > RECEIPT_BYTES {
        return Err(usage(
            "acquisition identity cannot fit complete receipt envelope",
        ));
    }
    Ok(Some(Request {
        repository,
        commit,
        path,
        expected,
        document,
        report,
        uri,
        json,
    }))
}
