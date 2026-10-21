use fission180_core::{
    Document, Extrusion, Operation, Plane, Profile, Sketch,
    features::{Feature, FeatureKind},
    kernel::{Backend, ExtrudeInput},
};
#[derive(Clone)]
pub struct Case {
    pub name: &'static str,
    pub base: ExtrudeInput,
    pub tools: Vec<(ExtrudeInput, Operation)>,
    pub volumes: Vec<f64>,
    pub areas: Vec<f64>,
    pub shells: Vec<usize>,
    pub bounds: Vec<[[f64; 3]; 2]>,
}
pub fn prism(
    profile: Profile,
    plane: Plane,
    origin: [f64; 3],
    distance: f64,
    feature: u64,
) -> ExtrudeInput {
    ExtrudeInput {
        feature_id: feature,
        region_id: 1,
        outer: profile,
        holes: vec![],
        loop_ids: vec![1],
        plane,
        origin,
        distance,
    }
}
pub fn block(lo: [f64; 3], hi: [f64; 3], feature: u64) -> ExtrudeInput {
    prism(
        Profile::Rectangle {
            a: [lo[0], lo[1]],
            b: [hi[0], hi[1]],
        },
        Plane::XY,
        [0., 0., lo[2]],
        hi[2] - lo[2],
        feature,
    )
}
pub fn triangle(points: [[f64; 2]; 3]) -> Profile {
    Profile::Polygon {
        points: points.to_vec(),
    }
}
pub fn cases() -> Vec<Case> {
    let root = 2f64.sqrt();
    let mut cases = vec![];
    for (operation, name, v, area, bounds) in [
        (
            Operation::Intersect,
            "wedge-intersect",
            1280.,
            448. + 320. * root,
            [[12., 0., 0.], [28., 20., 8.]],
        ),
        (
            Operation::Cut,
            "wedge-cut",
            6720.,
            2352. + 320. * root,
            [[0.; 3], [40., 20., 10.]],
        ),
        (
            Operation::Join,
            "wedge-join",
            9720.,
            3152. + 280. * root,
            [[0., -5., -2.], [40., 25., 10.]],
        ),
    ] {
        cases.push(Case {
            name,
            base: block([0.; 3], [40., 20., 10.], 2),
            tools: vec![(
                prism(
                    triangle([[10., -2.], [30., -2.], [20., 8.]]),
                    Plane::XZ,
                    [0., 25., 0.],
                    30.,
                    5,
                ),
                operation,
            )],
            volumes: vec![v],
            areas: vec![area],
            shells: vec![1],
            bounds: vec![bounds],
        });
    }
    for (operation, name, v, area) in [
        (
            Operation::Intersect,
            "triangles-intersect",
            1000. / 3.,
            200. + 100. * root,
        ),
        (
            Operation::Cut,
            "triangles-cut",
            500. / 3.,
            100. + 100. * root,
        ),
        (
            Operation::Join,
            "triangles-join",
            2000. / 3.,
            400. + 100. * root,
        ),
    ] {
        cases.push(Case {
            name,
            base: prism(
                triangle([[0., 0.], [10., 0.], [0., 10.]]),
                Plane::XY,
                [0.; 3],
                10.,
                2,
            ),
            tools: vec![(
                prism(
                    triangle([[0., 0.], [10., 0.], [0., 10.]]),
                    Plane::XZ,
                    [0., 10., 0.],
                    10.,
                    5,
                ),
                operation,
            )],
            volumes: vec![v],
            areas: vec![area],
            shells: vec![1],
            bounds: vec![[[0.; 3], [10.; 3]]],
        });
    }
    let mut cavity = Case {
        name: "cavity",
        base: block([0.; 3], [40., 20., 10.], 2),
        tools: vec![(block([10., 5., 3.], [30., 15., 7.], 5), Operation::Cut)],
        volumes: vec![7200.],
        areas: vec![3440.],
        shells: vec![2],
        bounds: vec![[[0.; 3], [40., 20., 10.]]],
    };
    cases.push(cavity.clone());
    cavity.name = "opened-cavity";
    cavity
        .tools
        .push((block([15., -1., 4.], [25., 10., 6.], 8), Operation::Cut));
    cavity.volumes = vec![7100.];
    cavity.areas = vec![3520.];
    cavity.shells = vec![1];
    cases.push(cavity);
    let mut island = cases.iter().find(|c| c.name == "cavity").unwrap().clone();
    island.name = "nested-island";
    island
        .tools
        .push((block([18., 8., 4.], [22., 12., 6.], 8), Operation::Join));
    island.volumes.push(32.);
    island.areas.push(64.);
    island.shells.push(1);
    island.bounds.push([[18., 8., 4.], [22., 12., 6.]]);
    cases.push(island);
    let mut empty = cases.iter().find(|c| c.name == "cavity").unwrap().clone();
    empty.name = "cavity-void-intersect";
    empty.tools.push((
        block([10., 5., 3.], [30., 15., 7.], 8),
        Operation::Intersect,
    ));
    empty.volumes.clear();
    empty.areas.clear();
    empty.shells.clear();
    empty.bounds.clear();
    cases.push(empty);
    let concave = Profile::Polygon {
        points: vec![
            [0., 0.],
            [12., 0.],
            [12., 4.],
            [4., 4.],
            [4., 12.],
            [0., 12.],
        ],
    };
    for (op, name, volumes, areas, bounds) in [
        (
            Operation::Cut,
            "concave-split",
            vec![160., 320.],
            vec![256., 352.],
            vec![[[0.; 3], [12., 12., 2.]], [[0., 0., 6.], [12., 12., 10.]]],
        ),
        (
            Operation::Intersect,
            "concave-intersect",
            vec![320.],
            vec![352.],
            vec![[[0., 0., 2.], [12., 12., 6.]]],
        ),
        (
            Operation::Join,
            "concave-join",
            vec![1264.],
            vec![904.],
            vec![[[-1., -1., 0.], [13., 13., 10.]]],
        ),
    ] {
        cases.push(Case {
            name,
            base: prism(concave.clone(), Plane::XY, [0.; 3], 10., 2),
            tools: vec![(
                prism(
                    Profile::Rectangle {
                        a: [-1., 2.],
                        b: [13., 6.],
                    },
                    Plane::XZ,
                    [0., 13., 0.],
                    14.,
                    5,
                ),
                op,
            )],
            shells: vec![1; volumes.len()],
            volumes,
            areas,
            bounds,
        });
    }
    for (op, name, v, area, bounds) in [
        (
            Operation::Cut,
            "polygon-hole-cut",
            536.,
            388. + 104. * root,
            [[2., 0., 0.], [12., 12., 10.]],
        ),
        (
            Operation::Intersect,
            "polygon-hole-intersect",
            744.,
            508. + 104. * root,
            [[0.; 3], [12., 12., 10.]],
        ),
        (
            Operation::Join,
            "polygon-hole-join",
            1544.,
            868. + 64. * root,
            [[0., -1., 0.], [12., 13., 12.]],
        ),
    ] {
        let mut base = block([0.; 3], [12., 12., 10.], 2);
        base.holes.push(Profile::Rectangle {
            a: [4.; 2],
            b: [8.; 2],
        });
        base.loop_ids.push(2);
        cases.push(Case {
            name,
            base,
            tools: vec![(
                prism(
                    triangle([[0., 0.], [12., 0.], [0., 12.]]),
                    Plane::XZ,
                    [0., 13., 0.],
                    14.,
                    5,
                ),
                op,
            )],
            volumes: vec![v],
            areas: vec![area],
            shells: vec![1],
            bounds: vec![bounds],
        });
    }
    let mut stepped = cases
        .iter()
        .find(|c| c.name == "wedge-cut")
        .unwrap()
        .clone();
    stepped.name = "stepped-cross-axis";
    stepped
        .tools
        .insert(0, (block([1., 1., 7.], [4., 4., 10.], 8), Operation::Cut));
    stepped.volumes[0] -= 27.;
    stepped.areas[0] += 36.;
    cases.push(stepped);
    let mut cavities = cases.iter().find(|c| c.name == "cavity").unwrap().clone();
    cavities.name = "two-cavities";
    cavities
        .tools
        .push((block([2.; 3], [6.; 3], 8), Operation::Cut));
    cavities.volumes[0] -= 64.;
    cavities.areas[0] += 96.;
    cavities.shells[0] = 3;
    cases.push(cavities);
    cases
}
/// Right-handed cyclic world-axis permutations; local origins are independently
/// translated and profile direction and extrusion conventions can be reversed.
pub fn reframe(case: &mut Case, permutation: usize, reverse_distance: bool, reverse_profile: bool) {
    let offset = [13., -17., 9.];
    let axes = [
        [0, 1, 2],
        [1, 2, 0],
        [2, 0, 1],
        [1, 0, 2],
        [0, 2, 1],
        [2, 1, 0],
    ][permutation];
    let rotate = |p: [f64; 3]| -> [f64; 3] { axes.map(|i| p[i]) };
    for input in std::iter::once(&mut case.base).chain(case.tools.iter_mut().map(|(i, _)| i)) {
        let old = input.clone();
        let n = rotate(old.plane.normal().to_array());
        let new_plane = if n[2].abs() > 0.5 {
            Plane::XY
        } else if n[1].abs() > 0.5 {
            Plane::XZ
        } else {
            Plane::YZ
        };
        let positive = n
            .iter()
            .zip(new_plane.normal().to_array())
            .map(|(a, b)| a * b)
            .sum::<f64>();
        let shift = [7., -3.];
        let local = |p: [f64; 2]| {
            let world = rotate(old.plane.point(p, [0.; 3]).to_array());
            let q = match new_plane {
                Plane::XY => [world[0], world[1]],
                Plane::XZ => [world[0], world[2]],
                Plane::YZ => [world[1], world[2]],
            };
            [q[0] - shift[0], q[1] - shift[1]]
        };
        let profile = |p: &Profile| {
            let mut points: Vec<_> = p
                .boundary()
                .iter()
                .map(|s| local(s.curve.point(0.)))
                .collect();
            if reverse_profile {
                points.reverse();
            }
            Profile::Polygon { points }
        };
        input.outer = profile(&old.outer);
        input.holes = old.holes.iter().map(profile).collect();
        input.plane = new_plane;
        let rotated = rotate(old.origin);
        let delta = new_plane.point(shift, [0.; 3]).to_array();
        input.origin = std::array::from_fn(|i| rotated[i] + offset[i] + delta[i]);
        input.distance = old.distance * positive;
        if reverse_distance {
            let delta = new_plane.normal() * input.distance;
            for i in 0..3 {
                input.origin[i] += delta[i];
            }
            input.distance = -input.distance;
        }
    }
    for bounds in &mut case.bounds {
        *bounds = bounds.map(|p| {
            let q = rotate(p);
            std::array::from_fn(|i| q[i] + offset[i])
        });
    }
}
pub fn document(case: &Case, backend: Backend, combine: bool) -> Document {
    let mut d = Document {
        backend,
        name: case.name.into(),
        ..Default::default()
    };
    for (i, (input, op)) in std::iter::once((&case.base, Operation::NewBody))
        .chain(case.tools.iter().map(|(t, op)| (t, *op)))
        .enumerate()
    {
        let id = i as u64 * 4 + 1;
        let mut s = Sketch {
            id,
            name: if i == 0 {
                "Base profile".into()
            } else {
                format!("Tool {i} profile")
            },
            plane: input.plane,
            origin: input.origin,
            ..Default::default()
        };
        s.add_profile(input.outer.clone()).unwrap();
        for hole in &input.holes {
            s.add_profile(hole.clone()).unwrap();
        }
        d.sketches.push(s);
        d.push_extrusion(Extrusion {
            id: id + 1,
            sketch_id: id,
            body_id: if i == 0 || !combine { 3 } else { id + 2 },
            distance: input.distance,
            operation: if combine { Operation::NewBody } else { op },
            name: if i == 0 {
                "Base extrusion".into()
            } else {
                format!("Tool {i} extrusion")
            },
            profile: 0,
            region_id: Some(1),
            distance_expression: None,
            result_body_ids: vec![],
            boundary_dependencies: None,
            options: Default::default(),
        });
        if i > 0 && combine {
            d.push_feature(Feature {
                id: id + 3,
                name: format!("{op:?}"),
                target: Some(3),
                outputs: vec![],
                kind: FeatureKind::Combine {
                    tool_body: id + 2,
                    operation: op,
                    keep_tool: false,
                },
            })
            .unwrap();
        }
    }
    d
}
