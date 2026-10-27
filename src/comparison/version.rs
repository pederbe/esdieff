//! A breaking revision should raise its `info` `version`, since RFC 9880 Section 3.1 recommends
//! a version that increases lexicographically over a model's life. A compared document pair
//! earns one warning when its bytes changed, it holds a breaking difference, and the later
//! version does not compare higher by Unicode code point order, following ADR 0044: Versions
//! of breaking revisions. The warning never changes a classification or the disposition.
use super::Revision;
use crate::{Catalogue, Field, JsonView, Source, SourceId};
use serde_json::{Value, json};

/// The code of the one warning a Revision comparison reports.
pub(crate) const CODE: &str = "comparison.version_not_increased";
pub(crate) const CITATION: &str = "RFC 9880 Section 3.1";
const MESSAGE: &str = "a document with a breaking difference has a version that is absent or does not compare higher than the earlier revision's";

/// A compared document pair with a breaking difference, acknowledged or not, whose later
/// `info` `version` is absent or does not compare higher than the earlier one by Unicode code
/// point order, following ADR 0044: Versions of breaking revisions.
pub struct VersionWarning<'a> {
    documents: (SourceId, SourceId),
    breaking: u32,
    earlier_version: &'a str,
    later_version: Option<&'a str>,
    /// The earlier version, and the later version or the nearest member that would hold it.
    locations: [Located<'a>; 2],
}
#[derive(Clone, Copy)]
struct Located<'a> {
    json: JsonView<'a>,
    pointer: &'static str,
}

/// A document's `info` `version`.
enum Version<'a> {
    Text(Located<'a>, &'a str),
    /// Absent, located at the nearest member that would hold it.
    Absent(Located<'a>),
}

/// The warning for one compared document pair whose walk found `breaking` breaking
/// differences, if it has one. Comparing the bytes and two members is linear in content the
/// Catalogues already admitted, so it charges nothing to the operation allowance.
pub(super) fn find<'a>(
    catalogues: [&'a Catalogue; 2],
    documents: (SourceId, SourceId),
    breaking: u32,
) -> Option<VersionWarning<'a>> {
    let earlier = catalogues[0].source(documents.0)?;
    let later = catalogues[1].source(documents.1)?;
    if breaking == 0 || earlier.raw_bytes() == later.raw_bytes() {
        return None;
    }
    let Version::Text(earlier, earlier_version) = version(earlier)? else {
        return None;
    };
    let (later, later_version) = match version(later)? {
        Version::Text(located, text) if text <= earlier_version => (located, Some(text)),
        Version::Text(..) => return None,
        Version::Absent(nearest) => (nearest, None),
    };
    Some(VersionWarning {
        documents,
        breaking,
        earlier_version,
        later_version,
        locations: [earlier, later],
    })
}

/// A document's `info` `version`, or `None` when it is not text or the structure holding it
/// is unavailable.
fn version(source: &Source) -> Option<Version<'_>> {
    let located = |json, pointer| Located { json, pointer };
    let root = source.json().present()?;
    let info = match root.get("info") {
        Field::Present(info) => info,
        Field::Absent => return Some(Version::Absent(located(root, ""))),
        Field::Unavailable(_) => return None,
    };
    match info.get("version") {
        Field::Present(json) => Some(Version::Text(
            located(json, "/info/version"),
            json.text().present()?,
        )),
        Field::Absent => Some(Version::Absent(located(info, "/info"))),
        Field::Unavailable(_) => None,
    }
}

impl<'a> VersionWarning<'a> {
    /// The stable warning code, `comparison.version_not_increased`.
    pub fn code(&self) -> &'static str {
        CODE
    }
    /// The earlier and later source IDs of the document pair.
    pub fn documents(&self) -> (SourceId, SourceId) {
        self.documents
    }
    pub fn earlier_version(&self) -> &'a str {
        self.earlier_version
    }
    /// Absent when the later document has no `info` `version`.
    pub fn later_version(&self) -> Option<&'a str> {
        self.later_version
    }
    /// The breaking differences of the document pair, acknowledged or not.
    pub fn breaking(&self) -> u32 {
        self.breaking
    }
    /// The source view of the version in one revision: the earlier document's
    /// `/info/version`, or the later document's `/info/version`, or its `/info` or root when
    /// that member is absent.
    pub fn location(&self, revision: Revision) -> JsonView<'a> {
        self.locations[revision.index()].json
    }
    /// The pointer of `location(revision)`.
    pub(crate) fn pointer(&self, revision: Revision) -> &'static str {
        self.locations[revision.index()].pointer
    }

    /// The warning's report condition. Its locations are source locations naming their
    /// revision, since both revisions number their sources from 1.
    pub(super) fn wire(&self) -> Value {
        let location = |revision: Revision| {
            let json = self.location(revision);
            let span = json.span();
            json!({"kind":"source","source_id":json.source_id().get(),"pointer":self.pointer(revision),
                "span":[span.start,span.end],"revision":revision.as_str()})
        };
        let mut arguments =
            json!({"earlier_version":self.earlier_version,"breaking_differences":self.breaking});
        if let Some(later) = self.later_version {
            arguments["later_version"] = json!(later);
        }
        json!({"code":CODE,"severity":"warning","basis":{"kind":"advice","citation":CITATION,"id":CODE},
            "message":MESSAGE,"primary":location(Revision::Later),"related":[location(Revision::Earlier)],
            "arguments":arguments})
    }
}
