use crate::http::geo_cache_path;
use std::{io::Read, sync::Mutex, time::Duration};
const MAX_WAYBACK_RELEASES: usize = 60_000;
const WAYBACK_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const WAYBACK_READ_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_WAYBACK_CONFIG_BYTES: u64 = 64 << 20;

// ───────────────────────────── Wayback (historical imagery) ─────────────────────────────

#[derive(Clone, Debug)]
pub struct WaybackRelease {
    pub(crate) id: u32,
    pub date: String,       // YYYY-MM-DD
    pub(crate) url: String, // template with {level}/{row}/{col}
}
/// Sorted newest first. A tile key stores the index into this list.
static WAYBACK: Mutex<Vec<WaybackRelease>> = Mutex::new(Vec::new());
const WAYBACK_CONFIG: &str =
    "https://s3-us-west-2.amazonaws.com/config.maptiles.arcgis.com/waybackconfig.json";

pub(crate) fn wayback_release(rel: u16) -> Option<WaybackRelease> {
    WAYBACK
        .lock()
        .ok()
        .and_then(|v| v.get(rel as usize).cloned())
}
pub(crate) fn wayback_tile_url(rel: u16, z: u8, x: u32, y: u32) -> String {
    match wayback_release(rel) {
        Some(r) => r
            .url
            .replace("{level}", &z.to_string())
            .replace("{row}", &y.to_string())
            .replace("{col}", &x.to_string())
            .replace("{M}", &r.id.to_string()),
        None => String::new(),
    }
}
fn parse_wayback_config(v: &serde_json::Value) -> Vec<WaybackRelease> {
    let mut out = Vec::new();
    let Some(obj) = v.as_object() else { return out };
    for (k, item) in obj {
        let Ok(id) = k.trim().parse::<u32>() else {
            continue;
        };
        let title = item.get("itemTitle").and_then(|t| t.as_str()).unwrap_or("");
        // "World Imagery (Wayback 2014-02-20)"
        let date = title
            .split("Wayback")
            .nth(1)
            .map(|t| t.trim().trim_end_matches(')').trim().to_string())
            .filter(|d| d.len() >= 10)
            .unwrap_or_else(|| title.to_string());
        if date.is_empty() {
            continue;
        }
        let url = item
            .get("itemURL")
            .and_then(|u| u.as_str())
            .map(|u| u.to_string())
            .unwrap_or_else(|| {
                format!("https://wayback.maptiles.arcgis.com/arcgis/rest/services/World_Imagery/WMTS/1.0.0/default028mm/MapServer/tile/{id}/{{level}}/{{row}}/{{col}}")
            });
        out.push(WaybackRelease { id, date, url });
    }
    out.sort_by(|a, b| b.date.cmp(&a.date).then(b.id.cmp(&a.id)));
    out.truncate(MAX_WAYBACK_RELEASES);
    out
}
/// Fetches the release list (network first so that new releases show up; disk cache as fallback).
pub fn load_wayback_releases() -> Result<Vec<WaybackRelease>, String> {
    let path = geo_cache_path(WAYBACK_CONFIG);
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(WAYBACK_CONNECT_TIMEOUT)
        .timeout_read(WAYBACK_READ_TIMEOUT)
        .user_agent("EarthCraft/0.2 (wayback release list)")
        .build();
    let net: Result<serde_json::Value, String> = agent
        .get(WAYBACK_CONFIG)
        .call()
        .map_err(|e| e.to_string())
        .and_then(|r| {
            let mut buf = Vec::new();
            r.into_reader()
                .take(MAX_WAYBACK_CONFIG_BYTES)
                .read_to_end(&mut buf)
                .map_err(|e| e.to_string())?;
            serde_json::from_slice::<serde_json::Value>(&buf)
                .map(|v| (v, buf))
                .map_err(|e| e.to_string())
        })
        .map(|(v, buf)| {
            if let Some(d) = path.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let _ = std::fs::write(&path, buf);
            v
        });
    let json = match net {
        Ok(v) => v,
        Err(e) => match std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
        {
            Some(v) => v,
            None => return Err(e),
        },
    };
    let list = parse_wayback_config(&json);
    if list.is_empty() {
        Err("lista de versões vazia".into())
    } else {
        Ok(list)
    }
}

/// Installs the release indices used by tile requests, preserving newest-first ordering.
pub fn set_wayback_releases(list: Vec<WaybackRelease>) {
    if let Ok(mut releases) = WAYBACK.lock() {
        *releases = list;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wayback_config_parses() {
        let v: serde_json::Value = serde_json::json!({
            "10": {"itemTitle": "World Imagery (Wayback 2014-02-20)", "itemURL": "https://x/tile/10/{level}/{row}/{col}"},
            "5000": {"itemTitle": "World Imagery (Wayback 2020-01-01)"},
        });
        let l = parse_wayback_config(&v);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].date, "2020-01-01"); // newest first
        assert!(l[0].url.contains("/5000/{level}/{row}/{col}"));
        assert_eq!(l[1].date, "2014-02-20");
    }
}
