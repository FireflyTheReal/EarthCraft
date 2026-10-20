//! Ordered patch preparation; the UI swaps one latest immutable dataset.
use super::LOCAL_LABEL_RANK;
use super::{BoundaryData, LabelKind, Layer, MapLabel, Patch, dedupe_city_labels};
use std::sync::{Arc, Mutex, mpsc};

pub struct BoundarySnapshot {
    pub data: Arc<BoundaryData>,
    pub names: Arc<Vec<MapLabel>>,
}

pub struct BoundaryUpdates {
    latest: Arc<Mutex<Option<BoundarySnapshot>>>,
}

fn apply(data: &mut BoundaryData, patch: Patch) {
    if patch.replace {
        match patch.layer {
            Layer::Countries => {
                data.countries.clear();
                data.country_regions.clear();
                data.labels.retain(|label| label.kind != LabelKind::Country);
            }
            Layer::States => {
                data.states.clear();
                data.states_big.clear();
                data.labels.retain(|label| label.kind != LabelKind::State);
            }
            Layer::Cities => {
                // Local labels arrive through Layer::WorldCities and outlive this replacement.
                data.cities.clear();
                data.labels.retain(|label| {
                    label.kind != LabelKind::City || label.rank == LOCAL_LABEL_RANK
                });
            }
            Layer::WorldCities | Layer::Metro => {}
        }
    }
    match patch.layer {
        Layer::Countries => {
            data.countries.extend(patch.lines);
            data.country_regions.extend(patch.regions);
            data.countries_loaded = true;
        }
        Layer::States => {
            data.states.extend(patch.lines);
            data.states_big.extend(patch.lines_big);
        }
        Layer::Cities => data.cities.extend(patch.lines),
        Layer::WorldCities => {
            data.world_cities = patch.lines;
            data.labels.retain(|label| {
                !(label.kind == LabelKind::City && label.rank == super::LOCAL_LABEL_RANK)
            });
        }
        Layer::Metro => {
            data.metro = patch.lines;
            data.labels.retain(|label| label.kind != LabelKind::Metro);
        }
    }
    data.labels.extend(patch.labels);
    dedupe_city_labels(&mut data.labels);
}

impl BoundaryUpdates {
    pub fn new(
        patches: mpsc::Receiver<Patch>,
        repaint: impl Fn() + Send + 'static,
    ) -> std::io::Result<Self> {
        let latest = Arc::new(Mutex::new(None));
        let output = Arc::downgrade(&latest);
        std::thread::Builder::new()
            .name("earthcraft-boundaries".into())
            .spawn(move || {
                let mut data = Arc::new(BoundaryData::default());
                while let Ok(patch) = patches.recv() {
                    if output.strong_count() == 0 {
                        return;
                    }
                    apply(Arc::make_mut(&mut data), patch);
                    // Drain already completed patches in order, then publish one snapshot.
                    for patch in patches.try_iter() {
                        apply(Arc::make_mut(&mut data), patch);
                    }
                    let snapshot = BoundarySnapshot {
                        data: data.clone(),
                        names: Arc::new(data.labels.clone()),
                    };
                    let Some(output) = output.upgrade() else {
                        return;
                    };
                    let previous = output.lock().unwrap().replace(snapshot);
                    drop(previous);
                    repaint();
                }
            })?;
        Ok(Self { latest })
    }

    pub fn take_latest(&self) -> Option<BoundarySnapshot> {
        self.latest.lock().unwrap().take()
    }
}

#[cfg(test)]
mod tests {
    use super::super::LabelKind;
    use super::*;
    use std::time::Duration;

    #[test]
    fn patch_preparation_preserves_order_deduplication_and_previous_snapshots() {
        let (tx, rx) = mpsc::channel();
        let (wake, wakes) = mpsc::channel();
        let updates = BoundaryUpdates::new(rx, move || {
            let _ = wake.send(());
        })
        .unwrap();
        let patch = |name: &str, rank| Patch {
            layer: Layer::Countries,
            lines: vec![Arc::new(vec![(0.0, 0.0), (1.0, 1.0)])],
            lines_big: vec![],
            labels: vec![MapLabel {
                lat: 0.0,
                lon: 0.0,
                en: name.into(),
                pt: "São Paulo".into(),
                kind: LabelKind::City,
                rank,
                big: false,
            }],
            regions: vec![],
            replace: false,
        };
        tx.send(patch("First", 5)).unwrap();
        wakes.recv_timeout(Duration::from_secs(10)).unwrap();
        let first = updates.take_latest().unwrap();
        tx.send(patch("Second", 1)).unwrap();
        wakes.recv_timeout(Duration::from_secs(10)).unwrap();
        let second = updates.take_latest().unwrap();
        assert_eq!(first.data.labels[0].en, "First");
        assert_eq!(second.data.labels.len(), 1);
        assert_eq!(second.names[0].en, "Second");
        assert_eq!(second.data.countries.len(), 2);
        assert!(Arc::ptr_eq(
            &first.data.countries[0],
            &second.data.countries[0]
        ));
    }

    #[test]
    fn local_and_metro_patches_replace_their_previous_labels() {
        fn label(name: &str, kind: LabelKind, rank: u8) -> MapLabel {
            MapLabel {
                lat: 10.0,
                lon: 10.0,
                en: name.into(),
                pt: name.into(),
                kind,
                rank,
                big: false,
            }
        }
        let mut data = BoundaryData {
            labels: vec![
                label("Base city", LabelKind::City, 5),
                label("Old local", LabelKind::City, super::super::LOCAL_LABEL_RANK),
                label("Old metro", LabelKind::Metro, 0),
            ],
            ..Default::default()
        };
        apply(
            &mut data,
            Patch {
                layer: Layer::WorldCities,
                lines: vec![],
                lines_big: vec![],
                labels: vec![label(
                    "New local",
                    LabelKind::City,
                    super::super::LOCAL_LABEL_RANK,
                )],
                regions: vec![],
                replace: false,
            },
        );
        assert!(data.labels.iter().any(|label| label.en == "Base city"));
        assert!(data.labels.iter().any(|label| label.en == "Old metro"));
        assert!(data.labels.iter().any(|label| label.en == "New local"));
        assert!(!data.labels.iter().any(|label| label.en == "Old local"));

        apply(
            &mut data,
            Patch {
                layer: Layer::Metro,
                lines: vec![],
                lines_big: vec![],
                labels: vec![label("New metro", LabelKind::Metro, 0)],
                regions: vec![],
                replace: false,
            },
        );
        assert!(!data.labels.iter().any(|label| label.en == "Old metro"));
        assert!(data.labels.iter().any(|label| label.en == "New metro"));
    }

    #[test]
    fn a_detail_upgrade_replaces_outlines_and_their_labels() {
        fn label(name: &str, kind: LabelKind, rank: u8) -> MapLabel {
            MapLabel {
                lat: 0.0,
                lon: 0.0,
                en: name.into(),
                pt: name.into(),
                kind,
                rank,
                big: false,
            }
        }
        let mut data = BoundaryData {
            countries: vec![Arc::new(vec![(0.0, 0.0), (1.0, 1.0)])],
            countries_loaded: true,
            labels: vec![
                label("Old country", LabelKind::Country, 0),
                label("Kept city", LabelKind::City, 5),
            ],
            ..Default::default()
        };
        apply(
            &mut data,
            Patch {
                layer: Layer::Countries,
                lines: vec![
                    Arc::new(vec![(0.0, 0.0), (2.0, 2.0)]),
                    Arc::new(vec![(3.0, 3.0), (4.0, 4.0)]),
                ],
                lines_big: vec![],
                labels: vec![label("New country", LabelKind::Country, 0)],
                regions: vec![],
                replace: true,
            },
        );
        assert_eq!(data.countries.len(), 2);
        assert!(data.countries.iter().all(|line| line[0].1 >= 0.0));
        assert!(!data.labels.iter().any(|l| l.en == "Old country"));
        assert!(data.labels.iter().any(|l| l.en == "New country"));
        assert!(data.labels.iter().any(|l| l.en == "Kept city"));
        assert!(data.countries_loaded);

        // Without the flag the same patch appends, preserving the old behaviour.
        apply(
            &mut data,
            Patch {
                layer: Layer::Countries,
                lines: vec![Arc::new(vec![(5.0, 5.0), (6.0, 6.0)])],
                lines_big: vec![],
                labels: vec![],
                regions: vec![],
                replace: false,
            },
        );
        assert_eq!(data.countries.len(), 3);
    }

    #[test]
    fn a_place_upgrade_keeps_local_detail_labels() {
        let label = |name: &str, rank| MapLabel {
            lat: 0.0,
            lon: 0.0,
            en: name.into(),
            pt: name.into(),
            kind: LabelKind::City,
            rank,
            big: false,
        };
        let mut data = BoundaryData {
            labels: vec![
                label("World city", 5),
                label("Local street", super::super::LOCAL_LABEL_RANK),
            ],
            ..Default::default()
        };
        apply(
            &mut data,
            Patch {
                layer: Layer::Cities,
                lines: vec![],
                lines_big: vec![],
                labels: vec![label("New world city", 3)],
                regions: vec![],
                replace: true,
            },
        );
        assert!(!data.labels.iter().any(|l| l.en == "World city"));
        assert!(data.labels.iter().any(|l| l.en == "New world city"));
        assert!(data.labels.iter().any(|l| l.en == "Local street"));
    }
}
