//! Explore profile controls and a normalized live monitor.
use crate::app::{
    App,
    controllers::{AXIS_LABEL_IDS, BUTTONS},
};
use earthcraft_core::controller::{
    ControllerAction, MAX_DEADZONE, MAX_SENSITIVITY, MIN_SENSITIVITY,
};
use eframe::egui;
impl App {
    pub(super) fn settings_controllers(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        ui.checkbox(
            &mut self.settings.controllers.enabled,
            lang.text("controllers-enabled"),
        );
        ui.weak(lang.text("controllers-help"));
        if let Some(error) = &self.controllers.error {
            ui.label(error);
        }
        egui::ComboBox::from_id_salt("controller-device")
            .selected_text(
                self.controllers
                    .devices
                    .iter()
                    .find(|device| device.key == self.settings.controllers.selected)
                    .map(|device| device.name.as_str())
                    .unwrap_or_else(|| lang.text("controllers-disconnected")),
            )
            .show_ui(ui, |ui| {
                for device in &self.controllers.devices {
                    ui.selectable_value(
                        &mut self.settings.controllers.selected,
                        device.key.clone(),
                        &device.name,
                    );
                }
            });
        if self.settings.controllers.selected.is_empty() {
            return;
        }
        let key = self.settings.controllers.selected.clone();
        let profile = self
            .settings
            .controllers
            .profiles
            .entry(key.clone())
            .or_default();
        if ui.button(lang.text("controllers-reset")).clicked() {
            *profile = Default::default();
        }
        if self.controllers.capture.is_some() {
            ui.label(lang.text("controllers-listening"));
            if ui.button(lang.text("settings-cancel-action")).clicked() {
                self.controllers.capture = None;
            }
        }
        if let Some((source, target)) = self.controllers.conflict {
            ui.label(lang.format(
                "controllers-conflict",
                &[
                    ("button", format!("{:?}", BUTTONS[target]).into()),
                    (
                        "action",
                        lang.text(profile.buttons[target].label_id()).into(),
                    ),
                ],
            ));
            ui.horizontal(|ui| {
                if ui.button(lang.text("controllers-replace")).clicked() {
                    profile.move_button(source, target, true);
                    self.controllers.conflict = None;
                }
                if ui.button(lang.text("settings-cancel-action")).clicked() {
                    self.controllers.conflict = None;
                }
            });
        }
        ui.weak(lang.text("controllers-conflicts-help"));
        for (action, binding) in profile.axes.iter_mut().enumerate() {
            ui.push_id(("axis", action), |ui| {
                ui.separator();
                ui.label(lang.text(
                    [
                        "controllers-pan-x",
                        "controllers-pan-y",
                        "controllers-heading",
                        "controllers-zoom-out",
                        "controllers-zoom-in",
                    ][action],
                ));
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("input")
                        .selected_text(
                            binding
                                .input
                                .map(|input| lang.text(AXIS_LABEL_IDS[input]))
                                .unwrap_or_else(|| lang.text("controllers-action-none")),
                        )
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut binding.input,
                                None,
                                lang.text("controllers-action-none"),
                            );
                            for (input, label) in AXIS_LABEL_IDS.iter().enumerate() {
                                ui.selectable_value(
                                    &mut binding.input,
                                    Some(input),
                                    lang.text(label),
                                );
                            }
                        });
                    if ui.button(lang.text("controllers-bind")).clicked() {
                        self.controllers.listen(action, true);
                    }
                    ui.checkbox(&mut binding.inverted, lang.text("controllers-invert"));
                });
                ui.add(
                    egui::Slider::new(&mut binding.deadzone, 0.0..=MAX_DEADZONE)
                        .text(lang.text("controllers-deadzone")),
                );
                ui.add(
                    egui::Slider::new(&mut binding.sensitivity, MIN_SENSITIVITY..=MAX_SENSITIVITY)
                        .text(lang.text("controllers-sensitivity")),
                );
                ui.add(
                    egui::Slider::new(&mut binding.curve, 1.0..=3.0)
                        .text(lang.text("controllers-curve")),
                );
            });
        }
        ui.separator();
        ui.label(lang.text("controllers-buttons"));
        for (input, logical) in profile.buttons.iter_mut().enumerate() {
            ui.push_id(("button", input), |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("{:?}", BUTTONS[input]));
                    egui::ComboBox::from_id_salt("action")
                        .selected_text(lang.text(logical.label_id()))
                        .show_ui(ui, |ui| {
                            for action in ControllerAction::ALL {
                                ui.selectable_value(logical, action, lang.text(action.label_id()));
                            }
                        });
                    if *logical != ControllerAction::None
                        && ui.button(lang.text("controllers-bind")).clicked()
                    {
                        self.controllers.listen(input, false);
                    }
                });
            });
        }
        ui.separator();
        ui.label(lang.text("controllers-monitor"));
        if let Some(device) = self
            .controllers
            .devices
            .iter()
            .find(|device| device.key == key)
        {
            for (input, value) in device.state.axes.iter().enumerate() {
                ui.label(format!("{}: {value:+.3}", lang.text(AXIS_LABEL_IDS[input])));
            }
            for (input, down) in device
                .state
                .buttons
                .iter()
                .enumerate()
                .filter(|(_, down)| **down)
            {
                let _ = down;
                ui.label(format!("{:?}", BUTTONS[input]));
            }
        } else {
            ui.label(lang.text("controllers-disconnected"));
        }
    }
}
