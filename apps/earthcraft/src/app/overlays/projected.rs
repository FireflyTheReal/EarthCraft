//! Cached administrative border projection and ordered geographic labels.
use super::{OverlayStyle, OverlayView};
use earthcraft_data::boundaries::{BoundaryData, LabelKind};
use earthcraft_render::overlays::{
    project_borders, project_labels, BorderKind, LabelIndex, OverlayProjection, ProjectedLabel,
    MAX_VISIBLE_LABELS,
};
use eframe::egui::{self, Align2, Color32, FontId, Rect, Shape, Stroke, Vec2};
const LABEL_PADDING_POINTS: f32 = 2.0;
const LABEL_HIGHLIGHT_OFFSET_POINTS: f32 = -0.5;

#[cfg(test)]
pub(super) fn build(
    painter: &egui::Painter,
    view: OverlayView,
    boundaries: &BoundaryData,
    style: OverlayStyle,
) -> Vec<Shape> {
    build_indexed(painter, view, boundaries, style, None)
}

pub(super) fn build_indexed(
    painter: &egui::Painter,
    view: OverlayView,
    boundaries: &BoundaryData,
    style: OverlayStyle,
    visible: Option<&earthcraft_render::overlays::VisibleBoundaries>,
) -> Vec<Shape> {
    let (basis, rect) = (&view.basis, view.rect);
    let mut shapes = Vec::new();
    let projection = OverlayProjection {
        basis,
        rect,
        altitude_km: view.altitude_km,
        layers: style.layers,
    };
    project_borders(&projection, boundaries, visible, |kind, segment| {
        let (color, width): (Color32, f32) = match kind {
            BorderKind::Country => (Color32::from_rgba_unmultiplied(255, 235, 150, 235), 1.15),
            BorderKind::State => (Color32::from_rgba_unmultiplied(120, 220, 255, 220), 1.0),
            BorderKind::City => (Color32::from_rgba_unmultiplied(255, 160, 80, 210), 0.7),
            BorderKind::Metro => (Color32::from_rgba_unmultiplied(255, 100, 210, 235), 1.5),
        };
        shapes.push(Shape::line_segment(segment, Stroke::new(width, color)));
    });
    let ms = style.map_scale;
    if style.layers.labels {
        // Collect what is visible, then place by priority (countries > states > metro > cities)
        // skipping any name that would overlap one already drawn.
        let cands = project_labels(&projection, &boundaries.labels);
        let mut placed = LabelIndex::default();
        for ProjectedLabel {
            position: pos,
            label,
        } in cands
        {
            if placed.len() >= MAX_VISIBLE_LABELS {
                break;
            }
            let (color, size) = match label.kind {
                LabelKind::Country => (Color32::WHITE, 12.0),
                LabelKind::State => (Color32::from_rgb(180, 230, 255), 10.0),
                LabelKind::City => (Color32::from_rgb(255, 245, 210), 9.0),
                LabelKind::Metro => (Color32::from_rgb(255, 170, 230), 10.0),
            };
            let font = FontId::proportional(size * ms);
            let text = label.text(style.lang);
            let galley = painter.layout_no_wrap(text.to_string(), font.clone(), color);
            let area = Rect::from_center_size(pos, galley.size()).expand(LABEL_PADDING_POINTS);
            if !placed.try_insert(area) {
                continue;
            }
            if galley.is_empty() {
                continue;
            }
            shapes.push(painter.fonts(|fonts| {
                Shape::text(
                    fonts,
                    pos,
                    Align2::CENTER_CENTER,
                    text,
                    font.clone(),
                    Color32::from_black_alpha(180),
                )
            }));
            shapes.push(painter.fonts(|fonts| {
                Shape::text(
                    fonts,
                    pos + Vec2::new(0.0, LABEL_HIGHLIGHT_OFFSET_POINTS),
                    Align2::CENTER_CENTER,
                    text,
                    font,
                    color,
                )
            }));
        }
    }
    shapes
}
