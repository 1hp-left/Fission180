use fission180_core::features::{Axis, Feature, FeatureKind, PlanarSupport, RegionRef};
use fission180_core::kernel::Backend;
use fission180_core::{Document, Extrusion, Model, Operation, Plane, Profile, Session, Sketch};
fn sketch(id: u64, origin: [f64; 3], profile: Profile) -> Sketch {
    let mut s = Sketch {
        id,
        name: format!("Sketch{id}"),
        origin,
        ..Default::default()
    };
    s.add_profile(profile).unwrap();
    s
}
fn rectangle(a: [f64; 2], b: [f64; 2]) -> Profile {
    Profile::Rectangle { a, b }
}
fn block(backend: Backend) -> Document {
    Document {
        backend,
        sketches: vec![sketch(1, [0.; 3], rectangle([0., 0.], [40., 30.]))],
        extrusions: vec![Extrusion {
            id: 2,
            name: "Block".into(),
            sketch_id: 1,
            profile: 0,
            region_id: Some(1),
            distance: 5.,
            distance_expression: None,
            operation: Operation::NewBody,
            body_id: 3,
            result_body_ids: vec![],
        }],
        ..Default::default()
    }
}
fn append(d: &mut Document, kind: FeatureKind, target: Option<u64>) {
    d.push_feature(Feature {
        id: d.next_id().unwrap(),
        name: kind.label().into(),
        target,
        outputs: vec![],
        kind,
    })
    .unwrap();
}
fn check_reopen(d: Document) -> Session {
    let s = Session::new(d).unwrap();
    let reopened =
        Session::new(Document::from_json(&s.document.to_json().unwrap()).unwrap()).unwrap();
    assert_eq!(s.document, reopened.document);
    assert_eq!(s.model.info().len(), reopened.model.info().len());
    for (a, b) in s.model.info().iter().zip(reopened.model.info()) {
        assert!((a.volume - b.volume).abs() < 1e-5);
        assert_eq!(a.id, b.id);
    }
    assert!(!s.model.display_mesh().unwrap().indices.is_empty());
    s
}
#[test]
fn revolve_and_partial_revolve_have_analytic_volume_and_area() {
    for degrees in [360., 180., -180.] {
        let mut d = Document::default();
        d.sketches
            .push(sketch(1, [0.; 3], rectangle([5., 0.], [10., 20.])));
        append(
            &mut d,
            FeatureKind::Revolve {
                region: RegionRef {
                    sketch_id: 1,
                    region_id: 1,
                },
                axis: Axis::Y,
                axis_origin: [0.; 3],
                degrees,
            },
            None,
        );
        let s = check_reopen(d);
        let info = &s.model.info()[0];
        assert!((info.volume - 1500. * std::f64::consts::PI * degrees.abs() / 360.).abs() < 1e-5);
        let expected = if degrees.abs() == 360. {
            750. * std::f64::consts::PI
        } else {
            375. * std::f64::consts::PI + 200.
        };
        assert!((info.surface_area - expected).abs() < 1e-5);
    }
}
#[test]
fn hole_fillet_chamfer_shell_and_face_offset_are_real_replayable_solids() {
    for backend in [Backend::Occt, Backend::Rust] {
        let mut d = block(backend);
        append(
            &mut d,
            FeatureKind::Hole {
                center: [20., 15.],
                start_z: 0.,
                diameter: "2 * 5 mm".into(),
                depth: "5 mm".into(),
            },
            Some(3),
        );
        assert!(
            (check_reopen(d).model.info()[0].volume - (6000. - 125. * std::f64::consts::PI)).abs()
                < 1e-5
        );
    }
    let pi = std::f64::consts::PI;
    for (kind, expected) in [
        (
            FeatureKind::Fillet {
                radius: "1 mm".into(),
            },
            Some(6000. - (4. - pi) * 75. + 16. - 14. * pi / 3.),
        ),
        (
            FeatureKind::Chamfer {
                distance: "1 mm".into(),
            },
            None,
        ),
        (
            FeatureKind::Shell {
                thickness: "1 mm".into(),
                open_axis: Axis::Z,
                positive: true,
            },
            Some(6000. - 38. * 28. * 4.),
        ),
        (
            FeatureKind::OffsetFace {
                distance: "2 mm".into(),
                axis: Axis::Z,
                positive: true,
            },
            Some(8400.),
        ),
    ] {
        let mut d = block(Backend::Occt);
        let name = kind.label();
        append(&mut d, kind, Some(3));
        let s = check_reopen(d);
        let volume = s.model.info()[0].volume;
        if let Some(expected) = expected {
            assert!(
                (volume - expected).abs() < 1e-4,
                "{name}: {volume} != {expected}"
            );
        } else {
            assert!(volume > 0. && volume < 6000.);
        }
        assert!(
            (Model::step_volume(&s.model.step_bytes().unwrap()).unwrap() - volume).abs() < 1e-5
        );
    }
}
#[test]
fn transforms_patterns_split_and_combine_preserve_body_references() {
    for backend in [Backend::Occt, Backend::Rust] {
        let mut d = block(backend);
        append(
            &mut d,
            FeatureKind::RectangularPattern {
                axis: Axis::X,
                spacing: "50".into(),
                count: 3,
            },
            Some(3),
        );
        let mut s = check_reopen(d);
        assert_eq!(s.model.info().len(), 3);
        let ids = s.document.features[0].outputs.clone();
        assert_eq!(ids.len(), 2);
        let mut changed = s.document.clone();
        append(
            &mut changed,
            FeatureKind::Translate {
                offset: ["5".into(), "0".into(), "0".into()],
                copy: false,
            },
            Some(ids[0]),
        );
        s.replace(changed).unwrap();
        let moved = s.model.info().into_iter().find(|b| b.id == ids[0]).unwrap();
        assert!((moved.bounds[0][0] - 55.).abs() < 1e-5);
        let mut d = block(backend);
        append(
            &mut d,
            FeatureKind::SplitBody {
                axis: Axis::X,
                position: "20 mm".into(),
            },
            Some(3),
        );
        let split = check_reopen(d);
        assert_eq!(split.model.info().len(), 2);
        for b in split.model.info() {
            assert!((b.volume - 3000.).abs() < 1e-5);
        }
        let other = split.document.features[0].outputs[1];
        let mut d = split.document.clone();
        append(
            &mut d,
            FeatureKind::Combine {
                tool_body: other,
                operation: Operation::Join,
                keep_tool: false,
            },
            Some(3),
        );
        let joined = check_reopen(d);
        assert_eq!(joined.model.info().len(), 1);
        assert!((joined.model.info()[0].volume - 6000.).abs() < 1e-5);
    }
    for kind in [
        FeatureKind::Mirror {
            axis: Axis::X,
            origin: [0.; 3],
        },
        FeatureKind::CircularPattern {
            axis: Axis::Z,
            origin: [0.; 3],
            degrees: 360.,
            count: 4,
        },
    ] {
        let mut d = block(Backend::Occt);
        let count = if matches!(kind, FeatureKind::Mirror { .. }) {
            2
        } else {
            4
        };
        append(&mut d, kind, Some(3));
        let s = check_reopen(d);
        assert_eq!(s.model.info().len(), count);
        for b in s.model.info() {
            assert!((b.volume - 6000.).abs() < 1e-5);
        }
    }
}
#[test]
fn straight_sweep_and_ruled_loft_match_analytic_prisms() {
    let mut d = Document::default();
    d.sketches.push(sketch(
        1,
        [0.; 3],
        Profile::Circle {
            center: [0., 0.],
            radius: 5.,
        },
    ));
    let path = Sketch {
        id: 2,
        name: "Path".into(),
        plane: Plane::XZ,
        ..Default::default()
    }
    .with_line([0., 0.], [0., 20.], &Default::default())
    .unwrap();
    let entity = *path.graph.as_ref().unwrap().entities.keys().next().unwrap();
    d.sketches.push(path);
    append(
        &mut d,
        FeatureKind::Sweep {
            region: RegionRef {
                sketch_id: 1,
                region_id: 1,
            },
            path_sketch: 2,
            path_entities: vec![entity],
        },
        None,
    );
    assert!((check_reopen(d).model.info()[0].volume - 500. * std::f64::consts::PI).abs() < 1e-5);
    let mut d = Document::default();
    d.sketches
        .push(sketch(1, [0.; 3], rectangle([0., 0.], [10., 20.])));
    d.sketches
        .push(sketch(2, [0., 0., 30.], rectangle([0., 0.], [10., 20.])));
    append(
        &mut d,
        FeatureKind::Loft {
            sections: vec![
                RegionRef {
                    sketch_id: 1,
                    region_id: 1,
                },
                RegionRef {
                    sketch_id: 2,
                    region_id: 1,
                },
            ],
            ruled: true,
        },
        None,
    );
    assert!((check_reopen(d).model.info()[0].volume - 6000.).abs() < 1e-4);
}
#[test]
fn face_supported_sketch_tracks_upstream_height_and_timeline_order() {
    for backend in [Backend::Occt, Backend::Rust] {
        let mut d = block(backend);
        append(
            &mut d,
            FeatureKind::Translate {
                offset: ["0".into(), "0".into(), "5".into()],
                copy: false,
            },
            Some(3),
        );
        let mut top = sketch(10, [0., 0., 10.], rectangle([0., 0.], [10., 10.]));
        top.support = Some(PlanarSupport {
            body_id: 3,
            axis: Axis::Z,
            positive: true,
            offset: 0.,
        });
        d.sketches.push(top);
        d.push_extrusion(Extrusion {
            id: 11,
            name: "Top".into(),
            sketch_id: 10,
            profile: 0,
            region_id: Some(1),
            distance: 2.,
            distance_expression: None,
            operation: Operation::NewBody,
            body_id: 12,
            result_body_ids: vec![],
        });
        let mut s = check_reopen(d);
        assert_eq!(s.document.timeline, vec![2, 4, 11]);
        let mut edit = s.document.clone();
        edit.extrusions[0].distance = 8.;
        s.replace(edit).unwrap();
        let top = s.model.info().into_iter().find(|b| b.id == 12).unwrap();
        assert!((top.bounds[0][2] - 13.).abs() < 1e-5);
        let accepted = s.document.clone();
        let mut bad = accepted.clone();
        bad.features[0].target = Some(9999);
        assert!(s.replace(bad).is_err());
        assert_eq!(s.document, accepted);
    }
}

#[test]
fn pattern_quantity_edit_suppression_delete_and_dangling_reference_rollback() {
    let mut d = block(Backend::Occt);
    append(
        &mut d,
        FeatureKind::RectangularPattern {
            axis: Axis::X,
            spacing: "50".into(),
            count: 3,
        },
        Some(3),
    );
    let mut s = Session::new(d).unwrap();
    let old_ids = s.document.features[0].outputs.clone();
    let pattern = s.document.features[0].id;
    let mut edit = s.document.clone();
    if let FeatureKind::RectangularPattern { count, .. } = &mut edit.features[0].kind {
        *count = 5;
    }
    s.replace(edit).unwrap();
    assert_eq!(&s.document.features[0].outputs[..2], old_ids.as_slice());
    assert_eq!(s.model.bodies.len(), 5);
    let last = *s.document.features[0].outputs.last().unwrap();
    let mut d = s.document.clone();
    append(
        &mut d,
        FeatureKind::Translate {
            offset: ["5".into(), "0".into(), "0".into()],
            copy: false,
        },
        Some(last),
    );
    s.replace(d).unwrap();
    let accepted = s.document.clone();
    let mut broken = accepted.clone();
    if let FeatureKind::RectangularPattern { count, .. } = &mut broken.features[0].kind {
        *count = 2;
    }
    assert!(s.replace(broken).is_err());
    assert_eq!(s.document, accepted);
    let suppressed = accepted.with_suppression(pattern, true).unwrap();
    assert_eq!(
        Session::new(suppressed.clone()).unwrap().model.bodies.len(),
        1
    );
    assert_eq!(
        Session::new(suppressed.with_suppression(pattern, false).unwrap())
            .unwrap()
            .model
            .bodies
            .len(),
        5
    );
    let deleted = accepted.without_feature(pattern).unwrap();
    assert!(deleted.features.is_empty());
    assert_eq!(Session::new(deleted).unwrap().model.bodies.len(), 1);
}

#[test]
fn unsupported_new_rust_feature_cannot_change_backend_or_last_good_geometry() {
    let mut s = Session::new(block(Backend::Rust)).unwrap();
    let before = s.document.clone();
    let mut d = before.clone();
    append(
        &mut d,
        FeatureKind::Fillet {
            radius: "1 mm".into(),
        },
        Some(3),
    );
    let error = s.replace(d).unwrap_err().to_string();
    assert!(error.contains("unsupported by Rust"));
    assert_eq!(s.document, before);
    assert_eq!(s.model.info()[0].backend, Backend::Rust);
    assert!((s.model.info()[0].volume - 6000.).abs() < 1e-5);
}

#[test]
fn legacy_writer_rejects_silent_loss_of_new_semantics() {
    let mut d = block(Backend::Occt);
    d.version = 1;
    assert!(d.to_json().is_err());
    d.sketches[0].graph = None;
    assert!(d.to_json().is_ok());
    d.extrusions[0].distance_expression = Some("2 + 3 mm".into());
    assert!(d.to_json().is_err());
    d.extrusions[0].distance_expression = None;
    d.suppressed.insert(2);
    assert!(d.to_json().is_err());
}
