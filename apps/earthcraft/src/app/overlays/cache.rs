//! One displayed view; tile generation and unrelated panels do not invalidate it.
use super::projected;
use earthcraft_core::{camera::Basis, language::Lang};
use earthcraft_data::boundaries::BoundaryData;
use eframe::egui::{self, Rect, Shape};

#[derive(Clone, Copy, PartialEq)]
pub(super) struct OverlayLayers {
    pub countries: bool,
    pub states: bool,
    pub cities: bool,
    pub metro: bool,
    pub labels: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) struct OverlayStyle {
    pub layers: OverlayLayers,
    pub map_scale: f32,
    pub lang: Lang,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) struct OverlayView {
    pub basis: Basis,
    pub rect: Rect,
    pub altitude_km: f64,
    pub pixels_per_point: f32,
}

#[derive(Default)]
pub(in crate::app) struct OverlayCache {
    key: Option<(OverlayView, OverlayStyle)>,
    shapes: Vec<Shape>,
}

impl OverlayCache {
    pub(super) fn invalidate(&mut self) {
        self.key = None;
    }

    pub(super) fn draw(
        &mut self,
        painter: &egui::Painter,
        view: OverlayView,
        boundaries: &BoundaryData,
        style: OverlayStyle,
    ) {
        if self.key != Some((view, style)) {
            self.shapes = projected::build(painter, view, boundaries, style);
            self.key = Some((view, style));
        }
        painter.extend(self.shapes.iter().cloned());
    }
}

#[cfg(test)]
mod tests;
