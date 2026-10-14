use earthcraft_render::aircraft::AircraftGeometry;
use eframe::egui::{self, Align2, Color32, FontId, Shape, Stroke, Vec2};

const TRAIL_WIDTH_POINTS: f32 = 1.2;
const OUTLINE_WIDTH_POINTS: f32 = 0.9;
const LABEL_FONT_POINTS: f32 = 11.0;
const LABEL_OFFSET_POINTS: Vec2 = Vec2::new(9.0, -6.0);
const SHADOW_OFFSET_POINTS: Vec2 = Vec2::new(1.0, 1.0);
const AIRCRAFT_FILL: Color32 = Color32::from_rgb(255, 206, 45);
const AIRCRAFT_OUTLINE: Color32 = Color32::from_rgb(36, 38, 39);
const LABEL_COLOR: Color32 = Color32::from_rgb(255, 235, 149);
const TRAIL_RGBA: [u8; 4] = [255, 197, 33, 95];

pub(super) fn build(painter: &egui::Painter, geometry: &AircraftGeometry) -> Vec<Shape> {
    #[cfg(feature = "profiling")]
    let _profile = earthcraft_data::profiling::Span::new("ui.aircraft_shapes");
    let ms = geometry.view.map_scale;
    let [r, g, b, a] = TRAIL_RGBA;
    let trail_stroke = Stroke::new(
        TRAIL_WIDTH_POINTS * ms,
        Color32::from_rgba_unmultiplied(r, g, b, a),
    );
    let outline = Stroke::new(OUTLINE_WIDTH_POINTS * ms, AIRCRAFT_OUTLINE);
    let mut output = Vec::new();
    let mut icons = Vec::new();
    let mut labels = geometry.labels.iter().copied().peekable();
    for (index, flight) in geometry.flights.iter().enumerate() {
        icons.push(Shape::line_segment(flight.trail, trail_stroke));
        icons.push(Shape::convex_polygon(
            flight.icon.to_vec(),
            AIRCRAFT_FILL,
            outline,
        ));
        if labels.peek() == Some(&index) {
            labels.next();
            flush_icons(painter, &mut icons, &mut output);
            let sample = &geometry.data.aircraft[flight.source_index];
            let code = if sample.callsign.is_empty() {
                sample.hex.to_uppercase()
            } else {
                sample.callsign.to_string()
            };
            let position = flight.position + LABEL_OFFSET_POINTS * ms;
            for (offset, color) in [
                (SHADOW_OFFSET_POINTS, Color32::BLACK),
                (Vec2::ZERO, LABEL_COLOR),
            ] {
                output.push(painter.fonts_mut(|fonts| {
                    Shape::text(
                        fonts,
                        position + offset,
                        Align2::LEFT_BOTTOM,
                        &code,
                        FontId::proportional(LABEL_FONT_POINTS * ms),
                        color,
                    )
                }));
            }
        }
    }
    flush_icons(painter, &mut icons, &mut output);
    output
}

fn flush_icons(painter: &egui::Painter, icons: &mut Vec<Shape>, output: &mut Vec<Shape>) {
    if icons.is_empty() {
        return;
    }
    let clipped = std::mem::take(icons)
        .into_iter()
        .map(|shape| egui::epaint::ClippedShape {
            clip_rect: painter.clip_rect(),
            shape,
        })
        .collect();
    // Only untextured icons/trails are cached as meshes. Text keeps its Galley so
    // font-atlas growth cannot invalidate baked glyph UVs. Flush before each label
    // to retain the original icon/label drawing order and upstream tessellation.
    for primitive in painter
        .ctx()
        .tessellate(clipped, painter.ctx().pixels_per_point())
    {
        match primitive.primitive {
            egui::epaint::Primitive::Mesh(mesh) => output.push(Shape::mesh(mesh)),
            egui::epaint::Primitive::Callback(_) => {
                unreachable!("icons contain only paths and lines")
            }
        }
    }
}
