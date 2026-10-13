//! One displayed view; tile generation and unrelated panels do not invalidate it.
use super::projected;
use earthcraft_core::{camera::Basis, language::Lang};
use earthcraft_data::boundaries::BoundaryData;
use earthcraft_render::overlays::{OverlayGeometry, OverlayWorker};
use eframe::egui::{self, Rect, Shape};
use std::sync::Arc;

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

pub(in crate::app) struct OverlayCache {
    key: Option<(OverlayView, OverlayStyle)>,
    shapes: Vec<Shape>,
    geometry: Option<Arc<OverlayGeometry>>,
    requested: Option<(earthcraft_render::overlays::OverlayView, usize)>,
    worker: OverlayWorker,
}

impl OverlayCache {
    pub(in crate::app) fn accept(&mut self, geometry: Arc<OverlayGeometry>) {
        if self
            .geometry
            .as_ref()
            .is_none_or(|old| !old.matches(geometry.view, &geometry.data))
        {
            self.key = None;
            self.geometry = Some(geometry);
        }
    }

    #[cfg(test)]
    pub(super) fn is_ready(&self, view: OverlayView, style: OverlayStyle) -> bool {
        self.key == Some((view, style))
    }
    pub(in crate::app) fn new(ctx: egui::Context) -> Self {
        Self {
            key: None,
            shapes: Vec::new(),
            geometry: None,
            requested: None,
            worker: OverlayWorker::new(move || ctx.request_repaint())
                .expect("start overlay preparation"),
        }
    }

    pub(super) fn invalidate(&mut self) {
        self.key = None;
        self.geometry = None;
        self.requested = None;
    }

    pub(super) fn draw(
        &mut self,
        painter: &egui::Painter,
        view: OverlayView,
        boundaries: &Arc<BoundaryData>,
        style: OverlayStyle,
    ) {
        if self.key != Some((view, style)) {
            let geometry_view = earthcraft_render::overlays::OverlayView {
                basis: view.basis,
                rect: view.rect,
                altitude_km: view.altitude_km,
                layers: style.layers,
            };
            if let Some(completed) = self.worker.take_completed() {
                if completed.matches(geometry_view, boundaries) {
                    self.geometry = Some(completed);
                }
            }
            if let Some(geometry) = self
                .geometry
                .as_ref()
                .filter(|geometry| geometry.matches(geometry_view, boundaries))
            {
                self.shapes = projected::build_prepared(painter, geometry, style);
                self.key = Some((view, style));
            } else {
                let request = (geometry_view, Arc::as_ptr(boundaries) as usize);
                if self.requested != Some(request) {
                    self.worker.submit(geometry_view, boundaries.clone());
                    self.requested = Some(request);
                }
            }
        }
        // Never paint shapes projected for a different displayed camera or layer set.
        if self.key == Some((view, style)) {
            painter.extend(self.shapes.iter().cloned());
        }
    }
}

#[cfg(test)]
mod tests;
