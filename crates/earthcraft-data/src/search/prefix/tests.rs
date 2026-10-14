use super::*;
use crate::search::merge_suggestions;
fn scene() -> Vec<MapLabel> {
    let names = [
        ("São Paulo", "São Paulo"),
        ("Sao Paulo", "Sao Paulo"),
        ("São País", "São Country"),
        ("İstanbul", "İstanbul"),
        ("東京", "東京"),
        ("São State", "São State"),
        ("Москва", "Москва"),
        ("SA\u{303}O PAULO", "SA\u{303}O PAULO"),
    ];
    (0..256)
        .map(|id| {
            let (en, pt) = names[id % names.len()];
            let kind = match id % 4 {
                0 => LabelKind::Country,
                1 => LabelKind::State,
                2 => LabelKind::City,
                _ => LabelKind::Metro,
            };
            MapLabel {
                lat: id as f64,
                lon: 0.0,
                en: en.into(),
                pt: pt.into(),
                kind,
                rank: (id % 10) as u8,
                big: id % 2 == 0,
            }
        })
        .collect()
}
fn signature(rows: &[Suggestion]) -> Vec<(String, u64, u64, u64)> {
    rows.iter()
        .map(|s| {
            (
                s.label.clone(),
                s.lat.to_bits(),
                s.lon.to_bits(),
                s.alt.to_bits(),
            )
        })
        .collect()
}
#[test]
fn fst_preserves_priority_stable_ties_both_languages_unicode_and_remote_merge() {
    let labels = Arc::new(scene());
    let index = NameIndex::new(labels.clone()).unwrap();
    let remote = [
        Suggestion {
            label: "Sao Paulo".into(),
            lat: 999.0,
            lon: 0.0,
            alt: 999.0,
        },
        Suggestion {
            label: "Remote".into(),
            lat: 1000.0,
            lon: 0.0,
            alt: 5.0,
        },
    ];
    for lang in Lang::ALL {
        for query in [
            "",
            "s",
            "sa",
            "sao",
            "SÃo P",
            "ist",
            "東",
            "東京",
            "мо",
            "Москва",
            "not found",
        ] {
            assert_eq!(
                signature(&index.merge(query, lang, &remote)),
                signature(&merge_suggestions(query, lang, &labels, &remote)),
                "{query} {}",
                lang.code()
            );
        }
    }
}
#[test]
fn async_index_publishes_latest_loaded_names_and_wakes_ui() {
    let runtime = crate::runtime::ServiceRuntime::new().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let search = LocalSearch::new(runtime.handle(), move || {
        let _ = tx.send(());
    });
    let labels = scene();
    search.update(&labels).unwrap();
    rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
    assert!(search.merge("sao", Lang::En, &[]).is_some());
    for _ in 0..20 {
        search.update(&labels).unwrap();
    }
    search.update(&[]).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some(rows) = search.merge("sao", Lang::En, &[]) {
            assert!(rows.is_empty());
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
    }
}
#[test]
#[ignore = "manual prefix index benchmark"]
fn prefix_index_time() {
    let labels: Arc<Vec<_>> = Arc::new(
        (0..16000)
            .map(|id| MapLabel {
                lat: id as f64,
                lon: 0.0,
                en: format!("São place {id}"),
                pt: format!("São lugar {id}"),
                kind: LabelKind::City,
                rank: (id % 10) as u8,
                big: false,
            })
            .collect(),
    );
    let start = std::time::Instant::now();
    let index = NameIndex::new(labels.clone()).unwrap();
    let build = start.elapsed();
    let mut old = std::time::Duration::ZERO;
    let mut new = std::time::Duration::ZERO;
    for _ in 0..100 {
        for query in ["sao", "sao place 12", "saox"] {
            let start = std::time::Instant::now();
            let reference = merge_suggestions(query, Lang::En, &labels, &[]);
            old += start.elapsed();
            let start = std::time::Instant::now();
            let result = index.merge(query, Lang::En, &[]);
            new += start.elapsed();
            assert_eq!(signature(&result), signature(&reference));
        }
    }
    println!(
        "16k names: build {:.3} ms; scan {:.3} ms/query; FST {:.3} ms/query; two language maps {} bytes (label snapshot separate)",
        build.as_secs_f64() * 1000.0,
        old.as_secs_f64() * 1000.0 / 300.0,
        new.as_secs_f64() * 1000.0 / 300.0,
        index.en.as_fst().as_bytes().len() + index.pt.as_fst().as_bytes().len()
    );
}
