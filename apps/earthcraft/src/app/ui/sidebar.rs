//! Sidebar shell: navigation and layout stay independent of scene/tool state.
use super::super::App;
use earthcraft_core::language::Lang;
use earthcraft_data::settings::sidebar::{
    MAX_SIDEBAR_WIDTH_POINTS, MIN_PINNED_PLACES_HEIGHT_POINTS, MIN_SIDEBAR_WIDTH_POINTS,
    SidebarLayout, SidebarPanel,
};
use eframe::egui;
mod explore;
mod layers;
mod places;
mod rail;

const HEADER_BUTTON_SIZE_POINTS: f32 = 24.0;
const PINNED_PLACES_MAX_HEIGHT_FRACTION: f32 = 0.65;

impl App {
    pub(in crate::app) fn side_panel(&mut self, ui: &mut egui::Ui) {
        let mut layout = self.settings.sidebar.clone();
        route_shortcuts(
            &mut layout,
            self.place_reveal_requested,
            self.ruler_reveal_requested,
        );
        let mut open = self.sidebar_open;
        self.kml_drop_rect = egui::Rect::NOTHING;
        self.sidebar_animating =
            show_shell(
                ui,
                &mut layout,
                &mut open,
                self.lang,
                |ui, panel| match panel {
                    SidebarPanel::Explore => self.sidebar_explore(ui),
                    SidebarPanel::Layers => self.sidebar_layers(ui),
                    SidebarPanel::Places => self.sidebar_places(ui),
                },
            );
        self.sidebar_open = open;
        self.settings.sidebar = layout;
    }
}

fn show_shell(
    ui: &mut egui::Ui,
    layout: &mut SidebarLayout,
    open: &mut bool,
    lang: Lang,
    mut draw_panel: impl FnMut(&mut egui::Ui, SidebarPanel),
) -> bool {
    rail::show(ui, layout, open, lang);
    let openness = ui.animate_bool_responsive(egui::Id::new("side").with("animation"), *open);
    let animating = openness != if *open { 1.0 } else { 0.0 };
    let mut expanded = *open;
    let mut header_open = expanded;
    let response = egui::Panel::show_switched(
        ui,
        &mut expanded,
        egui::Panel::left("side_collapsed")
            .frame(egui::Frame::NONE)
            .exact_size(0.0),
        expanded_sidebar_panel(layout.width_points),
        |ui, _expanded| {
            if animating || !*open {
                return;
            }
            let panel = layout.panel;
            panel_header(ui, layout, &mut header_open, panel, lang);
            ui.separator();
            if layout.places_pinned && panel != SidebarPanel::Places {
                let max_height = (ui.available_height() * PINNED_PLACES_MAX_HEIGHT_FRACTION)
                    .max(MIN_PINNED_PLACES_HEIGHT_POINTS);
                let pinned = egui::Panel::bottom("pinned-places")
                    .default_size(layout.pinned_places_height_points)
                    .size_range(MIN_PINNED_PLACES_HEIGHT_POINTS..=max_height)
                    .resizable(true)
                    .show(ui, |ui| {
                        panel_header(ui, layout, &mut header_open, SidebarPanel::Places, lang);
                        sidebar_scroll_area("pinned-places-scroll")
                            .show(ui, |ui| draw_panel(ui, SidebarPanel::Places));
                    });
                if !ui.input(|i| i.pointer.any_down()) {
                    layout.pinned_places_height_points = pinned.response.rect.height();
                }
            }
            sidebar_scroll_area(match panel {
                SidebarPanel::Explore => "explore-scroll",
                SidebarPanel::Layers => "layers-scroll",
                SidebarPanel::Places => "places-scroll",
            })
            .show(ui, |ui| draw_panel(ui, panel));
        },
    );
    if *open && !animating && !ui.input(|i| i.pointer.any_down()) {
        layout.width_points = response.response.rect.width();
    }
    *open = expanded && header_open;
    layout.open = *open;
    animating
}

/// Align the expanded sidebar; retain a usable coordinate slot when it is closed.
pub(super) fn outer_right(ui: &egui::Ui, layout: &SidebarLayout, open: bool) -> f32 {
    let rail_width = if layout.rail_visible {
        rail::RAIL_WIDTH_POINTS
    } else {
        0.0
    };
    let left = ui.content_rect().left();
    if open {
        egui::containers::panel::PanelState::load(ui, egui::Id::new("side"))
            .map_or(left + rail_width + layout.width_points, |state| {
                state.outer_rect.right()
            })
    } else {
        left + rail_width + layout.width_points
    }
}

fn panel_title(panel: SidebarPanel, lang: Lang) -> &'static str {
    lang.text(match panel {
        SidebarPanel::Explore => "sidebar-explore-title",
        SidebarPanel::Layers => "layers-section-title",
        SidebarPanel::Places => "sidebar-places-title",
    })
}

fn route_shortcuts(layout: &mut SidebarLayout, place: bool, ruler: bool) {
    if ruler {
        layout.panel = SidebarPanel::Explore;
    } else if place && !layout.places_pinned {
        layout.panel = SidebarPanel::Places;
    }
}

fn panel_header(
    ui: &mut egui::Ui,
    layout: &mut SidebarLayout,
    open: &mut bool,
    panel: SidebarPanel,
    lang: Lang,
) {
    ui.horizontal(|ui| {
        ui.heading(panel_title(panel, lang));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_sized([HEADER_BUTTON_SIZE_POINTS; 2], egui::Button::new("×"))
                .on_hover_text(lang.text(
                    if panel == SidebarPanel::Places && layout.places_pinned {
                        "sidebar-unpin-places-tooltip"
                    } else {
                        "sidebar-hide-tooltip"
                    },
                ))
                .clicked()
            {
                if panel == SidebarPanel::Places && layout.places_pinned {
                    layout.places_pinned = false;
                } else {
                    *open = false;
                }
            }
            if panel == SidebarPanel::Places {
                ui.toggle_value(
                    &mut layout.places_pinned,
                    lang.text("sidebar-pin-places-action"),
                )
                .on_hover_text(lang.text("sidebar-pin-places-tooltip"));
            }
            ui.toggle_value(&mut layout.rail_visible, "☰")
                .on_hover_text(lang.text("sidebar-rail-toggle"));
        });
    });
    if !layout.rail_visible && panel == layout.panel {
        ui.horizontal(|ui| {
            for choice in SidebarPanel::ALL {
                if ui
                    .selectable_label(layout.panel == choice, panel_title(choice, lang))
                    .clicked()
                {
                    layout.panel = choice;
                }
            }
        });
    }
}

fn expanded_sidebar_panel(width_points: f32) -> egui::Panel {
    egui::Panel::left("side")
        .default_size(width_points)
        .size_range(MIN_SIDEBAR_WIDTH_POINTS..=MAX_SIDEBAR_WIDTH_POINTS)
        .resizable(true)
}
fn sidebar_scroll_area(id: &str) -> egui::ScrollArea {
    egui::ScrollArea::both()
        .id_salt(id)
        .auto_shrink([false, false])
}

#[cfg(test)]
mod tests;
