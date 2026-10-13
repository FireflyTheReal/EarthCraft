//! One displayed view; tile generation and unrelated panels do not invalidate it.
use super::projected;
use earthcraft_core::{camera::Basis, language::Lang};
use earthcraft_data::boundaries::BoundaryData;
use eframe::egui::{self, Rect, Shape};

pub(super) use earthcraft_render::overlays::OverlayLayers;

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
    index: Option<earthcraft_render::overlays::BoundaryIndex>,
}

impl OverlayCache {
    pub(super) fn invalidate(&mut self) {
        self.key = None;
        self.index = None;
    }

    pub(super) fn draw(
        &mut self,
        painter: &egui::Painter,
        view: OverlayView,
        boundaries: &BoundaryData,
        style: OverlayStyle,
    ) {
        if self.key != Some((view, style)) {
            let index = self
                .index
                .get_or_insert_with(|| earthcraft_render::overlays::BoundaryIndex::new(boundaries));
            let visible = index.visible(&view.basis, view.rect);
            self.shapes =
                projected::build_indexed(painter, view, boundaries, style, Some(&visible));
            self.key = Some((view, style));
        }
        painter.extend(self.shapes.iter().cloned());
    }
}

#[cfg(test)]
mod tests;
