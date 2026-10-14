//! Cached administrative border projection and ordered geographic labels.
use super::OverlayStyle;
#[cfg(test)]
use super::OverlayView;
#[cfg(test)]
use earthcraft_data::boundaries::BoundaryData;
use earthcraft_data::boundaries::LabelKind;
use earthcraft_render::overlays::{BorderKind, LabelIndex, OverlayGeometry, MAX_VISIBLE_LABELS};
use eframe::egui::{self, Align2, Color32, FontId, Rect, Shape, Stroke, Vec2};
const LABEL_PADDING_POINTS: f32 = 2.0;
const LABEL_HIGHLIGHT_OFFSET_POINTS: f32 = -0.5;
const GRID_LABEL_FONT_POINTS: f32 = 12.0;
const GRID_LABEL_BACKGROUND_ALPHA: u8 = 180;

#[cfg(test)]
pub(super) fn build(
    painter: &egui::Painter,
    view: OverlayView,
    boundaries: &BoundaryData,
    style: OverlayStyle,
) -> Vec<Shape> {
    let projection = earthcraft_render::overlays::OverlayProjection {
        basis: &view.basis,
        rect: view.rect,
        altitude_km: view.altitude_km,
        layers: style.layers,
    };
    let mut borders = Vec::new();
    earthcraft_render::overlays::project_borders(&projection, boundaries, None, |kind, points| {
        borders.push((kind, points))
    });
    let labels = earthcraft_render::overlays::project_labels(&projection, &boundaries.labels);
    build_parts(
        painter,
        style,
        &borders,
        &view.grid_step_deg.map_or_else(Vec::new, |step| {
            earthcraft_render::overlays::grid::project_grid_labels(&view.basis, view.rect, step)
        }),
        view.rect,
        labels.iter().map(|label| (label.position, label.label)),
    )
}

pub(super) fn build_prepared(
    painter: &egui::Painter,
    geometry: &OverlayGeometry,
    style: OverlayStyle,
) -> Vec<Shape> {
    build_parts(
        painter,
        style,
        &geometry.borders,
        &geometry.grid_labels,
        geometry.view.rect,
        geometry
            .labels
            .iter()
            .map(|label| (label.position, &geometry.data.labels[label.source_index])),
    )
}

fn build_parts<'a>(
    painter: &egui::Painter,
    style: OverlayStyle,
    borders: &[(BorderKind, [egui::Pos2; 2])],
    grid_labels: &[earthcraft_render::overlays::grid::GridLabel],
    viewport: Rect,
    cands: impl IntoIterator<Item = (egui::Pos2, &'a earthcraft_data::boundaries::MapLabel)>,
) -> Vec<Shape> {
    let mut shapes = Vec::new();
    for (kind, segment) in borders {
        let (color, width): (Color32, f32) = match kind {
            BorderKind::Country => (Color32::from_rgba_unmultiplied(255, 235, 150, 235), 1.15),
            BorderKind::State => (Color32::from_rgba_unmultiplied(120, 220, 255, 220), 1.0),
            BorderKind::City => (Color32::from_rgba_unmultiplied(255, 160, 80, 210), 0.7),
            BorderKind::Metro => (Color32::from_rgba_unmultiplied(255, 100, 210, 235), 1.5),
        };
        shapes.push(Shape::line_segment(*segment, Stroke::new(width, color)));
    }
    let ms = style.map_scale;
    let mut placed = LabelIndex::default();
    if style.layers.labels {
        // Collect what is visible, then place by priority (countries > states > metro > cities)
        // skipping any name that would overlap one already drawn.
        for (pos, label) in cands {
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
            shapes.push(painter.fonts_mut(|fonts| {
                Shape::text(
                    fonts,
                    pos,
                    Align2::CENTER_CENTER,
                    text,
                    font.clone(),
                    Color32::from_black_alpha(180),
                )
            }));
            shapes.push(painter.fonts_mut(|fonts| {
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
    for label in grid_labels {
        use earthcraft_render::overlays::grid::GridAxis;
        let direction = match (label.axis, label.degrees >= 0.0) {
            (GridAxis::Latitude, true) => "compass-north",
            (GridAxis::Latitude, false) => "compass-south",
            (GridAxis::Longitude, true) => "compass-east",
            (GridAxis::Longitude, false) => "compass-west",
        };
        let value = format!("{:.3}", label.degrees.abs());
        let text = if label.degrees == 0.0 {
            "0°".into()
        } else {
            style.lang.format(
                "grid-coordinate-label",
                &[
                    (
                        "degrees",
                        value.trim_end_matches('0').trim_end_matches('.').into(),
                    ),
                    ("direction", style.lang.text(direction).into()),
                ],
            )
        };
        let galley = painter.layout_no_wrap(
            text,
            FontId::monospace(GRID_LABEL_FONT_POINTS * ms),
            Color32::WHITE,
        );
        let half_size = galley.size() * 0.5 + Vec2::splat(LABEL_PADDING_POINTS);
        if half_size.x * 2.0 > viewport.width() || half_size.y * 2.0 > viewport.height() {
            continue;
        }
        let anchor = match label.edge {
            earthcraft_render::overlays::grid::GridLabelEdge::Left => {
                label.position + Vec2::new(half_size.x, 0.0)
            }
            earthcraft_render::overlays::grid::GridLabelEdge::Bottom => {
                label.position - Vec2::new(0.0, half_size.y)
            }
        };
        let position = egui::pos2(
            anchor.x.clamp(
                viewport.left() + half_size.x,
                viewport.right() - half_size.x,
            ),
            anchor.y.clamp(
                viewport.top() + half_size.y,
                viewport.bottom() - half_size.y,
            ),
        );
        let area = Rect::from_center_size(position, galley.size()).expand(LABEL_PADDING_POINTS);
        if !viewport.contains_rect(area) || !placed.try_insert(area) {
            continue;
        }
        shapes.push(Shape::rect_filled(
            area,
            LABEL_PADDING_POINTS,
            Color32::from_black_alpha(GRID_LABEL_BACKGROUND_ALPHA),
        ));
        shapes.push(Shape::galley(
            position - galley.size() * 0.5,
            galley,
            Color32::WHITE,
        ));
    }
    shapes
}
