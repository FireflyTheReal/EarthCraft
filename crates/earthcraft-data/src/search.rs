use std::time::Duration;
const SEARCH_KM_PER_DEGREE: f64 = 111.2;
const SEARCH_VIEW_PADDING_RATIO: f64 = 1.6;
const MIN_SEARCH_ALTITUDE_KM: f64 = 0.5;
const MAX_SEARCH_ALTITUDE_KM: f64 = 12_000.0;
const SUGGESTION_REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

pub const COUNTRY_SEARCH_ALTITUDE_KM: f64 = 6000.0;
pub const CITY_SEARCH_ALTITUDE_KM: f64 = 40.0;
const DEFAULT_SEARCH_ALTITUDE_KM: f64 = 5.0;
const STATE_SEARCH_ALTITUDE_KM: f64 = 1500.0;
const COUNTY_SEARCH_ALTITUDE_KM: f64 = 300.0;
const LOCALITY_SEARCH_ALTITUDE_KM: f64 = 12.0;
const STREET_SEARCH_ALTITUDE_KM: f64 = 2.0;
const HOUSE_SEARCH_ALTITUDE_KM: f64 = 0.6;

#[derive(Clone)]
pub struct GeoResult {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub alt: f64,
}

pub fn parse_latlon(s: &str) -> Option<(f64, f64)> {
    let cleaned = s.replace([',', ';'], " ");
    let parts: Vec<&str> = cleaned.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }
    let (a, b) = (parts[0].parse::<f64>().ok()?, parts[1].parse::<f64>().ok()?);
    if a.abs() <= 90.0 && b.abs() <= 180.0 {
        Some((a, b))
    } else {
        None
    }
}

pub fn geocode(q: &str, lang: &str) -> Result<Vec<GeoResult>, crate::Error> {
    let resp = ureq::get("https://nominatim.openstreetmap.org/search")
        .set(
            "User-Agent",
            "EarthCraft/0.1 (visualizador educacional de globo 3D)",
        )
        .query("format", "json")
        .query("limit", "6")
        .query("accept-language", lang)
        .query("q", q)
        .call()
        .map_err(crate::Error::from)?;
    let v: serde_json::Value = resp.into_json().map_err(crate::Error::from)?;
    parse_geocode_response(v)
}

#[derive(serde::Deserialize)]
struct NominatimResult {
    #[serde(default, deserialize_with = "crate::json_fields::string_number")]
    lat: Option<f64>,
    #[serde(default, deserialize_with = "crate::json_fields::string_number")]
    lon: Option<f64>,
    #[serde(default, deserialize_with = "crate::json_fields::string_numbers")]
    boundingbox: Vec<f64>,
    #[serde(default, deserialize_with = "crate::json_fields::string")]
    display_name: Option<String>,
}
fn parse_geocode_response(value: serde_json::Value) -> Result<Vec<GeoResult>, crate::Error> {
    let mut out = Vec::new();
    for item in value.as_array().ok_or("resposta inválida")? {
        let Ok(row) = serde_json::from_value::<NominatimResult>(item.clone()) else {
            continue;
        };
        let (Some(lat), Some(lon)) = (row.lat, row.lon) else {
            continue;
        };
        let bb = row.boundingbox;
        let alt = if bb.len() == 4 {
            let span = (bb[1] - bb[0]).max((bb[3] - bb[2]) * lat.to_radians().cos().abs());
            (span * SEARCH_KM_PER_DEGREE * SEARCH_VIEW_PADDING_RATIO)
                .clamp(MIN_SEARCH_ALTITUDE_KM, MAX_SEARCH_ALTITUDE_KM)
        } else {
            DEFAULT_SEARCH_ALTITUDE_KM
        };
        out.push(GeoResult {
            name: row.display_name.unwrap_or_else(|| "?".into()),
            lat,
            lon,
            alt,
        });
    }
    Ok(out)
}

#[derive(Clone)]
pub struct Suggestion {
    pub label: String,
    pub lat: f64,
    pub lon: f64,
    pub alt: f64,
}

/// Search-as-you-type suggestions. Nominatim forbids autocomplete, so Photon (Komoot, built on
/// OpenStreetMap data and meant for this use) is used instead.
pub fn photon_suggest(q: &str, lang: &str) -> Result<Vec<Suggestion>, crate::Error> {
    let agent = ureq::AgentBuilder::new()
        .timeout(SUGGESTION_REQUEST_TIMEOUT)
        .user_agent("EarthCraft/0.2 (search suggestions)")
        .build();
    let mut req = agent
        .get("https://photon.komoot.io/api/")
        .query("q", q)
        .query("limit", "8");
    if lang == "en" {
        req = req.query("lang", "en"); // Photon only supports en/de/fr/it; others use local names
    }
    let v: serde_json::Value = req
        .call()
        .map_err(crate::Error::from)?
        .into_json()
        .map_err(crate::Error::from)?;
    let mut out: Vec<Suggestion> = Vec::new();
    for f in v["features"].as_array().into_iter().flatten() {
        let (Some(lon), Some(lat)) = (
            f.pointer("/geometry/coordinates/0")
                .and_then(|x| x.as_f64()),
            f.pointer("/geometry/coordinates/1")
                .and_then(|x| x.as_f64()),
        ) else {
            continue;
        };
        let pr = &f["properties"];
        let g = |k: &str| {
            pr[k]
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        };
        let mut parts: Vec<String> = Vec::new();
        let first = g("name").or_else(|| {
            g("street").map(|st| match g("housenumber") {
                Some(n) => format!("{st}, {n}"),
                None => st,
            })
        });
        parts.extend(first);
        for k in ["district", "city", "state", "country"] {
            if let Some(x) = g(k) {
                if !parts.contains(&x) {
                    parts.push(x);
                }
            }
        }
        if parts.is_empty() {
            continue;
        }
        let ex: Vec<f64> = pr["extent"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_f64()).collect())
            .unwrap_or_default();
        let alt = if ex.len() == 4 {
            let span = (ex[1] - ex[3])
                .abs()
                .max((ex[2] - ex[0]).abs() * lat.to_radians().cos().abs());
            (span * SEARCH_KM_PER_DEGREE * SEARCH_VIEW_PADDING_RATIO)
                .clamp(MIN_SEARCH_ALTITUDE_KM, MAX_SEARCH_ALTITUDE_KM)
        } else {
            match pr["type"].as_str().unwrap_or("") {
                "country" => COUNTRY_SEARCH_ALTITUDE_KM,
                "state" => STATE_SEARCH_ALTITUDE_KM,
                "county" => COUNTY_SEARCH_ALTITUDE_KM,
                "city" => CITY_SEARCH_ALTITUDE_KM,
                "district" | "locality" => LOCALITY_SEARCH_ALTITUDE_KM,
                "street" => STREET_SEARCH_ALTITUDE_KM,
                "house" => HOUSE_SEARCH_ALTITUDE_KM,
                _ => DEFAULT_SEARCH_ALTITUDE_KM,
            }
        };
        let label = parts.join(", ");
        if !out.iter().any(|s| s.label == label) {
            out.push(Suggestion {
                label,
                lat,
                lon,
                alt,
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_search_skips_invalid_rows_and_retains_defaults() {
        let rows = parse_geocode_response(serde_json::json!([
            {"lat":"-15","lon":"-50","display_name":"Brasília"},
            {"lat":"invalid","lon":"10"},
            {"lat":"1","lon":"2","display_name":42,"boundingbox":["bad"]}
        ]))
        .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "Brasília");
        assert_eq!(rows[1].name, "?");
        assert_eq!(rows[1].alt, DEFAULT_SEARCH_ALTITUDE_KM);
    }

    #[test]
    fn coordinates_accept_existing_separators_and_bounds() {
        for input in ["-15, -50", "-15; -50", " -15  -50 "] {
            assert_eq!(parse_latlon(input), Some((-15.0, -50.0)));
        }
        assert_eq!(parse_latlon("90, 180"), Some((90.0, 180.0)));
        for input in ["91, 0", "0, 181", "NaN, 0", "inf, 0", "one, two", "1, 2, 3"] {
            assert_eq!(parse_latlon(input), None, "{input}");
        }
    }
}
