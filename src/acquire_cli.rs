//! Explicit pinned acquisition remains private to the CLI under
//! ADR 0035: Explicit pinned document acquisition.
use crate::{
    cli_style::{self, ColorMode},
    document_cli::{Error, prepare_native_outputs},
};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    io::{IsTerminal, Write},
};
mod output;
mod receipt;
mod request;
mod transport;
use receipt::{Failure, Receipt};
use request::Request;
use transport::ResponseSource;

const RECEIPT_BYTES: usize = 64 * 1024;
const BODY_BYTES: usize = 4 * 1024 * 1024;
const HEADER_BYTES: usize = 64 * 1024;
const ORIGIN: &str = "https://raw.githubusercontent.com/";

/// `stdout_terminal` stands in for the process stdout so tests can choose a destination.
fn acquire(
    args: &[OsString],
    color: &mut ColorMode,
    stdout_terminal: bool,
    source: &ResponseSource,
    stdout: &mut dyn Write,
) -> Result<u8, Error> {
    let Some(request) = request::parse(args, color)? else {
        stdout
            .write_all(cli_style::acquire_help(color.for_terminal(stdout_terminal)).as_bytes())
            .map_err(|error| (4, format!("cannot emit acquisition help: {error}")))?;
        return Ok(0);
    };
    let (document, report) = prepare_native_outputs(request.document, request.report)?;
    let mut receipt = Receipt {
        request: &request,
        actual: None,
        count: None,
        committed: false,
        verified_file: None,
        staging: None,
        failures: Vec::new(),
    };
    let fetched = source.fetch(&request);
    #[cfg(test)]
    if let Some(change) = &source.faults.after_fetch {
        change();
    }
    match fetched {
        Ok(bytes) => {
            let actual: String = Sha256::digest(&bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            receipt.count = Some(bytes.len());
            if !actual.eq_ignore_ascii_case(request.expected) {
                receipt.failures.push(Failure::new(
                    "digest",
                    "complete response SHA-256 differs from requested digest",
                ));
            }
            receipt.actual = Some(actual);
            if receipt.failures.is_empty() {
                output::deliver(&bytes, &document, &report, &mut receipt, source);
            }
        }
        Err(failure) => receipt.failures.push(failure),
    }
    let mut rendered = if request.json {
        receipt.json()
    } else {
        // A receipt file is never a terminal, whatever stdout is.
        let style = color.for_terminal(report.path().is_none() && stdout_terminal);
        receipt.human_within(style, RECEIPT_BYTES)
    };
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    if rendered.len() > RECEIPT_BYTES {
        return Err((
            4,
            format!(
                "receipt exceeded reserved envelope. Document committed {} at {}",
                receipt.committed,
                receipt::label(request.document)
            ),
        ));
    }
    output::deliver_receipt(rendered.as_bytes(), &report, &document, stdout, source, &receipt)
        .map_err(|error| {
            (
                4,
                format!(
                    "acquisition receipt output failed: {error}. Verified document {} at {}",
                    if receipt.committed {
                        if receipt.verified_file.as_ref().is_some_and(|file| document.confirm(file).is_ok()) {
                            "exists"
                        } else {
                            "was committed in the preflight directory; requested path availability is unconfirmed"
                        }
                    } else {
                        "was not committed"
                    },
                    receipt::label(request.document)
                ),
            )
        })?;
    Ok(receipt.code())
}
pub(super) fn run(args: &[OsString], color: &mut ColorMode) -> Result<u8, Error> {
    acquire(
        args,
        color,
        std::io::stdout().is_terminal(),
        &ResponseSource::default(),
        &mut std::io::stdout().lock(),
    )
}

#[cfg(test)]
mod color_tests;
#[cfg(test)]
mod offline_tests;
#[cfg(test)]
mod receipt_tests;
#[cfg(test)]
mod support;
#[cfg(test)]
mod tests;
