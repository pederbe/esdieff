use serde_json::Value;

pub const GENERIC: &str = "rfc3986-uri-syntax/1";
pub const RELATIVE: &str = "rfc3986-reference-expression/1";
pub const HTTP: &str = "rfc9110-http-identity/1";
pub const HTTPS: &str = "rfc9110-https-identity/1";
pub const BARE: &str = "rfc9562-bare-text/1";
pub const OPAQUE: &str = "uuid-opaque-value/1";
pub const RULES: [(&str, &str); 6] = [
    (GENERIC, "3986"),
    (RELATIVE, "3986"),
    (HTTP, "9110"),
    (HTTPS, "9110"),
    (BARE, "9562"),
    (OPAQUE, "9562"),
];

// A reference without a scheme consults the relative-expression rule.
pub fn relative_rule_ids() -> &'static [&'static str] {
    &[RELATIVE]
}

pub fn assert_wire_and_human_use(wire: &Value, human: &str, used: &[&str]) {
    let baseline = wire.get("baseline").unwrap().as_object().unwrap();
    // .get distinguishes the required sample field from absence. Indexing does not.
    let records = baseline
        .get("format_knowledge")
        .expect("sample field present")
        .as_array()
        .expect("established usage owner: array, including empty");
    assert_eq!(records.len(), used.len());
    assert!(records.len() <= 6);
    let mut ids = std::collections::HashSet::new();
    for record in records {
        let id = record.get("id").unwrap().as_str().unwrap();
        assert!(used.contains(&id) && ids.insert(id));
        let rfc = RULES.iter().find(|(known, _)| *known == id).unwrap().1;
        assert_eq!(record.get("rfc").unwrap().as_str().unwrap(), rfc);
        for (key, max) in [("id", 96), ("sections", 64), ("scope", 256)] {
            let text = record.get(key).unwrap().as_str().unwrap();
            assert!(!text.is_empty() && text.len() <= max);
        }
    }
    for (id, _) in RULES {
        assert_eq!(human.contains(id), used.contains(&id), "human rule {id}");
    }
    let interpretations = baseline.get("interpretations").unwrap().as_array().unwrap();
    for (rule, interpretation) in [(BARE, "uuid-bare-json/1"), (OPAQUE, "uuid-opaque-value/1")] {
        let expected = used.contains(&rule);
        assert_eq!(
            interpretations
                .iter()
                .any(|v| v.as_str() == Some(interpretation)),
            expected
        );
        // Existing interpretation labels disambiguate opacity facts from a
        // format record with the same ID. Whole human prose is not frozen.
        assert_eq!(
            human
                .lines()
                .any(|line| line.strip_prefix("Interpretation: ") == Some(interpretation)),
            expected,
            "human interpretation {interpretation}"
        );
    }
}
