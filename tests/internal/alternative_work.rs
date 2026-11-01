//! Finite semantic work covers alternative collection indexing.
//! Follows ADR 0009: Bounded processing and reporting.
use esdieff::{
    Admission, AlternativeId, Catalogue, CheckRequest, Class, EffectiveRequest, Field,
    InspectRequest, Limits, Lookup, SampleKind, SampleRequest, SourceInput, View,
};

fn wide_choices() -> String {
    let mut text = String::from(
        "{\"info\":{},\"sdfProperty\":{\"Bad\":{\"multipleOf\":0}},\"sdfData\":{\"Many\":{\"type\":\"object\",\"properties\":{",
    );
    for index in 0..8 {
        if index > 0 {
            text.push(',');
        }
        text.push_str(&format!("\"p{index}\":{{\"minimum\":{index}}}"));
    }
    text.push_str("},\"sdfChoice\":{");
    for index in 0..8 {
        text.push_str(&format!("\"known{index}\":{{}},"));
    }
    for index in 0..128 {
        if index > 0 {
            text.push(',');
        }
        text.push_str(&format!("\"b{index}\":false"));
    }
    text.push_str("}},\"Known\":{\"minimum\":7}}}");
    text
}

#[test]
fn unavailable_choice_rows_consume_work_and_preserve_independent_views() {
    let text = wide_choices();
    // This ceiling admits source parsing and composition but leaves too little
    // semantic work to index the inherited branches and all unavailable rows.
    for limited in [true, false] {
        let limits = if limited {
            Limits::default().decoding_work(4752)
        } else {
            Limits::default()
        };
        let catalogue =
            Catalogue::with_limits(&[SourceInput::supplied("model", text.as_bytes())], limits)
                .unwrap();
        let source = catalogue.source_id("model").unwrap();
        let model = catalogue.effective(EffectiveRequest::new(source)).unwrap();
        let Lookup::Found(many) = model.definition(Class::Data, "Many") else {
            panic!("Many remains established")
        };
        let data = many.data_description().require().unwrap();
        if limited {
            match data.alternatives() {
                Field::Unavailable(reason) => {
                    assert_eq!(reason, "alternative collection work limit");
                }
                _ => panic!("a collection cannot be indexed without charging each row"),
            }
        } else {
            let rows = data.alternatives().require().unwrap();
            assert_eq!(rows.len(), 136);
            for (index, (id, branch)) in rows.enumerate() {
                if index < 8 {
                    assert_eq!(id, AlternativeId::Choice(&format!("known{index}")));
                    assert!(matches!(branch, Field::Present(_)));
                } else {
                    assert_eq!(id, AlternativeId::Choice(&format!("b{}", index - 8)));
                    assert!(matches!(branch, Field::Unavailable(_)));
                }
            }
        }
        assert!(!model.complete());
        let Lookup::Found(known) = model.definition(Class::Data, "Known") else {
            panic!("independent Known remains established")
        };
        assert_eq!(
            known
                .data_description()
                .require()
                .unwrap()
                .minimum()
                .require()
                .unwrap()
                .lexeme(),
            "7"
        );
        assert_eq!(
            catalogue.source(source).unwrap().raw_bytes(),
            text.as_bytes()
        );
        assert!(
            data.json()
                .source_json()
                .raw_json()
                .contains("\"b127\":false")
        );
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert!(!report.outcome().complete());
        assert!(report.outcome().errors() >= 1);
        assert!(report.to_json().contains("parameter.positive_step"));
    }
}

#[test]
fn public_generation_stays_strict_while_reports_render_partial_alternatives() {
    let text = br#"{"info":{},"sdfData":{"Mode":{"items":{"type":"array","nullable":false,"minimum":1,"maximum":2,"sdfChoice":{"partial":{},"known":{"type":"string","nullable":true}}}}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let source = catalogue.source_id("model").unwrap();
    let model = catalogue.effective(EffectiveRequest::new(source)).unwrap();
    let Lookup::Found(mode) = model.definition(Class::Data, "Mode") else {
        panic!("Mode")
    };
    for (id, data) in mode
        .data_description()
        .require()
        .unwrap()
        .items()
        .require()
        .unwrap()
        .alternatives()
        .require()
        .unwrap()
    {
        let data = data.require().unwrap();
        if id == AlternativeId::Choice("partial") {
            assert!(matches!(
                data.json().to_json(),
                Field::Unavailable("effective subtree unavailable")
            ));
        } else {
            assert!(matches!(data.json().to_json(), Field::Present(_)));
        }
    }
    let report = catalogue
        .inspect(InspectRequest::new(source, "/sdfData/Mode").view(View::Effective))
        .unwrap();
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let owner = wire["result"]["alternatives"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["owner"]["pointer"] == "/sdfData/Mode/items")
        .unwrap();
    let partial = owner["branches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"]["name"] == "partial")
        .unwrap();
    assert_eq!(partial["state"], "partial");
    assert_eq!(partial["complete"], false);
    assert_eq!(partial["json"]["minimum"], 1);
    assert_eq!(partial["json"]["maximum"], 2);
    assert_eq!(partial["json"]["nullable"], false);
}

#[derive(Clone, Copy, Debug)]
enum Alternatives {
    Enum,
    Choice,
}

/// A string Property `p` with `size` alternatives for `"v0"`, `"v1"` and so on.
fn alternatives_model(alternatives: Alternatives, size: usize) -> String {
    let collection = match alternatives {
        Alternatives::Enum => {
            let values: Vec<String> = (0..size).map(|index| format!("\"v{index}\"")).collect();
            format!("\"enum\":[{}]", values.join(","))
        }
        Alternatives::Choice => {
            let branches: Vec<String> = (0..size)
                .map(|index| format!("\"c{index}\":{{\"const\":\"v{index}\"}}"))
                .collect();
            format!("\"sdfChoice\":{{{}}}", branches.join(","))
        }
    };
    format!(
        "{{\"sdfObject\":{{\"o\":{{\"sdfProperty\":{{\"p\":{{\"type\":\"string\",{collection}}}}}}}}}}}"
    )
}

fn admission(
    alternatives: Alternatives,
    size: usize,
    sample: &str,
    limits: Limits,
) -> Option<Admission> {
    let model = alternatives_model(alternatives, size);
    let sample = format!("\"{sample}\"");
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied("model", model.as_bytes()),
            SourceInput::supplied("sample", sample.as_bytes()).sample(),
        ],
        limits,
    )
    .ok()?;
    let report = catalogue
        .check_sample(
            SampleRequest::new(
                catalogue.source_id("model")?,
                SampleKind::Property,
                "/sdfObject/o/sdfProperty/p",
            )
            .sample(catalogue.source_id("sample")?),
        )
        .ok()?;
    Some(report.sample_result()?.admission())
}

/// The smallest work allowance that admits the last alternative's value.
fn admitting_work(alternatives: Alternatives, size: usize) -> usize {
    let last = format!("v{}", size - 1);
    let admits = |work| {
        admission(
            alternatives,
            size,
            &last,
            Limits::default().decoding_work(work),
        ) == Some(Admission::Admitted)
    };
    let (mut low, mut high) = (1, 100_000_000);
    assert!(
        admits(high),
        "{size} alternatives admit within the largest allowance"
    );
    while low < high {
        let middle = low + (high - low) / 2;
        if admits(middle) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

#[test]
fn a_large_enum_decides_its_sample_within_the_default_allowance() {
    assert_eq!(
        admission(Alternatives::Enum, 200, "v199", Limits::default()),
        Some(Admission::Admitted)
    );
    assert_eq!(
        admission(Alternatives::Enum, 200, "x", Limits::default()),
        Some(Admission::Rejected)
    );
}

#[test]
fn sample_work_grows_linearly_with_the_alternative_count() {
    for alternatives in [Alternatives::Enum, Alternatives::Choice] {
        let work: Vec<usize> = [25, 50, 100, 200]
            .into_iter()
            .map(|size| admitting_work(alternatives, size))
            .collect();
        eprintln!("{alternatives:?} admitting work for 25, 50, 100 and 200 values: {work:?}");
        // Doubling the alternatives doubles the extra work when it is linear,
        // and quadruples it when it is quadratic.
        for sizes in work.windows(3) {
            let (first, second) = (sizes[1] - sizes[0], sizes[2] - sizes[1]);
            assert!(
                second <= first * 5 / 2,
                "{alternatives:?} work grows faster than linearly: {work:?}"
            );
        }
    }
}
