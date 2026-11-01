use super::color_support;
use color_support::plain;
use esdieff::{
    Assessment, AuthorObject, AuthorRequest, Catalogue, HumanStyle, Limits, SourceInput,
};
const SOURCE: &str = r#"{"namespace":{"b":"https://example.com/base"},"defaultNamespace":"b","sdfObject":{"Sensor":{"sdfProperty":{"reading":{"type":"integer"}}}}}"#;
fn request(source: &str, limits: Limits) -> AuthorRequest {
    let catalogue =
        Catalogue::with_limits(&[SourceInput::supplied("base", source.as_bytes())], limits)
            .unwrap();
    let source = catalogue.source_id("base").unwrap();
    AuthorRequest::new(catalogue, "error recipe", "product", "failed")
        .unwrap()
        .namespace("reuse", "https://example.com/base")
        .unwrap()
        .object(AuthorObject::new("warning", source, "/sdfObject/Sensor", "reuse", false).unwrap())
        .unwrap()
}
#[test]
fn generation_limits_and_report_omissions_preserve_the_plain_facts() {
    let local = r#"{"sdfObject":{"Sensor":{}}}"#;
    for (source, limits, expected_code) in [
        (local, Limits::default(), "author.local_only_object"),
        (
            SOURCE,
            Limits::default().catalogue_bytes(SOURCE.len() + 40),
            "author.generation_limit",
        ),
    ] {
        let result = request(source, limits).author().unwrap();
        assert_eq!(result.generation(), esdieff::Generation::Unavailable);
        assert_eq!(
            result.outcome().disposition(),
            esdieff::Disposition::Undetermined
        );
        assert!(result.generated_document().is_none());
        let ordinary = result.to_human();
        let styled = result.to_human_with_style(HumanStyle::Ansi);
        assert_eq!(plain(styled.as_bytes()), ordinary);
        assert!(styled.contains("\x1b[1mGeneration:\x1b[0m \x1b[33munavailable\x1b[0m"));
        assert!(styled.contains(expected_code));
    }
    let source = SOURCE.replace(
        "\"type\":\"integer\"",
        "\"type\":\"integer\",\"minimum\":\"cold\",\"vendor:meaning\":{}",
    );
    let limit = 40_000;
    let result = request(
        &source,
        Limits::default().report_bytes(limit).diagnostic_bytes(1),
    )
    .checking(Assessment::Initial)
    .unwrap()
    .author()
    .unwrap();
    let ordinary = result.to_human();
    let styled = result.to_human_with_style(HumanStyle::Ansi);
    assert_eq!(plain(styled.as_bytes()), ordinary);
    assert!(ordinary.contains("Diagnostic details omitted:"));
    assert!(ordinary.contains("Errors: 1"));
    assert!(ordinary.len() <= limit);
    assert!(styled.len() <= limit);
    assert_eq!(result.outcome().disposition(), esdieff::Disposition::Failed);
    assert!(!ordinary.contains("error formal."));
}
