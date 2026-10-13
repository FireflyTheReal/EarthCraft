use crate::paths::cache_dir;
use std::{io::Read, path::PathBuf, time::Duration};
const MAX_CACHE_FILENAME_CHARS: usize = 120;
const JSON_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const JSON_READ_TIMEOUT: Duration = Duration::from_secs(90);
const JSON_REQUEST_ATTEMPTS: usize = 2;
const MAX_JSON_RESPONSE_BYTES: u64 = 1 << 30;

pub(crate) fn geo_cache_path(url: &str) -> PathBuf {
    let clean: String = url
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let start = clean.len().saturating_sub(MAX_CACHE_FILENAME_CHARS);
    cache_dir().join("geo").join(&clean[start..])
}

/// Downloads (or reads from disk) JSON, with a 1 GiB read cap for large boundary files.
/// Connect/read timeouts and one retry preserve the existing request policy.
pub(crate) fn fetch_json(url: &str) -> Result<serde_json::Value, String> {
    let path = geo_cache_path(url);
    if let Ok(bytes) = std::fs::read(&path) {
        if let Ok(v) = serde_json::from_slice(&bytes) {
            return Ok(v);
        }
    }
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(JSON_CONNECT_TIMEOUT)
        .timeout_read(JSON_READ_TIMEOUT)
        .user_agent("EarthCraft/0.1 (map boundary overlay)")
        .build();
    let mut last = String::new();
    for _ in 0..JSON_REQUEST_ATTEMPTS {
        let resp = match agent.get(url).call() {
            Ok(r) => r,
            Err(e) => {
                last = e.to_string();
                continue;
            }
        };
        let mut buf = Vec::new();
        if let Err(e) = resp
            .into_reader()
            .take(MAX_JSON_RESPONSE_BYTES)
            .read_to_end(&mut buf)
        {
            last = e.to_string();
            continue;
        }
        match serde_json::from_slice::<serde_json::Value>(&buf) {
            Ok(v) => {
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let _ = std::fs::write(&path, &buf);
                return Ok(v);
            }
            Err(e) => {
                last = format!("JSON inválido: {e}");
            }
        }
    }
    Err(last)
}
