use earthcraft_core::language::Lang;
use earthcraft_data::aircraft::{AircraftSnapshot, ProviderStatus};
use eframe::egui;

fn provider_text(status: &ProviderStatus, local: bool, lang: Lang) -> String {
    match status.loaded {
        Some(loaded) => lang.format(
            "aircraft-updated-status",
            &[(
                "minutes",
                (loaded.elapsed().as_secs() / 60).to_string().into(),
            )],
        ),
        None => lang
            .text(if local && !status.attempted {
                "aircraft-zoom-in-status"
            } else {
                "aircraft-loading-status"
            })
            .into(),
    }
}

pub(in crate::app) fn draw_status(ui: &mut egui::Ui, snapshot: &AircraftSnapshot, lang: Lang) {
    ui.small(lang.format(
        "aircraft-summary-status",
        &[
            ("count", snapshot.aircraft.len().to_string().into()),
            (
                "global",
                provider_text(&snapshot.global, false, lang).into(),
            ),
            ("local", provider_text(&snapshot.local, true, lang).into()),
        ],
    ));
    for (source, error) in [
        ("OpenSky", &snapshot.global.error),
        ("ADSB.lol", &snapshot.local.error),
        ("Aircraft preparation", &snapshot.error),
    ] {
        if let Some(error) = error {
            ui.small(Lang::En.format(
                "aircraft-source-error",
                &[
                    ("source", source.into()),
                    ("error", error.to_string().into()),
                ],
            ));
        }
    }
}
