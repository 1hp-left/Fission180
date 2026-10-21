#[path = "../examples/support/axial_fixtures.rs"]
#[allow(dead_code)]
mod fixtures;
use fission180_core::{
    Document, Model, Operation, Plane, Session,
    kernel::{
        Backend, Geometry, SolidKernel,
        native::{self, NativeSolid},
    },
};
use fixtures::*;
use std::f64::consts::PI;
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-5, "{a} != {b}");
}
fn execute(case: &Case, backend: Backend) -> Vec<Geometry> {
    let mut result = vec![backend.extrude(&case.base).unwrap()];
    for (tool, op) in &case.tools {
        let tool = backend.extrude(tool).unwrap();
        result = result
            .iter()
            .flat_map(|a| backend.boolean(a, &tool, *op, 97).unwrap())
            .collect();
    }
    result
}
fn mesh_volume(s: &NativeSolid, n: usize) -> f64 {
    let m = s.mesh(n).unwrap();
    assert!(
        m.face_ids
            .iter()
            .flatten()
            .all(|id| s.topology().faces.iter().any(|f| f.id == *id))
    );
    assert!(
        m.edge_ids
            .iter()
            .all(|id| s.topology().edges.iter().any(|e| e.id == *id))
    );
    let origin = cadrum::DVec3::from_array(s.bounds()[0]);
    m.indices
        .chunks_exact(3)
        .map(|t| {
            let p: Vec<_> = t
                .iter()
                .map(|i| cadrum::DVec3::from_array(m.vertices[*i]) - origin)
                .collect();
            p[0].dot(p[1].cross(p[2])) / 6.
        })
        .sum()
}
#[test]
fn required_axial_cases_analytic_measurements_topology_mesh_and_independent_occt() {
    for case in cases() {
        let rust = execute(&case, Backend::Rust);
        let oracle = execute(&case, Backend::Occt);
        assert_eq!(rust.len(), case.volumes.len(), "{}", case.name);
        assert_eq!(rust.len(), oracle.len(), "{} oracle", case.name);
        let expected = case.volumes.iter().sum::<f64>();
        close(rust.iter().map(Geometry::volume).sum(), expected);
        close(oracle.iter().map(Geometry::volume).sum(), expected);
        let area = rust.iter().map(Geometry::area).sum::<f64>();
        close(area, oracle.iter().map(Geometry::area).sum());
        if let Some(expected) = case.area {
            close(area, expected);
        }
        let mut rv: Vec<_> = rust.iter().map(Geometry::volume).collect();
        rv.sort_by(f64::total_cmp);
        let mut ev = case.volumes.clone();
        ev.sort_by(f64::total_cmp);
        for (a, b) in rv.into_iter().zip(ev) {
            close(a, b);
        }
        for s in &rust {
            let Geometry::Rust(s) = s else { panic!() };
            s.topology().validate().unwrap();
            assert!(s.planar_region().is_none());
            let sections = s.occupied_sections().unwrap();
            close(
                sections.iter().map(|(z, r)| (z[1] - z[0]) * r.area()).sum(),
                s.volume(),
            );
            let analytic = (s.volume(), s.area(), s.bounds());
            for n in [24, 192] {
                let mesh = mesh_volume(s, n);
                assert!(
                    (mesh - s.volume()).abs() < s.volume() * 0.025,
                    "{} mesh",
                    case.name
                );
                assert_eq!(analytic, (s.volume(), s.area(), s.bounds()));
            }
            let peer = oracle
                .iter()
                .find(|o| {
                    (o.volume() - s.volume()).abs() < 1e-5
                        && o.bounds()
                            .iter()
                            .flatten()
                            .zip(s.bounds().iter().flatten())
                            .all(|(a, b)| (a - b).abs() < 2e-5)
                })
                .unwrap();
            close(peer.area(), s.area());
        }
        if oracle.is_empty() {
            continue;
        }
        let mut bytes = vec![];
        cadrum::Solid::write_step(
            oracle.iter().map(|s| {
                if let Geometry::Occt(s) = s {
                    s
                } else {
                    panic!()
                }
            }),
            &mut bytes,
        )
        .unwrap();
        close(Model::step_volume(&bytes).unwrap(), expected);
    }
}
#[test]
fn every_principal_frame_translated_origins_and_reversed_negative_directions() {
    for plane in [Plane::XY, Plane::XZ, Plane::YZ] {
        for negative in [false, true] {
            for reverse in [false, true] {
                for mut case in cases() {
                    reframe(&mut case, plane, negative, reverse);
                    let rust = execute(&case, Backend::Rust);
                    let oracle = execute(&case, Backend::Occt);
                    assert_eq!(rust.len(), oracle.len());
                    close(
                        rust.iter().map(Geometry::volume).sum(),
                        case.volumes.iter().sum(),
                    );
                    close(
                        rust.iter().map(Geometry::area).sum(),
                        oracle.iter().map(Geometry::area).sum(),
                    );
                    for s in rust {
                        assert!(oracle.iter().any(|o| {
                            o.bounds()
                                .iter()
                                .flatten()
                                .zip(s.bounds().iter().flatten())
                                .all(|(a, b)| (a - b).abs() < 2e-5)
                        }));
                        let Geometry::Rust(s) = s else { panic!() };
                        s.topology().validate().unwrap();
                        mesh_volume(&s, 24);
                    }
                }
            }
        }
    }
}
#[test]
fn chained_stepped_operands_transforms_identity_conservation_and_exact_bounds() {
    let case = cases()
        .into_iter()
        .find(|c| c.name == "unequal-circle-join")
        .unwrap();
    let mut output = execute(&case, Backend::Rust);
    let Geometry::Rust(joined) = output.remove(0) else {
        panic!()
    };
    let moved = joined.translated([30., -7., 9.], 50).unwrap();
    close(moved.volume(), joined.volume());
    close(moved.area(), joined.area());
    for i in 0..3 {
        for j in 0..2 {
            close(
                moved.bounds()[j][i],
                joined.bounds()[j][i] + [30., -7., 9.][i],
            );
        }
    }
    let b = joined.translated([4., 0., 2.], 51).unwrap();
    let union = native::boolean(&joined, &b, Operation::Join, 55).unwrap();
    let intersection = native::boolean(&joined, &b, Operation::Intersect, 56).unwrap();
    let difference = native::boolean(&joined, &b, Operation::Cut, 57).unwrap();
    let sum = |r: &Vec<NativeSolid>| r.iter().map(NativeSolid::volume).sum::<f64>();
    close(
        sum(&union) + sum(&intersection),
        joined.volume() + b.volume(),
    );
    close(sum(&difference) + sum(&intersection), joined.volume());
    for (op, v) in [
        (Operation::Join, joined.volume()),
        (Operation::Intersect, joined.volume()),
        (Operation::Cut, 0.),
    ] {
        let r = native::boolean(&joined, &joined, op, 60).unwrap();
        close(sum(&r), v);
        for s in r {
            s.topology().validate().unwrap();
        }
    }
    let tool = NativeSolid::extrude(prism(rect(-30., -30., 60., 60.), 6., 8., 70)).unwrap();
    let cut = native::boolean(&joined, &tool, Operation::Cut, 71).unwrap();
    assert_eq!(cut.len(), 2);
    close(
        cut[0].volume() + cut[1].volume(),
        joined.volume() - 2. * (200. * PI - lens(10., 10., 10.)),
    );
    for s in union
        .iter()
        .chain(&intersection)
        .chain(&difference)
        .chain(&cut)
    {
        s.topology().validate().unwrap();
        mesh_volume(s, 32);
    }
    // Axially separated outputs remain separate when rejoined. A real bridge
    // restores one solid, rather than reviving either operand's old envelope.
    let box_shape = NativeSolid::extrude(prism(rect(0., 0., 40., 20.), 0., 10., 80)).unwrap();
    let gap = NativeSolid::extrude(prism(rect(-1., -1., 42., 22.), 4., 6., 81)).unwrap();
    let pieces = native::boolean(&box_shape, &gap, Operation::Cut, 82).unwrap();
    let separated = native::boolean(&pieces[0], &pieces[1], Operation::Join, 83).unwrap();
    assert_eq!(separated.len(), 2);
    close(sum(&separated), 6400.);
    let bridge = NativeSolid::extrude(prism(rect(0., 0., 40., 20.), 4., 6., 84)).unwrap();
    let connected = native::boolean(&pieces[0], &bridge, Operation::Join, 85).unwrap();
    let restored = native::boolean(&connected[0], &pieces[1], Operation::Join, 86).unwrap();
    assert_eq!(restored.len(), 1);
    close(restored[0].volume(), 8000.);
    close(restored[0].area(), 2800.);
    let translated_tool = NativeSolid::extrude({
        let mut t = prism(rect(-30., -30., 60., 60.), 6., 8., 70);
        t.origin = [30., -7., 15.];
        t
    })
    .unwrap();
    let moved_cut = native::boolean(&moved, &translated_tool, Operation::Cut, 72).unwrap();
    close(sum(&moved_cut), sum(&cut));
}
#[test]
fn replay_edits_stable_ids_saved_v5_axial_splits_and_transactional_failures() {
    for case in cases() {
        let mut session = Session::new(document(&case, Backend::Rust)).unwrap();
        let expected = case.volumes.iter().sum();
        close(
            session.model.info().iter().map(|b| b.volume).sum(),
            expected,
        );
        let bytes = session.document.to_json().unwrap();
        let reopened = Session::new(Document::from_json(&bytes).unwrap()).unwrap();
        assert_eq!(reopened.document, session.document);
        if case.name == "axial-gap-split" {
            assert_eq!(session.document.extrusions[1].result_body_ids, vec![3, 6]);
            let lower = session.model.bodies.iter().find(|b| b.id == 3).unwrap();
            close(lower.solid.bounds()[1][2], 4.);
            let mut edit = session.document.clone();
            edit.extrusions[1].distance = 1.;
            session.replace(edit).unwrap();
            close(session.model.info().iter().map(|b| b.volume).sum(), 7200.);
            let mut changed = session.document.clone();
            changed.extrusions[1].distance = 6.;
            assert!(session.replace(changed).is_err());
        }
        if case.name != "blind-sidewall-notch" {
            continue;
        }
        let ids = |m: &Model| {
            let Geometry::Rust(s) = &m.bodies[0].solid else {
                panic!()
            };
            s.topology().edges.iter().map(|e| e.id).collect::<Vec<_>>()
        };
        let old_ids = ids(&session.model);
        let mut edit = session.document.clone();
        edit.parameters.insert("depth".into(), "4 mm".into());
        edit.extrusions[1].distance_expression = Some("-depth".into());
        edit.extrusions[1].distance = -4.;
        session.replace(edit).unwrap();
        close(session.model.info()[0].volume, 8000. - 50. * PI);
        assert_eq!(ids(&session.model), old_ids);
        let accepted = session.document.clone();
        let volume = session.model.info()[0].volume;
        let mut changed = accepted.clone();
        changed.sketches[1].origin[0] = 100.;
        assert!(session.replace(changed).is_err());
        let fail = |_: &Model| {
            Err(fission180_core::Error::Invalid(
                "injected mesh failure".into(),
            ))
        };
        let mut edit = accepted.clone();
        edit = edit.with_parameter("depth".into(), "5 mm".into()).unwrap();
        assert!(session.replace_with_display(edit, fail).is_err());
        assert_eq!(accepted, session.document);
        close(session.model.info()[0].volume, volume);
        assert!(session.undo_with_display(fail).is_err());
        assert_eq!(accepted, session.document);
        session.undo().unwrap();
        close(session.model.info()[0].volume, expected);
        session.redo().unwrap();
        close(session.model.info()[0].volume, volume);
    }
}
#[test]
fn enclosed_cavities_tangent_contacts_invalid_frames_and_near_resolution_are_rejected() {
    let base = NativeSolid::extrude(prism(rect(-20., -20., 40., 40.), 0., 10., 2)).unwrap();
    for tool in [
        prism(disk(0., 4.), 3., 7., 5),
        prism(disk(0., 4.), 10. - 1e-6, 12., 5),
    ] {
        assert!(
            native::boolean(
                &base,
                &NativeSolid::extrude(tool).unwrap(),
                Operation::Cut,
                9
            )
            .is_err()
        );
    }
    let a = NativeSolid::extrude(prism(disk(0., 5.), 0., 5., 2)).unwrap();
    for x in [10., 10. + 1e-6] {
        let b = NativeSolid::extrude(prism(disk(x, 5.), 5., 10., 5)).unwrap();
        assert!(native::boolean(&a, &b, Operation::Join, 9).is_err());
        assert!(
            native::boolean(&a, &b, Operation::Intersect, 9)
                .unwrap()
                .is_empty()
        );
    }
    let b = NativeSolid::extrude(prism(disk(8., 5.), 5., 10., 5)).unwrap();
    let joined = native::boolean(&a, &b, Operation::Join, 9).unwrap();
    assert_eq!(joined.len(), 1);
    close(joined[0].volume(), 250. * PI);
    joined[0].topology().validate().unwrap();
    let mut nonparallel = prism(disk(0., 4.), 0., 12., 5);
    nonparallel.plane = Plane::XZ;
    assert!(
        native::boolean(
            &base,
            &NativeSolid::extrude(nonparallel).unwrap(),
            Operation::Cut,
            9
        )
        .is_err()
    );
    assert!(native::boolean(&base, &base, Operation::NewBody, 9).is_err());
}

#[test]
fn equal_envelopes_do_not_hide_different_occupied_sections_or_step_tools() {
    let base = prism(rect(-20., -20., 40., 40.), 0., 10., 2);
    let inputs = [
        prism(disk(-8., 3.), 10., 7., 5),
        prism(disk(8., 3.), 0., 4., 8),
    ];
    let mut pairs = vec![];
    for backend in [Backend::Rust, Backend::Occt] {
        let a = backend.extrude(&base).unwrap();
        let pockets: Vec<_> = inputs
            .iter()
            .map(|input| {
                backend
                    .boolean(
                        &a,
                        &backend.extrude(input).unwrap(),
                        Operation::Cut,
                        input.feature_id,
                    )
                    .unwrap()
                    .remove(0)
            })
            .collect();
        for (op, expected) in [
            (Operation::Cut, 36. * PI),
            (Operation::Join, 16000.),
            (Operation::Intersect, 16000. - 63. * PI),
        ] {
            let r = backend.boolean(&pockets[0], &pockets[1], op, 10).unwrap();
            close(r.iter().map(Geometry::volume).sum(), expected);
            if backend == Backend::Rust {
                for s in &r {
                    let Geometry::Rust(s) = s else { panic!() };
                    s.topology().validate().unwrap();
                    mesh_volume(s, 48);
                }
            }
            pairs.push((op, backend, r.iter().map(Geometry::area).sum::<f64>()));
        }
    }
    for op in [Operation::Cut, Operation::Join, Operation::Intersect] {
        let areas: Vec<_> = pairs
            .iter()
            .filter(|(o, _, _)| *o == op)
            .map(|(_, _, a)| *a)
            .collect();
        close(areas[0], areas[1]);
    }
}
#[test]
fn islands_reconnect_across_positive_faces_and_boundary_only_cap_contact_is_rejected() {
    let base = NativeSolid::extrude(prism(rect(-20., -10., 40., 20.), 0., 10., 2)).unwrap();
    let tool = NativeSolid::extrude(prism(rect(-2., -15., 4., 30.), 2., 8., 5)).unwrap();
    let r = native::boolean(&base, &tool, Operation::Cut, 7).unwrap();
    assert_eq!(r.len(), 1);
    close(r[0].volume(), 8000. - 480.);
    assert_eq!(r[0].occupied_sections().unwrap()[1].1.regions.len(), 2);
    r[0].topology().validate().unwrap();
    for (x, y) in [(10., 0.), (10., 10.)] {
        let a = NativeSolid::extrude(prism(rect(0., 0., 10., 10.), 0., 5., 2)).unwrap();
        let b = NativeSolid::extrude(prism(rect(x, y, 10., 10.), 5., 10., 5)).unwrap();
        assert!(native::boolean(&a, &b, Operation::Join, 7).is_err());
    }
}
#[test]
fn stepped_transform_ancestry_growth_is_bounded_and_failure_retains_the_shape() {
    let case = cases()
        .into_iter()
        .find(|c| c.name == "blind-sidewall-notch")
        .unwrap();
    let Geometry::Rust(mut s) = execute(&case, Backend::Rust).remove(0) else {
        panic!()
    };
    for i in 1..=64 {
        s = s.translated([0.; 3], 100 + i).unwrap();
    }
    let accepted = (s.volume(), s.area(), s.bounds(), s.output_signature());
    assert!(
        s.translated([0.; 3], 165)
            .unwrap_err()
            .to_string()
            .contains("64 instances")
    );
    assert_eq!(
        accepted,
        (s.volume(), s.area(), s.bounds(), s.output_signature())
    );
}

#[test]
fn repeated_axial_joins_stop_at_an_explicit_resource_limit_and_keep_the_last_result() {
    let mut accepted = NativeSolid::extrude(prism(rect(0., 0., 10., 10.), 0., 1., 2)).unwrap();
    for i in 1..=64 {
        let tool = NativeSolid::extrude(prism(
            rect(0., 0., 10., 10.),
            i as f64,
            i as f64 + 1.,
            10 + i,
        ))
        .unwrap();
        match native::boolean(&accepted, &tool, Operation::Join, 100 + i) {
            Ok(mut outputs) => {
                assert_eq!(outputs.len(), 1);
                accepted = outputs.remove(0);
                close(accepted.volume(), 100. * (i as f64 + 1.));
                accepted.topology().validate().unwrap();
            }
            Err(error) => {
                let message = error.to_string();
                assert!(message.contains("exceeds"), "{message}");
                close(accepted.volume(), 100. * i as f64);
                close(accepted.bounds()[1][2], i as f64);
                accepted.topology().validate().unwrap();
                return;
            }
        }
    }
    panic!("Axial event/section growth must stop at or before 64 levels");
}
