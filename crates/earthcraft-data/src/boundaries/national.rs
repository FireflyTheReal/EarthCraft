//! National municipal boundary services with keyless GeoJSON/ArcGIS access.
//! The table is deliberately small: countries without a stable JSON endpoint use geoBoundaries.
use super::geojson::{collect_geojson_lines, feature_name, label_center};
use super::{Detail, LOCAL_LABEL_RANK, LabelKind, MapLabel};
use crate::http::fetch_json;
use std::sync::Arc;

const PAGE_SIZE: usize = 1000;
const MAX_PAGES: usize = 20;
const WFS_RADIUS_DEG: f64 = 0.30;
const WFS_COUNT: usize = 1000;

const US_URL: &str =
    "https://tigerweb.geo.census.gov/arcgis/rest/services/TIGERweb/State_County/MapServer/1/query";
const CANADA_URL: &str = "https://geo.statcan.gc.ca/geo_wa/rest/services/2021/Cartographic_boundary_files/MapServer/4/query";
const FRANCE_URL: &str = "https://data.geopf.fr/wfs/ows";
const GERMANY_URL: &str = "https://sgx.geodatenzentrum.de/wfs_vg250";
const SPAIN_URL: &str = "https://services1.arcgis.com/nCKYwcSONQTkPA4K/arcgis/rest/services/Municipios/FeatureServer/0/query";

const US_NAMES: &[&str] = &["NAME"];
const CANADA_NAMES: &[&str] = &["CDNAME"];
const FRANCE_NAMES: &[&str] = &["nom_officiel"];
const GERMANY_NAMES: &[&str] = &["gen"];
const SPAIN_NAMES: &[&str] = &["NAMEUNIT"];

pub(super) fn uses_viewport(iso3: &str) -> bool {
    matches!(iso3, "FRA" | "DEU" | "ESP")
}

pub(super) fn source_label(iso3: &str) -> Option<&'static str> {
    match iso3 {
        "USA" => Some("US Census TIGERweb"),
        "CAN" => Some("Statistics Canada"),
        "FRA" => Some("IGN ADMIN EXPRESS"),
        "DEU" => Some("BKG VG250"),
        "ESP" => Some("IGN municipios"),
        _ => None,
    }
}

fn query_url(
    base: &str,
    params: impl IntoIterator<Item = (&'static str, String)>,
) -> Result<String, crate::Error> {
    let mut url =
        reqwest::Url::parse(base).map_err(|error| crate::Error::from(error.to_string()))?;
    url.query_pairs_mut().extend_pairs(params);
    Ok(url.into())
}

fn city_label(feature: &serde_json::Value, name_keys: &[&str]) -> Option<MapLabel> {
    let geometry = feature.get("geometry").filter(|value| !value.is_null())?;
    let (lat, lon) = label_center(geometry)?;
    let name = feature_name(feature, name_keys)?;
    Some(MapLabel {
        lat,
        lon,
        en: name.clone(),
        pt: name,
        kind: LabelKind::City,
        rank: LOCAL_LABEL_RANK,
        big: false,
    })
}

fn geojson_consumer(
    name_keys: &'static [&'static str],
) -> impl Fn(&serde_json::Value, &mut Detail) + Clone + Send + 'static {
    move |feature, out| {
        out.features += 1;
        let Some(geometry) = feature.get("geometry").filter(|value| !value.is_null()) else {
            return;
        };
        if let Some(coords) = geometry.get("coordinates") {
            collect_geojson_lines(coords, &mut out.lines);
        }
        if let Some(label) = city_label(feature, name_keys) {
            out.labels.push(label);
        }
    }
}

fn bbox_params(lat: f64, lon: f64) -> String {
    let lat0 = (lat - WFS_RADIUS_DEG).clamp(-85.0, 85.0);
    let lat1 = (lat + WFS_RADIUS_DEG).clamp(-85.0, 85.0);
    let lon0 = (lon - WFS_RADIUS_DEG).max(-180.0);
    let lon1 = (lon + WFS_RADIUS_DEG).min(180.0);
    format!("{lon0:.4},{lat0:.4},{lon1:.4},{lat1:.4},EPSG:4326")
}

async fn arcgis_geojson(
    base: &str,
    params: Vec<(&'static str, String)>,
    names: &'static [&'static str],
    paginate: bool,
    cache: bool,
) -> Result<Detail, crate::Error> {
    let mut out = Detail::default();
    for page in 0..MAX_PAGES {
        let mut page_params = params.clone();
        page_params.push(("resultOffset", (page * PAGE_SIZE).to_string()));
        page_params.push(("resultRecordCount", PAGE_SIZE.to_string()));
        let url = query_url(base, page_params)?;
        let batch = if cache {
            crate::features::fetch(&url, geojson_consumer(names)).await?
        } else {
            crate::features::fetch_ephemeral(&url, geojson_consumer(names)).await?
        };
        let count = batch.features;
        out.lines.extend(batch.lines);
        out.labels.extend(batch.labels);
        out.features += count;
        if !paginate || count < PAGE_SIZE {
            break;
        }
    }
    Ok(out)
}

fn esri_center(rings: &[serde_json::Value]) -> Option<(f64, f64)> {
    let ring = rings
        .iter()
        .max_by_key(|ring| ring.as_array().map_or(0, Vec::len))?;
    let (mut min_lat, mut max_lat) = (f64::MAX, f64::MIN);
    let (mut min_lon, mut max_lon) = (f64::MAX, f64::MIN);
    for point in ring.as_array()? {
        let pair = point.as_array()?;
        let (lon, lat) = (pair.first()?.as_f64()?, pair.get(1)?.as_f64()?);
        if lat.is_finite() && lon.is_finite() {
            min_lat = min_lat.min(lat);
            max_lat = max_lat.max(lat);
            min_lon = min_lon.min(lon);
            max_lon = max_lon.max(lon);
        }
    }
    (min_lat <= max_lat).then_some(((min_lat + max_lat) / 2.0, (min_lon + max_lon) / 2.0))
}

fn parse_esri(value: &serde_json::Value, name_keys: &[&str]) -> Detail {
    let mut out = Detail::default();
    for feature in value
        .get("features")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        out.features += 1;
        let Some(rings) = feature
            .pointer("/geometry/rings")
            .and_then(|v| v.as_array())
        else {
            continue;
        };
        for ring in rings {
            let line: Vec<(f64, f64)> = ring
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|point| {
                    let pair = point.as_array()?;
                    let lon = pair.first()?.as_f64()?;
                    let lat = pair.get(1)?.as_f64()?;
                    (lat.is_finite() && lon.is_finite() && lat.abs() <= 90.0 && lon.abs() <= 180.0)
                        .then_some((lat, lon))
                })
                .collect();
            if line.len() >= 2 {
                out.lines.push(Arc::new(line));
            }
        }
        if let Some((lat, lon)) = esri_center(rings)
            && let Some(name) = feature
                .get("attributes")
                .and_then(|attributes| {
                    name_keys
                        .iter()
                        .find_map(|key| attributes.get(*key).and_then(|v| v.as_str()))
                })
                .filter(|name| !name.trim().is_empty())
        {
            out.labels.push(MapLabel {
                lat,
                lon,
                en: name.to_owned(),
                pt: name.to_owned(),
                kind: LabelKind::City,
                rank: LOCAL_LABEL_RANK,
                big: false,
            });
        }
    }
    out
}

async fn arcgis_esri(
    base: &str,
    params: Vec<(&'static str, String)>,
    names: &'static [&'static str],
) -> Result<Detail, crate::Error> {
    let mut out = Detail::default();
    for page in 0..MAX_PAGES {
        let mut page_params = params.clone();
        page_params.push(("resultOffset", (page * PAGE_SIZE).to_string()));
        page_params.push(("resultRecordCount", PAGE_SIZE.to_string()));
        let url = query_url(base, page_params)?;
        let value = fetch_json(&url).await?;
        let batch = crate::cpu::run(crate::cpu::Service::Features, move || {
            Ok(parse_esri(&value, names))
        })
        .await?;
        let count = batch.features;
        out.lines.extend(batch.lines);
        out.labels.extend(batch.labels);
        out.features += count;
        if count < PAGE_SIZE {
            break;
        }
    }
    Ok(out)
}

async fn united_states() -> Result<Detail, crate::Error> {
    arcgis_geojson(
        US_URL,
        vec![
            ("where", "1=1".into()),
            ("outFields", "NAME,STATE,COUNTY,GEOID".into()),
            ("returnGeometry", "true".into()),
            ("outSR", "4326".into()),
            ("maxAllowableOffset", "0.005".into()),
            ("geometryPrecision", "4".into()),
            ("orderByFields", "GEOID".into()),
            ("f", "geojson".into()),
        ],
        US_NAMES,
        true,
        true,
    )
    .await
}

async fn canada() -> Result<Detail, crate::Error> {
    arcgis_esri(
        CANADA_URL,
        vec![
            ("where", "1=1".into()),
            ("outFields", "CDUID,CDNAME".into()),
            ("returnGeometry", "true".into()),
            ("outSR", "4326".into()),
            ("maxAllowableOffset", "0.005".into()),
            ("geometryPrecision", "3".into()),
            ("orderByFields", "CDUID".into()),
            ("f", "json".into()),
        ],
        CANADA_NAMES,
    )
    .await
}

async fn france(lat: f64, lon: f64) -> Result<Detail, crate::Error> {
    let url = query_url(
        FRANCE_URL,
        vec![
            ("SERVICE", "WFS".into()),
            ("VERSION", "2.0.0".into()),
            ("REQUEST", "GetFeature".into()),
            (
                "TYPENAMES",
                "LIMITES_ADMINISTRATIVES_EXPRESS.LATEST:commune".into(),
            ),
            ("BBOX", bbox_params(lat, lon)),
            ("COUNT", WFS_COUNT.to_string()),
            ("SRSNAME", "EPSG:4326".into()),
            ("outputFormat", "application/json".into()),
        ],
    )?;
    crate::features::fetch_ephemeral(&url, geojson_consumer(FRANCE_NAMES)).await
}

async fn germany(lat: f64, lon: f64) -> Result<Detail, crate::Error> {
    let url = query_url(
        GERMANY_URL,
        vec![
            ("SERVICE", "WFS".into()),
            ("VERSION", "2.0.0".into()),
            ("REQUEST", "GetFeature".into()),
            ("TYPENAMES", "vg250:vg250_gem".into()),
            ("BBOX", bbox_params(lat, lon)),
            ("COUNT", WFS_COUNT.to_string()),
            ("SRSNAME", "EPSG:4326".into()),
            ("outputFormat", "application/json".into()),
        ],
    )?;
    crate::features::fetch_ephemeral(&url, geojson_consumer(GERMANY_NAMES)).await
}

async fn spain(lat: f64, lon: f64) -> Result<Detail, crate::Error> {
    let lat0 = (lat - WFS_RADIUS_DEG).clamp(-85.0, 85.0);
    let lat1 = (lat + WFS_RADIUS_DEG).clamp(-85.0, 85.0);
    let lon0 = (lon - WFS_RADIUS_DEG).max(-180.0);
    let lon1 = (lon + WFS_RADIUS_DEG).min(180.0);
    arcgis_geojson(
        SPAIN_URL,
        vec![
            ("where", "1=1".into()),
            (
                "geometry",
                format!("{lon0:.4},{lat0:.4},{lon1:.4},{lat1:.4}"),
            ),
            ("geometryType", "esriGeometryEnvelope".into()),
            ("inSR", "4326".into()),
            ("spatialRel", "esriSpatialRelIntersects".into()),
            ("outFields", "NAMEUNIT,CODIGOINE,OBJECTID".into()),
            ("returnGeometry", "true".into()),
            ("outSR", "4326".into()),
            ("maxAllowableOffset", "0.002".into()),
            ("orderByFields", "OBJECTID".into()),
            ("f", "geojson".into()),
        ],
        SPAIN_NAMES,
        true,
        false,
    )
    .await
}

pub(super) async fn load(iso3: &str, lat: f64, lon: f64) -> Option<Result<Detail, crate::Error>> {
    match iso3 {
        "USA" => Some(united_states().await),
        "CAN" => Some(canada().await),
        "FRA" => Some(france(lat, lon).await),
        "DEU" => Some(germany(lat, lon).await),
        "ESP" => Some(spain(lat, lon).await),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_sources_refresh_on_pan_but_country_files_do_not() {
        assert!(uses_viewport("FRA"));
        assert!(uses_viewport("DEU"));
        assert!(uses_viewport("ESP"));
        assert!(!uses_viewport("USA"));
        assert!(!uses_viewport("MEX"));
    }

    #[test]
    fn bbox_uses_wfs_longitude_latitude_axis_order() {
        assert_eq!(
            bbox_params(48.8566, 2.3522),
            "2.0522,48.5566,2.6522,49.1566,EPSG:4326"
        );
    }

    #[test]
    fn esri_rings_are_projected_and_named() {
        let value = serde_json::json!({
            "features": [{
                "attributes": {"CDNAME": "Division No. 1"},
                "geometry": {"rings": [[[-60.0, 50.0], [-60.0, 51.0], [-59.0, 51.0], [-60.0, 50.0]]]}
            }]
        });
        let detail = parse_esri(&value, CANADA_NAMES);
        assert_eq!(detail.features, 1);
        assert_eq!(detail.lines.len(), 1);
        assert_eq!(detail.labels[0].en, "Division No. 1");
    }

    #[test]
    fn arcgis_query_encodes_pagination() {
        let url = query_url(
            US_URL,
            vec![
                ("where", "1=1".into()),
                ("resultOffset", "2000".into()),
                ("f", "geojson".into()),
            ],
        )
        .unwrap();
        assert!(url.contains("resultOffset=2000"));
        assert!(url.contains("f=geojson"));
    }
}
