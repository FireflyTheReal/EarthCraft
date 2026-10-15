//! One bounded frontend worker discovers installed fonts and loads selected faces.
use eframe::egui;
use fontdb::{Database, Family, Query};
use std::sync::{Arc, mpsc};
const MAX_FONT_BYTES: u64 = 64 * 1024 * 1024;
const MAP_FAMILY: &str = "earthcraft-map";
#[derive(Clone, Debug, PartialEq, Eq)]
struct Request {
    ui: String,
    map: String,
}
struct Face {
    bytes: Vec<u8>,
    index: u32,
}
enum Response {
    Families(Vec<String>),
    Loaded {
        request: Request,
        ui: Option<Face>,
        map: Option<Face>,
        errors: Vec<String>,
    },
}
pub(super) struct FontService {
    send: Option<mpsc::SyncSender<Request>>,
    receive: mpsc::Receiver<Response>,
    wanted: Option<Request>,
    pending: Option<Request>,
    pub(super) families: Vec<String>,
    pub(super) discovering: bool,
    pub(super) error: Option<String>,
}
impl FontService {
    pub(super) fn new(ctx: egui::Context) -> Self {
        let (send, requests) = mpsc::sync_channel::<Request>(1);
        let (responses, receive) = mpsc::sync_channel(2);
        let worker = std::thread::Builder::new()
            .name("earthcraft-fonts".into())
            .spawn(move || {
                let mut db = Database::new();
                db.load_system_fonts();
                let mut families: Vec<_> = db
                    .faces()
                    .flat_map(|face| face.families.iter().map(|(name, _)| name.clone()))
                    .collect();
                families.sort();
                families.dedup();
                if responses.send(Response::Families(families)).is_err() {
                    return;
                }
                ctx.request_repaint();
                while let Ok(mut request) = requests.recv() {
                    while let Ok(newer) = requests.try_recv() {
                        request = newer;
                    }
                    let mut errors = Vec::new();
                    let mut load = |family: &str| match load_face(&db, family) {
                        Ok(face) => face,
                        Err(error) => {
                            errors.push(error);
                            None
                        }
                    };
                    let ui = load(&request.ui);
                    let map = load(&request.map);
                    if responses
                        .send(Response::Loaded {
                            request,
                            ui,
                            map,
                            errors,
                        })
                        .is_err()
                    {
                        return;
                    }
                    ctx.request_repaint();
                }
            });
        let (send, error) = match worker {
            Ok(_) => (Some(send), None),
            Err(error) => (
                None,
                Some(format!("Font worker initialization failed: {error}")),
            ),
        };
        Self {
            send,
            receive,
            wanted: None,
            pending: None,
            families: Vec::new(),
            discovering: error.is_none(),
            error,
        }
    }
    pub(super) fn poll(&mut self, ctx: &egui::Context, ui_family: &str, map_family: &str) -> bool {
        let desired = Request {
            ui: ui_family.into(),
            map: map_family.into(),
        };
        if self.wanted.as_ref() != Some(&desired) {
            self.wanted = Some(desired.clone());
            self.pending = Some(desired.clone());
        }
        if let Some(request) = self.pending.take()
            && let Some(send) = &self.send
        {
            match send.try_send(request) {
                Ok(()) => {}
                Err(mpsc::TrySendError::Full(request)) => self.pending = Some(request),
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    self.error = Some("Font worker disconnected.".into());
                    self.send = None;
                }
            }
        }
        let mut changed = false;
        while let Ok(response) = self.receive.try_recv() {
            match response {
                Response::Families(families) => {
                    self.families = families;
                    self.discovering = false;
                }
                Response::Loaded {
                    request,
                    ui,
                    map,
                    errors,
                } if request == desired => {
                    ctx.set_fonts(definitions(ui, map));
                    self.error = if errors.is_empty() {
                        None
                    } else {
                        Some(errors.join("\n"))
                    };
                    changed = true;
                }
                Response::Loaded { .. } => {}
            }
        }
        changed
    }
}
fn load_face(db: &Database, family: &str) -> Result<Option<Face>, String> {
    let families = [if family.is_empty() {
        Family::SansSerif
    } else {
        Family::Name(family)
    }];
    let Some(id) = db.query(&Query {
        families: &families,
        ..Query::default()
    }) else {
        if family.is_empty() {
            return Ok(None);
        }
        return Err(format!(
            "Installed font '{family}' is unavailable; using bundled fonts."
        ));
    };
    if let Some((fontdb::Source::File(path), _)) = db.face_source(id) {
        let size = std::fs::metadata(path)
            .map_err(|error| format!("Font metadata failed: {error}"))?
            .len();
        if size > MAX_FONT_BYTES {
            return Err(format!("Font '{family}' exceeds the font loading limit."));
        }
    }
    let face = db
        .with_face_data(id, |bytes, index| Face {
            bytes: bytes.to_owned(),
            index,
        })
        .ok_or_else(|| format!("Font '{family}' could not be read; using bundled fonts."))?;
    ab_glyph::FontRef::try_from_slice_and_index(&face.bytes, face.index).map_err(|error| {
        format!("Font '{family}' is unsupported: {error}; using bundled fonts.")
    })?;
    Ok(Some(face))
}
fn definitions(ui: Option<Face>, map: Option<Face>) -> egui::FontDefinitions {
    let mut definitions = egui::FontDefinitions::default();
    let mut map_fonts = definitions.families[&egui::FontFamily::Proportional].clone();
    for (name, face, family) in [
        ("installed-ui", ui, egui::FontFamily::Proportional),
        (
            "installed-map",
            map,
            egui::FontFamily::Name(MAP_FAMILY.into()),
        ),
    ] {
        if let Some(face) = face {
            let mut data = egui::FontData::from_owned(face.bytes);
            data.index = face.index;
            definitions.font_data.insert(name.into(), Arc::new(data));
            if family == egui::FontFamily::Proportional {
                definitions
                    .families
                    .get_mut(&family)
                    .expect("default proportional family")
                    .insert(0, name.into());
            } else {
                map_fonts.insert(0, name.into());
            }
        }
    }
    definitions
        .families
        .insert(egui::FontFamily::Name(MAP_FAMILY.into()), map_fonts);
    definitions
}
pub(super) fn map_font(ctx: &egui::Context, size: f32) -> egui::FontId {
    let family = egui::FontFamily::Name(MAP_FAMILY.into());
    let available = ctx.fonts(|fonts| fonts.definitions().families.contains_key(&family));
    egui::FontId::new(
        size,
        if available {
            family
        } else {
            egui::FontFamily::Proportional
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_font_falls_back_and_map_family_is_independent() {
        let db = Database::new();
        assert!(load_face(&db, "missing-test-font").is_err());
        assert!(load_face(&db, "").unwrap().is_none());
        let defs = definitions(None, None);
        assert_eq!(
            defs.families[&egui::FontFamily::Name(MAP_FAMILY.into())],
            defs.families[&egui::FontFamily::Proportional]
        );
        let bytes = defs.font_data.values().next().unwrap().font.to_vec();
        let custom = definitions(
            Some(Face {
                bytes: bytes.clone(),
                index: 0,
            }),
            Some(Face { bytes, index: 0 }),
        );
        assert_eq!(
            custom.families[&egui::FontFamily::Proportional][0],
            "installed-ui"
        );
        assert_eq!(
            custom.families[&egui::FontFamily::Name(MAP_FAMILY.into())][0],
            "installed-map"
        );
        assert!(
            !custom.families[&egui::FontFamily::Name(MAP_FAMILY.into())]
                .contains(&"installed-ui".to_owned())
        );
    }
}
