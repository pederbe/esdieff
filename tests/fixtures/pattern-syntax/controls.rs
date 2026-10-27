// Unicode syntax controls for the qualified private parser subset.
pub const INVALID: &[&str] = &[
    "[",
    "(",
    r"\q",
    "a{2,1}",
    r"\u{+61}",
    r"\u+041",
    r"[\u{+61}]",
    r"[\u+041]",
    r"(?<\u{+61}>b)",
    r"(?<x\u+061>b)",
    r"(?<a>b)\k<\u{+61}>",
    r"(?<xa>b)\k<x\u+061>",
    r"(?<a\u003E>b)",
    r"(?<a\u{3E}>b)",
    r"(?<a\u{00003e}>b)",
    r"(?<a>b)\k<a\u003E>",
    r"(?<a>b)\k<a\u{3E}>",
    r"(?<a>b)\k<a\u{00003e}>",
];

pub const VALID: &[&str] = &[
    "é",
    r"^\u0041$",
    r"^[\u{1F980}]$",
    r"^\uD83E\uDD80$",
    r"^\u003E$",
    r"^[\u{3E}]$",
    r"^(?<\u0061>b)\k<a>$",
    r"^(?<a0>b)\k<a\u0030>$",
    r"^\\u+041$",
    r"^[\\u{+61}]$",
    r"^\\k<a\u003E>$",
    r"^[(?<a\u003E>)]$",
    r"^(?<a>b)\k<a>\u003E$",
];

pub const UNSUPPORTED: &[&str] = &[
    r"(?<x>a)|(?<x>b)",
    r"(?<\u0078>a)|(?<x>b)",
    r"(?<\uD835\uDC9C>a)",
];

pub fn matching_only_capacity_controls() -> [String; 3] {
    [
        format!("(?<{}>a)", "x".repeat(125)),
        format!("(?<{}>a)", r"\u0078".repeat(125)),
        "()".repeat(256),
    ]
}
