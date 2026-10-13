use super::*;
use earthcraft_core::camera::Cam;
use earthcraft_data::boundaries::{LabelKind, MapLabel};
use eframe::egui::{pos2, vec2, Color32, LayerId, Pos2};
use std::sync::Arc;

fn scene() -> (OverlayView, OverlayStyle, BoundaryData) {
    let cam = Cam {
        lat: 0.0,
        lon: 0.0,
        heading: 0.0,
        alt: 1000.0,
    };
    let view = OverlayView {
        basis: cam.basis(4.0 / 3.0),
        rect: Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0)),
        altitude_km: cam.alt,
        pixels_per_point: 1.0,
    };
    let style = OverlayStyle {
        layers: OverlayLayers {
            countries: true,
            states: true,
            cities: false,
            metro: false,
            labels: true,
        },
        map_scale: 1.0,
        lang: Lang::En,
    };
    let data = BoundaryData {
        countries: vec![Arc::new(vec![(0.0, 0.0), (1.0, 1.0)])],
        labels: vec![label("Country", LabelKind::Country, 0)],
        ..Default::default()
    };
    (view, style, data)
}

fn label(text: &str, kind: LabelKind, rank: u8) -> MapLabel {
    MapLabel {
        lat: 0.0,
        lon: 0.0,
        en: text.into(),
        pt: format!("PT {text}"),
        kind,
        rank,
        big: true,
    }
}

fn texts(shapes: &[Shape]) -> Vec<&str> {
    shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Text(text) if text.fallback_color != Color32::from_black_alpha(180) => {
                Some(text.galley.job.text.as_str())
            }
            _ => None,
        })
        .collect()
}

fn headless(test: impl FnOnce(&egui::Painter)) {
    let ctx = egui::Context::default();
    ctx.begin_frame(egui::RawInput::default());
    test(&ctx.layer_painter(LayerId::background()));
    let _ = ctx.end_frame();
}

#[test]
fn unchanged_view_reuses_shapes_and_boundary_invalidation_rebuilds() {
    headless(|painter| {
        let (view, style, mut data) = scene();
        let mut cache = OverlayCache::default();
        cache.draw(painter, view, &data, style);
        let before = cache.shapes.clone();
        data.labels[0].en = "Replacement".into();
        // A data patch explicitly invalidates. Until then unchanged keys reuse
        // exactly the same projected shapes, including their shared text galleys.
        cache.draw(painter, view, &data, style);
        assert_eq!(cache.shapes, before);
        cache.invalidate();
        cache.draw(painter, view, &data, style);
        assert_eq!(texts(&cache.shapes), ["Replacement"]);
        assert_ne!(cache.shapes, before);
    });
}

#[test]
fn camera_viewport_altitude_density_layers_language_and_map_scale_invalidate() {
    headless(|painter| {
        let (view, style, mut data) = scene();
        let variants = [
            (
                OverlayView {
                    basis: Cam {
                        lat: 0.0,
                        lon: 0.0,
                        heading: 30.0,
                        alt: 1000.0,
                    }
                    .basis(4.0 / 3.0),
                    ..view
                },
                style,
            ),
            (
                OverlayView {
                    rect: view.rect.translate(vec2(25.0, 0.0)),
                    ..view
                },
                style,
            ),
            (
                OverlayView {
                    altitude_km: 2000.0,
                    ..view
                },
                style,
            ),
            (
                OverlayView {
                    pixels_per_point: 2.0,
                    ..view
                },
                style,
            ),
            (
                view,
                OverlayStyle {
                    layers: OverlayLayers {
                        states: false,
                        ..style.layers
                    },
                    ..style
                },
            ),
            (
                view,
                OverlayStyle {
                    lang: Lang::PtBr,
                    ..style
                },
            ),
            (
                view,
                OverlayStyle {
                    map_scale: 1.5,
                    ..style
                },
            ),
        ];
        for (next_view, next_style) in variants {
            let mut cache = OverlayCache::default();
            data.labels[0].en = "Before".into();
            data.labels[0].pt = "Antes".into();
            cache.draw(painter, view, &data, style);
            data.labels[0].en = "After".into();
            data.labels[0].pt = "Depois".into();
            cache.draw(painter, next_view, &data, next_style);
            assert_eq!(
                texts(&cache.shapes),
                [if next_style.lang == Lang::PtBr {
                    "Depois"
                } else {
                    "After"
                }]
            );
        }
    });
}

#[test]
fn projected_shapes_preserve_priority_stable_ties_and_border_endpoints() {
    headless(|painter| {
        let (view, style, mut data) = scene();
        data.labels = vec![
            label("City", LabelKind::City, 0),
            label("State", LabelKind::State, 0),
            label("First country", LabelKind::Country, 1),
            label("Second country", LabelKind::Country, 1),
        ];
        let shapes = projected::build(painter, view, &data, style);
        assert_eq!(texts(&shapes), ["First country"]);
        match &shapes[0] {
            Shape::LineSegment { points, stroke } => {
                assert_eq!(points[0], pos2(400.0, 300.0));
                assert_eq!(
                    points[1],
                    view.basis
                        .project(earthcraft_core::math::ll_to_v3(1.0, 1.0), view.rect)
                        .unwrap()
                );
                assert_eq!(stroke.width, 1.15);
            }
            _ => panic!("country border must precede labels"),
        }
        let hidden = OverlayStyle {
            layers: OverlayLayers {
                countries: false,
                labels: false,
                ..style.layers
            },
            ..style
        };
        assert!(projected::build(painter, view, &data, hidden).is_empty());
        data.labels = vec![
            label("Later rank", LabelKind::Country, 5),
            label("Earlier rank", LabelKind::Country, 0),
        ];
        assert_eq!(
            texts(&projected::build(painter, view, &data, style)),
            ["Earlier rank"]
        );
        data.labels = vec![
            label("City", LabelKind::City, 0),
            label("Metro", LabelKind::Metro, 0),
            label("State", LabelKind::State, 0),
        ];
        let metro_enabled = OverlayStyle {
            layers: OverlayLayers {
                metro: true,
                ..style.layers
            },
            ..style
        };
        assert_eq!(
            texts(&projected::build(painter, view, &data, metro_enabled)),
            ["State"]
        );
        let states_hidden = OverlayStyle {
            layers: OverlayLayers {
                states: false,
                ..metro_enabled.layers
            },
            ..metro_enabled
        };
        assert_eq!(
            texts(&projected::build(painter, view, &data, states_hidden)),
            ["Metro"]
        );
    });
}

#[test]
fn label_limit_remains_five_hundred() {
    headless(|painter| {
        let (mut view, style, mut data) = scene();
        view.rect = Rect::from_min_size(Pos2::ZERO, vec2(50_000.0, 50_000.0));
        view.altitude_km = 14_000.0;
        view.basis = Cam {
            lat: 0.0,
            lon: 0.0,
            heading: 0.0,
            alt: view.altitude_km,
        }
        .basis(1.0);
        data.labels = (0..625)
            .map(|i| MapLabel {
                lat: -30.0 + (i / 25) as f64 * 2.4,
                lon: -30.0 + (i % 25) as f64 * 2.4,
                ..label("Z", LabelKind::Country, 0)
            })
            .collect();
        assert_eq!(
            texts(&projected::build(painter, view, &data, style)).len(),
            500
        );
    });
}
