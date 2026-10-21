//! Reusable analytic acceptance specifications; OCCT operands are built separately.
use fission180_core::{
    Document, Operation, Profile, Sketch,
    kernel::{Backend, ExtrudeInput},
};
use std::f64::consts::PI;
#[path = "prism_fixtures.rs"]
#[allow(dead_code)]
mod prism;
pub use prism::{disk, rect};
pub fn prism(profile: Profile, lo: f64, hi: f64, feature: u64) -> ExtrudeInput {
    let mut input = prism::input(profile, vec![], feature);
    input.origin[2] = lo;
    input.distance = hi - lo;
    input
}
pub fn lens(r: f64, s: f64, d: f64) -> f64 {
    r * r * ((d * d + r * r - s * s) / (2. * d * r)).acos()
        + s * s * ((d * d + s * s - r * r) / (2. * d * s)).acos()
        - 0.5 * ((-d + r + s) * (d + r - s) * (d - r + s) * (d + r + s)).sqrt()
}
pub struct Case {
    pub name: &'static str,
    pub base: ExtrudeInput,
    pub tools: Vec<(ExtrudeInput, Operation)>,
    pub volumes: Vec<f64>,
    pub area: Option<f64>,
}
pub fn cases() -> Vec<Case> {
    let base = prism(rect(0., 0., 40., 20.), 0., 10., 2);
    let notch = prism(
        Profile::Circle {
            center: [20., 0.],
            radius: 5.,
        },
        10.,
        7.,
        5,
    );
    let hole = |y| {
        prism(
            Profile::Circle {
                center: [20., y],
                radius: 1.,
            },
            -1.,
            11.,
            8,
        )
    };
    let l = 200. * PI / 3. - 50. * 3f64.sqrt();
    let mut cases = vec![
        Case {
            name: "blind-sidewall-notch",
            base: base.clone(),
            tools: vec![(notch.clone(), Operation::Cut)],
            volumes: vec![8000. - 37.5 * PI],
            area: Some(2800. - 30. + 15. * PI),
        },
        Case {
            name: "notch-contained-through-hole",
            base: base.clone(),
            tools: vec![(notch.clone(), Operation::Cut), (hole(2.), Operation::Cut)],
            volumes: vec![8000. - 44.5 * PI],
            area: Some(2770. + 27. * PI),
        },
        Case {
            name: "notch-partial-through-hole",
            base: base.clone(),
            tools: vec![(notch, Operation::Cut), (hole(4.5), Operation::Cut)],
            volumes: vec![8000. - 47.5 * PI + 3. * lens(5., 1., 4.5)],
            area: None,
        },
        Case {
            name: "axial-gap-split",
            base: base.clone(),
            tools: vec![(prism(rect(-1., -1., 42., 22.), 4., 6., 5), Operation::Cut)],
            volumes: vec![3200., 3200.],
            area: Some(4160.),
        },
        Case {
            name: "unequal-circle-join",
            base: prism(disk(-5., 10.), 0., 10., 2),
            tools: vec![(prism(disk(5., 10.), 5., 15., 5), Operation::Join)],
            volumes: vec![2000. * PI - 5. * l],
            area: Some(2200. * PI / 3. - 2. * l),
        },
        Case {
            name: "unequal-circle-intersect",
            base: prism(disk(-5., 10.), 0., 10., 2),
            tools: vec![(prism(disk(5., 10.), 5., 15., 5), Operation::Intersect)],
            volumes: vec![5. * l],
            area: Some(200. * PI / 3. + 2. * l),
        },
        Case {
            name: "axially-disjoint-join",
            base: prism(disk(0., 5.), 0., 5., 2),
            tools: vec![(prism(disk(0., 5.), 10., 15., 5), Operation::Join)],
            volumes: vec![125. * PI, 125. * PI],
            area: Some(200. * PI),
        },
    ];
    for (name, depth, overlap) in [
        ("opposite-separated", 3., 0.),
        ("opposite-meeting", 5., 0.),
        ("opposite-breakthrough", 6., 2.),
    ] {
        cases.push(Case {
            name,
            base: prism(rect(-30., -30., 60., 60.), 0., 10., 2),
            tools: vec![
                (prism(disk(-5., 10.), 10., 10. - depth, 5), Operation::Cut),
                (prism(disk(5., 10.), 0., depth, 8), Operation::Cut),
            ],
            volumes: vec![36000. - 200. * PI * depth + overlap * l],
            area: Some(
                9600. + 40. * PI * depth
                    - 40. * PI / 3. * overlap
                    - if depth >= 5. { 2. * l } else { 0. },
            ),
        });
    }
    cases.push(Case {
        name: "stepped-full-removal",
        base: base.clone(),
        tools: vec![
            (
                prism(
                    Profile::Circle {
                        center: [20., 0.],
                        radius: 5.,
                    },
                    10.,
                    7.,
                    5,
                ),
                Operation::Cut,
            ),
            (prism(rect(-1., -1., 42., 22.), -1., 11., 8), Operation::Cut),
        ],
        volumes: vec![],
        area: Some(0.),
    });
    let line = |start, end| fission180_core::boundary::BoundarySegment {
        source: None,
        curve: fission180_core::boundary::PlanarCurve::Line { start, end },
    };
    let arc = |center, start_angle, sweep| fission180_core::boundary::BoundarySegment {
        source: None,
        curve: fission180_core::boundary::PlanarCurve::Arc {
            geometry: fission180_core::arc::ArcGeometry {
                center,
                radius: 4.,
                start_angle,
                sweep,
            },
        },
    };
    let capsule = Profile::Curved {
        segments: vec![
            line([18., -4.], [28., -4.]),
            arc([28., 0.], -PI / 2., PI),
            line([28., 4.], [18., 4.]),
            arc([18., 0.], PI / 2., PI),
        ],
    };
    cases.push(Case {
        name: "mixed-sidewall-notch",
        base: base.clone(),
        tools: vec![(prism(capsule, 10., 7., 5), Operation::Cut)],
        volumes: vec![7880. - 24. * PI],
        area: Some(2776. + 12. * PI),
    });
    let mut annulus = prism(disk(0., 8.), 10., 7., 5);
    annulus.holes.push(disk(0., 3.));
    annulus.loop_ids.push(2);
    cases.push(Case {
        name: "blind-annular-island",
        base: prism(rect(-30., -30., 60., 60.), 0., 10., 2),
        tools: vec![(annulus, Operation::Cut)],
        volumes: vec![36000. - 165. * PI],
        area: Some(9600. + 66. * PI),
    });
    cases
}
pub fn document(case: &Case, backend: Backend) -> Document {
    let mut d = Document {
        backend,
        ..Default::default()
    };
    for (i, (input, op)) in std::iter::once((&case.base, Operation::NewBody))
        .chain(case.tools.iter().map(|(t, op)| (t, *op)))
        .enumerate()
    {
        let sketch_id = (i as u64) * 3 + 1;
        let mut s = Sketch {
            id: sketch_id,
            plane: input.plane,
            origin: input.origin,
            ..Default::default()
        };
        s.add_profile(input.outer.clone()).unwrap();
        for hole in &input.holes {
            s.add_profile(hole.clone()).unwrap();
        }
        d.sketches.push(s);
        d.extrusions.push(prism::extrusion(
            sketch_id + 1,
            sketch_id,
            3,
            input.distance,
            op,
        ));
    }
    d
}

/// Equivalent supported frames, with independently shifted tool-local coordinates.
pub fn reframe(case: &mut Case, plane: fission180_core::Plane, negative: bool, reverse: bool) {
    use fission180_core::boundary::PlanarCurve;
    let origin = [13., -17., 9.];
    let normal = plane.normal().to_array();
    for input in std::iter::once(&mut case.base).chain(case.tools.iter_mut().map(|(t, _)| t)) {
        let mut lo = input.origin[2];
        let mut distance = input.distance;
        if negative {
            lo = -lo;
            distance = -distance;
        }
        if reverse {
            lo += distance;
            distance = -distance;
        }
        let delta = if input.feature_id == 2 {
            [0., 0.]
        } else {
            [7., -3.]
        };
        let dp = plane.point(delta, [0.; 3]).to_array();
        let at = |p: [f64; 2]| [p[0] - delta[0], p[1] - delta[1]];
        let shifted = |p: &Profile| match p {
            Profile::Rectangle { a, b } => Profile::Rectangle {
                a: at(*a),
                b: at(*b),
            },
            Profile::Circle { center, radius } => Profile::Circle {
                center: at(*center),
                radius: *radius,
            },
            Profile::Polygon { points } => Profile::Polygon {
                points: points.iter().copied().map(at).collect(),
            },
            Profile::Curved { segments } => Profile::Curved {
                segments: segments
                    .iter()
                    .cloned()
                    .map(|mut s| {
                        s.curve = match s.curve {
                            PlanarCurve::Line { start, end } => PlanarCurve::Line {
                                start: at(start),
                                end: at(end),
                            },
                            PlanarCurve::Arc { geometry: mut a } => {
                                a.center = at(a.center);
                                PlanarCurve::Arc { geometry: a }
                            }
                        };
                        s
                    })
                    .collect(),
            },
        };
        input.outer = shifted(&input.outer);
        input.holes = input.holes.iter().map(shifted).collect();
        input.plane = plane;
        input.origin = std::array::from_fn(|i| origin[i] + normal[i] * lo + dp[i]);
        input.distance = distance;
    }
}
