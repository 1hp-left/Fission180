//! Original vector drawings reconstructed from the observed control silhouettes.
use eframe::egui::{self, Color32 as C, Painter, Pos2, Shape, Stroke, Vec2, vec2};
#[derive(Clone, Copy, PartialEq)]
pub enum Icon {
    Home,
    Grid,
    File,
    Save,
    Undo,
    Redo,
    Sketch,
    Extrude,
    Revolve,
    Hole,
    Pattern,
    Sphere,
    Scissors,
    Cube,
    Shell,
    Move,
    Table,
    Configure,
    Construct,
    Measure,
    Insert,
    Assemble,
    Select,
    Line,
    Rectangle,
    Circle,
    Spline,
    Polygon,
    Dimension,
    Fillet,
    Offset,
    Constraint,
    Equal,
    Parallel,
    Check,
    Eye,
    Folder,
    Gear,
    Orbit,
    Pan,
    Zoom,
    Fit,
    Display,
    Camera,
    History,
    Help,
    Bell,
}

pub fn icon(p: &Painter, c: Pos2, size: f32, kind: Icon) {
    let scale = size / 32.;
    let pt = |x: f32, y: f32| c + vec2(x * scale, y * scale);
    let pale = C::from_rgb(210, 218, 222);
    let blue = C::from_rgb(134, 202, 239);
    let cyan = C::from_rgb(97, 186, 232);
    let gray = C::from_rgb(160, 177, 194);
    let green = C::from_rgb(98, 199, 129);
    let line = |a: (f32, f32), b: (f32, f32), col: C, w: f32| {
        p.line_segment([pt(a.0, a.1), pt(b.0, b.1)], Stroke::new(w * scale, col));
    };
    let poly = |v: &[(f32, f32)], fill: C, stroke: C| {
        p.add(Shape::convex_polygon(
            v.iter().map(|&(x, y)| pt(x, y)).collect(),
            fill,
            Stroke::new(0.8 * scale, stroke),
        ));
    };
    let rect = |x: f32, y: f32, w: f32, h: f32, fill: C, col: C| {
        let r = egui::Rect::from_min_size(pt(x, y), vec2(w * scale, h * scale));
        p.rect(
            r,
            0.,
            fill,
            Stroke::new(scale, col),
            egui::StrokeKind::Inside,
        );
    };
    let cube = |col: C| {
        poly(
            &[(-11., -7.), (0., -13.), (12., -7.), (1., -1.)],
            col.gamma_multiply(1.1),
            pale,
        );
        poly(
            &[(-11., -7.), (1., -1.), (1., 13.), (-11., 7.)],
            col.gamma_multiply(0.76),
            pale,
        );
        poly(&[(1., -1.), (12., -7.), (12., 7.), (1., 13.)], col, pale);
    };
    match kind {
        Icon::Cube | Icon::Extrude | Icon::Shell | Icon::Configure | Icon::Hole => {
            cube(if kind == Icon::Extrude {
                blue
            } else {
                C::from_rgb(213, 218, 220)
            });
            if kind == Icon::Extrude {
                line((8., -13.), (8., -20.), pale, 2.);
                poly(&[(4., -17.), (8., -22.), (12., -17.)], pale, pale);
            }
            if kind == Icon::Shell {
                rect(-6., -4., 13., 13., C::from_rgb(69, 85, 102), blue);
            }
            if kind == Icon::Hole {
                p.circle_filled(pt(1., 2.), 6. * scale, C::from_rgb(57, 67, 79));
                p.circle_stroke(pt(1., 2.), 6. * scale, Stroke::new(1.5 * scale, blue));
            }
        }
        Icon::Sketch => {
            rect(-13., -11., 22., 23., C::TRANSPARENT, pale);
            line((-15., -7.), (12., -7.), gray, 0.7);
            line((-9., -15.), (-9., 14.), gray, 0.7);
            line((-9., 8.), (5., -6.), blue, 1.8);
            line((6., 8.), (6., 20.), green, 4.);
            line((0., 14.), (12., 14.), green, 4.);
        }
        Icon::Rectangle => {
            rect(-13., -9., 26., 18., C::TRANSPARENT, pale);
            for (x, y) in [(-13., -9.), (13., -9.), (13., 9.), (-13., 9.)] {
                rect(x - 1.5, y - 1.5, 3., 3., cyan, cyan);
            }
        }
        Icon::Circle => {
            p.circle_stroke(c, 12. * scale, Stroke::new(scale, pale));
            line((-8., 8.), (8., -8.), cyan, 1.);
            p.circle_filled(c, 1.5 * scale, cyan);
            for (x, y) in [(-9., 8.), (9., -8.)] {
                p.circle_filled(pt(x, y), 2. * scale, cyan);
            }
        }
        Icon::Line | Icon::Spline => {
            let points = if kind == Icon::Line {
                vec![
                    pt(-13., 8.),
                    pt(3., 8.),
                    pt(13., 1.),
                    pt(10., -9.),
                    pt(2., -9.),
                ]
            } else {
                vec![
                    pt(-13., 13.),
                    pt(-9., -8.),
                    pt(-3., -9.),
                    pt(4., 7.),
                    pt(11., 3.),
                    pt(14., -13.),
                ]
            };
            p.add(Shape::line(points.clone(), Stroke::new(scale, pale)));
            for point in points.iter().step_by(2) {
                p.circle_filled(*point, 1.6 * scale, cyan);
            }
        }
        Icon::Polygon => {
            poly(&[(-12., 12.), (0., -13.), (13., 12.)], C::TRANSPARENT, pale);
            line((0., -13.), (0., 12.), cyan, 1.);
        }
        Icon::Dimension | Icon::Measure => {
            line((-14., -7.), (14., -7.), pale, 1.);
            line((-13., -13.), (-13., 12.), pale, 1.);
            line((13., -13.), (13., 12.), pale, 1.);
            line((-10., -10.), (-13., -7.), pale, 1.);
            line((10., -4.), (13., -7.), pale, 1.);
            if kind == Icon::Measure {
                rect(-14., 3., 29., 7., C::from_rgb(226, 201, 109), pale);
                for i in 0..7 {
                    line(
                        (-11. + i as f32 * 4., 3.),
                        (-11. + i as f32 * 4., 6.),
                        C::from_rgb(80, 75, 61),
                        1.,
                    );
                }
            }
        }
        Icon::Scissors => {
            p.circle_stroke(pt(-5., 8.), 4. * scale, Stroke::new(2. * scale, pale));
            p.circle_stroke(pt(5., 8.), 4. * scale, Stroke::new(2. * scale, pale));
            line((-3., 5.), (10., -13.), pale, 2.);
            line((3., 5.), (-10., -13.), pale, 2.);
        }
        Icon::Sphere | Icon::Revolve => {
            p.circle_filled(c, 11. * scale, blue);
            p.circle_stroke(c, 11. * scale, Stroke::new(scale, pale));
            p.add(Shape::ellipse_stroke(
                c,
                vec2(11., 5.) * scale,
                Stroke::new(scale, pale),
            ));
            if kind == Icon::Revolve {
                line((-15., -1.), (-15., 9.), pale, 1.5);
                poly(&[(-18., 7.), (-14., 12.), (-11., 6.)], pale, pale);
            }
        }
        Icon::Move | Icon::Pan => {
            for (x, y) in [(0., -13.), (0., 13.), (-13., 0.), (13., 0.)] {
                line((0., 0.), (x, y), pale, 2.);
                let d = Vec2::new(x, y).normalized();
                let n = vec2(-d.y, d.x);
                let tip = pt(x, y);
                p.add(Shape::convex_polygon(
                    vec![
                        tip,
                        tip - d * 5. * scale + n * 3. * scale,
                        tip - d * 5. * scale - n * 3. * scale,
                    ],
                    pale,
                    Stroke::NONE,
                ));
            }
        }
        Icon::Pattern | Icon::Assemble => {
            for (x, y) in [(-10., -10.), (7., -10.), (-10., 7.), (7., 7.)] {
                rect(x, y, 7., 7., blue, pale);
            }
            line((-3., -6.), (7., -6.), gray, 1.);
            line((-6., -3.), (-6., 7.), gray, 1.);
        }
        Icon::Construct => {
            poly(
                &[(-13., -10.), (-3., -15.), (-3., 10.), (-13., 15.)],
                C::from_rgb(179, 204, 181),
                pale,
            );
            poly(
                &[(-1., -11.), (7., -15.), (7., 10.), (-1., 14.)],
                C::from_rgb(219, 219, 187),
                pale,
            );
            poly(
                &[(9., -9.), (14., -12.), (14., 12.), (9., 15.)],
                C::from_rgb(197, 179, 186),
                pale,
            );
        }
        Icon::Insert => {
            rect(-13., -12., 27., 25., blue, pale);
            poly(
                &[(-12., 11.), (-3., 0.), (3., 7.), (8., 2.), (13., 11.)],
                C::from_rgb(64, 117, 174),
                blue,
            );
            p.circle_filled(pt(6., -5.), 3. * scale, C::from_rgb(233, 223, 136));
        }
        Icon::Select => {
            rect(
                -14.,
                -14.,
                25.,
                25.,
                C::TRANSPARENT,
                C::from_rgb(93, 166, 127),
            );
            rect(-10., -10., 17., 17., C::TRANSPARENT, gray);
            poly(
                &[(1., 0.), (5., 16.), (9., 10.), (15., 10.)],
                pale,
                C::from_rgb(39, 47, 59),
            );
        }
        Icon::Fillet | Icon::Offset => {
            if kind == Icon::Fillet {
                line((-13., 14.), (-13., -1.), pale, 1.);
                let pts = (0..=12)
                    .map(|i| {
                        let t = i as f32 / 12. * std::f32::consts::FRAC_PI_2;
                        pt(0. - 13. * t.cos(), 0. - 13. * t.sin())
                    })
                    .collect();
                p.add(Shape::line(pts, Stroke::new(scale, blue)));
                line((0., -13.), (13., -13.), pale, 1.);
            } else {
                for j in [-5., 0., 5.] {
                    let pts = (0..=16)
                        .map(|i| {
                            let t = -std::f32::consts::FRAC_PI_2
                                + i as f32 / 16. * std::f32::consts::PI;
                            pt(8. - (12. + j) * t.cos(), (12. + j) * t.sin())
                        })
                        .collect();
                    p.add(Shape::line(pts, Stroke::new(scale, blue)));
                }
            }
        }
        Icon::Constraint => {
            line((-10., -14.), (-10., 13.), C::from_rgb(213, 132, 141), 2.);
            for y in [3., 7., 11.] {
                line((-15., y), (-6., y - 4.), C::from_rgb(213, 132, 141), 1.);
            }
            line((-3., 12.), (13., 12.), C::from_rgb(213, 132, 141), 2.);
        }
        Icon::Equal => {
            line((-10., -4.), (10., -4.), C::from_rgb(222, 140, 149), 3.);
            line((-10., 5.), (10., 5.), pale, 3.);
        }
        Icon::Parallel => {
            line((-9., 10.), (7., -10.), C::from_rgb(222, 140, 149), 1.5);
            line((-2., 10.), (14., -10.), pale, 1.5);
        }
        Icon::Check => {
            p.circle_filled(c, 14. * scale, green);
            line((-7., 0.), (-2., 5.), C::WHITE, 3.);
            line((-2., 5.), (7., -6.), C::WHITE, 3.);
        }
        Icon::Home => {
            poly(
                &[
                    (-12., -1.),
                    (0., -12.),
                    (12., -1.),
                    (9., -1.),
                    (9., 12.),
                    (2., 12.),
                    (2., 3.),
                    (-3., 3.),
                    (-3., 12.),
                    (-9., 12.),
                    (-9., -1.),
                ],
                gray,
                gray,
            );
        }
        Icon::Grid | Icon::Table => {
            for x in 0..3 {
                for y in 0..3 {
                    rect(
                        -12. + x as f32 * 9.,
                        -12. + y as f32 * 9.,
                        6.,
                        6.,
                        gray,
                        gray,
                    );
                }
            }
        }
        Icon::File => {
            poly(
                &[(-9., -13.), (3., -13.), (10., -6.), (10., 13.), (-9., 13.)],
                gray,
                gray,
            );
            line((3., -13.), (3., -6.), pale, 1.);
            line((3., -6.), (10., -6.), pale, 1.);
        }
        Icon::Save => {
            rect(-12., -12., 24., 24., gray, gray);
            rect(-6., -11., 12., 9., C::from_rgb(60, 70, 86), gray);
            rect(-7., 3., 14., 9., C::from_rgb(60, 70, 86), gray);
        }
        Icon::Undo | Icon::Redo => {
            let sign = if kind == Icon::Undo { 1. } else { -1. };
            let pts = vec![
                pt(-10. * sign, -3.),
                pt(0., -7.),
                pt(9. * sign, -2.),
                pt(12. * sign, 4.),
            ];
            p.add(Shape::line(pts, Stroke::new(2. * scale, gray)));
            poly(
                &[(-13. * sign, -3.), (-5. * sign, -9.), (-5. * sign, 2.)],
                gray,
                gray,
            );
        }
        Icon::Eye => {
            p.add(Shape::ellipse_filled(c, vec2(13., 7.) * scale, pale));
            p.circle_filled(c, 4. * scale, C::from_rgb(58, 68, 82));
        }
        Icon::Folder => {
            poly(
                &[
                    (-13., -10.),
                    (-3., -10.),
                    (0., -6.),
                    (14., -6.),
                    (14., 11.),
                    (-13., 11.),
                ],
                pale,
                gray,
            );
        }
        Icon::Gear => {
            p.circle_stroke(c, 8. * scale, Stroke::new(5. * scale, gray));
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                line(
                    (8. * a.cos(), 8. * a.sin()),
                    (13. * a.cos(), 13. * a.sin()),
                    gray,
                    4.,
                );
            }
            p.circle_filled(c, 3. * scale, C::from_rgb(60, 70, 86));
        }
        Icon::Zoom => {
            p.circle_stroke(pt(-3., -3.), 8. * scale, Stroke::new(2. * scale, pale));
            line((3., 3.), (12., 12.), pale, 2.);
            line((-7., -3.), (1., -3.), pale, 1.);
            line((-3., -7.), (-3., 1.), pale, 1.);
        }
        Icon::Fit | Icon::Camera => {
            rect(-12., -8., 24., 18., C::TRANSPARENT, pale);
            rect(-5., -13., 10., 5., gray, pale);
            if kind == Icon::Camera {
                p.circle_stroke(c, 5. * scale, Stroke::new(2. * scale, blue));
            }
        }
        Icon::Display => {
            rect(-13., -10., 26., 18., gray, pale);
            line((0., 8.), (0., 13.), pale, 2.);
            line((-7., 13.), (7., 13.), pale, 2.);
        }
        Icon::Orbit => {
            p.add(Shape::ellipse_stroke(
                c,
                vec2(14., 6.) * scale,
                Stroke::new(1.4 * scale, pale),
            ));
            line((0., -11.), (0., 3.), pale, 1.5);
            poly(&[(-3., -8.), (0., -13.), (3., -8.)], pale, pale);
        }
        Icon::History => {
            p.circle_stroke(c, 12. * scale, Stroke::new(1.5 * scale, gray));
            line((0., -7.), (0., 1.), pale, 1.5);
            line((0., 1.), (6., 1.), pale, 1.5);
        }
        Icon::Help => {
            p.circle_filled(c, 12. * scale, gray);
            p.text(
                c,
                egui::Align2::CENTER_CENTER,
                "?",
                egui::FontId::proportional(19. * scale),
                C::from_rgb(49, 58, 72),
            );
        }
        Icon::Bell => {
            poly(
                &[
                    (-9., 7.),
                    (-6., 2.),
                    (-6., -7.),
                    (-2., -11.),
                    (3., -11.),
                    (7., -7.),
                    (7., 2.),
                    (10., 7.),
                ],
                gray,
                gray,
            );
            p.circle_filled(pt(0., 11.), 2. * scale, gray);
        }
    }
}

pub fn arrow(p: &Painter, c: Pos2, col: C, down: bool) {
    let points = if down {
        vec![c + vec2(-3., -1.5), c + vec2(3., -1.5), c + vec2(0., 2.)]
    } else {
        vec![c + vec2(-1.5, -3.), c + vec2(-1.5, 3.), c + vec2(2., 0.)]
    };
    p.add(Shape::convex_polygon(points, col, Stroke::NONE));
}
