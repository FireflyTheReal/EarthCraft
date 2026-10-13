//! Immutable FST prefix indexes, rebuilt off the UI thread when loaded names change.
use super::{
    local::{
        label_altitude, merge_remote, MAX_LOCAL_SUGGESTIONS, MAX_RANK_PRIORITY,
        MIN_LOCAL_PREFIX_CHARS,
    },
    Suggestion,
};
use crate::boundaries::{LabelKind, MapLabel};
use earthcraft_core::language::{fold_name, Lang};
use fst::{automaton::Str, Automaton, IntoStreamer, Streamer};
use std::{
    collections::{BTreeMap, BinaryHeap},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

fn kind_rank(label: &MapLabel) -> u8 {
    match label.kind {
        LabelKind::Country => 0,
        LabelKind::State => 1,
        LabelKind::City => 2,
        LabelKind::Metro => 3,
    }
}
#[derive(Eq, PartialEq, Ord, PartialOrd)]
struct LabelPriority {
    rank: u8,
    kind: u8,
    name_bytes: usize,
    source_id: usize,
}

struct NameIndex {
    labels: Arc<Vec<MapLabel>>,
    en: fst::Map<Vec<u8>>,
    pt: fst::Map<Vec<u8>>,
}
impl NameIndex {
    fn new(labels: Arc<Vec<MapLabel>>) -> Result<Self, crate::Error> {
        fn build(labels: &[MapLabel], lang: Lang) -> Result<fst::Map<Vec<u8>>, crate::Error> {
            let mut best = BTreeMap::<String, usize>::new();
            for (id, label) in labels.iter().enumerate() {
                let key = fold_name(label.text(lang));
                let order = |id: usize| {
                    let row = &labels[id];
                    (row.rank.min(MAX_RANK_PRIORITY), kind_rank(row), id)
                };
                best.entry(key)
                    .and_modify(|current| {
                        if order(id) < order(*current) {
                            *current = id;
                        }
                    })
                    .or_insert(id);
            }
            Ok(fst::Map::from_iter(
                best.into_iter().map(|(key, id)| (key, id as u64)),
            )?)
        }
        let en = build(&labels, Lang::En)?;
        let pt = build(&labels, Lang::PtBr)?;
        Ok(Self { labels, en, pt })
    }
    fn merge(&self, query: &str, lang: Lang, remote: &[Suggestion]) -> Vec<Suggestion> {
        let prefix = fold_name(query.trim());
        let mut out = Vec::new();
        if prefix.chars().count() >= MIN_LOCAL_PREFIX_CHARS {
            let index = match lang {
                Lang::En => &self.en,
                Lang::PtBr => &self.pt,
            };
            let mut stream = index.search(Str::new(&prefix).starts_with()).into_stream();
            let mut hits = BinaryHeap::with_capacity(MAX_LOCAL_SUGGESTIONS);
            while let Some((key, id)) = stream.next() {
                let label = &self.labels[id as usize];
                let priority = LabelPriority {
                    rank: label.rank.min(MAX_RANK_PRIORITY),
                    kind: kind_rank(label),
                    name_bytes: key.len(),
                    source_id: id as usize,
                };
                if hits.len() < MAX_LOCAL_SUGGESTIONS {
                    hits.push(priority);
                } else if hits.peek().is_some_and(|worst| priority < *worst) {
                    hits.pop();
                    hits.push(priority);
                }
            }
            // FST traversal is lexical; preserve the original rank, kind, byte
            // length and stable source-order ties. Keep only the best four candidates,
            // avoiding a full sort/allocation for very broad prefixes.
            for priority in hits.into_sorted_vec() {
                let label = &self.labels[priority.source_id];
                out.push(Suggestion {
                    label: label.text(lang).into(),
                    lat: label.lat,
                    lon: label.lon,
                    alt: label_altitude(label),
                });
            }
        }
        merge_remote(out, remote)
    }
}
struct Snapshot {
    generation: u64,
    labels: Arc<Vec<MapLabel>>,
}
#[derive(Default)]
struct Published {
    generation: u64,
    index: Option<Arc<NameIndex>>,
    error: Option<crate::Error>,
}

pub struct LocalSearch {
    snapshots: tokio::sync::watch::Sender<Option<Arc<Snapshot>>>,
    generation: AtomicU64,
    published: Arc<Mutex<Published>>,
}
impl LocalSearch {
    pub fn new(
        runtime: &tokio::runtime::Handle,
        repaint: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let (snapshots, mut receiver) = tokio::sync::watch::channel::<Option<Arc<Snapshot>>>(None);
        let published = Arc::new(Mutex::new(Published::default()));
        let output = published.clone();
        runtime.spawn(async move {
            while receiver.changed().await.is_ok() {
                let Some(snapshot) = receiver.borrow_and_update().clone() else {
                    continue;
                };
                let generation = snapshot.generation;
                let result = crate::cpu::run(crate::cpu::Service::Features, move || {
                    NameIndex::new(snapshot.labels.clone())
                })
                .await;
                let mut state = output.lock().unwrap();
                *state = match result {
                    Ok(index) => Published {
                        generation,
                        index: Some(Arc::new(index)),
                        error: None,
                    },
                    Err(error) => Published {
                        generation,
                        index: None,
                        error: Some(error),
                    },
                };
                drop(state);
                repaint();
            }
        });
        Self {
            snapshots,
            generation: AtomicU64::new(0),
            published,
        }
    }
    /// Coalesce pending rebuilds; one immutable name snapshot is built at a time.
    pub fn update(&self, labels: &[MapLabel]) -> Result<(), crate::Error> {
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        self.snapshots
            .send(Some(Arc::new(Snapshot {
                generation,
                labels: Arc::new(labels.to_vec()),
            })))
            .map_err(|_| crate::Error::from("local prefix index worker is unavailable"))
    }
    /// None while a current index is being built; callers retain the original scan fallback.
    pub fn merge(&self, query: &str, lang: Lang, remote: &[Suggestion]) -> Option<Vec<Suggestion>> {
        let state = self.published.lock().unwrap();
        if state.generation != self.generation.load(Ordering::Acquire) {
            return None;
        }
        let index = state.index.clone()?;
        drop(state);
        Some(index.merge(query, lang, remote))
    }
    pub fn take_error(&self) -> Option<crate::Error> {
        self.published.lock().unwrap().error.take()
    }
}

#[cfg(test)]
mod tests;
