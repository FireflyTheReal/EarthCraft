use super::App;
use earthcraft_core::{language::Lang, math::now_unix};
use earthcraft_data::kml::{load_kml, save_kml};
use eframe::egui;
use std::path::PathBuf;
impl App {
    pub(super) fn resolve_kml_path(&self, p: &str) -> PathBuf {
        let path = PathBuf::from(p.trim());
        if path.is_absolute() {
            path
        } else {
            PathBuf::from(&self.settings.kml_dir).join(path)
        }
    }

    pub(super) fn save_kml_to(&mut self, path: PathBuf) {
        let lg = self.lang;
        if let Some(d) = path.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        self.status = match save_kml(&path.to_string_lossy(), &self.placemarks) {
            Ok(_) => {
                self.kml_path = path.to_string_lossy().into_owned();
                lg.format(
                    "kml-save-success",
                    &[("path", path.display().to_string().into())],
                )
            }
            Err(e) => Lang::En.format("kml-save-error", &[("error", e.to_string().into())]),
        };
    }

    pub(super) fn load_kml_from(&mut self, path: &std::path::Path) {
        let lg = self.lang;
        match load_kml(&path.to_string_lossy(), lg) {
            Ok(v) => {
                self.status = lg.format(
                    "kml-import-success",
                    &[
                        ("count", v.len().to_string().into()),
                        ("path", path.display().to_string().into()),
                    ],
                );
                self.kml_path = path.to_string_lossy().into_owned();
                if let Some(first) = v.first() {
                    let (la, lo) = (first.lat, first.lon);
                    self.fly_to(la, lo, if v.len() == 1 { 3.0 } else { 2000.0 });
                }
                self.placemarks.extend(v);
            }
            Err(e) => {
                self.status = Lang::En.format("kml-open-error", &[("error", e.to_string().into())])
            }
        }
    }

    pub(super) fn pick_kml_open(&mut self) {
        let dir = PathBuf::from(&self.settings.kml_dir);
        let mut dlg = rfd::FileDialog::new()
            .add_filter("KML", &["kml", "KML"])
            .set_title(self.lang.text("kml-open-dialog-title"));
        if dir.is_dir() {
            dlg = dlg.set_directory(&dir);
        }
        if let Some(path) = dlg.pick_file() {
            self.load_kml_from(&path);
        }
    }

    pub(super) fn pick_kml_save_as(&mut self) {
        let dir = PathBuf::from(&self.settings.kml_dir);
        let name = std::path::Path::new(&self.kml_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("placemarks.kml")
            .to_string();
        let mut dlg = rfd::FileDialog::new()
            .add_filter("KML", &["kml"])
            .set_file_name(name)
            .set_title(self.lang.text("kml-save-dialog-title"));
        if dir.is_dir() {
            dlg = dlg.set_directory(&dir);
        }
        if let Some(mut path) = dlg.save_file() {
            if path.extension().is_none() {
                path.set_extension("kml");
            }
            self.save_kml_to(path);
        }
    }

    pub(super) fn save_screenshot(&mut self) {
        let lg = self.lang;
        self.status = match &self.image {
            Some((w, h, buf)) => {
                let dir = PathBuf::from(&self.settings.screenshot_dir);
                let _ = std::fs::create_dir_all(&dir);
                let path = dir.join(format!("earthcraft_{}.png", now_unix() as u64));
                match image::save_buffer(&path, buf, *w as u32, *h as u32, image::ColorType::Rgba8)
                {
                    Ok(_) => lg.format(
                        "screenshot-save-success",
                        &[("path", path.display().to_string().into())],
                    ),
                    Err(e) => {
                        Lang::En.format("screenshot-save-error", &[("error", e.to_string().into())])
                    }
                }
            }
            None => Lang::En.text("screenshot-empty-error").into(),
        };
    }

    pub(super) fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<egui::DroppedFile> = ctx.input(|i| i.raw.dropped_files.clone());
        for f in dropped {
            let Some(path) = f.path else { continue };
            let is_kml = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("kml"));
            if is_kml {
                self.load_kml_from(&path);
            } else {
                self.status = Lang::En.text("kml-drop-type-error").into();
            }
        }
    }
}
