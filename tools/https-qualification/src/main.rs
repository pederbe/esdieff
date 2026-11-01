use esdieff_https_qualification::{fetch, sha256};
fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        eprintln!("usage: esdieff-https-qualification HTTPS_URI EXPECTED_SHA256");
        return std::process::ExitCode::from(2);
    }
    match fetch(&args[0], &args[1]) {
        Ok(bytes) => {
            println!("verified {} bytes sha256={}", bytes.len(), sha256(&bytes));
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("acquisition failed: {error:?}");
            std::process::ExitCode::FAILURE
        }
    }
}
