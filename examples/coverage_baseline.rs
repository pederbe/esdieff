//! A bounded ordinary catalogue observer. Build before timing; verify before measure.
use esdieff::{
    AssessmentStatus, Catalogue, CheckRequest, Class, EffectiveRequest, Field, Lookup, SourceInput,
};
use std::{fs, hint::black_box, io::Read, path::Path, time::Instant};

const INITIAL_AREAS: [&str; 8] = [
    "document-envelope/1",
    "base-roles-names/1",
    "metadata-namespaces/1",
    "established-composition/1",
    "basic-parameters/1",
    "alternatives-requiredness/1",
    "extensions-policies/1",
    "advisory-four/1",
];

fn workload() -> Vec<(String, Vec<u8>)> {
    let mut rows = vec![
        (
            "device".into(),
            include_bytes!("../tests/fixtures/coverage/native-1.json").to_vec(),
        ),
        (
            "controller".into(),
            include_bytes!("../tests/fixtures/coverage/native-2.json").to_vec(),
        ),
        (
            "level".into(),
            include_bytes!("../tests/fixtures/coverage/native-3.json").to_vec(),
        ),
    ];
    for n in 0..29 {
        let value = serde_json::json!({"info":{"title":format!("Ordinary sensor {n}")},
            "sdfData":{"Reading":{"type":"number","nullable":false,"minimum":0,"maximum":100,"unit":"%"}},
            "sdfObject":{"Sensor":{"sdfRequired":["reading"],
                "sdfProperty":{"reading":{"sdfRef":"#/sdfData/Reading","writable":false}},
                "sdfAction":{"reset":{}},
                "sdfEvent":{"changed":{"sdfOutputData":{"sdfRef":"#/sdfData/Reading"}}}}}});
        rows.push((
            format!("sensor-{n:02}"),
            serde_json::to_vec(&value).unwrap(),
        ));
    }
    rows
}

fn process_memory() -> Option<String> {
    let mut bytes = String::new();
    fs::File::open("/proc/self/status")
        .ok()?
        .take(65536)
        .read_to_string(&mut bytes)
        .ok()?;
    Some(
        bytes
            .lines()
            .filter(|line| line.starts_with("VmHWM:") || line.starts_with("VmRSS:"))
            .collect::<Vec<_>>()
            .join("; "),
    )
}

fn main() {
    let rows = workload();
    let mode = std::env::args().nth(1).unwrap_or_else(|| "verify".into());
    if mode == "emit" {
        let destination = std::env::args()
            .nth(2)
            .expect("explicit workload output directory");
        fs::create_dir_all(&destination).unwrap();
        for (name, bytes) in &rows {
            fs::write(Path::new(&destination).join(format!("{name}.json")), bytes).unwrap();
        }
        println!("32 deterministic source documents emitted");
        return;
    }
    assert!(mode == "verify" || mode == "measure");
    let measuring = mode == "measure";
    let attempts: Vec<_> = rows
        .iter()
        .map(|(key, bytes)| SourceInput::supplied(key, bytes))
        .collect();
    let input_bytes: usize = rows.iter().map(|(_, bytes)| bytes.len()).sum();
    let before_memory = measuring.then(process_memory).flatten();
    let start = measuring.then(Instant::now);
    let catalogue = Catalogue::from_inputs(&attempts).unwrap();
    let acquired = measuring.then(Instant::now);
    let report = catalogue.check(CheckRequest::new()).unwrap();
    let checked = measuring.then(Instant::now);
    let inspected = catalogue
        .effective(EffectiveRequest::new(
            catalogue.source_id("controller").unwrap(),
        ))
        .unwrap();
    let effective = measuring.then(Instant::now);
    let json = report.to_json();
    let human = report.to_human();
    let rendered = measuring.then(Instant::now);
    assert_eq!(rows.len(), 32);
    assert!(
        report.outcome().complete(),
        "Initial assessment must complete before accepting a baseline"
    );
    assert_eq!(report.outcome().errors(), 0);
    assert_eq!(report.outcome().disposition(), esdieff::Disposition::Passed);
    for area in INITIAL_AREAS {
        assert!(matches!(
            report.assessment_area_status(area),
            Field::Present(AssessmentStatus::Assessed | AssessmentStatus::NotApplicable)
        ));
    }
    assert!(inspected.complete());
    let Lookup::Found(controller) = inspected.definition(Class::Object, "Controller") else {
        panic!("Controller missing")
    };
    let level = controller
        .children(Class::Property)
        .require()
        .unwrap()
        .find(|(name, _)| *name == "level")
        .unwrap()
        .1
        .require()
        .unwrap();
    let level = level.data_description().require().unwrap();
    assert_eq!(level.minimum().require().unwrap().lexeme(), "0");
    assert_eq!(level.maximum().require().unwrap().lexeme(), "80");
    black_box((&catalogue, &report, &inspected, &json, &human));
    let after_memory = measuring.then(process_memory).flatten();
    let phases = start.map(|start| serde_json::json!({
        "acquisition_and_parse_ns":acquired.unwrap().duration_since(start).as_nanos(),
        "check_ns":checked.unwrap().duration_since(acquired.unwrap()).as_nanos(),
        "independent_effective_ns":effective.unwrap().duration_since(checked.unwrap()).as_nanos(),
        "json_and_human_ns":rendered.unwrap().duration_since(effective.unwrap()).as_nanos(),
        "measured_total_ns":rendered.unwrap().duration_since(start).as_nanos(),
    }));
    println!(
        "{}",
        serde_json::json!({"mode":mode,"sources":32,"input_bytes":input_bytes,
        "limits":"default","request":"initial/catalogue","complete":report.outcome().complete(),
        "errors":report.outcome().errors(),"warnings":report.outcome().warnings(),"controller_min":"0","controller_max":"80",
        "json_bytes":json.len(),"human_bytes":human.len(),"phases":phases,
        "memory_before":before_memory,"memory_after":after_memory,
        "memory_scope":"whole process including workload, observer and retained catalogue/report/effective/JSON/human owners"})
    );
}
