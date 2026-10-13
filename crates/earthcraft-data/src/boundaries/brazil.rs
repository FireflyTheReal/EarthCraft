use super::geojson::{collect_geojson_lines, feature_name, label_center, outer_outline};
use super::{log_status, LabelKind, Layer, Line, MapLabel, Patch, StatusLog};
use crate::http::fetch_json;
use std::{
    collections::HashMap,
    sync::{mpsc, Arc},
};
const IBGE: &str = "https://servicodados.ibge.gov.br/api";
const BR_UFS: [u32; 27] = [
    11, 12, 13, 14, 15, 16, 17, 21, 22, 23, 24, 25, 26, 27, 28, 29, 31, 32, 33, 35, 41, 42, 43, 50,
    51, 52, 53,
];

struct Muni {
    code: u32,
    name: String,
    lines: Vec<Line>,
    center: Option<(f64, f64)>,
}

fn json_code(v: Option<&serde_json::Value>) -> Option<u32> {
    let v = v?;
    v.as_u64()
        .map(|n| n as u32)
        .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
}

async fn load_brazil_state(uf: u32, include_geometry: bool) -> Result<Vec<Muni>, crate::Error> {
    let mut names: HashMap<u32, String> = HashMap::new();
    if let Ok(v) = fetch_json(&format!("{IBGE}/v1/localidades/estados/{uf}/municipios")).await {
        for m in v.as_array().into_iter().flatten() {
            if let (Some(id), Some(n)) = (
                json_code(m.get("id")),
                m.get("nome").and_then(|n| n.as_str()),
            ) {
                names.insert(id, n.to_owned());
            }
        }
    }
    let urls = [
        format!("{IBGE}/v3/malhas/estados/{uf}?intrarregiao=municipio&qualidade=intermediaria&formato=application/vnd.geo+json"),
        format!("{IBGE}/v3/malhas/estados/{uf}?intrarregiao=municipio&qualidade=minima&formato=application/vnd.geo+json"),
        format!("https://raw.githubusercontent.com/tbrugz/geodata-br/master/geojson/geojs-{uf}-mun.json"),
    ];
    let names = Arc::new(names);
    let mut last = String::new();
    for url in &urls {
        let names = names.clone();
        let result =
            crate::features::fetch(url, move |f: &serde_json::Value, out: &mut Vec<Muni>| {
                let code = json_code(f.get("properties").and_then(|p| {
                    p.get("codarea")
                        .or_else(|| p.get("id"))
                        .or_else(|| p.get("CD_MUN"))
                }));
                let Some(code) = code else {
                    return;
                };
                let Some(geometry) = f.get("geometry") else {
                    return;
                };
                let mut lines = Vec::new();
                if include_geometry {
                    if let Some(coords) = geometry.get("coordinates") {
                        collect_geojson_lines(coords, &mut lines);
                    }
                }
                let name = names
                    .get(&code)
                    .cloned()
                    .or_else(|| feature_name(f, &["name", "NM_MUN", "nome"]))
                    .unwrap_or_default();
                out.push(Muni {
                    code,
                    name,
                    lines,
                    center: label_center(geometry),
                });
            })
            .await;
        match result {
            Ok(out) if !out.is_empty() => return Ok(out),
            Ok(_) => last = "no municipalities in the document".into(),
            Err(error) => last = error.to_string(),
        }
    }
    Err(format!("State {uf}: {last}").into())
}

// Brazil: municipality outlines/names from IBGE (official API, GitHub mirror as fallback), and
// metropolitan regions built from IBGE's list of member municipalities (outer outline only).
pub(super) async fn load_brazil(log: &StatusLog, tx: &mpsc::Sender<Patch>, include_geometry: bool) {
    // One state at a time bounds large JSON/geometry allocations.
    let mut results = Vec::new();
    for uf in BR_UFS {
        results.push(load_brazil_state(uf, include_geometry).await);
    }
    let mut munis: Vec<Muni> = Vec::new();
    let mut errs = Vec::new();
    for r in results {
        match r {
            Ok(m) => munis.extend(m),
            Err(e) => errs.push(e.to_string()),
        }
    }
    if !errs.is_empty() {
        log_status(log, format!("✖ Municipalities: {}", errs.join("; ")));
    }
    if munis.is_empty() {
        return;
    }

    let mut lines = Vec::new();
    let mut labels = Vec::new();
    for m in &munis {
        lines.extend(m.lines.iter().cloned());
        if let (false, Some((lat, lon))) = (m.name.is_empty(), m.center) {
            labels.push(MapLabel {
                lat,
                lon,
                en: m.name.clone(),
                pt: m.name.clone(),
                kind: LabelKind::City,
                rank: 9,
                big: false,
            });
        }
    }
    log_status(
        log,
        format!("✔ Municipalities: {} municipalities", munis.len()),
    );
    let _ = tx.send(Patch {
        layer: Layer::Cities,
        lines,
        lines_big: vec![],
        labels,
    });

    if !include_geometry {
        return;
    }
    let url = format!("{IBGE}/v1/localidades/regioes-metropolitanas");
    let json = match fetch_json(&url).await {
        Ok(j) => j,
        Err(e) => {
            log_status(log, format!("✖ Metropolitan regions: {url}: {e}"));
            return;
        }
    };
    let patch = match crate::cpu::run(crate::cpu::Service::Features, move || {
        Ok(metropolitan_patch(&munis, &json))
    })
    .await
    {
        Ok(patch) => patch,
        Err(error) => {
            log_status(log, format!("✖ Metropolitan regions: {error}"));
            return;
        }
    };
    let Patch {
        lines: mlines,
        labels: mlabels,
        ..
    } = patch;
    if mlines.is_empty() {
        log_status(
            log,
            "✖ Metropolitan regions: IBGE response contains no municipalities".into(),
        );
    } else {
        log_status(log, format!("✔ Metropolitan regions: {}", mlabels.len()));
        let _ = tx.send(Patch {
            layer: Layer::Metro,
            lines: mlines,
            lines_big: vec![],
            labels: mlabels,
        });
    }
}

fn metropolitan_patch(munis: &[Muni], json: &serde_json::Value) -> Patch {
    let by_code: HashMap<u32, &Muni> = munis.iter().map(|m| (m.code, m)).collect();
    let (mut mlines, mut mlabels) = (Vec::new(), Vec::new());
    for rm in json.as_array().into_iter().flatten() {
        let Some(name) = rm.get("nome").and_then(|n| n.as_str()) else {
            continue;
        };
        let members: Vec<&Muni> = rm
            .get("municipios")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .filter_map(|m| json_code(m.get("id")).and_then(|c| by_code.get(&c).copied()))
            .collect();
        if members.is_empty() {
            continue;
        }
        mlines.extend(outer_outline(members.iter().flat_map(|m| m.lines.iter())));
        let centers: Vec<(f64, f64)> = members.iter().filter_map(|m| m.center).collect();
        if !centers.is_empty() {
            let n = centers.len() as f64;
            let (lat, lon) = (
                centers.iter().map(|c| c.0).sum::<f64>() / n,
                centers.iter().map(|c| c.1).sum::<f64>() / n,
            );
            mlabels.push(MapLabel {
                lat,
                lon,
                en: name.to_owned(),
                pt: name.to_owned(),
                kind: LabelKind::Metro,
                rank: 0,
                big: false,
            });
        }
    }
    Patch {
        layer: Layer::Metro,
        lines: mlines,
        lines_big: Vec::new(),
        labels: mlabels,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metropolitan_membership_keeps_names_centers_and_existing_outline_rules() {
        let municipalities = vec![Muni {
            code: 1,
            name: "City".into(),
            lines: vec![Arc::new(vec![
                (0.0, 0.0),
                (0.0, 1.0),
                (1.0, 1.0),
                (0.0, 0.0),
            ])],
            center: Some((2.0, 3.0)),
        }];
        let json = serde_json::json!([{"nome":"Metro","municipios":[{"id":1}]}]);
        let patch = metropolitan_patch(&municipalities, &json);
        assert!(!patch.lines.is_empty());
        assert_eq!(patch.labels.len(), 1);
        assert_eq!(patch.labels[0].en, "Metro");
        assert_eq!(patch.labels[0].lat, 2.0);
        assert_eq!(patch.labels[0].lon, 3.0);
    }
}
