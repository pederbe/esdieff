//! Lookups in the recorded content-format registries, shared by model checking and Revision
//! comparison so that both charge and record a lookup alike. Follows ADR 0029:
//! Content-format declarations and recorded registry guidance.
//!
//! Each lookup is a binary search that pays its fixed key width before reading a compared row
//! and records the registry as consulted once it reads one.
use super::table;
use crate::allowance::charge;
use crate::registry::{CONTENT_RECORDS, State};
use std::cmp::Ordering;

/// The recorded CoAP Content-Formats, as their bit in the registry and content-format states.
pub(crate) const NUMERIC: u8 = 1 << CONTENT_RECORDS.start;
/// The recorded media types.
pub(crate) const MEDIA: u8 = 1 << (CONTENT_RECORDS.start + 1);
/// The recorded HTTP content codings.
pub(crate) const CODINGS: u8 = 1 << (CONTENT_RECORDS.start + 2);
/// The flag of a recorded coding that RFC 9110 Section 8.4.1 makes equivalent to another,
/// such as `x-gzip`.
pub(super) const DEPRECATED: u8 = 2;

/// A recorded registry of names, compared without ASCII case.
#[derive(Clone, Copy)]
pub(super) enum Names {
    Media,
    Codings,
}
impl Names {
    pub(super) fn bit(self) -> u8 {
        match self {
            Self::Media => MEDIA,
            Self::Codings => CODINGS,
        }
    }
    fn rows(self) -> &'static [(&'static str, u8)] {
        match self {
            Self::Media => table::MEDIA,
            Self::Codings => table::CODINGS,
        }
    }
    /// The longest recorded key.
    fn key_bytes(self) -> usize {
        match self {
            Self::Media => 255,
            Self::Codings => 12,
        }
    }
}

/// The recorded CoAP tuple with this identifier, whose spelling is `bytes` long, or `Err` when
/// `work` runs out.
pub(super) fn numeric(
    number: u16,
    bytes: usize,
    registry: &mut State,
    work: &mut usize,
) -> Result<Option<table::Numeric>, ()> {
    let (mut low, mut high) = (0, table::NUMERIC.len());
    while low < high {
        if !charge(work, 1 + bytes + std::mem::size_of::<table::Numeric>()) {
            return Err(());
        }
        registry.consulted |= NUMERIC;
        let mid = low + (high - low) / 2;
        let row = table::NUMERIC[mid];
        match number.cmp(&row.id) {
            Ordering::Equal => return Ok(Some(row)),
            Ordering::Less => high = mid,
            Ordering::Greater => low = mid + 1,
        }
    }
    Ok(None)
}

/// The recorded flags of `text` among `names`, or `Err` when `work` runs out. ASCII folding and
/// comparison borrow the authored spelling without allocating.
pub(super) fn named(
    text: &str,
    names: Names,
    registry: &mut State,
    work: &mut usize,
) -> Result<Option<u8>, ()> {
    let rows = names.rows();
    let (mut low, mut high) = (0, rows.len());
    while low < high {
        if !charge(work, 1 + text.len() + names.key_bytes()) {
            return Err(());
        }
        registry.consulted |= names.bit();
        let mid = low + (high - low) / 2;
        let (key, flags) = rows[mid];
        match text
            .bytes()
            .map(|byte| byte.to_ascii_lowercase())
            .cmp(key.bytes())
        {
            Ordering::Equal => return Ok(Some(flags)),
            Ordering::Less => high = mid,
            Ordering::Greater => low = mid + 1,
        }
    }
    Ok(None)
}
