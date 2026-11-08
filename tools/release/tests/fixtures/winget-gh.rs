#![forbid(unsafe_code)]
use std::{
    env, fs,
    io::{self, Read},
    path::PathBuf,
};

fn quoted(s: &str) -> String {
    format!(
        "\"{}\"",
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
    )
}

fn main() {
    let root = PathBuf::from(env::var_os("FIXTURE_ROOT").unwrap());
    let args: Vec<_> = env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("submit") {
        assert!(!root.join("created").exists(), "created a duplicate PR");
        for entry in fs::read_dir(args.last().unwrap()).unwrap() {
            let text = fs::read_to_string(entry.unwrap().path()).unwrap();
            assert!(!text.contains("yaml-language-server"));
            assert!(text.contains("ManifestVersion: 1.28.0"));
        }
        fs::write(root.join("created"), "yes").unwrap();
        return;
    }
    if args.iter().any(|a| a == "search/issues") {
        if root.join("created").exists() {
            println!(
                "{{\"incomplete_results\":false,\"total_count\":1,\"items\":[{{\"number\":7,\"title\":\"Publisher.Tool version 1.2.3\"}}]}}"
            );
        } else {
            println!("{{\"incomplete_results\":false,\"total_count\":0,\"items\":[]}}");
        }
    } else if args.iter().any(|a| a == "user") {
        println!("{{\"login\":\"fixture\"}}");
    } else if args.iter().any(|a| a.contains("/check-runs")) {
        println!(
            "[{{\"check_runs\":[{{\"id\":42,\"name\":\"license/cla\",\"conclusion\":\"success\",\"app\":{{\"slug\":\"microsoft-github-policy-service\"}}}}]}}]"
        );
    } else if args.iter().any(|a| a.contains("/files?")) {
        println!(
            "[{{\"filename\":\"manifests/p/Publisher/Tool/1.2.3/Publisher.Tool.yaml\"}},{{\"filename\":\"manifests/p/Publisher/Tool/1.2.3/Publisher.Tool.installer.yaml\"}},{{\"filename\":\"manifests/p/Publisher/Tool/1.2.3/Publisher.Tool.locale.en-US.yaml\"}}]"
        );
    } else if let Some(path) = args.iter().find(|a| a.contains("?ref=")) {
        let name = path.split('?').next().unwrap().rsplit('/').next().unwrap();
        let text = fs::read_to_string(root.join("manifests").join(name)).unwrap();
        let kind = text
            .lines()
            .find_map(|line| line.strip_prefix("ManifestType: "))
            .unwrap();
        let mut lines: Vec<_> = text.lines().filter(|line| !line.starts_with('#')).collect();
        lines.reverse();
        print!(
            "# Created using wingetcreate 1.12.13.0\r\n# yaml-language-server: $schema=https://aka.ms/winget-manifest.{kind}.1.28.0.schema.json\r\n\r\n{}\r\n",
            lines.join("\r\n")
        );
    } else if args.iter().any(|a| a.ends_with("pulls/7")) {
        if args.iter().any(|a| a == "PATCH") {
            if root.join("fail-patch").exists() {
                eprintln!("fixture: description update failed");
                std::process::exit(1);
            }
            let mut input = String::new();
            io::stdin().read_to_string(&mut input).unwrap();
            let body = input
                .trim()
                .strip_prefix("{\"body\":")
                .unwrap()
                .strip_suffix('}')
                .unwrap();
            fs::write(root.join("body-json"), body).unwrap();
        }
        let body = fs::read_to_string(root.join("body-json"))
            .unwrap_or_else(|_| quoted(&fs::read_to_string(root.join("template")).unwrap()));
        println!(
            "{{\"body\":{body},\"state\":\"open\",\"user\":{{\"login\":\"fixture\"}},\"head\":{{\"sha\":\"abc\"}}}}"
        );
    } else if args.iter().any(|a| a.contains("/contents/")) {
        eprintln!("gh: Not Found (HTTP 404)");
        std::process::exit(1);
    } else {
        panic!("unexpected gh invocation: {args:?}");
    }
}
