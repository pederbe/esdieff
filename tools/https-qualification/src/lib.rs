//! Candidate exercise only. The application continues to use its existing transport.
use sha2::{Digest, Sha256};
use std::{io::Read, sync::Arc, time::Duration};
use ureq::{Agent, tls::TlsConfig};

pub const BODY_BYTES: usize = 4 * 1024 * 1024;
pub const HEADER_BYTES: usize = 64 * 1024;
pub const ORIGIN: &str = "https://raw.githubusercontent.com/";

#[derive(Debug)]
pub enum Failure {
    Transport(ureq::Error),
    Body(std::io::Error),
    Status(u16),
    Encoding,
    BodyLimit,
    Digest,
    Origin,
}

/// Explicit provider, independent of process-wide defaults. No client credentials.
pub fn tls_config() -> TlsConfig {
    TlsConfig::builder()
        .unversioned_rustls_crypto_provider(Arc::new(rustls_provider_rustcrypto_pq::provider()))
        .build()
}

fn config(tls: TlsConfig, connect: Duration, whole: Duration) -> ureq::config::Config {
    Agent::config_builder()
        .tls_config(tls)
        .proxy(None)
        .https_only(true)
        .http_status_as_error(false)
        .max_redirects(0)
        .max_idle_connections(0)
        .max_response_header_size(HEADER_BYTES)
        .input_buffer_size(16 * 1024)
        .output_buffer_size(16 * 1024)
        .timeout_resolve(Some(connect))
        .timeout_connect(Some(connect))
        .timeout_global(Some(whole))
        .build()
}

/// Does not write files. Returns only the exact complete digest-verified body.
///
/// Known qualification gaps: the two connection phase budgets are independent,
/// and ureq's header cap is per response rather than cumulative across 1xx.
/// These are deliberately exercised in tests, not presented as settled behavior.
pub fn fetch(uri: &str, expected: &str) -> Result<Vec<u8>, Failure> {
    if !uri.starts_with(ORIGIN) {
        return Err(Failure::Origin);
    }
    let agent = config(
        tls_config(),
        Duration::from_secs(10),
        Duration::from_secs(30),
    )
    .new_agent();
    receive(&agent, uri, expected)
}

fn receive(agent: &Agent, uri: &str, expected: &str) -> Result<Vec<u8>, Failure> {
    let mut response = agent
        .get(uri)
        .header("Accept-Encoding", "identity")
        .call()
        .map_err(Failure::Transport)?;
    if response.status().as_u16() != 200 {
        return Err(Failure::Status(response.status().as_u16()));
    }
    for encoding in response.headers().get_all("content-encoding") {
        if !encoding.as_bytes().eq_ignore_ascii_case(b"identity") {
            return Err(Failure::Encoding);
        }
    }
    if response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .is_some_and(|n| n > BODY_BYTES as u64)
    {
        return Err(Failure::BodyLimit);
    }
    let mut bytes = Vec::new();
    let mut reader = response.body_mut().as_reader();
    let mut block = [0; 16 * 1024];
    loop {
        // Read at most one byte beyond the ceiling, without retaining it.
        let allowance = block.len().min(BODY_BYTES - bytes.len() + 1);
        let n = reader
            .read(&mut block[..allowance])
            .map_err(Failure::Body)?;
        if n == 0 {
            break;
        }
        if n > BODY_BYTES - bytes.len() {
            return Err(Failure::BodyLimit);
        }
        bytes.extend_from_slice(&block[..n]);
    }
    let actual = sha256(&bytes);
    if actual != expected {
        return Err(Failure::Digest);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;

pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
