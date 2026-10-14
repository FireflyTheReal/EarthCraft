//! Headless egui passes intentionally discard texture uploads instead of rendering.
use eframe::egui;

pub(crate) fn run(
    ctx: &egui::Context,
    input: egui::RawInput,
    ui: impl FnMut(&mut egui::Ui),
) -> egui::FullOutput {
    let mut output = ctx.run_ui(input, ui);
    output.textures_delta.clear();
    output
}
