use super::App;
use earthcraft_core::language::fold_name;
use earthcraft_data::boundaries::{LabelKind, MapLabel};
use earthcraft_data::search::{geocode, parse_latlon, photon_suggest, Suggestion};
use earthcraft_data::search::{CITY_SEARCH_ALTITUDE_KM, COUNTRY_SEARCH_ALTITUDE_KM};
use eframe::egui;
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
const SUGGESTION_DEBOUNCE_DELAY: Duration = Duration::from_millis(280);
const SUGGESTION_POLL_INTERVAL: Duration = Duration::from_millis(90);

const BIG_STATE_SEARCH_ALTITUDE_KM: f64 = 3000.0;
const STATE_SEARCH_ALTITUDE_KM: f64 = 1200.0;
const METRO_SEARCH_ALTITUDE_KM: f64 = 150.0;
const MAX_LOCAL_SUGGESTIONS: usize = 4;

impl App {
    pub(super) fn on_query_changed(&mut self) {
        let q = self.query.trim().to_string();
        self.sugg_sel = None;
        self.sugg_last_edit = Instant::now();
        if q.is_empty() || parse_latlon(&q).is_some() {
            self.sugg_open = false;
            self.suggestions.clear();
            self.remote_sugg.clear();
            return;
        }
        self.sugg_open = true;
        let fq = fold_name(&q);
        self.remote_sugg
            .retain(|s| fold_name(&s.label).contains(&fq));
        self.rebuild_suggestions();
    }

    /// Instant local matches (countries, states, cities already loaded) + remote results.
    pub(super) fn rebuild_suggestions(&mut self) {
        let fq = fold_name(self.query.trim());
        let mut out: Vec<Suggestion> = Vec::new();
        if fq.chars().count() >= 2 {
            let mut hits: Vec<(u8, u8, usize, &MapLabel)> = Vec::new();
            for l in &self.boundaries.labels {
                let name = fold_name(l.text(self.lang));
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
                let label = l.text(self.lang).to_string();
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
        for r in &self.remote_sugg {
            if out.len() >= 8 {
                break;
            }
            if !out
                .iter()
                .any(|o| fold_name(&o.label) == fold_name(&r.label))
            {
                out.push(r.clone());
            }
        }
        self.suggestions = out;
        if self.sugg_sel.is_some_and(|i| i >= self.suggestions.len()) {
            self.sugg_sel = None;
        }
    }

    pub(super) fn poll_suggestions(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.sugg_rx {
            if let Ok((q, res)) = rx.try_recv() {
                self.sugg_rx = None;
                if let Ok(v) = res {
                    if q == self.query.trim() {
                        self.remote_sugg = v;
                        self.rebuild_suggestions();
                    }
                }
                ctx.request_repaint();
            }
        }
        let q = self.query.trim().to_string();
        if self.sugg_open && q.chars().count() >= 2 && q != self.sugg_sent && self.sugg_rx.is_none()
        {
            if self.sugg_last_edit.elapsed() >= SUGGESTION_DEBOUNCE_DELAY {
                self.sugg_sent = q.clone();
                let (tx, rx) = mpsc::channel();
                self.sugg_rx = Some(rx);
                let (ctx2, lang) = (ctx.clone(), self.lang.code());
                self.runtime.spawn(async move {
                    let r = photon_suggest(&q, lang).await;
                    let _ = tx.send((q, r));
                    ctx2.request_repaint();
                });
            } else {
                ctx.request_repaint_after(SUGGESTION_POLL_INTERVAL);
            }
        }
    }

    pub(super) fn choose_suggestion(&mut self, i: usize) {
        if let Some(s) = self.suggestions.get(i).cloned() {
            self.query = s.label.clone();
            self.sugg_sent = s.label.clone();
            self.sugg_open = false;
            self.sugg_sel = None;
            self.results.clear();
            self.fly_to(s.lat, s.lon, s.alt);
            self.status = format!("{} {}", self.lang.pick("Going to", "Indo para"), s.label);
        }
    }

    pub(super) fn poll_search(&mut self) {
        let lg = self.lang;
        let Some(rx) = &self.search_rx else { return };
        if let Ok(res) = rx.try_recv() {
            self.search_rx = None;
            match res {
                Ok(v) if v.is_empty() => {
                    self.status = lg
                        .pick("No results found.", "Nenhum resultado encontrado.")
                        .into()
                }
                Ok(v) => {
                    self.status = format!("{} {}", v.len(), lg.pick("result(s).", "resultado(s)."));
                    let first = v[0].clone();
                    self.results = v;
                    self.fly_to(first.lat, first.lon, first.alt);
                }
                Err(e) => {
                    self.status = format!("{} {e}", lg.pick("Search error:", "Erro na busca:"))
                }
            }
        }
    }

    pub(super) fn start_search(&mut self, ctx: &egui::Context) {
        let q = self.query.trim().to_string();
        if q.is_empty() {
            return;
        }
        if let Some((lat, lon)) = parse_latlon(&q) {
            self.fly_to(lat, lon, 3.0);
            self.status = format!(
                "{} {lat:.5}, {lon:.5}",
                self.lang.pick("Going to", "Indo para")
            );
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.search_rx = Some(rx);
        self.status = self.lang.pick("Searching…", "Buscando…").into();
        let ctx = ctx.clone();
        let lang_code = self.lang.code();
        self.runtime.spawn(async move {
            let _ = tx.send(geocode(&q, lang_code).await);
            ctx.request_repaint();
        });
    }
}
