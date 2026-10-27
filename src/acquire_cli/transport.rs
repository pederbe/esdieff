use super::{BODY_BYTES, Failure, HEADER_BYTES, ORIGIN, Request};
use curl::easy::{Easy2, Handler, HttpVersion, List, NetRc, WriteError};
use std::time::Duration;

struct Response {
    bytes: Vec<u8>,
    headers: usize,
    status: Option<u16>,
    failure: Option<Failure>,
}
impl Response {
    fn fail(&mut self, phase: &'static str, message: impl AsRef<str>) -> bool {
        if self.failure.is_none() {
            self.failure = Some(Failure::new(phase, message));
        }
        false
    }
}
impl Handler for Response {
    fn header(&mut self, line: &[u8]) -> bool {
        let Some(count) = self
            .headers
            .checked_add(line.len())
            .filter(|count| *count <= HEADER_BYTES)
        else {
            return self.fail("limits", "response headers exceed 64 KiB");
        };
        self.headers = count;
        if line.starts_with(b"HTTP/") {
            self.status = line
                .split(|byte| byte.is_ascii_whitespace())
                .nth(1)
                .and_then(|value| std::str::from_utf8(value).ok())
                .and_then(|value| value.parse().ok());
            if self.status.is_none() {
                return self.fail("http", "unrecognized HTTP status");
            }
        } else if let Some(colon) = line.iter().position(|byte| *byte == b':') {
            let (name, value) = line.split_at(colon);
            if name.eq_ignore_ascii_case(b"content-encoding")
                && !value[1..].trim_ascii().eq_ignore_ascii_case(b"identity")
            {
                return self.fail("http", "unsupported response content encoding");
            }
            if name.eq_ignore_ascii_case(b"content-length")
                && let Ok(value) = std::str::from_utf8(value[1..].trim_ascii())
                && value
                    .parse::<u64>()
                    .is_ok_and(|size| size > BODY_BYTES as u64)
            {
                return self.fail("limits", "response body exceeds 4 MiB");
            }
        } else if line == b"\r\n" && self.status.is_some_and(|status| status > 200) {
            return self.fail(
                "http",
                format!("HTTP {} is not an accepted response", self.status.unwrap()),
            );
        }
        true
    }
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        if self.failure.is_some() {
            return Ok(0);
        }
        let Some(size) = self
            .bytes
            .len()
            .checked_add(data.len())
            .filter(|size| *size <= BODY_BYTES)
        else {
            self.fail("limits", "response body exceeds 4 MiB");
            return Ok(0);
        };
        self.bytes.reserve_exact(size - self.bytes.len());
        self.bytes.extend_from_slice(data);
        Ok(data.len())
    }
}

pub(super) struct ResponseSource {
    pub(super) connect: Duration,
    pub(super) whole: Duration,
    #[cfg(test)]
    pub(super) target: Option<String>,
    #[cfg(test)]
    pub(super) observed: std::cell::Cell<usize>,
    #[cfg(test)]
    pub(super) faults: TestFaults,
}
impl Default for ResponseSource {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(10),
            whole: Duration::from_secs(30),
            #[cfg(test)]
            target: None,
            #[cfg(test)]
            observed: std::cell::Cell::new(0),
            #[cfg(test)]
            faults: TestFaults::default(),
        }
    }
}
impl ResponseSource {
    pub(super) fn fetch(&self, request: &Request<'_>) -> Result<Vec<u8>, Failure> {
        // Refuse a resolver that cannot honor the connection allowance.
        let version = curl::Version::get();
        if !version.feature_async_dns() || !version.feature_ssl() {
            return Err(Failure::new(
                "transport",
                "HTTPS acquisition requires TLS and asynchronous DNS support",
            ));
        }
        if !request.uri.starts_with(ORIGIN) {
            return Err(Failure::new(
                "transport",
                "acquisition origin is not permitted",
            ));
        }
        let mut easy = Easy2::new(Response {
            bytes: Vec::new(),
            headers: 0,
            status: None,
            failure: None,
        });
        let configure = (|| -> Result<(), curl::Error> {
            easy.url(&request.uri)?;
            easy.proxy("")?;
            easy.follow_location(false)?;
            easy.max_redirections(0)?;
            easy.netrc(NetRc::Ignored)?;
            easy.ssl_verify_peer(true)?;
            easy.ssl_verify_host(true)?;
            easy.connect_timeout(self.connect)?;
            easy.timeout(self.whole)?;
            easy.buffer_size(16 * 1024)?;
            easy.http_version(HttpVersion::V11)?;
            easy.http_content_decoding(false)?;
            easy.get(true)?;
            let mut headers = List::new();
            headers.append("Accept-Encoding: identity")?;
            easy.http_headers(headers)?;
            // The one private response seam exists only in test executables.
            #[cfg(test)]
            if let Some(target) = &self.target {
                easy.url(&format!(
                    "{target}{}",
                    request.uri.strip_prefix(ORIGIN).expect("fixed origin")
                ))?;
            }
            Ok(())
        })();
        configure.map_err(|error| {
            Failure::new(
                "transport",
                format!("cannot configure acquisition: {error}"),
            )
        })?;
        #[cfg(test)]
        self.observed.set(self.observed.get() + 1);
        let performed = easy.perform();
        if let Some(failure) = easy.get_ref().failure.clone() {
            return Err(failure);
        }
        if let Err(error) = performed {
            let phase = if error.is_operation_timedout() || error.code() == 100 {
                "limits"
            } else {
                "transport"
            };
            return Err(Failure::new(
                phase,
                format!("transfer did not complete: {error}"),
            ));
        }
        if easy.response_code().ok() != Some(200) {
            return Err(Failure::new("http", "response was not HTTP 200"));
        }
        Ok(std::mem::take(&mut easy.get_mut().bytes))
    }
}
#[cfg(test)]
#[derive(Default)]
pub(super) struct TestFaults {
    pub(super) stage: bool,
    pub(super) commit: bool,
    pub(super) cleanup: bool,
    pub(super) create_document: bool,
    pub(super) after_receipt_check: Option<Box<dyn Fn()>>,
    pub(super) receipt_write: bool,
    pub(super) after_fetch: Option<Box<dyn Fn()>>,
    pub(super) after_document_check: Option<Box<dyn Fn()>>,
    pub(super) staging_disappears: bool,
}
