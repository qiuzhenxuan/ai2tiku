use super::Page;
use eframe::egui::{self, Color32, Pos2, Rect, Shape, Stroke, Vec2};
use std::sync::OnceLock;

const JET_SEGMENTS: usize = 14;
const JET_PARTICLES: usize = 72;

fn quad(mesh: &mut egui::Mesh, points: [Pos2; 4], color: Color32) {
    let start = mesh.vertices.len() as u32;
    for point in points {
        mesh.colored_vertex(point, color);
    }
    mesh.add_triangle(start, start + 1, start + 2);
    mesh.add_triangle(start, start + 2, start + 3);
}

// The immutable mountain profile used to run more than 1,600 exp/sin calls per frame.
// Cache its normalized geometry; only window scaling is repeated during animation.
fn mountain_profiles() -> &'static [[f32; 181]; 3] {
    static PROFILES: OnceLock<[[f32; 181]; 3]> = OnceLock::new();
    PROFILES.get_or_init(|| {
        std::array::from_fn(|layer| {
            std::array::from_fn(|i| {
                let x = i as f32 / 180.0;
                let shift = layer as f32 * 0.06;
                let h = 0.12 * (-((x - 0.73 + shift) / 0.10).powi(2)).exp()
                    + 0.19 * (-((x - 0.90 + shift) / 0.055).powi(2)).exp()
                    + 0.05 * (-((x - 0.22 - shift) / 0.12).powi(2)).exp();
                let edge = ((x * 43.0).sin() * 0.006 + (x * 103.0).sin() * 0.002) * h * 5.0;
                h + edge + layer as f32 * 0.018
            })
        })
    })
}

fn rocket_exhaust(rect: Rect, progress: f32, color: Color32, time: f64) -> egui::Mesh {
    let mut mesh = egui::Mesh::default();
    mesh.vertices
        .reserve(4 * (JET_SEGMENTS + 1) + 4 * JET_PARTICLES);
    mesh.indices.reserve(12 * JET_SEGMENTS + 6 * JET_PARTICLES);
    let s = rect.width() / 24.0;
    let point = |x: f32, y: f32| Pos2::new(rect.left() + x * s, rect.top() + y * s);
    let ink = |alpha: f32| {
        Color32::from_rgba_unmultiplied(
            color.r(),
            color.g(),
            color.b(),
            (alpha * progress * color.a() as f32).round() as u8,
        )
    };
    // A continuous nozzle core and a softer surrounding plume. Small smooth pressure
    // waves flow down the plume; neither the nozzle nor the rocket jitters sideways.
    let length = 12.5 + (time * 8.0).sin() as f32 * 0.35;
    for (outer, opacity) in [(true, 0.24), (false, 0.78)] {
        let start = mesh.vertices.len() as u32;
        for row in 0..=JET_SEGMENTS {
            let t = row as f32 / JET_SEGMENTS as f32;
            let wave = (time * 16.0 - t as f64 * 19.0).sin() as f32;
            let width = if outer {
                (1.65 + t * 3.5) * (1.0 - t).powf(0.75)
            } else {
                (1.15 + t * 0.7) * (1.0 - t) * (1.0 + wave * 0.12 * t)
            };
            let alpha = opacity * (1.0 - t).powf(0.85);
            mesh.colored_vertex(point(12.0 - width, 18.0 + t * length), ink(alpha));
            mesh.colored_vertex(point(12.0 + width, 18.0 + t * length), ink(alpha));
            if row > 0 {
                let at = start + row as u32 * 2;
                mesh.add_triangle(at - 2, at - 1, at);
                mesh.add_triangle(at - 1, at + 1, at);
            }
        }
    }
    // 72 evenly emitted fine streaks, all in the same mesh as the plume. Lifetime
    // fades at both ends so recycling does not blink; travel accelerates downward.
    for i in 0..JET_PARTICLES {
        let phase = (time * 2.4 + i as f64 / JET_PARTICLES as f64).rem_euclid(1.0) as f32;
        let lane = ((i * 37 % 71) as f32 / 35.0 - 1.0).clamp(-1.0, 1.0);
        let spread = 0.45 + phase * phase * 2.8;
        let x = 12.0 + lane * spread;
        let y = 18.0 + 13.0 * (0.4 * phase + 0.6 * phase * phase);
        let fade_in = (phase / 0.06).min(1.0);
        let alpha = fade_in * (1.0 - phase).powf(1.35) * 0.60;
        let width = 0.18 + (i % 3) as f32 * 0.065;
        let length = 0.35 + phase * 0.9;
        quad(
            &mut mesh,
            [
                point(x - width, y),
                point(x + width, y),
                point(x + width * 0.45, y + length),
                point(x - width * 0.45, y + length),
            ],
            ink(alpha),
        );
    }
    mesh
}

// Reversible, staggered sub-strokes within the navigation's 400 ms transition.
// Every part settles by progress = 1; no timers survive pointer or focus changes.
fn stagger(progress: f32, delay: f32) -> f32 {
    let t = ((progress - delay) / (1.0 - delay)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn background(p: &egui::Painter, rect: Rect, dark: bool) {
    let color = if dark {
        Color32::from_white_alpha(4)
    } else {
        Color32::from_black_alpha(5)
    };
    // One batch for the static paper and mountain backdrop, rather than 183 shapes.
    let mut mesh = egui::Mesh::default();
    mesh.vertices.reserve(3 * 181 * 2 + 180 * 4);
    mesh.indices.reserve(3 * 180 * 6 + 180 * 6);
    for (layer, profile) in mountain_profiles().iter().enumerate() {
        let start = mesh.vertices.len() as u32;
        for (i, height) in profile.iter().enumerate() {
            let x = i as f32 / 180.0;
            let alpha = if dark { 5 + layer * 3 } else { 7 + layer * 4 };
            let ink = if dark {
                Color32::from_white_alpha(alpha as u8)
            } else {
                Color32::from_black_alpha(alpha as u8)
            };
            let top = Pos2::new(
                rect.left() + x * rect.width(),
                rect.bottom() - height * rect.height(),
            );
            mesh.colored_vertex(top, ink);
            mesh.colored_vertex(Pos2::new(top.x, rect.bottom()), Color32::TRANSPARENT);
            if i > 0 {
                let at = start + (i * 2) as u32;
                mesh.add_triangle(at - 2, at - 1, at);
                mesh.add_triangle(at - 1, at + 1, at);
            }
        }
    }
    // Sparse paper fibres: deterministic, no animation or full-screen shader.
    for i in 0..180 {
        let x = ((i * 47) % 997) as f32 / 997.0;
        let y = ((i * 113) % 991) as f32 / 991.0;
        let at = Pos2::new(
            rect.left() + x * rect.width(),
            rect.top() + y * rect.height(),
        );
        quad(
            &mut mesh,
            [
                at,
                at + Vec2::new(2.0, 0.4),
                at + Vec2::new(2.0, 0.9),
                at + Vec2::new(0.0, 0.5),
            ],
            color,
        );
    }
    p.add(Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::*;
    fn shapes(page: Page, progress: f32, time: f64) -> Vec<Shape> {
        let ctx = egui::Context::default();
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(100.0))),
                ..Default::default()
            },
            |ctx| {
                let painter = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new("icon_test"),
                ));
                nav_icon(
                    &painter,
                    Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::splat(24.0)),
                    page,
                    progress,
                    Color32::BLACK,
                    time,
                );
            },
        );
        output.shapes.into_iter().map(|s| s.shape).collect()
    }
    #[test]
    fn rocket_rises_without_tilting_and_particles_follow_the_nozzle_downward() {
        let idle = shapes(Page::Deploy, 0.0, 0.2);
        let hover = shapes(Page::Deploy, 1.0, 0.2);
        let (Shape::Path(a), Shape::Path(b)) = (&idle[0], &hover[0]) else {
            panic!("rocket body missing")
        };
        for (a, b) in a.points.iter().zip(&b.points) {
            assert!((a.x - b.x).abs() < 0.001);
            assert!((a.y - b.y - 4.0).abs() < 0.001);
        }
        let Shape::Path(nozzle) = &hover[4] else {
            panic!("nozzle missing")
        };
        let nozzle_y = nozzle.points[1].y;
        let Shape::Mesh(plume) = &hover[5] else {
            panic!("continuous exhaust missing")
        };
        assert_eq!(hover.len(), 6, "Plume and particles share a single mesh");
        assert!(
            plume.vertices.len() > 256,
            "Dense spray must replace the six-dot exhaust"
        );
        assert!(
            plume.vertices.iter().all(|v| {
                v.pos.x.is_finite()
                    && v.pos.y >= nozzle_y
                    && v.pos.y <= nozzle_y + 14.5
                    && (v.pos.x - 22.0).abs() < 3.6
            }),
            "Exhaust must stay below the moving nozzle and within its row"
        );
        let before = shapes(Page::Deploy, 1.0, 0.08);
        let now = shapes(Page::Deploy, 1.0, 0.10);
        let later = shapes(Page::Deploy, 1.0, 0.12);
        let y = |shapes: &[Shape]| {
            let Shape::Mesh(mesh) = &shapes[5] else {
                panic!("missing plume")
            };
            mesh.vertices[2 * (JET_SEGMENTS + 1) * 2].pos.y
        };
        assert!(y(&now) > y(&before));
        assert!(
            y(&later) - y(&now) > y(&now) - y(&before),
            "Particles accelerate downward"
        );
        assert_eq!(
            idle.iter()
                .filter(|s| matches!(s, Shape::Circle(_)))
                .count(),
            1
        );
        assert_eq!(idle, shapes(Page::Deploy, 0.0, 9.0));
    }

    #[test]
    fn exhaust_is_continuous_bounded_and_theme_neutral() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(24.0));
        for color in [Color32::from_gray(29), Color32::from_gray(238)] {
            for tick in 0..120 {
                let mesh = rocket_exhaust(rect, 1.0, color, tick as f64 / 60.0);
                assert!(
                    mesh.vertices
                        .iter()
                        .any(|v| v.pos.y == 18.0 && v.color.a() > 150),
                    "Nozzle core must never blink between particle lifetimes"
                );
                assert!(mesh.vertices.iter().all(|v| {
                    v.pos.x >= 8.4
                        && v.pos.x <= 15.6
                        && v.pos.y >= 18.0
                        && v.pos.y <= 32.5
                        && v.color.r() == v.color.g()
                        && v.color.g() == v.color.b()
                }));
            }
        }
        let invisible = rocket_exhaust(rect, 0.0, Color32::BLACK, 0.2);
        assert!(invisible.vertices.iter().all(|v| v.color.a() == 0));
    }
    #[test]
    fn every_navigation_icon_has_a_distinct_internal_hover_state() {
        for page in Page::ALL {
            assert_ne!(shapes(page, 0.0, 0.2), shapes(page, 1.0, 0.2), "{page:?}");
        }
    }

    #[test]
    fn internal_parts_start_in_sequence_and_restore_exactly() {
        for page in [
            Page::Dashboard,
            Page::Providers,
            Page::Questions,
            Page::Records,
            Page::Stats,
        ] {
            let idle = shapes(page, 0.0, 0.2);
            let early = shapes(page, 0.05, 0.2);
            let (first, later) = match page {
                Page::Records | Page::Stats => (1, 3),
                _ => (0, 1),
            };
            assert_ne!(
                idle[first], early[first],
                "{page:?}: first part should move"
            );
            assert_eq!(
                idle[later], early[later],
                "{page:?}: later part should wait"
            );
            assert_eq!(idle, shapes(page, 0.0, 9.0));
        }
    }
}

pub fn nav_icon(
    p: &egui::Painter,
    rect: Rect,
    page: Page,
    progress: f32,
    color: Color32,
    time: f64,
) {
    let c = rect.center();
    let s = rect.width() / 24.0;
    let stroke = Stroke::new(1.55_f32, color);
    let pt = |x: f32, y: f32| Pos2::new(rect.left() + x * s, rect.top() + y * s);
    let line = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([pt(a.0, a.1), pt(b.0, b.1)], stroke);
    };
    match page {
        Page::Dashboard => {
            for (i, (x, y)) in [(4.0, 4.0), (14.0, 4.0), (4.0, 14.0), (14.0, 14.0)]
                .into_iter()
                .enumerate()
            {
                let d = stagger(progress, i as f32 * 0.0875);
                p.rect_stroke(
                    Rect::from_min_size(
                        pt(
                            x + if x < 12.0 { -d } else { d },
                            y + if y < 12.0 { -d } else { d },
                        ),
                        Vec2::splat(6.0 * s),
                    ),
                    1,
                    stroke,
                    egui::StrokeKind::Inside,
                );
            }
        }
        Page::Deploy => {
            let lift = progress * 4.0 * s;
            let base = rect.translate(Vec2::new(0.0, -lift));
            let q = |x: f32, y: f32| Pos2::new(base.left() + x * s, base.top() + y * s);
            let points = vec![
                q(7.0, 16.0),
                q(7.0, 10.0),
                q(8.0, 6.0),
                q(12.0, 2.0),
                q(16.0, 6.0),
                q(17.0, 10.0),
                q(17.0, 16.0),
                q(7.0, 16.0),
            ];
            p.add(Shape::line(points, stroke));
            p.circle_stroke(q(12.0, 9.0), 2.0 * s, stroke);
            p.add(Shape::line(
                vec![q(7.0, 11.0), q(4.0, 16.0), q(4.0, 20.0), q(8.0, 17.0)],
                stroke,
            ));
            p.add(Shape::line(
                vec![q(17.0, 11.0), q(20.0, 16.0), q(20.0, 20.0), q(16.0, 17.0)],
                stroke,
            ));
            p.add(Shape::line(
                vec![q(9.0, 16.0), q(9.0, 18.0), q(15.0, 18.0), q(15.0, 16.0)],
                stroke,
            ));
            if progress > 0.0 {
                p.add(Shape::mesh(rocket_exhaust(base, progress, color, time)));
            }
        }
        Page::Ocs => {
            let d = progress * 2.0;
            line((8.0, 2.0 - d), (8.0, 8.0 - d));
            line((16.0, 2.0 - d), (16.0, 8.0 - d));
            p.add(Shape::line(
                vec![
                    pt(6.0, 8.0 - d),
                    pt(18.0, 8.0 - d),
                    pt(18.0, 12.0 - d),
                    pt(16.0, 16.0 - d),
                    pt(8.0, 16.0 - d),
                    pt(6.0, 12.0 - d),
                    pt(6.0, 8.0 - d),
                ],
                stroke,
            ));
            line((12.0, 16.0 - d), (12.0, 22.0));
        }
        Page::Providers => {
            for (i, (x, y, r, growth, rotation)) in [
                (10.0, 10.0, 7.0, 0.12, 12.0_f32),
                (19.0, 18.0, 3.0, 0.20, -15.0_f32),
            ]
            .into_iter()
            .enumerate()
            {
                let phase = stagger(progress, i as f32 * 0.225);
                let scale = 1.0 + growth * phase;
                let angle = rotation.to_radians() * phase;
                let star = |dx: f32, dy: f32| {
                    pt(
                        x + (dx * angle.cos() - dy * angle.sin()) * scale,
                        y + (dx * angle.sin() + dy * angle.cos()) * scale,
                    )
                };
                p.add(Shape::line(
                    vec![
                        star(0.0, -r),
                        star(1.5, -1.5),
                        star(r, 0.0),
                        star(1.5, 1.5),
                        star(0.0, r),
                        star(-1.5, 1.5),
                        star(-r, 0.0),
                        star(-1.5, -1.5),
                        star(0.0, -r),
                    ],
                    stroke,
                ));
            }
        }
        Page::Questions => {
            for i in 0..3 {
                let y =
                    6.0 + i as f32 * 6.0 - stagger(progress, i as f32 * 0.0875) * (2 - i) as f32;
                let mut v = Vec::new();
                for j in 0..41 {
                    let a = j as f32 / 40.0 * std::f32::consts::TAU;
                    v.push(pt(12.0 + a.cos() * 8.0, y + a.sin() * 2.8));
                }
                p.add(Shape::line(v, stroke));
            }
            let top = 6.0 - stagger(progress, 0.0) * 2.0;
            line((4.0, top), (4.0, 18.0));
            line((20.0, top), (20.0, 18.0));
        }
        Page::Records => {
            for i in 0..3 {
                let y = 5.0 + i as f32 * 7.0;
                let phase = stagger(progress, i as f32 * 0.0875);
                p.circle_stroke(pt(4.0, y), 1.0 * s, stroke);
                line((9.0 + phase, y), (20.0 + phase * 1.5, y));
            }
        }
        Page::Stats => {
            line((3.0, 22.0), (22.0, 22.0));
            for (i, h) in [8.0, 16.0, 11.0].into_iter().enumerate() {
                let x = 5.0 + i as f32 * 5.5;
                let hh = h * (1.0 + stagger(progress, i as f32 * 0.0875) * 0.12);
                p.rect_stroke(
                    Rect::from_min_max(pt(x, 20.0 - hh), pt(x + 3.2, 20.0)),
                    1,
                    stroke,
                    egui::StrokeKind::Inside,
                );
            }
        }
        Page::Settings => {
            p.circle_stroke(c, 3.0 * s, stroke);
            let a = progress * 150.0_f32.to_radians();
            let mut v = Vec::new();
            for i in 0..33 {
                let t = i as f32 / 32.0 * std::f32::consts::TAU + a;
                let r = if i % 4 < 2 { 9.0 } else { 7.0 };
                v.push(c + Vec2::angled(t) * r * s);
            }
            p.add(Shape::line(v, stroke));
        }
    }
}
