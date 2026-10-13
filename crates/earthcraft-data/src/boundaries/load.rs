use super::geojson::{collect_geojson_lines, feature_name, label_center, outer_outline};
use super::{is_big_country, log_status, LabelKind, Layer, Line, MapLabel, Patch, StatusLog};
use crate::http::fetch_json;
use std::{collections::HashMap, sync::mpsc};
const NE: &str = "https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson";
const IBGE: &str = "https://servicodados.ibge.gov.br/api";
const BR_UFS: [u32; 27] = [
    11, 12, 13, 14, 15, 16, 17, 21, 22, 23, 24, 25, 26, 27, 28, 29, 31, 32, 33, 35, 41, 42, 43, 50,
    51, 52, 53,
];

// Background async tasks deliver each completed layer through the application channel.
pub fn load_boundaries_async(
    log: StatusLog,
    runtime: &tokio::runtime::Handle,
) -> mpsc::Receiver<Patch> {
    let (tx, rx) = mpsc::channel();

    let (t, l) = (tx.clone(), log.clone());
    runtime.spawn(async move {
        let urls = [
            format!("{NE}/ne_50m_admin_0_countries.geojson"),
            "https://raw.githubusercontent.com/datasets/geo-countries/main/data/countries.geojson"
                .to_string(),
            format!("{NE}/ne_110m_admin_0_countries.geojson"),
        ];
        if let Some(p) = load_polygon_layer(
            &l,
            &urls,
            PolygonLayer {
                what: "Países/Countries",
                layer: Layer::Countries,
                kind: LabelKind::Country,
                en_keys: &["NAME", "ADMIN", "name"],
                pt_keys: &["NAME_PT", "name_pt", "NAME", "ADMIN"],
                rank: 0,
            },
        )
        .await
        {
            let _ = t.send(p);
        }
    });

    let (t, l) = (tx.clone(), log.clone());
    runtime.spawn(async move {
        let urls = [
            format!("{NE}/ne_10m_admin_1_states_provinces.geojson"),
            format!("{NE}/ne_50m_admin_1_states_provinces.geojson"),
        ];
        if let Some(p) = load_polygon_layer(
            &l,
            &urls,
            PolygonLayer {
                what: "Estados/States",
                layer: Layer::States,
                kind: LabelKind::State,
                en_keys: &["name_en", "name", "NAME_1", "gn_name"],
                pt_keys: &["name_pt", "name", "name_en", "NAME_1"],
                rank: 0,
            },
        )
        .await
        {
            let _ = t.send(p);
        }
    });

    let (t, l) = (tx.clone(), log.clone());
    runtime.spawn(async move {
        let urls = [
            format!("{NE}/ne_10m_populated_places.geojson"),
            format!("{NE}/ne_50m_populated_places.geojson"),
        ];
        if let Some(p) = load_places(&l, &urls).await {
            let _ = t.send(p);
        }
    });

    let t = tx.clone();
    runtime.spawn(async move {
        load_brazil(&log, &t).await;
    });

    rx
}

async fn first_json(log: &StatusLog, what: &str, urls: &[String]) -> Option<serde_json::Value> {
    let mut last = String::new();
    for url in urls {
        match fetch_json(url).await {
            Ok(v) => return Some(v),
            Err(e) => last = format!("{url}: {e}"),
        }
    }
    log_status(log, format!("✖ {what}: {last}"));
    None
}

struct PolygonLayer<'a> {
    what: &'a str,
    layer: Layer,
    kind: LabelKind,
    en_keys: &'a [&'a str],
    pt_keys: &'a [&'a str],
    rank: u8,
}

async fn load_polygon_layer(
    log: &StatusLog,
    urls: &[String],
    options: PolygonLayer<'_>,
) -> Option<Patch> {
    let PolygonLayer {
        what,
        layer,
        kind,
        en_keys,
        pt_keys,
        rank,
    } = options;
    let json = first_json(log, what, urls).await?;
    let features = json.get("features").and_then(|v| v.as_array());
    let Some(features) = features else {
        log_status(log, format!("✖ {what}: arquivo sem 'features'"));
        return None;
    };
    let (mut lines, mut lines_big, mut labels) = (Vec::new(), Vec::new(), Vec::new());
    for f in features {
        let Some(geometry) = f.get("geometry") else {
            continue;
        };
        let big = layer == Layer::States && is_big_country(f);
        if let Some(c) = geometry.get("coordinates") {
            collect_geojson_lines(c, if big { &mut lines_big } else { &mut lines });
        }
        if let (Some(en), Some((lat, lon))) = (feature_name(f, en_keys), label_center(geometry)) {
            let pt = feature_name(f, pt_keys).unwrap_or_else(|| en.clone());
            labels.push(MapLabel {
                lat,
                lon,
                en,
                pt,
                kind,
                rank,
                big,
            });
        }
    }
    log_status(
        log,
        format!(
            "✔ {what}: {} linhas, {} nomes",
            lines.len() + lines_big.len(),
            labels.len()
        ),
    );
    Some(Patch {
        layer,
        lines,
        lines_big,
        labels,
    })
}

async fn load_places(log: &StatusLog, urls: &[String]) -> Option<Patch> {
    let json = first_json(log, "Cidades/Cities", urls).await?;
    let mut labels = Vec::new();
    for f in json
        .get("features")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        let c = f
            .pointer("/geometry/coordinates")
            .and_then(|v| v.as_array());
        let (Some(c), Some(en)) = (c, feature_name(f, &["NAME", "name", "NAMEASCII"])) else {
            continue;
        };
        let (Some(lon), Some(lat)) = (
            c.first().and_then(|v| v.as_f64()),
            c.get(1).and_then(|v| v.as_f64()),
        ) else {
            continue;
        };
        let pt = feature_name(f, &["NAME_PT", "name_pt"]).unwrap_or_else(|| en.clone());
        let props = f.get("properties");
        let rank = props
            .and_then(|p| p.get("SCALERANK").or_else(|| p.get("scalerank")))
            .and_then(|v| v.as_f64())
            .unwrap_or(5.0)
            .clamp(0.0, 8.0) as u8;
        labels.push(MapLabel {
            lat,
            lon,
            en,
            pt,
            kind: LabelKind::City,
            rank,
            big: false,
        });
    }
    log_status(log, format!("✔ Cidades/Cities: {} nomes", labels.len()));
    Some(Patch {
        layer: Layer::Cities,
        lines: vec![],
        lines_big: vec![],
        labels,
    })
}

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

async fn load_brazil_state(uf: u32) -> Result<Vec<Muni>, crate::Error> {
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
    let mut last = String::new();
    for url in &urls {
        let json = match fetch_json(url).await {
            Ok(j) => j,
            Err(e) => {
                last = e.to_string();
                continue;
            }
        };
        let mut out = Vec::new();
        for f in json
            .get("features")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
        {
            let props = f.get("properties");
            let code = json_code(props.and_then(|p| {
                p.get("codarea")
                    .or_else(|| p.get("id"))
                    .or_else(|| p.get("CD_MUN"))
            }));
            let Some(code) = code else {
                continue;
            };
            let Some(geometry) = f.get("geometry") else {
                continue;
            };
            let mut lines = Vec::new();
            if let Some(c) = geometry.get("coordinates") {
                collect_geojson_lines(c, &mut lines);
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
        }
        if !out.is_empty() {
            return Ok(out);
        }
        last = "sem municípios no arquivo".into();
    }
    Err(format!("UF {uf}: {last}").into())
}

// Brazil: municipality outlines/names from IBGE (official API, GitHub mirror as fallback), and
// metropolitan regions built from IBGE's list of member municipalities (outer outline only).
async fn load_brazil(log: &StatusLog, tx: &mpsc::Sender<Patch>) {
    let jobs: Vec<_> = BR_UFS
        .into_iter()
        .map(|uf| tokio::spawn(load_brazil_state(uf)))
        .collect();
    let mut results = Vec::new();
    for job in jobs {
        results.push(match job.await {
            Ok(result) => result,
            Err(error) => Err(error.into()),
        });
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
        log_status(log, format!("✖ Municípios: {}", errs.join("; ")));
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
    log_status(log, format!("✔ Municípios: {} municípios", munis.len()));
    let _ = tx.send(Patch {
        layer: Layer::Cities,
        lines,
        lines_big: vec![],
        labels,
    });

    let url = format!("{IBGE}/v1/localidades/regioes-metropolitanas");
    let json = match fetch_json(&url).await {
        Ok(j) => j,
        Err(e) => {
            log_status(log, format!("✖ Regiões metropolitanas: {url}: {e}"));
            return;
        }
    };
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
    if mlines.is_empty() {
        log_status(
            log,
            "✖ Regiões metropolitanas: resposta do IBGE sem municípios".into(),
        );
    } else {
        log_status(log, format!("✔ Regiões metropolitanas: {}", mlabels.len()));
        let _ = tx.send(Patch {
            layer: Layer::Metro,
            lines: mlines,
            lines_big: vec![],
            labels: mlabels,
        });
    }
}
