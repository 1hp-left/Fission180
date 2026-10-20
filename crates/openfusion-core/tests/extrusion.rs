use fission180_core::{
    Document, Extrusion, Operation, Plane, Profile, Session, Sketch,
    extrusion::{Direction, Extent, ExtrudeOptions},
    kernel::Backend,
};
fn sketch(id: u64, plane: Plane, origin: [f64; 3], profile: Profile) -> Sketch {
    let mut s = Sketch {
        id,
        name: format!("Sketch{id}"),
        plane,
        origin,
        ..Default::default()
    };
    s.add_profile(profile).unwrap();
    s
}
fn extrusion(id: u64, sketch: u64, body: u64, distance: f64, operation: Operation) -> Extrusion {
    Extrusion {
        id,
        name: format!("Extrude{id}"),
        sketch_id: sketch,
        profile: 0,
        region_id: Some(1),
        distance,
        distance_expression: None,
        operation,
        body_id: body,
        result_body_ids: vec![],
        boundary_dependencies: None,
        options: Default::default(),
    }
}
fn block(backend: Backend, plane: Plane) -> Document {
    Document {
        backend,
        sketches: vec![sketch(
            1,
            plane,
            [0.; 3],
            Profile::Rectangle {
                a: [0., 0.],
                b: [40., 30.],
            },
        )],
        extrusions: vec![extrusion(2, 1, 3, 5., Operation::NewBody)],
        ..Default::default()
    }
}
fn reopen(d: Document) -> Session {
    let s = Session::new(d).unwrap();
    let d = Document::from_json(&s.document.to_json().unwrap()).unwrap();
    let opened = Session::new(d).unwrap();
    assert_eq!(s.document, opened.document);
    opened
}
#[test]
fn offset_symmetric_and_two_sided_extents_match_analytic_volume_and_bounds() {
    for backend in [Backend::Occt, Backend::Rust] {
        for plane in [Plane::XY, Plane::XZ, Plane::YZ] {
            for direction in [Direction::Symmetric, Direction::TwoSides] {
                let mut d = block(backend, plane);
                d.parameters.insert("start".into(), "3 mm".into());
                d.extrusions[0].distance = if direction == Direction::Symmetric {
                    10.
                } else {
                    4.
                };
                d.extrusions[0].options = ExtrudeOptions {
                    offset_start: true,
                    start_offset: "start".into(),
                    direction,
                    second_distance: "2 * 3 mm".into(),
                    ..Default::default()
                };
                let s = reopen(d);
                let info = &s.model.info()[0];
                assert!((info.volume - 12000.).abs() < 1e-4);
                let normal = plane.normal();
                let axis = (0..3).find(|i| normal[*i] != 0.).unwrap();
                let ends = if direction == Direction::Symmetric {
                    [-2., 8.]
                } else {
                    [-3., 7.]
                };
                let mut world = ends.map(|v| v * normal[axis]);
                world.sort_by(f64::total_cmp);
                assert!(
                    (info.bounds[0][axis] - world[0]).abs() < 1e-5
                        && (info.bounds[1][axis] - world[1]).abs() < 1e-5
                );
                let edited = s
                    .document
                    .with_parameter("start".into(), "6 mm".into())
                    .unwrap();
                let changed = Session::new(edited).unwrap();
                assert!(
                    (changed.model.info()[0].bounds[0][axis] - (world[0] + 3. * normal[axis]))
                        .abs()
                        < 1e-5
                );
            }
        }
    }
}
#[test]
fn through_all_curved_cut_tracks_target_depth_on_both_backends_and_all_planes() {
    for backend in [Backend::Occt, Backend::Rust] {
        for plane in [Plane::XY, Plane::XZ, Plane::YZ] {
            for reverse in [false, true] {
                let mut d = block(backend, plane);
                let sign = if reverse { -1. } else { 1. };
                d.extrusions[0].distance = sign * 5.;
                d.sketches.push(sketch(
                    4,
                    plane,
                    (plane.normal() * (-3. * sign)).to_array(),
                    Profile::Circle {
                        center: [20., 15.],
                        radius: 5.,
                    },
                ));
                let mut cut = extrusion(5, 4, 3, sign, Operation::Cut);
                cut.options.extent = Extent::ThroughAll;
                d.extrusions.push(cut);
                let mut s = reopen(d);
                assert!(
                    (s.model.info()[0].volume - (6000. - 125. * std::f64::consts::PI)).abs() < 1e-4
                );
                let mut changed = s.document.clone();
                changed.extrusions[0].distance = sign * 8.;
                s.replace(changed).unwrap();
                assert!(
                    (s.model.info()[0].volume - (9600. - 200. * std::f64::consts::PI)).abs() < 1e-4
                );
                let before = s.document.clone();
                let mut backwards = before.clone();
                backwards.extrusions[1].distance = -sign;
                assert!(s.replace(backwards).is_err());
                assert_eq!(s.document, before);
            }
        }
    }
}
#[test]
fn symmetric_through_all_covers_both_sides_and_invalid_drafts_retain_geometry() {
    for backend in [Backend::Occt, Backend::Rust] {
        let mut d = block(backend, Plane::XY);
        d.extrusions[0].options.offset_start = true;
        d.extrusions[0].options.start_offset = "-2".into();
        d.sketches.push(sketch(
            4,
            Plane::XY,
            [0.; 3],
            Profile::Circle {
                center: [20., 15.],
                radius: 5.,
            },
        ));
        let mut cut = extrusion(5, 4, 3, 1., Operation::Cut);
        cut.options.extent = Extent::ThroughAll;
        cut.options.direction = Direction::Symmetric;
        d.extrusions.push(cut);
        let mut s = reopen(d);
        let before = s.document.clone();
        assert!((s.model.info()[0].volume - (6000. - 125. * std::f64::consts::PI)).abs() < 1e-4);
        let mut incomplete = before.clone();
        incomplete.extrusions[0].options.start_offset = "2 *".into();
        assert!(s.replace(incomplete).is_err());
        assert_eq!(s.document, before);
        let mut unsupported = before.clone();
        unsupported.extrusions[0].options.extent = Extent::ThroughAll;
        assert!(s.replace(unsupported).is_err());
        assert_eq!(s.document, before);
    }
}
