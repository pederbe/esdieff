//! Exact admission controls follow ADR0025: Selected numeric samples.
use esdieff::{Admission, Catalogue, Limits, SampleKind, SampleRequest, SourceInput};
fn admission(qualities: &str, sample: &[u8]) -> (Admission, bool) {
    let model = format!("{{\"sdfProperty\":{{\"p\":{{{qualities}}}}}}}");
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model.as_bytes()),
        SourceInput::supplied("s", sample).sample(),
    ])
    .unwrap();
    let report = catalogue
        .check_sample(
            SampleRequest::new(
                catalogue.source_id("m").unwrap(),
                SampleKind::Property,
                "/sdfProperty/p",
            )
            .sample(catalogue.source_id("s").unwrap()),
        )
        .unwrap();
    (
        report.sample_result().unwrap().admission(),
        report.outcome().complete(),
    )
}
#[test]
fn exact_step_and_mathematical_integrality_do_not_round() {
    for (step, sample, expected) in [
        ("0.1", "0.3", Admission::Admitted),
        ("0.1", "0.30000000000000004", Admission::Rejected),
        ("0.2", "3", Admission::Admitted),
        ("2", "0.5", Admission::Rejected),
        ("0.5", "1e1000000", Admission::Admitted),
        ("0.5", "10e1000000", Admission::Admitted),
        ("10e1000000", "10e1000000", Admission::Admitted),
        ("10e-1000000", "100e-1000000", Admission::Admitted),
        ("1e1000000", "0e-1000000", Admission::Admitted),
        ("3", "1e1000000", Admission::Rejected),
        ("0.06", "0.12", Admission::Admitted),
        ("0.06", "0.15", Admission::Rejected),
        ("0.2", "-3", Admission::Admitted),
        ("0.2", "-0", Admission::Admitted),
    ] {
        assert_eq!(
            admission(
                &format!("\"type\":\"number\",\"multipleOf\":{step}"),
                sample.as_bytes()
            ),
            (expected, true),
            "sample {sample}, step {step}"
        );
    }
    for sample in ["10.0", "1e1", "-0", "9007199254740993"] {
        assert_eq!(
            admission("\"type\":\"integer\"", sample.as_bytes()),
            (Admission::Admitted, true)
        );
    }
    assert_eq!(
        admission("\"type\":\"integer\"", b"10.5"),
        (Admission::Rejected, true)
    );
}
#[test]
fn parser_numeric_work_is_not_reset_for_the_sample_comparison() {
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied(
                "m",
                br#"{"sdfProperty":{"p":{"type":"number","maximum":100}}}"#,
            ),
            SourceInput::supplied("s", b"1").sample(),
        ],
        Limits::default().numeric_work(4),
    )
    .unwrap();
    for _ in 0..2 {
        let report = catalogue
            .check_sample(
                SampleRequest::new(
                    catalogue.source_id("m").unwrap(),
                    SampleKind::Property,
                    "/sdfProperty/p",
                )
                .sample(catalogue.source_id("s").unwrap()),
            )
            .unwrap();
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Undetermined
        );
        assert!(!report.outcome().complete());
        assert_eq!(report.outcome().errors(), 0);
    }
}
#[test]
fn independent_bound_proof_survives_unrelated_model_characteristics() {
    assert_eq!(
        admission(
            r#""type":"number","maximum":100,"description":false,"readable":"bad","default":[],"properties":{"unused":{"sdfRef":"bad"}},"sdfRequired":["bad:ref"]"#,
            b"101"
        ),
        (Admission::Rejected, true)
    );
    assert_eq!(
        admission(r#""type":"number","maximum":100,"minimum":"bad""#, b"101"),
        (Admission::Rejected, false)
    );
    assert_eq!(
        admission(r#""type":"number","maximum":100,"const":101"#, b"101"),
        (Admission::Rejected, true)
    );
    assert_eq!(
        admission(
            r#""type":"number","maximum":100,"const":101,"title":"t""#,
            b"101"
        ),
        (Admission::Rejected, false)
    );
    // Undeclared extension vocabulary leaves the const comparison intact, following
    // ADR 0037: Undeclared extension vocabulary.
    assert_eq!(
        admission(
            r#""type":"number","maximum":100,"const":101,"unknownEqualityQuality":true"#,
            b"101"
        ),
        (Admission::Rejected, true)
    );
    assert_eq!(
        admission(
            r#""type":"number","maximum":100,"sdfChoice":{"other":{"maximum":150}}"#,
            b"101"
        ),
        (Admission::Admitted, true)
    );
}

#[test]
fn exact_bounds_keep_exclusivity_sign_and_wide_integer_order() {
    for (quality, sample, expected) in [
        (r#""minimum":-2"#, "-2", Admission::Admitted),
        (
            r#""minimum":-2"#,
            "-2.0000000000000001",
            Admission::Rejected,
        ),
        (r#""exclusiveMinimum":-2"#, "-2", Admission::Rejected),
        (
            r#""maximum":9007199254740992"#,
            "9007199254740993",
            Admission::Rejected,
        ),
        (r#""maximum":0"#, "-0", Admission::Admitted),
        (r#""exclusiveMaximum":0"#, "-0", Admission::Rejected),
        (r#""minimum":1e-1000000"#, "1e-1000000", Admission::Admitted),
        (
            r#""exclusiveMaximum":1e1000000"#,
            "1e999999",
            Admission::Admitted,
        ),
    ] {
        assert_eq!(
            admission(&format!(r#""type":"number",{quality}"#), sample.as_bytes()),
            (expected, true),
            "{quality}, {sample}"
        );
    }
}

#[test]
fn step_matches_a_small_integer_ratio_oracle_across_decimal_scales() {
    for coefficient in -12i128..=12 {
        for step in 1i128..=12 {
            for difference in -2i32..=2 {
                let (numerator, denominator) = if difference >= 0 {
                    (coefficient * 10i128.pow(difference as u32), step)
                } else {
                    (coefficient, step * 10i128.pow(difference.unsigned_abs()))
                };
                let expected = if numerator % denominator == 0 {
                    Admission::Admitted
                } else {
                    Admission::Rejected
                };
                for shared_scale in -1..=1 {
                    let sample = format!("{coefficient}e{}", shared_scale + difference);
                    let qualities =
                        format!(r#""type":"number","multipleOf":{step}e{shared_scale}"#);
                    assert_eq!(
                        admission(&qualities, sample.as_bytes()),
                        (expected, true),
                        "{qualities}, {sample}"
                    );
                }
            }
        }
    }
}

#[test]
fn bounds_and_step_share_the_same_numeric_remainder() {
    for (qualities, complete) in [
        (r#""type":"number","maximum":100"#, true),
        (r#""type":"number","multipleOf":3"#, true),
        (r#""type":"number","maximum":100,"multipleOf":3"#, false),
    ] {
        let model = format!(r#"{{"sdfProperty":{{"p":{{{qualities}}}}}}}"#);
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("m", model.as_bytes()),
                SourceInput::supplied("s", b"6").sample(),
            ],
            Limits::default().numeric_work(25),
        )
        .unwrap();
        let report = catalogue
            .check_sample(
                SampleRequest::new(
                    catalogue.source_id("m").unwrap(),
                    SampleKind::Property,
                    "/sdfProperty/p",
                )
                .sample(catalogue.source_id("s").unwrap()),
            )
            .unwrap();
        assert_eq!(report.outcome().complete(), complete, "{qualities}");
        assert_eq!(
            report.sample_result().unwrap().admission(),
            if complete {
                Admission::Admitted
            } else {
                Admission::Undetermined
            }
        );
        assert_eq!(report.outcome().errors(), 0);
    }
}
