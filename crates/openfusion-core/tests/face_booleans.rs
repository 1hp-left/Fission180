#[path = "../examples/support/face_fixtures.rs"]
#[allow(dead_code)]
mod fixtures;
use fission180_core::{
    Operation,
    kernel::{
        Backend, Geometry, SolidKernel,
        native::{self, NativeSolid},
    },
};
use fixtures::*;
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-5, "{a} != {b}");
}
fn execute(case: &Case, backend: Backend) -> Vec<Geometry> {
    let mut result = vec![backend.extrude(&case.base).unwrap()];
    for (tool, op) in &case.tools {
        let b = backend.extrude(tool).unwrap();
        // A later operation addresses one material body, as in the document.
        let a = result.remove(0);
        let mut next = backend
            .boolean(&a, &b, *op, tool.feature_id + 100)
            .unwrap_or_else(|e| panic!("{} {backend:?}: {e}", case.name));
        next.extend(result);
        result = next;
    }
    result
}
fn check(case: &Case) {
    let rust = execute(case, Backend::Rust);
    let oracle = execute(case, Backend::Occt);
    assert_eq!(rust.len(), case.volumes.len(), "{}", case.name);
    assert_eq!(rust.len(), oracle.len(), "{} OCCT", case.name);
    let mut expected: Vec<_> = (0..case.volumes.len()).collect();
    expected.sort_by(|a, b| case.volumes[*a].total_cmp(&case.volumes[*b]));
    let mut actual: Vec<_> = rust.iter().collect();
    actual.sort_by(|a, b| a.volume().total_cmp(&b.volume()));
    for (a, i) in actual.into_iter().zip(expected) {
        close(a.volume(), case.volumes[i]);
        close(a.area(), case.areas[i]);
        for (a, b) in a
            .bounds()
            .iter()
            .flatten()
            .zip(case.bounds[i].iter().flatten())
        {
            close(*a, *b);
        }
        let Geometry::Rust(s) = a else { panic!() };
        s.topology().validate().unwrap();
        assert_eq!(s.topology().shell_count(), case.shells[i]);
        assert!(s.planar_region().is_none());
        assert!(s.occupied_sections().is_none());
        let peer = oracle
            .iter()
            .find(|o| (o.volume() - a.volume()).abs() < 1e-5)
            .unwrap();
        close(peer.area(), a.area());
        let analytic = (s.volume(), s.area(), s.bounds(), s.output_signature());
        for n in [8, 192] {
            let mesh = s.mesh(n).unwrap();
            let origin = s.bounds()[0];
            let mut volume = 0.;
            let mut area = 0.;
            for t in mesh.indices.chunks_exact(3) {
                let p: Vec<_> = t
                    .iter()
                    .map(|i| {
                        cadrum::DVec3::from_array(mesh.vertices[*i])
                            - cadrum::DVec3::from_array(origin)
                    })
                    .collect();
                volume += p[0].dot(p[1].cross(p[2])) / 6.;
                area += (p[1] - p[0]).cross(p[2] - p[0]).length() / 2.;
            }
            close(volume, s.volume());
            close(area, s.area());
            assert!(
                mesh.face_ids.iter().flatten().all(|id| s
                    .topology()
                    .faces
                    .iter()
                    .any(|f| f.id == *id))
            );
            assert_eq!(
                analytic,
                (s.volume(), s.area(), s.bounds(), s.output_signature())
            );
        }
    }
}
#[test]
fn required_planar_3d_cases_analytic_and_separately_constructed_occt() {
    for case in cases() {
        check(&case);
    }
}
#[test]
fn permutations_translated_frames_and_both_direction_conventions() {
    for permutation in 0..6 {
        for distance in [false, true] {
            for profile in [false, true] {
                for mut case in cases() {
                    reframe(&mut case, permutation, distance, profile);
                    check(&case);
                }
            }
        }
    }
}
#[test]
fn general_results_chain_translate_and_obey_conservation() {
    let case = cases()
        .into_iter()
        .find(|c| c.name == "triangles-join")
        .unwrap();
    let mut r = execute(&case, Backend::Rust);
    let Geometry::Rust(a) = r.remove(0) else {
        panic!()
    };
    let b = a.translated([3., 2., 1.], 82).unwrap();
    close(a.volume(), b.volume());
    close(a.area(), b.area());
    let sum = |v: &[NativeSolid]| v.iter().map(NativeSolid::volume).sum::<f64>();
    let cut = native::boolean(&a, &b, Operation::Cut, 90).unwrap();
    let intersection = native::boolean(&a, &b, Operation::Intersect, 91).unwrap();
    let join = native::boolean(&a, &b, Operation::Join, 92).unwrap();
    close(sum(&cut) + sum(&intersection), a.volume());
    close(sum(&join) + sum(&intersection), a.volume() + b.volume());
    for (op, expected) in [
        (Operation::Cut, 0.),
        (Operation::Join, a.volume()),
        (Operation::Intersect, a.volume()),
    ] {
        close(sum(&native::boolean(&a, &a, op, 93).unwrap()), expected);
    }
}
#[test]
fn replay_extrude_combine_parameters_stable_ancestry_history_and_failed_display() {
    use fission180_core::{Document, Model, Session};
    for case in cases() {
        for combine in [false, true] {
            let mut s = Session::new(document(&case, Backend::Rust, combine))
                .unwrap_or_else(|e| panic!("{} combine={combine}: {e}", case.name));
            close(
                s.model.info().iter().map(|b| b.volume).sum(),
                case.volumes.iter().sum(),
            );
            let reopened =
                Session::new(Document::from_json(&s.document.to_json().unwrap()).unwrap()).unwrap();
            assert_eq!(s.document, reopened.document);
            if !["wedge-cut", "cavity"].contains(&case.name) {
                continue;
            }
            let index = if case.name == "cavity" { 1 } else { 0 };
            let depth = if case.name == "cavity" { 3. } else { 9. };
            let ids = |m: &Model| {
                m.bodies
                    .iter()
                    .map(|b| {
                        let Geometry::Rust(s) = &b.solid else {
                            panic!()
                        };
                        let mut faces = s.topology().faces.iter().map(|f| f.id).collect::<Vec<_>>();
                        faces.sort();
                        let mut edges = s.topology().edges.iter().map(|e| e.id).collect::<Vec<_>>();
                        edges.sort();
                        (b.id, s.output_signature(), faces, edges)
                    })
                    .collect::<Vec<_>>()
            };
            let before = ids(&s.model);
            let old = s.document.clone();
            let mut edit = s.document.clone();
            edit.parameters
                .insert("depth".into(), format!("{depth} mm"));
            edit.extrusions[index].distance = depth;
            edit.extrusions[index].distance_expression = Some("depth".into());
            s.replace(edit).unwrap();
            assert_eq!(ids(&s.model), before);
            close(
                s.model.info()[0].volume,
                if case.name == "cavity" { 7400. } else { 5920. },
            );
            let accepted = s.document.clone();
            let fail = |_: &Model| {
                Err(fission180_core::Error::Invalid(
                    "injected planar mesh failure".into(),
                ))
            };
            let edit = accepted
                .with_parameter("depth".into(), format!("{} mm", depth - 0.25))
                .unwrap();
            assert!(s.replace_with_display(edit, fail).is_err());
            assert_eq!(s.document, accepted);
            let mut invalid = accepted.clone();
            invalid.extrusions[index].distance_expression = None;
            invalid.extrusions[index].distance = 0.;
            assert!(s.replace(invalid).is_err());
            assert_eq!(s.document, accepted);
            let mut changed = accepted.clone();
            changed.sketches[1].origin[0] += 100.;
            assert!(s.replace(changed).is_err());
            assert_eq!(s.document, accepted);
            s.undo().unwrap();
            assert_eq!(s.document, old);
            s.redo().unwrap();
            assert_eq!(s.document, accepted);
        }
    }
}
#[test]
fn regularized_shared_face_edge_point_and_coincident_contacts() {
    let a = NativeSolid::extrude(block([0.; 3], [10.; 3], 2)).unwrap();
    for (lo, hi, expected) in [
        ([10., 0., 0.], [20., 10., 10.], Some((2000., 1000., 1))),
        ([10., 2., 2.], [20., 8., 8.], Some((1360., 840., 1))),
        ([10., 10., 0.], [20., 20., 10.], None),
        ([10.; 3], [20.; 3], None),
        ([11.; 3], [21.; 3], Some((2000., 1200., 2))),
    ] {
        let mut tool = block(lo, hi, 5);
        // The same box expressed on another sketch plane must exercise face CSG.
        tool.plane = fission180_core::Plane::XZ;
        tool.origin = [0., hi[1], 0.];
        tool.outer = fission180_core::Profile::Rectangle {
            a: [lo[0], lo[2]],
            b: [hi[0], hi[2]],
        };
        tool.distance = hi[1] - lo[1];
        let b = NativeSolid::extrude(tool).unwrap();
        assert!(
            native::boolean(&a, &b, Operation::Intersect, 10)
                .unwrap()
                .is_empty()
        );
        let cut = native::boolean(&a, &b, Operation::Cut, 10).unwrap();
        close(cut[0].volume(), 1000.);
        let join = native::boolean(&a, &b, Operation::Join, 10);
        if let Some((volume, area, bodies)) = expected {
            let join = join.unwrap();
            assert_eq!(join.len(), bodies);
            close(join.iter().map(NativeSolid::volume).sum(), volume);
            close(join.iter().map(NativeSolid::area).sum(), area);
        } else {
            assert!(join.is_err());
        }
    }
    let mut tool = block([0.; 3], [10.; 3], 5);
    tool.plane = fission180_core::Plane::YZ;
    tool.distance = 10.;
    let b = NativeSolid::extrude(tool).unwrap();
    assert!(
        native::boolean(&a, &b, Operation::Cut, 10)
            .unwrap()
            .is_empty()
    );
    for op in [Operation::Intersect, Operation::Join] {
        let r = native::boolean(&a, &b, op, 10).unwrap();
        assert_eq!(r.len(), 1);
        close(r[0].volume(), 1000.);
        close(r[0].area(), 600.);
    }
}
#[test]
fn unresolved_curved_invalid_and_resource_cases_fail_without_modifying_inputs() {
    use fission180_core::{Plane, Profile};
    let a = NativeSolid::extrude(block([0.; 3], [10.; 3], 2)).unwrap();
    let signature = a.topology().faces.iter().map(|f| f.id).collect::<Vec<_>>();
    for gap in [1e-6, -1e-6] {
        let b = NativeSolid::extrude(prism(
            Profile::Rectangle {
                a: [10. + gap, 0.],
                b: [20., 10.],
            },
            Plane::XZ,
            [0., 10., 0.],
            10.,
            5,
        ))
        .unwrap();
        assert!(native::boolean(&a, &b, Operation::Join, 10).is_err());
    }
    let curved = NativeSolid::extrude(prism(
        Profile::Circle {
            center: [5.; 2],
            radius: 2.,
        },
        Plane::XZ,
        [0., 10., 0.],
        10.,
        5,
    ))
    .unwrap();
    assert!(native::boolean(&a, &curved, Operation::Cut, 10).is_err());
    let mut bad = a.topology().clone();
    bad.faces[0].loops[0][0].reversed = !bad.faces[0].loops[0][0].reversed;
    assert!(bad.validate().is_err());
    let circle = Profile::Polygon {
        points: (0..512)
            .map(|i| {
                let a = i as f64 * std::f64::consts::TAU / 512.;
                [100. * a.cos(), 100. * a.sin()]
            })
            .collect(),
    };
    let huge = NativeSolid::extrude(prism(circle.clone(), Plane::XY, [0.; 3], 10., 20)).unwrap();
    let tool = NativeSolid::extrude(prism(circle, Plane::XZ, [0.; 3], 10., 21)).unwrap();
    let error = native::boolean(&huge, &tool, Operation::Cut, 22)
        .unwrap_err()
        .to_string();
    assert!(error.contains("budget"), "{error}");
    let case = cases().into_iter().find(|c| c.name == "cavity").unwrap();
    let mut r = execute(&case, Backend::Rust);
    let Geometry::Rust(mut s) = r.remove(0) else {
        panic!()
    };
    for _ in 0..64 {
        s = s.translated([0.; 3], 99).unwrap();
    }
    let volume = s.volume();
    assert!(s.translated([0.; 3], 99).is_err());
    close(s.volume(), volume);
    let joined = native::boolean(&a, &s, Operation::Join, 100).unwrap();
    assert!(
        joined.iter().all(|s| s.translated([0.; 3], 101).is_err()),
        "CSG must not reset a tool's transform ancestry budget"
    );
    assert!(s.translated([f64::NAN, 0., 0.], 100).is_err());
    assert_eq!(
        signature,
        a.topology().faces.iter().map(|f| f.id).collect::<Vec<_>>()
    );
    close(a.volume(), 1000.);
}
#[test]
fn v5_different_axis_box_output_order_and_changed_output_count_require_repair() {
    use fission180_core::{Document, Plane, Profile, Session};
    let mut case = Case {
        name: "v5-cross-axis-split",
        base: block([0.; 3], [10.; 3], 2),
        tools: vec![(
            prism(
                Profile::Rectangle {
                    a: [4., -1.],
                    b: [6., 11.],
                },
                Plane::XZ,
                [0., 11., 0.],
                12.,
                5,
            ),
            Operation::Cut,
        )],
        volumes: vec![400.; 2],
        areas: vec![],
        shells: vec![1; 2],
        bounds: vec![],
    };
    for combine in [false, true] {
        let mut s = Session::new(document(&case, Backend::Rust, combine)).unwrap();
        let body = s.model.bodies.iter().find(|b| b.id == 3).unwrap();
        close(body.solid.bounds()[1][0], 4.);
        let saved = s.document.to_json().unwrap();
        assert!(saved.contains("\"version\": 5"));
        let reopened = Session::new(Document::from_json(&saved).unwrap()).unwrap();
        assert_eq!(s.document, reopened.document);
        let accepted = s.document.clone();
        let mut changed = accepted.clone();
        changed.sketches[1].origin[0] = 20.;
        assert!(s.replace(changed).is_err());
        assert_eq!(s.document, accepted);
    }
    case.name = "v5-disjoint-join";
    case.base = block([100., 0., 0.], [110., 10., 10.], 2);
    case.tools[0] = (
        prism(
            Profile::Rectangle {
                a: [0.; 2],
                b: [10.; 2],
            },
            Plane::XZ,
            [0., 10., 0.],
            10.,
            5,
        ),
        Operation::Join,
    );
    let s = Session::new(document(&case, Backend::Rust, true)).unwrap();
    close(
        s.model
            .bodies
            .iter()
            .find(|b| b.id == 3)
            .unwrap()
            .solid
            .bounds()[0][0],
        0.,
    );
}
#[test]
fn asymmetric_nonparallel_prisms_and_chained_results_match_independent_oracles() {
    use fission180_core::{Plane, Profile};
    // Retained deterministic perturbations exercise partial finite intersections,
    // rather than only axis permutations of the same arrangement.
    for i in 0..16 {
        let t = i as f64;
        let a = prism(
            Profile::Polygon {
                points: vec![[0., 0.], [17., 0.], [13., 9.], [6., 5.], [-2., 12.]],
            },
            Plane::XY,
            [11., -9., 2.],
            8.,
            2,
        );
        let b = prism(
            Profile::Polygon {
                points: vec![[3. + t * 0.13, -1.], [15. + t * 0.07, 1.], [9., 13.]],
            },
            Plane::XZ,
            [7., 7. + t * 0.11, -1.],
            22.,
            5,
        );
        for op in [Operation::Cut, Operation::Join, Operation::Intersect] {
            let left = Backend::Rust.extrude(&a).unwrap();
            let right = Backend::Rust.extrude(&b).unwrap();
            let oracle_left = Backend::Occt.extrude(&a).unwrap();
            let oracle_right = Backend::Occt.extrude(&b).unwrap();
            let result = Backend::Rust
                .boolean(&left, &right, op, 20)
                .unwrap_or_else(|e| panic!("i{i} {op:?}: {e}"));
            let oracle = Backend::Occt
                .boolean(&oracle_left, &oracle_right, op, 20)
                .unwrap();
            assert_eq!(result.len(), oracle.len());
            close(
                result.iter().map(Geometry::volume).sum(),
                oracle.iter().map(Geometry::volume).sum(),
            );
            close(
                result.iter().map(Geometry::area).sum(),
                oracle.iter().map(Geometry::area).sum(),
            );
            let tool = block([17., -8., 3.], [19., 9., 7.], 25);
            let tool_r = Backend::Rust.extrude(&tool).unwrap();
            let tool_o = Backend::Occt.extrude(&tool).unwrap();
            let r: Vec<_> = result
                .iter()
                .flat_map(|a| {
                    Backend::Rust
                        .boolean(a, &tool_r, Operation::Cut, 30)
                        .unwrap_or_else(|e| panic!("chained i{i} {op:?}: {e}"))
                })
                .collect();
            let o: Vec<_> = oracle
                .iter()
                .flat_map(|a| {
                    Backend::Occt
                        .boolean(a, &tool_o, Operation::Cut, 30)
                        .unwrap()
                })
                .collect();
            assert_eq!(r.len(), o.len());
            close(
                r.iter().map(Geometry::volume).sum(),
                o.iter().map(Geometry::volume).sum(),
            );
            close(
                r.iter().map(Geometry::area).sum(),
                o.iter().map(Geometry::area).sum(),
            );
        }
    }
}
