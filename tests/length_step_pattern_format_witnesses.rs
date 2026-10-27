//! Breaking witnesses and compatibility proofs for changed string lengths, `multipleOf`,
//! `pattern` and `format`. Follows ADR 0039: Client-side revision comparison.
use esdieff::{Catalogue, Classification, ComparisonRequest};
use serde_json::{Value, json};

mod comparison_support;
use comparison_support::{at, catalogue, client, revised, thing, witness};

/// A station whose `code` and `setpoint` only Clients supply and whose `name` and `reading`
/// only the Thing supplies.
const STATION: &str = r#"{"sdfObject": {"Station": {"sdfProperty": {
    "code": {"type": "string", "readable": false, "observable": false, "writable": true},
    "name": {"type": "string", "writable": false},
    "setpoint": {"type": "number", "readable": false, "observable": false, "writable": true},
    "reading": {"type": "number", "writable": false}
}}}}"#;

/// The station with each `(property, quality, value)` revision applied, where a null value
/// removes the quality.
fn station(revisions: &[(&str, &str, Value)]) -> String {
    revised(STATION, |document| {
        let properties = &mut document["sdfObject"]["Station"]["sdfProperty"];
        for (property, quality, value) in revisions {
            let property = properties[*property].as_object_mut().unwrap();
            if value.is_null() {
                property.remove(*quality);
            } else {
                property.insert((*quality).to_owned(), value.clone());
            }
        }
    })
}

/// The earlier and later stations with each revision applied to its own side.
fn stations(old: &[(&str, &str, Value)], new: &[(&str, &str, Value)]) -> (Catalogue, Catalogue) {
    (
        catalogue(&[("station", &station(old))]),
        catalogue(&[("station", &station(new))]),
    )
}

#[test]
fn a_tightened_length_of_client_data_breaks_with_a_string_just_across_the_tighter_bound() {
    let (earlier, later) = stations(
        &[
            ("code", "minLength", json!(2)),
            ("code", "maxLength", json!(8)),
        ],
        &[
            ("code", "minLength", json!(4)),
            ("code", "maxLength", json!(6)),
        ],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let minimum = at(&comparison, "/code/minLength");
    assert_eq!(minimum.classification(), Classification::Breaking);
    assert_eq!(witness(&minimum), client("\"aaa\""));
    let maximum = at(&comparison, "/code/maxLength");
    assert_eq!(maximum.classification(), Classification::Breaking);
    assert_eq!(witness(&maximum), client("\"aaaaaaa\""));
}

/// Both length bounds of `property`.
fn lengths(property: &str, minimum: u32, maximum: u32) -> [(&str, &'static str, Value); 2] {
    [
        (property, "minLength", json!(minimum)),
        (property, "maxLength", json!(maximum)),
    ]
}

#[test]
fn a_loosened_length_of_thing_data_breaks_and_one_of_client_data_is_compatible() {
    let earlier = [lengths("code", 4, 6), lengths("name", 4, 6)].concat();
    let later = [lengths("code", 2, 8), lengths("name", 2, 8)].concat();
    let (earlier, later) = stations(&earlier, &later);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    for (suffix, value) in [
        ("/name/minLength", "\"aaa\""),
        ("/name/maxLength", "\"aaaaaaa\""),
    ] {
        let difference = at(&comparison, suffix);
        assert_eq!(difference.classification(), Classification::Breaking);
        assert_eq!(witness(&difference), thing(value), "{suffix}");
    }
    for suffix in ["/code/minLength", "/code/maxLength"] {
        let difference = at(&comparison, suffix);
        assert_eq!(difference.classification(), Classification::Compatible);
        assert!(difference.witness().is_none());
    }
}

#[test]
fn a_tightened_length_of_thing_data_is_compatible() {
    let (earlier, later) = stations(&lengths("name", 2, 8), &lengths("name", 4, 6));
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Classification::Compatible);
    assert_eq!(comparison.differences().len(), 2);
}

#[test]
fn an_added_minimum_length_of_zero_is_compatible_in_either_direction() {
    let zero = |property| (property, "minLength", json!(0));
    let (earlier, later) = stations(&[], &[zero("code"), zero("name")]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Classification::Compatible);
    assert_eq!(comparison.differences().len(), 2);
}

#[test]
fn an_added_maximum_or_removed_minimum_length_breaks_with_the_length_just_across_it() {
    let (earlier, later) = stations(
        &[("name", "minLength", json!(3))],
        &[("code", "maxLength", json!(5))],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let code = at(&comparison, "/code/maxLength");
    assert_eq!(witness(&code), client("\"aaaaaa\""));
    let name = at(&comparison, "/name/minLength");
    assert_eq!(witness(&name), thing("\"aa\""));
}

/// The classification and reason code of the one difference at `suffix`, which has no witness.
fn unsettled(
    comparison: &esdieff::RevisionComparison<'_>,
    suffix: &str,
) -> (Classification, String) {
    let difference = at(comparison, suffix);
    assert!(difference.witness().is_none(), "{suffix}");
    let code = difference
        .reason()
        .map_or(String::new(), |reason| reason.code().to_owned());
    (difference.classification(), code)
}

const UNPROVEN: (Classification, &str) = (Classification::Undetermined, "comparison.unproven");

#[test]
fn a_length_candidate_another_constraint_excludes_gives_way_to_a_representative_value() {
    // The enum excludes `"aa"` in both revisions, and its first value meets only the minimum
    // length that the Client-supplied code gains and the Thing-supplied name loses.
    let listed = |property| (property, "enum", json!(["ab", "abcd"]));
    let minimum = |property| (property, "minLength", json!(3));
    let (earlier, later) = stations(
        &[listed("code"), listed("name"), minimum("name")],
        &[listed("code"), listed("name"), minimum("code")],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(
        witness(&at(&comparison, "/code/minLength")),
        client("\"ab\"")
    );
    assert_eq!(
        witness(&at(&comparison, "/name/minLength")),
        thing("\"ab\"")
    );
}

#[test]
fn a_length_change_whose_candidates_other_constraints_exclude_stays_undetermined() {
    // The pattern rejects every string of `a`, and a length beyond the value capabilities is
    // never generated.
    let digits = ("code", "pattern", json!("^[0-9]+$"));
    let huge = ("name", "maxLength", json!(1e30));
    let (earlier, later) = stations(
        &[digits.clone(), ("code", "minLength", json!(2)), huge],
        &[digits, ("code", "minLength", json!(4))],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    for suffix in ["/code/minLength", "/name/maxLength"] {
        let (classification, code) = unsettled(&comparison, suffix);
        assert_eq!((classification, code.as_str()), UNPROVEN, "{suffix}");
    }

    // The pattern rejects every string of `a` as Thing-supplied data loses its minimum too.
    let digits = ("name", "pattern", json!("^[0-9]+$"));
    let (earlier, later) = stations(
        &[digits.clone(), ("name", "minLength", json!(3))],
        &[digits],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let (classification, code) = unsettled(&comparison, "/name/minLength");
    assert_eq!((classification, code.as_str()), UNPROVEN);
}

fn step(property: &str, step: f64) -> (&str, &'static str, Value) {
    (property, "multipleOf", json!(step))
}

#[test]
fn a_coarser_step_breaks_client_data_and_a_finer_step_breaks_thing_data_with_the_finer_step() {
    let (earlier, later) = stations(
        &[step("setpoint", 0.25), step("reading", 0.5)],
        &[step("setpoint", 0.5), step("reading", 0.25)],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let setpoint = at(&comparison, "/setpoint/multipleOf");
    assert_eq!(setpoint.classification(), Classification::Breaking);
    assert_eq!(witness(&setpoint), client("0.25"));
    let reading = at(&comparison, "/reading/multipleOf");
    assert_eq!(reading.classification(), Classification::Breaking);
    assert_eq!(witness(&reading), thing("0.25"));

    let comparison = ComparisonRequest::new(&later, &earlier).compare().unwrap();
    assert_eq!(comparison.outcome(), Classification::Compatible);
    assert_eq!(comparison.differences().len(), 2);
}

#[test]
fn a_step_that_neither_divides_nor_is_divided_breaks_each_direction_with_its_own_step() {
    let (earlier, later) = stations(
        &[step("setpoint", 0.3), step("reading", 0.3)],
        &[step("setpoint", 0.5), step("reading", 0.5)],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(
        witness(&at(&comparison, "/setpoint/multipleOf")),
        client("0.3")
    );
    assert_eq!(
        witness(&at(&comparison, "/reading/multipleOf")),
        thing("0.5")
    );
}

#[test]
fn an_added_step_breaks_client_data_and_a_removed_one_thing_data_with_half_the_step() {
    let (earlier, later) = stations(&[step("reading", 0.5)], &[step("setpoint", 0.5)]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(
        witness(&at(&comparison, "/setpoint/multipleOf")),
        client("0.25")
    );
    assert_eq!(
        witness(&at(&comparison, "/reading/multipleOf")),
        thing("0.25")
    );
    // Half of a step of 3 is no integer, and 1 is no multiple of 3.
    let integer = |property| (property, "type", json!("integer"));
    let (earlier, later) = stations(
        &[
            integer("setpoint"),
            integer("reading"),
            step("reading", 3.0),
        ],
        &[
            integer("setpoint"),
            integer("reading"),
            step("setpoint", 3.0),
        ],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(
        witness(&at(&comparison, "/setpoint/multipleOf")),
        client("1")
    );
    assert_eq!(witness(&at(&comparison, "/reading/multipleOf")), thing("1"));
}

#[test]
fn a_removed_step_of_client_data_and_an_added_one_of_thing_data_are_compatible() {
    let (earlier, later) = stations(&[step("setpoint", 0.5)], &[step("reading", 0.5)]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Classification::Compatible);
    assert_eq!(comparison.differences().len(), 2);
}

#[test]
fn a_step_whose_candidates_another_constraint_excludes_stays_undetermined() {
    // Only 14 meets the constant, and it is a multiple of both steps.
    let pinned = |property| (property, "const", json!(14));
    let (earlier, later) = stations(
        &[
            pinned("setpoint"),
            step("setpoint", 2.0),
            pinned("reading"),
            step("reading", 2.0),
        ],
        &[
            pinned("setpoint"),
            step("setpoint", 7.0),
            pinned("reading"),
            step("reading", 7.0),
        ],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    for suffix in ["/setpoint/multipleOf", "/reading/multipleOf"] {
        let (classification, code) = unsettled(&comparison, suffix);
        assert_eq!((classification, code.as_str()), UNPROVEN, "{suffix}");
    }
}

#[test]
fn a_representative_value_witnesses_an_added_step_that_half_the_step_and_one_miss() {
    // Neither 3.5 nor 1 is integer data of at least 10, but the earlier lower bound is.
    let bounded = [
        ("setpoint", "type", json!("integer")),
        ("setpoint", "minimum", json!(10)),
    ];
    let (earlier, later) = stations(&bounded, &[&bounded[..], &[step("setpoint", 7.0)]].concat());
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(
        witness(&at(&comparison, "/setpoint/multipleOf")),
        client("10")
    );
}

fn pattern<'a>(property: &'a str, pattern: &str) -> (&'a str, &'static str, Value) {
    (property, "pattern", json!(pattern))
}

#[test]
fn an_added_pattern_breaks_client_data_and_a_removed_one_thing_data_with_a_representative_value() {
    let (earlier, later) = stations(
        &[pattern("name", "^[0-9]+$")],
        &[pattern("code", "^[0-9]+$")],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let code = at(&comparison, "/code/pattern");
    assert_eq!(code.classification(), Classification::Breaking);
    assert_eq!(witness(&code), client("\"\""));
    let name = at(&comparison, "/name/pattern");
    assert_eq!(name.classification(), Classification::Breaking);
    assert_eq!(witness(&name), thing("\"\""));

    let comparison = ComparisonRequest::new(&later, &earlier).compare().unwrap();
    assert_eq!(comparison.outcome(), Classification::Compatible);
    assert_eq!(comparison.differences().len(), 2);
}

#[test]
fn a_changed_pattern_breaks_only_when_a_representative_value_separates_the_revisions() {
    // A one-letter code matches only the earlier pattern, while the empty name that the
    // later revision represents matches neither.
    let short = ("code", "minLength", json!(1));
    let (earlier, later) = stations(
        &[
            short.clone(),
            pattern("code", "^[a-z]+$"),
            pattern("name", "^[a-z]+$"),
        ],
        &[
            short,
            pattern("code", "^[0-9]+$"),
            pattern("name", "^[0-9]+$"),
        ],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(witness(&at(&comparison, "/code/pattern")), client("\"a\""));
    let (classification, code) = unsettled(&comparison, "/name/pattern");
    assert_eq!((classification, code.as_str()), UNPROVEN);
}

fn format<'a>(property: &'a str, format: &str) -> (&'a str, &'static str, Value) {
    (property, "format", json!(format))
}

#[test]
fn an_added_format_breaks_client_data_and_a_removed_one_thing_data_with_a_representative_value() {
    let (earlier, later) = stations(&[format("name", "uuid")], &[format("code", "date-time")]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let code = at(&comparison, "/code/format");
    assert_eq!(code.classification(), Classification::Breaking);
    assert_eq!(witness(&code), client("\"\""));
    let name = at(&comparison, "/name/format");
    assert_eq!(name.classification(), Classification::Breaking);
    assert_eq!(witness(&name), thing("\"\""));

    let comparison = ComparisonRequest::new(&later, &earlier).compare().unwrap();
    assert_eq!(comparison.outcome(), Classification::Compatible);
    assert_eq!(comparison.differences().len(), 2);
}

#[test]
fn a_changed_format_breaks_with_a_conventional_value_of_the_revision_that_admits_it() {
    let (earlier, later) = stations(
        &[format("code", "date"), format("name", "date")],
        &[format("code", "date-time"), format("name", "date-time")],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(
        witness(&at(&comparison, "/code/format")),
        client("\"1970-01-01\"")
    );
    assert_eq!(
        witness(&at(&comparison, "/name/format")),
        thing("\"1970-01-01T00:00:00Z\"")
    );
}

#[test]
fn a_uri_that_becomes_a_uri_reference_widens_and_the_reverse_narrows() {
    // Every URI is a URI-reference, and `""` is a URI-reference but no URI.
    let formats = |name: &str| [format("code", name), format("name", name)];
    let (earlier, later) = stations(&formats("uri"), &formats("uri-reference"));
    for (old, new, client_data, thing_data) in [
        (&earlier, &later, None, Some(thing("\"\""))),
        (&later, &earlier, Some(client("\"\"")), None),
    ] {
        let comparison = ComparisonRequest::new(old, new).compare().unwrap();
        for (suffix, expected) in [("/code/format", client_data), ("/name/format", thing_data)] {
            let difference = at(&comparison, suffix);
            let classification = match expected {
                Some(_) => Classification::Breaking,
                None => Classification::Compatible,
            };
            assert_eq!(difference.classification(), classification, "{suffix}");
            assert_eq!(difference.witness().map(|_| witness(&difference)), expected);
        }
    }
}

#[test]
fn a_format_whose_candidates_another_constraint_excludes_stays_undetermined() {
    // The pattern rejects `""` as the Client-supplied code gains a format and the
    // Thing-supplied name loses one.
    let digits = |property| pattern(property, "^[0-9]+$");
    let (earlier, later) = stations(
        &[digits("code"), digits("name"), format("name", "uuid")],
        &[digits("code"), digits("name"), format("code", "uuid")],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    for suffix in ["/code/format", "/name/format"] {
        let (classification, code) = unsettled(&comparison, suffix);
        assert_eq!((classification, code.as_str()), UNPROVEN, "{suffix}");
    }
}

#[test]
fn a_format_that_sample_value_checking_does_not_check_is_not_classified() {
    // `email` is outside the six checked formats, and a format beside no string type is not
    // checked at all.
    let untyped = || ("setpoint", "type", Value::Null);
    let (earlier, later) = stations(
        &[untyped()],
        &[
            format("code", "email"),
            untyped(),
            format("setpoint", "date"),
        ],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    for suffix in ["/code/format", "/setpoint/format"] {
        let (classification, code) = unsettled(&comparison, suffix);
        assert_eq!(
            (classification, code.as_str()),
            (Classification::Undetermined, "comparison.not_classified"),
            "{suffix}"
        );
    }
}

#[test]
fn both_reports_show_length_step_pattern_and_format_witnesses_alike() {
    let (earlier, later) = stations(
        &[step("reading", 0.5), format("name", "date")],
        &[
            ("code", "minLength", json!(2)),
            step("reading", 0.25),
            pattern("code", "^[a-z]+$"),
        ],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Classification::Breaking);
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    let human = comparison.to_human();
    let station = "/sdfObject/Station/sdfProperty";
    for (quality, raw, direction, earlier, later, absent) in [
        (
            "code/minLength",
            "\"a\"",
            "client",
            "admitted",
            "rejected",
            [" (absent)", ""],
        ),
        (
            "code/pattern",
            "\"\"",
            "client",
            "admitted",
            "rejected",
            [" (absent)", ""],
        ),
        (
            "reading/multipleOf",
            "0.25",
            "thing",
            "rejected",
            "admitted",
            ["", ""],
        ),
        (
            "name/format",
            "\"\"",
            "thing",
            "rejected",
            "admitted",
            ["", " (absent)"],
        ),
    ] {
        let pointer = format!("{station}/{quality}");
        let difference = wire["result"]["differences"]
            .as_array()
            .unwrap()
            .iter()
            .find(|difference| difference["earlier"]["location"]["pointer"] == json!(pointer))
            .unwrap_or_else(|| panic!("{pointer}"));
        assert_eq!(difference["classification"], "breaking");
        assert_eq!(
            difference["witness"],
            json!({"direction": format!("{direction}_supplied"), "raw_json": raw,
                "admission": {"earlier": earlier, "later": later}})
        );
        let name = quality.split('/').next_back().unwrap();
        let [earlier_absent, later_absent] = absent;
        let block = format!(
            "  changed quality {name} ({direction}-supplied data)\n    witness {raw} for {direction}-supplied data: earlier {earlier}, later {later}\n    earlier station {pointer}{earlier_absent}\n    later station {pointer}{later_absent}\n"
        );
        assert!(human.contains(&block), "{block}\n{human}");
    }
}

/// The code with a shared `maxLength` beside a choice of these alternatives.
fn chosen(maximum: u32, alternatives: Value) -> [(&'static str, &'static str, Value); 2] {
    [
        ("code", "maxLength", json!(maximum)),
        ("code", "sdfChoice", alternatives),
    ]
}

#[test]
fn a_shared_length_beside_a_choice_is_judged_on_the_alternatives_that_inherit_it() {
    // `word` inherits the shared maximum length and `short` states its own, so lowering the
    // shared maximum narrows `word` alone.
    let alternatives = || json!({"short": {"maxLength": 2}, "word": {"description": "Any word"}});
    let (earlier, later) = stations(&chosen(8, alternatives()), &chosen(6, alternatives()));
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let maximum = at(&comparison, "/code/maxLength");
    assert_eq!(maximum.classification(), Classification::Breaking);
    assert_eq!(witness(&maximum), client("\"aaaaaaa\""));

    // When every alternative states its own maximum, the shared one constrains nothing.
    let alternatives = || json!({"short": {"maxLength": 2}, "word": {"maxLength": 9}});
    let (earlier, later) = stations(&chosen(8, alternatives()), &chosen(6, alternatives()));
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let maximum = at(&comparison, "/code/maxLength");
    assert_eq!(maximum.classification(), Classification::Compatible);
}
