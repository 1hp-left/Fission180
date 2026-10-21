use fission180_core::{
    Document, Extrusion, Model, Operation, Plane, Profile, Session, Sketch,
    arc::ArcGeometry,
    boundary::{BoundarySegment, PlanarCurve},
    features::{Feature, FeatureKind},
    kernel::{Backend, ExtrudeInput, Geometry, SolidKernel, native::NativeSolid},
};
use std::f64::consts::{FRAC_PI_2, PI};
fn line(start: [f64; 2], end: [f64; 2]) -> BoundarySegment {
    BoundarySegment {
        source: None,
        curve: PlanarCurve::Line { start, end },
    }
}
fn arc(center: [f64; 2], radius: f64, start_angle: f64, sweep: f64) -> BoundarySegment {
    BoundarySegment {
        source: None,
        curve: PlanarCurve::Arc {
            geometry: ArcGeometry {
                center,
                radius,
                start_angle,
                sweep,
            },
        },
    }
}
fn semicircle() -> Profile {
    Profile::Curved {
        segments: vec![arc([0.; 2], 10., 0., PI), line([-10., 0.], [10., 0.])],
    }
}
fn capsule() -> Profile {
    Profile::Curved {
        segments: vec![
            line([-10., -5.], [10., -5.]),
            arc([10., 0.], 5., -FRAC_PI_2, PI),
            line([10., 5.], [-10., 5.]),
            arc([-10., 0.], 5., FRAC_PI_2, PI),
        ],
    }
}
fn input(profile: Profile) -> ExtrudeInput {
    ExtrudeInput {
        feature_id: 2,
        region_id: 1,
        outer: profile,
        holes: vec![],
        loop_ids: vec![1],
        plane: Plane::XY,
        origin: [0.; 3],
        distance: 10.,
    }
}
fn block() -> ExtrudeInput {
    input(Profile::Rectangle {
        a: [-30.; 2],
        b: [30.; 2],
    })
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-5, "{a} != {b}");
}
fn mesh_volume(s: &NativeSolid, resolution: usize) -> f64 {
    let m = s.mesh(resolution).unwrap();
    let origin = s.bounds()[0];
    for id in m.face_ids.iter().flatten() {
        assert!(
            s.topology().faces.iter().any(|f| f.id == *id),
            "missing face {id:?}"
        );
    }
    for id in &m.edge_ids {
        assert!(
            s.topology().edges.iter().any(|e| e.id == *id),
            "missing edge {id:?}"
        );
    }
    m.indices
        .chunks_exact(3)
        .map(|tri| {
            let p = tri
                .iter()
                .map(|i| {
                    cadrum::DVec3::from_array(m.vertices[*i]) - cadrum::DVec3::from_array(origin)
                })
                .collect::<Vec<_>>();
            p[0].dot(p[1].cross(p[2])) / 6.
        })
        .sum()
}
/// Independent test oracle: OCCT builds and cuts exact curves from the same
/// analytic inputs. Rust results are never constructed from this adapter.
fn occt_cut_test_adapter(a: &ExtrudeInput, b: &ExtrudeInput) -> Vec<Geometry> {
    let a = Backend::Occt.extrude(a).unwrap();
    let b = Backend::Occt.extrude(b).unwrap();
    Backend::Occt.boolean(&a, &b, Operation::Cut, 99).unwrap()
}
#[test]
fn contained_blind_curved_cuts_all_caps_frames_directions_and_step() {
    for profile in [
        Profile::Circle {
            center: [0.; 2],
            radius: 3.,
        },
        semicircle(),
        capsule(),
    ] {
        for plane in [Plane::XY, Plane::XZ, Plane::YZ] {
            for depth in [10., -10.] {
                for high in [false, true] {
                    for backwards in [false, true] {
                        let mut a = block();
                        a.plane = plane;
                        a.distance = depth;
                        a.origin = [13., -7., 11.];
                        let normal = plane.normal().to_array();
                        let interval = if depth > 0. { [0., 10.] } else { [-10., 0.] };
                        let (start, end) = if high {
                            (interval[1] - 3., interval[1] + 4.)
                        } else {
                            (interval[0] - 4., interval[0] + 3.)
                        };
                        let mut b = input(profile.clone());
                        b.plane = plane;
                        b.feature_id = 5;
                        let (start, end) = if backwards {
                            (end, start)
                        } else {
                            (start, end)
                        };
                        b.origin = std::array::from_fn(|i| a.origin[i] + normal[i] * start);
                        b.distance = end - start;
                        // Deliberately use a different in-plane coordinate frame for the tool.
                        let tangent = plane.point([2., 1.], [0.; 3]).to_array();
                        for (v, d) in b.origin.iter_mut().zip(tangent) {
                            *v += d;
                        }
                        let native = NativeSolid::extrude(a.clone()).unwrap();
                        let cutter = NativeSolid::extrude(b.clone()).unwrap();
                        let solids = fission180_core::kernel::native::boolean(
                            &native,
                            &cutter,
                            Operation::Cut,
                            7,
                        )
                        .unwrap();
                        assert_eq!(solids.len(), 1);
                        let s = &solids[0];
                        s.topology().validate().unwrap();
                        close(
                            s.volume(),
                            native.volume() - profile.signed_area().abs() * 3.,
                        );
                        close(s.area(), native.area() + profile.perimeter() * 3.);
                        assert_eq!(s.bounds(), native.bounds());
                        let expected = (s.volume(), s.area(), s.bounds());
                        for resolution in [16, 96] {
                            let actual = mesh_volume(s, resolution);
                            assert!(
                                (actual - s.volume()).abs()
                                    < profile.signed_area().abs() * 3. * 0.04,
                                "{actual} {}",
                                s.volume()
                            );
                            assert_eq!(expected, (s.volume(), s.area(), s.bounds()));
                        }
                        let oracle = occt_cut_test_adapter(&a, &b);
                        assert_eq!(oracle.len(), 1);
                        close(s.volume(), oracle[0].volume());
                        close(s.area(), oracle[0].area());
                        let Geometry::Occt(o) = &oracle[0] else {
                            panic!()
                        };
                        let mut bytes = vec![];
                        cadrum::Solid::write_step([o], &mut bytes).unwrap();
                        close(Model::step_volume(&bytes).unwrap(), s.volume());
                    }
                }
            }
        }
    }
}
#[test]
fn disjoint_pockets_through_holes_translation_and_body_outcomes_preserve_geometry() {
    let a = NativeSolid::extrude(block()).unwrap();
    let mut b = input(Profile::Circle {
        center: [-10., 0.],
        radius: 3.,
    });
    b.origin[2] = 7.;
    b.distance = 8.;
    b.feature_id = 5;
    let cut = fission180_core::kernel::native::boolean(
        &a,
        &NativeSolid::extrude(b).unwrap(),
        Operation::Cut,
        5,
    )
    .unwrap()
    .remove(0);
    let mut b = input(Profile::Circle {
        center: [10., 0.],
        radius: 2.,
    });
    b.origin[2] = -2.;
    b.distance = 20.;
    b.feature_id = 6;
    let through = fission180_core::kernel::native::boolean(
        &cut,
        &NativeSolid::extrude(b).unwrap(),
        Operation::Cut,
        6,
    )
    .unwrap()
    .remove(0);
    close(through.volume(), 36000. - 27. * PI - 40. * PI);
    close(through.area(), a.area() + 18. * PI + 40. * PI - 8. * PI);
    through.topology().validate().unwrap();
    mesh_volume(&through, 96);
    let translated = through.translated([11., -9., 3.], 9).unwrap();
    close(translated.volume(), through.volume());
    close(translated.area(), through.area());
    for j in 0..2 {
        for i in 0..3 {
            close(
                translated.bounds()[j][i],
                through.bounds()[j][i] + [11., -9., 3.][i],
            );
        }
    }
    mesh_volume(&translated, 96);
    let mut b = input(Profile::Circle {
        center: [0., 10.],
        radius: 2.,
    });
    b.distance = 3.;
    b.feature_id = 8;
    let two = fission180_core::kernel::native::boolean(
        &through,
        &NativeSolid::extrude(b).unwrap(),
        Operation::Cut,
        8,
    )
    .unwrap()
    .remove(0);
    close(two.volume(), through.volume() - 12. * PI);
    assert_eq!(two.provenance.last().unwrap().parents, vec![6]);
    assert!(
        two.provenance
            .iter()
            .all(|p| !p.parents.contains(&p.feature))
    );
    two.topology().validate().unwrap();
    mesh_volume(&two, 96);
    assert!(
        fission180_core::kernel::native::boolean(&two, &two, Operation::Cut, 99)
            .unwrap()
            .is_empty()
    );
    let distant = two.translated([200., 0., 0.], 10).unwrap();
    assert_eq!(
        fission180_core::kernel::native::boolean(&two, &distant, Operation::Join, 11)
            .unwrap()
            .len(),
        2
    );
    assert!(
        fission180_core::kernel::native::boolean(&two, &distant, Operation::Intersect, 12)
            .unwrap()
            .is_empty()
    );
    // Equal rectangular envelopes do not make a pocketed body and a box equal.
    let filled = fission180_core::kernel::native::boolean(&two, &a, Operation::Join, 13).unwrap();
    assert_eq!(filled.len(), 1);
    close(filled[0].volume(), a.volume());
    close(filled[0].area(), a.area());
    filled[0].topology().validate().unwrap();
}
#[test]
fn curved_target_with_existing_hole_and_bound_topology_accepts_disjoint_blind_cut() {
    let offset = Profile::Curved {
        segments: vec![arc([10., 0.], 10., 0., PI), line([0.; 2], [20., 0.])],
    };
    assert!(!offset.contains_point([-8., 0.]));
    assert!(offset.contains_point([10., 5.]));
    assert!(!offset.contains_point([10., 0.]));
    let mut a = input(Profile::Circle {
        center: [0.; 2],
        radius: 30.,
    });
    a.holes.push(Profile::Circle {
        center: [-10., 0.],
        radius: 2.,
    });
    a.loop_ids.push(2);
    let mut b = input(semicircle());
    b.feature_id = 5;
    b.origin = [10., 0., 7.];
    b.distance = 7.;
    let mut target = NativeSolid::extrude(a.clone()).unwrap();
    target.bind_body(99);
    let mut result = fission180_core::kernel::native::boolean(
        &target,
        &NativeSolid::extrude(b.clone()).unwrap(),
        Operation::Cut,
        7,
    )
    .unwrap()
    .remove(0);
    result.topology().validate().unwrap();
    assert!(result.topology().faces.iter().all(|f| f.id.body == 99));
    close(result.volume(), 8810. * PI);
    let oracle = occt_cut_test_adapter(&a, &b);
    close(result.volume(), oracle[0].volume());
    close(result.area(), oracle[0].area());
    assert!((mesh_volume(&result, 128) - result.volume()).abs() < result.volume() * 0.01);
    result.bind_body(100);
    assert!(result.topology().edges.iter().all(|e| e.id.body == 100));
    mesh_volume(&result, 32);
    let mut corrupt = result.topology().clone();
    let face = corrupt
        .faces
        .iter_mut()
        .find(|f| {
            matches!(
                f.surface,
                fission180_core::kernel::native::Surface::Cylinder { .. }
            )
        })
        .unwrap();
    face.reversed ^= true;
    assert!(corrupt.validate().is_err());
}
#[test]
fn contained_cut_rejects_enclosed_and_tangent_voids_and_accepts_sidewall_crossings() {
    let a = NativeSolid::extrude(block()).unwrap();
    for (profile, start, distance) in [
        (semicircle(), 2., 3.), // enclosed cavity
        (
            Profile::Circle {
                center: [29., 0.],
                radius: 3.,
            },
            7.,
            8.,
        ), // side wall crossing
        (
            Profile::Circle {
                center: [27., 0.],
                radius: 3.,
            },
            7.,
            8.,
        ), // side wall tangency
    ] {
        let mut b = input(profile);
        b.feature_id = 5;
        b.origin[2] = start;
        b.distance = distance;
        let result = fission180_core::kernel::native::boolean(
            &a,
            &NativeSolid::extrude(b.clone()).unwrap(),
            Operation::Cut,
            5,
        );
        if start == 7.
            && matches!(
                b.outer,
                Profile::Circle {
                    center: [29., 0.],
                    ..
                }
            )
        {
            let result = result.unwrap();
            let oracle = occt_cut_test_adapter(&block(), &b);
            close(result[0].volume(), oracle[0].volume());
            close(result[0].area(), oracle[0].area());
            result[0].topology().validate().unwrap();
        } else {
            assert!(result.is_err());
        }
    }
    let mut b = input(semicircle());
    b.feature_id = 5;
    b.origin[2] = 7.;
    b.distance = 8.;
    let tool = NativeSolid::extrude(b).unwrap();
    let cut = fission180_core::kernel::native::boolean(&a, &tool, Operation::Cut, 5)
        .unwrap()
        .remove(0);
    let repeated = fission180_core::kernel::native::boolean(&cut, &tool, Operation::Cut, 6)
        .unwrap()
        .remove(0);
    close(repeated.volume(), cut.volume());
    close(repeated.area(), cut.area());
    repeated.topology().validate().unwrap();
    let mut annulus = input(Profile::Circle {
        center: [0.; 2],
        radius: 4.,
    });
    annulus.holes = vec![Profile::Circle {
        center: [0.; 2],
        radius: 2.,
    }];
    annulus.loop_ids.push(2);
    annulus.origin[2] = 7.;
    annulus.distance = 8.;
    annulus.feature_id = 7;
    let annular = fission180_core::kernel::native::boolean(
        &a,
        &NativeSolid::extrude(annulus).unwrap(),
        Operation::Cut,
        7,
    )
    .unwrap()
    .remove(0);
    close(annular.volume(), 36000. - 36. * PI);
    close(annular.area(), 9600. + 36. * PI);
    annular.topology().validate().unwrap();
}
fn extrusion(id: u64, sketch: u64, body: u64, distance: f64, operation: Operation) -> Extrusion {
    Extrusion {
        id,
        name: format!("Feature {id}"),
        sketch_id: sketch,
        profile: 0,
        region_id: Some(1),
        distance,
        operation,
        body_id: body,
        distance_expression: None,
        result_body_ids: vec![],
        boundary_dependencies: None,
        options: Default::default(),
    }
}
#[test]
fn blind_cut_parameter_replay_save_history_dependency_failures_and_hole_command() {
    let mut base = Sketch {
        id: 1,
        ..Default::default()
    };
    base.add_profile(Profile::Rectangle {
        a: [-30.; 2],
        b: [30.; 2],
    })
    .unwrap();
    let mut tool = Sketch {
        id: 4,
        origin: [0., 0., 7.],
        ..Default::default()
    };
    tool.add_profile(semicircle()).unwrap();
    let d = Document {
        backend: Backend::Rust,
        sketches: vec![base, tool],
        extrusions: vec![
            extrusion(2, 1, 3, 10., Operation::NewBody),
            extrusion(5, 4, 3, 8., Operation::Cut),
        ],
        ..Default::default()
    };
    let mut session = Session::new(d).unwrap();
    close(session.model.info()[0].volume, 36000. - 150. * PI);
    let mut edit = session.document.clone();
    edit.parameters.insert("depth".into(), "2 mm".into());
    edit.extrusions[1].distance = -2.;
    edit.extrusions[1].distance_expression = Some("-depth".into());
    edit.sketches[1].origin[2] = 10.;
    session.replace(edit).unwrap();
    close(session.model.info()[0].volume, 36000. - 100. * PI);
    let Geometry::Rust(shape) = &session.model.bodies[0].solid else {
        panic!()
    };
    let identities: Vec<_> = shape.topology().edges.iter().map(|e| e.id).collect();
    let edited = session
        .document
        .with_parameter("depth".into(), "4 mm".into())
        .unwrap();
    session.replace(edited).unwrap();
    close(session.model.info()[0].volume, 36000. - 200. * PI);
    let Geometry::Rust(shape) = &session.model.bodies[0].solid else {
        panic!()
    };
    assert_eq!(
        identities,
        shape
            .topology()
            .edges
            .iter()
            .map(|e| e.id)
            .collect::<Vec<_>>()
    );
    session.undo().unwrap();
    close(session.model.info()[0].volume, 36000. - 100. * PI);
    session.redo().unwrap();
    close(session.model.info()[0].volume, 36000. - 200. * PI);
    let reopened =
        Session::new(Document::from_json(&session.document.to_json().unwrap()).unwrap()).unwrap();
    close(
        reopened.model.info()[0].volume,
        session.model.info()[0].volume,
    );
    let accepted = session.document.clone();
    let mut invalid = accepted.clone();
    invalid.sketches[1].origin[0] = 29.;
    assert!(session.replace(invalid).is_err());
    assert_eq!(accepted, session.document);
    close(session.model.info()[0].volume, 36000. - 200. * PI);
    let mut hole = accepted;
    hole.extrusions.pop();
    hole.sketches.pop();
    hole.parameters.insert("diameter".into(), "6 mm".into());
    hole.push_feature(Feature {
        id: 8,
        name: "Blind hole".into(),
        target: Some(3),
        outputs: vec![],
        kind: FeatureKind::Hole {
            center: [0.; 2],
            start_z: 0.,
            diameter: "diameter".into(),
            depth: "3 mm".into(),
        },
    })
    .unwrap();
    let mut hole = Session::new(hole).unwrap();
    close(hole.model.info()[0].volume, 36000. - 27. * PI);
    assert_eq!(hole.model.info()[0].backend, Backend::Rust);
    hole.replace(
        hole.document
            .with_parameter("diameter".into(), "8 mm".into())
            .unwrap(),
    )
    .unwrap();
    close(hole.model.info()[0].volume, 36000. - 48. * PI);
    let before = hole.document.clone();
    assert!(
        hole.replace(
            before
                .with_parameter("diameter".into(), "70 mm".into())
                .unwrap()
        )
        .is_err()
    );
    assert_eq!(hole.document, before);
}
