#[path = "../examples/support/prism_fixtures.rs"]
mod fixtures;
use fission180_core::{
    Document, Model, Operation, Plane, Session,
    kernel::{
        Backend, Geometry, SolidKernel,
        native::{self, NativeSolid},
    },
};
use fixtures::*;
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-5, "{a} != {b}");
}
fn mesh_volume(s: &NativeSolid, resolution: usize) -> f64 {
    let m = s.mesh(resolution).unwrap();
    let o = cadrum::DVec3::from_array(s.bounds()[0]);
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
            let p: Vec<_> = tri
                .iter()
                .map(|i| cadrum::DVec3::from_array(m.vertices[*i]) - o)
                .collect();
            p[0].dot(p[1].cross(p[2])) / 6.
        })
        .sum()
}
#[test]
fn required_analytic_prisms_independent_occt_oracles_and_step() {
    for c in cases() {
        let a = NativeSolid::extrude(c.a.clone()).unwrap();
        let b = NativeSolid::extrude(c.b.clone()).unwrap();
        let r = native::boolean(&a, &b, c.op, 7).unwrap_or_else(|e| panic!("{}: {e}", c.name));
        assert_eq!(r.len(), c.areas.len(), "{}", c.name);
        let area: f64 = c.areas.iter().sum();
        close(r.iter().map(NativeSolid::volume).sum(), area * 5.);
        close(
            r.iter().map(NativeSolid::area).sum(),
            2. * area + c.perimeter * 5.,
        );
        let mut measured: Vec<_> = r.iter().map(NativeSolid::volume).collect();
        measured.sort_by(f64::total_cmp);
        let mut expected: Vec<_> = c.areas.iter().map(|a| a * 5.).collect();
        expected.sort_by(f64::total_cmp);
        for (a, b) in measured.iter().zip(expected) {
            close(*a, b);
        }
        for s in &r {
            s.topology().validate().unwrap();
            assert_eq!(s.provenance.last().unwrap().operation, c.op);
            let analytic = (s.volume(), s.area(), s.bounds());
            for resolution in [24, 192] {
                let v = mesh_volume(s, resolution);
                assert!(
                    (v - s.volume()).abs() < s.volume() * 0.035,
                    "{} mesh {v} != {}",
                    c.name,
                    s.volume()
                );
                assert_eq!(analytic, (s.volume(), s.area(), s.bounds()));
            }
            let moved = s.translated([100., -20., 7.], 8).unwrap();
            close(moved.volume(), s.volume());
            mesh_volume(&moved, 32);
        }
        // Independent oracle operands are built from the input specification.
        let a = Backend::Occt.extrude(&c.a).unwrap();
        let b = Backend::Occt.extrude(&c.b).unwrap();
        let oracle = Backend::Occt.boolean(&a, &b, c.op, 7).unwrap();
        assert_eq!(r.len(), oracle.len(), "{} OCCT body count", c.name);
        close(oracle.iter().map(Geometry::volume).sum(), area * 5.);
        close(
            oracle.iter().map(Geometry::area).sum(),
            2. * area + c.perimeter * 5.,
        );
        if !oracle.is_empty() {
            let solids: Vec<_> = oracle
                .iter()
                .map(|s| {
                    if let Geometry::Occt(s) = s {
                        s
                    } else {
                        panic!()
                    }
                })
                .collect();
            let mut bytes = vec![];
            cadrum::Solid::write_step(solids, &mut bytes).unwrap();
            close(Model::step_volume(&bytes).unwrap(), area * 5.);
        }
    }
}
#[test]
fn all_principal_frames_opposite_directions_and_analytic_bounds() {
    for plane in [Plane::XY, Plane::XZ, Plane::YZ] {
        for negative in [false, true] {
            for backwards in [false, true] {
                let mut a = input(disk(0., 10.), vec![], 2);
                a.plane = plane;
                a.origin = [13., -17., 9.];
                a.distance = if negative { -5. } else { 5. };
                let mut b = input(disk(0., 10.), vec![], 5);
                b.plane = plane;
                let offset = plane.point([10., 0.], [0.; 3]).to_array();
                let normal = plane.normal().to_array();
                b.origin = std::array::from_fn(|i| {
                    a.origin[i]
                        + offset[i]
                        + if backwards {
                            normal[i] * a.distance
                        } else {
                            0.
                        }
                });
                b.distance = if backwards { -a.distance } else { a.distance };
                for op in [Operation::Cut, Operation::Join, Operation::Intersect] {
                    let r = native::boolean(
                        &NativeSolid::extrude(a.clone()).unwrap(),
                        &NativeSolid::extrude(b.clone()).unwrap(),
                        op,
                        7,
                    )
                    .unwrap();
                    let oa = Backend::Occt.extrude(&a).unwrap();
                    let ob = Backend::Occt.extrude(&b).unwrap();
                    let oracle = Backend::Occt.boolean(&oa, &ob, op, 7).unwrap();
                    close(r[0].volume(), oracle[0].volume());
                    close(r[0].area(), oracle[0].area());
                    for (a, b) in r[0]
                        .bounds()
                        .iter()
                        .flatten()
                        .zip(oracle[0].bounds().iter().flatten())
                    {
                        assert!((a - b).abs() < 2e-5, "bounds {a} != {b}");
                    }
                    r[0].topology().validate().unwrap();
                    mesh_volume(&r[0], 48);
                }
            }
        }
    }
}
#[test]
fn replay_extrusion_combine_expressions_save_history_split_and_rollback() {
    for combine in [false, true] {
        let c = cases()
            .into_iter()
            .find(|c| c.name == "strip-split")
            .unwrap();
        let mut session = Session::new(document(&c.a, &c.b, c.op, Backend::Rust, combine)).unwrap();
        assert_eq!(session.model.bodies.len(), 2);
        let ids: Vec<_> = session.model.bodies.iter().map(|b| b.id).collect();
        assert_ne!(ids[0], ids[1]);
        assert_eq!(session.document.version, 5);
        let saved = session.document.to_json().unwrap();
        let reopened = Session::new(Document::from_json(&saved).unwrap()).unwrap();
        assert_eq!(reopened.document, session.document);
        assert_eq!(
            ids,
            reopened
                .model
                .bodies
                .iter()
                .map(|b| b.id)
                .collect::<Vec<_>>()
        );
        let signatures: Vec<_> = session
            .model
            .bodies
            .iter()
            .map(|b| {
                if let Geometry::Rust(s) = &b.solid {
                    s.output_signature()
                } else {
                    panic!()
                }
            })
            .collect();
        let mut edit = session.document.clone();
        edit.parameters.insert("length".into(), "7 mm".into());
        for e in &mut edit.extrusions {
            e.distance_expression = Some("length".into());
            e.distance = 7.;
        }
        session.replace(edit).unwrap();
        close(
            session.model.info().iter().map(|b| b.volume).sum(),
            760. * 7.,
        );
        assert_eq!(
            signatures,
            session
                .model
                .bodies
                .iter()
                .map(|b| if let Geometry::Rust(s) = &b.solid {
                    s.output_signature()
                } else {
                    panic!()
                })
                .collect::<Vec<_>>()
        );
        session.undo().unwrap();
        close(session.model.info().iter().map(|b| b.volume).sum(), 3800.);
        session.redo().unwrap();
        let accepted = session.document.clone();
        let info = session.model.info();
        let mut bad = accepted.clone();
        bad.sketches[1].origin[0] = 100.;
        assert!(session.replace(bad).is_err());
        assert_eq!(session.document, accepted);
        close(session.model.info()[0].volume, info[0].volume);
        assert!(session.model.step_bytes().is_err());
    }
    assert_eq!(Document::default().backend, Backend::Occt);
}
#[test]
fn tangent_solid_contacts_empty_intersection_and_pocket_safety() {
    let a = NativeSolid::extrude(input(disk(0., 10.), vec![], 2)).unwrap();
    let b = NativeSolid::extrude(input(disk(20., 10.), vec![], 5)).unwrap();
    assert!(native::boolean(&a, &b, Operation::Join, 7).is_err());
    assert!(
        native::boolean(&a, &b, Operation::Intersect, 7)
            .unwrap()
            .is_empty()
    );
    let a = NativeSolid::extrude(input(rect(0., 0., 40., 20.), vec![], 2)).unwrap();
    let b = NativeSolid::extrude(input(rect(40., 0., 10., 20.), vec![], 5)).unwrap();
    let joined = native::boolean(&a, &b, Operation::Join, 7).unwrap();
    close(joined[0].volume(), 5000.);
    joined[0].topology().validate().unwrap();
    assert!(
        native::boolean(&a, &b, Operation::Intersect, 7)
            .unwrap()
            .is_empty()
    );
    let mut pocket = input(
        fission180_core::Profile::Circle {
            center: [10., 10.],
            radius: 2.,
        },
        vec![],
        5,
    );
    pocket.origin[2] = 3.;
    pocket.distance = 4.;
    let p = native::boolean(
        &a,
        &NativeSolid::extrude(pocket).unwrap(),
        Operation::Cut,
        7,
    )
    .unwrap()
    .remove(0);
    assert!(native::boolean(&p, &b, Operation::Join, 8).is_err());
    let mut exhausted = input(disk(0., 10.), vec![], 2);
    exhausted.loop_ids = vec![u64::MAX];
    assert!(NativeSolid::extrude(exhausted).is_err());
}
