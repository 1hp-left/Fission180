use fission180_core::{
    Document, Extrusion, Model, Operation, Plane, Profile, Session, Sketch,
    kernel::native::{Curve, NativeSolid, Surface},
    kernel::{Backend, ExtrudeInput, Geometry, SolidKernel},
};

fn rectangle(a: [f64; 2], b: [f64; 2]) -> Profile {
    Profile::Rectangle { a, b }
}
fn sketch(id: u64, profiles: Vec<Profile>) -> Sketch {
    let mut s = Sketch {
        id,
        name: format!("Sketch{id}"),
        next_profile_id: 1,
        ..Default::default()
    };
    for p in profiles {
        s.add_profile(p).unwrap();
    }
    s
}
fn extrusion(
    id: u64,
    sketch_id: u64,
    region_id: u64,
    body_id: u64,
    distance: f64,
    operation: Operation,
) -> Extrusion {
    Extrusion {
        id,
        name: format!("Extrude{id}"),
        sketch_id,
        profile: 0,
        region_id: Some(region_id),
        distance,
        distance_expression: None,
        operation,
        body_id,
        result_body_ids: vec![],
    }
}
fn block(backend: Backend) -> Document {
    Document {
        backend,
        sketches: vec![sketch(1, vec![rectangle([0., 0.], [40., 30.])])],
        extrusions: vec![extrusion(2, 1, 1, 3, 5., Operation::NewBody)],
        ..Default::default()
    }
}
#[test]
fn exhausted_ids_are_errors_without_overflow() {
    let mut d = block(Backend::Occt);
    d.sketches[0].id = u64::MAX;
    d.extrusions[0].sketch_id = u64::MAX;
    assert!(d.validate().is_ok());
    assert!(d.next_id().is_err());
    let before = d.to_json().unwrap();
    let mut s = Session::new(d).unwrap();
    assert!(s.document.next_id().is_err());
    assert_eq!(s.document.to_json().unwrap(), before);
    assert!(!s.undo().unwrap());
}
#[test]
fn stable_region_dependencies_survive_reorder_and_reject_removal() {
    let mut d = block(Backend::Occt);
    d.sketches[0]
        .add_profile(rectangle([60., 0.], [80., 20.]))
        .unwrap();
    let initial = Model::build(&d).unwrap().info()[0].volume;
    d.sketches[0].profiles.swap(0, 1);
    d.sketches[0].profile_ids.swap(0, 1);
    assert_eq!(Model::build(&d).unwrap().info()[0].volume, initial);
    d.sketches[0].profiles.remove(1);
    d.sketches[0].profile_ids.remove(1);
    assert!(Model::build(&d).is_err());
}
#[test]
fn legacy_indices_resolve_before_regions_and_ambiguous_migration_never_writes() {
    let mut d = block(Backend::Occt);
    d.version = 1;
    d.sketches[0]
        .profiles
        .insert(0, rectangle([60., 0.], [70., 10.]));
    d.extrusions[0].profile = 1;
    let migrated = Document::from_json(&d.to_json().unwrap()).unwrap();
    assert_eq!(migrated.version, 2);
    assert_eq!(migrated.extrusions[0].region_id, Some(2));
    assert!((Model::build(&migrated).unwrap().info()[0].volume - 6000.).abs() < 1e-5);
    d.sketches[0].profiles.push(Profile::Circle {
        center: [20., 15.],
        radius: 5.,
    });
    let original = d.to_json().unwrap();
    assert!(
        Document::from_json(&original)
            .unwrap_err()
            .to_string()
            .contains("ambiguous")
    );
    assert_eq!(d.to_json().unwrap(), original);
}
#[test]
fn holes_split_empty_results_and_explicit_output_identity_both_backends() {
    for backend in [Backend::Occt, Backend::Rust] {
        let mut d = block(backend);
        d.sketches[0]
            .add_profile(Profile::Circle {
                center: [20., 15.],
                radius: 5.,
            })
            .unwrap();
        let model = Model::build(&d).unwrap();
        assert_eq!(model.info()[0].backend, backend);
        assert!((model.info()[0].volume - (6000. - 125. * std::f64::consts::PI)).abs() < 1e-5);
        let mut split = block(backend);
        split
            .sketches
            .push(sketch(4, vec![rectangle([18., -1.], [22., 31.])]));
        let mut cut = extrusion(5, 4, 1, 3, 5., Operation::Cut);
        cut.result_body_ids = vec![3, 6];
        split.extrusions.push(cut);
        let model = Model::build(&split).unwrap();
        let info = model.info();
        assert_eq!(info.len(), 2);
        assert_eq!(info.iter().map(|b| b.id).collect::<Vec<_>>(), vec![3, 6]);
        for b in info {
            assert!((b.volume - 2700.).abs() < 1e-5);
        }
        assert_eq!(
            Document::from_json(&split.to_json().unwrap()).unwrap(),
            split
        );
        split.extrusions[1].result_body_ids.clear();
        assert!(Model::build(&split).is_err());
        let mut empty = block(backend);
        empty
            .sketches
            .push(sketch(4, vec![rectangle([-1., -1.], [41., 31.])]));
        empty
            .extrusions
            .push(extrusion(5, 4, 1, 3, 5., Operation::Cut));
        assert!(Model::build(&empty).unwrap().bodies.is_empty());
        // Later references to a removed target fail explicitly.
        empty
            .extrusions
            .push(extrusion(7, 4, 1, 3, 5., Operation::Cut));
        assert!(Model::build(&empty).is_err());
    }
}
fn input(outer: Profile, holes: Vec<Profile>) -> ExtrudeInput {
    ExtrudeInput {
        feature_id: 10,
        region_id: 1,
        loop_ids: (1..=(holes.len() + 1) as u64).collect(),
        outer,
        holes,
        plane: Plane::XY,
        origin: [0.; 3],
        distance: 5.,
    }
}
#[test]
fn rust_curved_brep_topology_mass_bounds_and_occt_differential() {
    for outer in [
        rectangle([0., 0.], [40., 30.]),
        Profile::Circle {
            center: [20., 15.],
            radius: 18.,
        },
    ] {
        for plane in [Plane::XY, Plane::XZ, Plane::YZ] {
            for distance in [5., -5.] {
                let mut input = input(
                    outer.clone(),
                    vec![Profile::Circle {
                        center: [20., 15.],
                        radius: 5.,
                    }],
                );
                input.plane = plane;
                input.distance = distance;
                input.origin = [13., -7., 8.];
                let native = NativeSolid::extrude(input.clone()).unwrap();
                native.topology().validate().unwrap();
                assert!(
                    native
                        .topology()
                        .edges
                        .iter()
                        .any(|e| matches!(e.curve, Curve::Circle { .. }))
                );
                assert!(
                    native
                        .topology()
                        .faces
                        .iter()
                        .any(|f| matches!(f.surface, Surface::Cylinder { .. }))
                );
                let other = Backend::Occt.extrude(&input).unwrap();
                assert!((native.volume() - other.volume()).abs() < 1e-5);
                assert!((native.area() - other.area()).abs() < 1e-5);
                for (a, b) in native
                    .bounds()
                    .into_iter()
                    .flatten()
                    .zip(other.bounds().into_iter().flatten())
                {
                    assert!((a - b).abs() < 1e-5);
                }
                let analytic = native.volume();
                for segments in [32, 128] {
                    let mesh = native.mesh(segments).unwrap();
                    assert!(!mesh.indices.is_empty());
                    assert_eq!(mesh.face_ids.len(), mesh.indices.len() / 3);
                    assert_eq!(native.volume(), analytic);
                    // Independently integrate the oriented display shell, allowing chord error.
                    let volume: f64 = mesh
                        .indices
                        .chunks_exact(3)
                        .map(|i| {
                            let a = mesh.vertices[i[0]];
                            let b = mesh.vertices[i[1]];
                            let c = mesh.vertices[i[2]];
                            (a[0] * (b[1] * c[2] - b[2] * c[1])
                                + a[1] * (b[2] * c[0] - b[0] * c[2])
                                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                                / 6.
                        })
                        .sum();
                    assert!(
                        (volume - analytic).abs() / analytic < 0.01,
                        "{volume} != {analytic}"
                    );
                }
                let mut corrupt = native.topology().clone();
                corrupt.faces[0].loops[0][0].reversed ^= true;
                assert!(corrupt.validate().is_err());
            }
        }
    }
}
#[test]
fn rust_boolean_unsupported_is_explicit_without_occt_geometry() {
    let a = Backend::Rust
        .extrude(&input(rectangle([0., 0.], [40., 30.]), vec![]))
        .unwrap();
    let mut tool = input(
        Profile::Circle {
            center: [20., 15.],
            radius: 5.,
        },
        vec![],
    );
    tool.distance = 3.;
    let b = Backend::Rust.extrude(&tool).unwrap();
    assert!(
        Backend::Rust
            .boolean(&a, &b, Operation::Cut, 11)
            .err()
            .unwrap()
            .to_string()
            .contains("unsupported")
    );
    assert!(matches!(a, Geometry::Rust(_)));
}

#[test]
fn persisted_parameter_dependencies_recompute_sketch_and_extrusion_together() {
    for backend in [Backend::Occt, Backend::Rust] {
        let mut d = block(backend);
        d.parameters.insert("width".into(), "40 mm".into());
        d.parameters.insert("depth".into(), "2 * 3 mm".into());
        d.sketches[0] = d.sketches[0]
            .with_dimensions(
                1,
                std::collections::BTreeMap::from([
                    ("width".into(), "width".into()),
                    ("height".into(), "30".into()),
                ]),
                &d.parameters,
            )
            .unwrap();
        d.extrusions[0].distance_expression = Some("depth".into());
        d = d.recomputed().unwrap();
        let mut session = Session::new(d.clone()).unwrap();
        let changed = d.with_parameter("width".into(), "80 mm".into()).unwrap();
        session.replace(changed.clone()).unwrap();
        assert!((session.model.info()[0].volume - 14400.).abs() < 1e-5);
        session.undo().unwrap();
        assert!((session.model.info()[0].volume - 7200.).abs() < 1e-5);
        session.redo().unwrap();
        assert!((session.model.info()[0].volume - 14400.).abs() < 1e-5);
        let reopened = Document::from_json(&session.document.to_json().unwrap()).unwrap();
        assert_eq!(
            reopened.extrusions[0].distance_expression.as_deref(),
            Some("depth")
        );
        assert_eq!(
            reopened.sketches[0].dimension_expressions[&1]["width"],
            "width"
        );
        assert!((Model::build(&reopened).unwrap().info()[0].volume - 14400.).abs() < 1e-5);
        assert!(
            reopened
                .with_parameter("width".into(), "width + 1".into())
                .is_err()
        );
    }
}

#[test]
fn split_output_ids_are_materialized_persisted_and_targetable() {
    for backend in [Backend::Occt, Backend::Rust] {
        let mut d = block(backend);
        d.sketches
            .push(sketch(4, vec![rectangle([18., -1.], [22., 31.])]));
        d.extrusions.push(extrusion(5, 4, 1, 3, 5., Operation::Cut));
        let mut session = Session::new(d).unwrap();
        let ids = session.document.extrusions[1].result_body_ids.clone();
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0], 3);
        let mut edit = session.document.clone();
        edit.sketches
            .push(sketch(20, vec![rectangle([22., 0.], [40., 30.])]));
        edit.extrusions
            .push(extrusion(21, 20, 1, ids[1], 5., Operation::Cut));
        session.replace(edit).unwrap();
        assert_eq!(session.model.info().len(), 1);
        assert_eq!(session.model.info()[0].id, 3);
        assert!((session.model.info()[0].volume - 2700.).abs() < 1e-5);
        let reopened =
            Session::new(Document::from_json(&session.document.to_json().unwrap()).unwrap())
                .unwrap();
        assert_eq!(reopened.model.info().len(), 1);
        assert_eq!(reopened.model.info()[0].id, 3);
    }
}

#[test]
fn rust_brep_boundary_reconstructed_by_independent_occt_and_step_reader() {
    use cadrum::{DVec3, Edge, Solid};
    // This adapter exists only in the test. It consumes exact Rust B-rep boundary
    // geometry, never its mesh, and is not a production modeling fallback.
    for plane in [Plane::XY, Plane::XZ, Plane::YZ] {
        let mut i = input(
            rectangle([0., 0.], [40., 30.]),
            vec![Profile::Circle {
                center: [20., 15.],
                radius: 5.,
            }],
        );
        i.plane = plane;
        i.distance = -5.;
        let rust = NativeSolid::extrude(i).unwrap();
        let brep = rust.topology();
        let mut edges = vec![];
        for boundary in &brep.faces[0].loops {
            for co in boundary {
                let edge = &brep.edges[co.edge];
                edges.push(match edge.curve {
                    Curve::Circle {
                        center,
                        normal,
                        radius,
                    } => Edge::circle(radius, DVec3::from_array(normal))
                        .unwrap()
                        .translate(DVec3::from_array(center)),
                    Curve::Line => {
                        let mut v = edge.vertices;
                        if co.reversed {
                            v.swap(0, 1);
                        }
                        Edge::line(
                            DVec3::from_array(brep.vertices[v[0]].point),
                            DVec3::from_array(brep.vertices[v[1]].point),
                        )
                        .unwrap()
                    }
                });
            }
        }
        let Surface::Plane { origin: base, .. } = brep.faces[0].surface else {
            panic!()
        };
        let Surface::Plane { origin: top, .. } = brep.faces[1].surface else {
            panic!()
        };
        let imported =
            Solid::extrude(&edges, DVec3::from_array(top) - DVec3::from_array(base)).unwrap();
        assert!((imported.volume() - rust.volume()).abs() < 1e-5);
        assert!((imported.area() - rust.area()).abs() < 1e-5);
        let mut step = vec![];
        Solid::write_step([&imported], &mut step).unwrap();
        assert!((Model::step_volume(&step).unwrap() - rust.volume()).abs() < 1e-5);
    }
}

#[test]
fn native_boolean_identities_and_translated_negative_extrusions() {
    for k in 1..=30 {
        let x = k as f64 * 0.37;
        let mut i = input(rectangle([x, -x], [x + 10. + x, -x + 20.]), vec![]);
        i.origin = [-3., 7., k as f64];
        i.distance = -(k as f64 + 1.);
        let native = Backend::Rust.extrude(&i).unwrap();
        let occt = Backend::Occt.extrude(&i).unwrap();
        assert!((native.volume() - occt.volume()).abs() < 1e-5);
        assert!(
            Backend::Rust
                .boolean(&native, &native, Operation::Cut, 99)
                .unwrap()
                .is_empty()
        );
        assert!(
            (Backend::Rust
                .boolean(&native, &native, Operation::Join, 99)
                .unwrap()[0]
                .volume()
                - native.volume())
            .abs()
                < 1e-7
        );
    }
    let circle = Backend::Rust
        .extrude(&input(
            Profile::Circle {
                center: [0., 0.],
                radius: 5.,
            },
            vec![],
        ))
        .unwrap();
    assert!(
        Backend::Rust
            .boolean(&circle, &circle, Operation::Cut, 99)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn polygon_regions_with_holes_and_large_legacy_loop_are_validated_analytically() {
    let concave = Profile::Polygon {
        points: vec![
            [0., 0.],
            [40., 0.],
            [40., 30.],
            [25., 30.],
            [25., 15.],
            [0., 15.],
        ],
    };
    let hole = Profile::Circle {
        center: [10., 7.],
        radius: 3.,
    };
    let native = Backend::Rust
        .extrude(&input(concave.clone(), vec![hole.clone()]))
        .unwrap();
    let occt = Backend::Occt.extrude(&input(concave, vec![hole])).unwrap();
    assert!((native.volume() - occt.volume()).abs() < 1e-5);
    assert!((native.area() - occt.area()).abs() < 1e-5);
    assert!(!native.mesh().unwrap().indices.is_empty());
    let points = (0..200)
        .map(|i| {
            let t = std::f64::consts::TAU * i as f64 / 200.;
            [20. * t.cos(), 20. * t.sin()]
        })
        .collect();
    let mut d = block(Backend::Occt);
    d.version = 1;
    d.sketches[0].profiles = vec![Profile::Polygon { points }];
    let migrated = Document::from_json(&d.to_json().unwrap()).unwrap();
    assert!(Model::build(&migrated).unwrap().info()[0].volume > 6000.);
    let mut touching = block(Backend::Rust);
    touching.sketches[0]
        .add_profile(Profile::Circle {
            center: [20., 15.],
            radius: 15.,
        })
        .unwrap();
    assert!(touching.validate().is_err());
}
