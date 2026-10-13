//! Ranking and merging place suggestions independently of UI state.
use super::{Suggestion, CITY_SEARCH_ALTITUDE_KM, COUNTRY_SEARCH_ALTITUDE_KM};
use crate::boundaries::{LabelKind, MapLabel};
use earthcraft_core::language::{fold_name, Lang};
const BIG_STATE_SEARCH_ALTITUDE_KM: f64 = 3000.0;
const STATE_SEARCH_ALTITUDE_KM: f64 = 1200.0;
const METRO_SEARCH_ALTITUDE_KM: f64 = 150.0;
const MAX_LOCAL_SUGGESTIONS: usize = 4;

const MAX_SUGGESTIONS: usize = 8;
pub fn merge_suggestions(
    query: &str,
    lang: Lang,
    labels: &[MapLabel],
    remote: &[Suggestion],
) -> Vec<Suggestion> {
    let fq = fold_name(query.trim());
    let mut out: Vec<Suggestion> = Vec::new();
    if fq.chars().count() >= 2 {
        let mut hits: Vec<(u8, u8, usize, &MapLabel)> = Vec::new();
        for l in labels {
            let name = fold_name(l.text(lang));
            let kind_rank = match l.kind {
                LabelKind::Country => 0,
                LabelKind::State => 1,
                LabelKind::City => 2,
                LabelKind::Metro => 3,
            };
            if name.starts_with(&fq) {
                hits.push((kind_rank, l.rank, name.len(), l));
            }
        }
        hits.sort_by_key(|a| (a.1.min(3), a.0, a.2));
        for (_, _, _, l) in hits {
            if out.len() >= MAX_LOCAL_SUGGESTIONS {
                break;
            }
            let label = l.text(lang).to_string();
            if out.iter().any(|o| fold_name(&o.label) == fold_name(&label)) {
                continue;
            }
            let alt = match l.kind {
                LabelKind::Country => COUNTRY_SEARCH_ALTITUDE_KM,
                LabelKind::State => {
                    if l.big {
                        BIG_STATE_SEARCH_ALTITUDE_KM
                    } else {
                        STATE_SEARCH_ALTITUDE_KM
                    }
                }
                LabelKind::Metro => METRO_SEARCH_ALTITUDE_KM,
                LabelKind::City => CITY_SEARCH_ALTITUDE_KM,
            };
            out.push(Suggestion {
                label,
                lat: l.lat,
                lon: l.lon,
                alt,
            });
        }
    }
    for r in remote {
        if out.len() >= MAX_SUGGESTIONS {
            break;
        }
        if !out
            .iter()
            .any(|o| fold_name(&o.label) == fold_name(&r.label))
        {
            out.push(r.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_priority_duplicate_names_and_remote_order_survive_merge() {
        let labels: Vec<_> = [
            ("São Paulo", LabelKind::City, 2),
            ("Sao Paulo", LabelKind::City, 9),
            ("Sao Country", LabelKind::Country, 0),
        ]
        .into_iter()
        .map(|(name, kind, rank)| MapLabel {
            lat: 0.0,
            lon: 0.0,
            en: name.into(),
            pt: name.into(),
            kind,
            rank,
            big: false,
        })
        .collect();
        let remote = vec![
            Suggestion {
                label: "São Paulo".into(),
                lat: 1.0,
                lon: 2.0,
                alt: 100.0,
            },
            Suggestion {
                label: "Sao Remote".into(),
                lat: 3.0,
                lon: 4.0,
                alt: 200.0,
            },
        ];
        let output = merge_suggestions("sao", Lang::En, &labels, &remote);
        assert_eq!(
            output
                .iter()
                .map(|row| row.label.as_str())
                .collect::<Vec<_>>(),
            ["Sao Country", "São Paulo", "Sao Remote"]
        );
        assert_eq!(output[1].alt, CITY_SEARCH_ALTITUDE_KM);
        assert_eq!(output[2].lat, 3.0);
    }
}
