#[path = "../examples/support/prism_fixtures.rs"]
#[allow(dead_code)]
mod fixtures;
use fission180_core::{
    Document, Model, Operation, Plane, Profile, Session, Sketch,
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
fn target() -> fission180_core::kernel::ExtrudeInput {
    let mut a = input(rect(-30., -30., 60., 60.), vec![], 2);
    a.distance = 10.;
    a
}
fn tool(x: f64, depth: f64, feature: u64) -> fission180_core::kernel::ExtrudeInput {
    let mut b = input(disk(x, 10.), vec![], feature);
    b.origin[2] = 10.;
    b.distance = -depth;
    b
}
fn cut(a: &NativeSolid, b: &fission180_core::kernel::ExtrudeInput, feature: u64) -> NativeSolid {
    native::boolean(
        a,
        &NativeSolid::extrude(b.clone()).unwrap(),
        Operation::Cut,
        feature,
    )
    .unwrap()
    .remove(0)
}
fn mesh_volume(s: &NativeSolid, resolution: usize) -> f64 {
    let m = s.mesh(resolution).unwrap();
    let o = cadrum::DVec3::from_array(s.bounds()[0]);
    for id in m.face_ids.iter().flatten() {
        assert!(
            s.topology().faces.iter().any(|f| f.id == *id),
            "face {id:?}"
        );
    }
    for id in &m.edge_ids {
        assert!(
            s.topology().edges.iter().any(|e| e.id == *id),
            "edge {id:?}"
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
fn overlapping_equal_and_unequal_depths_account_for_overlap_once_and_oracle_step() {
    let lens = 200. * PI / 3. - 50. * 3f64.sqrt();
    for (da, db) in [(3f64, 3f64), (3., 5.), (5., 3.)] {
        let a = target();
        let b = tool(0., da, 5);
        let c = tool(10., db, 8);
        let base = NativeSolid::extrude(a.clone()).unwrap();
        let first = cut(&base, &b, 5);
        let result = cut(&first, &c, 8);
        let expected = 36000. - 100. * PI * (da + db) + lens * da.min(db);
        let area = 9600. + 20. * PI * (da - db).abs() + 80. * PI / 3. * da.min(db);
        close(result.volume(), expected);
        close(result.area(), area);
        result.topology().validate().unwrap();
        let sections = result.occupied_sections().unwrap();
        assert_eq!(sections.len(), if da == db { 2 } else { 3 });
        close(
            sections.iter().map(|(z, r)| (z[1] - z[0]) * r.area()).sum(),
            expected,
        );
        let analytic = (result.volume(), result.area(), result.bounds());
        for resolution in [24, 192] {
            let v = mesh_volume(&result, resolution);
            assert!((v - expected).abs() < 150., "{v} != {expected}");
            assert_eq!(analytic, (result.volume(), result.area(), result.bounds()));
        }
        let occt = Backend::Occt;
        let mut oracle = occt.extrude(&a).unwrap();
        for t in [&b, &c] {
            let cutter = occt.extrude(t).unwrap();
            oracle = occt
                .boolean(&oracle, &cutter, Operation::Cut, t.feature_id)
                .unwrap()
                .remove(0);
        }
        close(oracle.volume(), expected);
        close(oracle.area(), area);
        let Geometry::Occt(oracle) = oracle else {
            panic!()
        };
        let mut bytes = vec![];
        cadrum::Solid::write_step([&oracle], &mut bytes).unwrap();
        close(Model::step_volume(&bytes).unwrap(), expected);
        let moved = result.translated([31., -7., 9.], 12).unwrap();
        close(moved.volume(), expected);
        close(moved.area(), area);
        mesh_volume(&moved, 48);
    }
}
#[test]
fn repeated_contained_annular_and_disjoint_through_cuts_preserve_stepped_sections() {
    let base = NativeSolid::extrude(target()).unwrap();
    let b = tool(0., 3., 5);
    let first = cut(&base, &b, 5);
    let repeated = cut(&first, &b, 8);
    close(first.volume(), repeated.volume());
    close(first.area(), repeated.area());
    repeated.topology().validate().unwrap();
    let deeper = cut(&repeated, &tool(0., 5., 9), 9);
    close(deeper.volume(), 36000. - 500. * PI);
    close(deeper.area(), 9600. + 100. * PI);
    let mut through = input(
        Profile::Circle {
            center: [-20., 0.],
            radius: 2.,
        },
        vec![],
        12,
    );
    through.distance = 10.;
    let through = cut(&deeper, &through, 12);
    close(through.volume(), 36000. - 540. * PI);
    close(through.area(), 9600. + 100. * PI + 40. * PI - 8. * PI);
    through.topology().validate().unwrap();
    mesh_volume(&through, 64);
    let mut annular = tool(0., 3., 15);
    annular.outer = disk(0., 8.);
    annular.holes = vec![disk(0., 3.)];
    annular.loop_ids.push(2);
    let annular = cut(&base, &annular, 15);
    close(annular.volume(), 36000. - 165. * PI);
    close(annular.area(), 9600. + 66. * PI);
    mesh_volume(&annular, 64);
}
#[test]
fn both_end_caps_principal_planes_negative_extrusion_and_translated_tool_frames() {
    let lens = 200. * PI / 3. - 50. * 3f64.sqrt();
    for plane in [Plane::XY, Plane::XZ, Plane::YZ] {
        for negative in [false, true] {
            for high in [false, true] {
                let mut a = target();
                a.plane = plane;
                a.origin = [13., -17., 9.];
                a.distance = if negative { -10. } else { 10. };
                let normal = plane.normal().to_array();
                let t = if negative { [-10., 0.] } else { [0., 10.] };
                let opening = if high { t[1] } else { t[0] };
                let mut solid = NativeSolid::extrude(a.clone()).unwrap();
                for (x, depth, id) in [(0., 3., 5), (10., 5., 8)] {
                    let mut b = input(disk(0., 10.), vec![], id);
                    b.plane = plane;
                    let inplane = plane.point([x, 0.], [0.; 3]).to_array();
                    b.origin =
                        std::array::from_fn(|i| a.origin[i] + inplane[i] + normal[i] * opening);
                    b.distance = if high { -depth } else { depth };
                    solid = cut(&solid, &b, id);
                }
                close(solid.volume(), 36000. - 800. * PI + 3. * lens);
                close(solid.area(), 9600. + 120. * PI);
                solid.topology().validate().unwrap();
                mesh_volume(&solid, 32);
            }
        }
    }
}
#[test]
fn replay_depth_edits_save_history_and_invalid_retention() {
    let a = target();
    let b = tool(0., 3., 5);
    let c = tool(10., 5., 8);
    let mut d = document(&a, &b, Operation::Cut, Backend::Rust, false);
    d.sketches.push(Sketch {
        id: 7,
        origin: c.origin,
        profiles: vec![c.outer.clone()],
        profile_ids: vec![1],
        next_profile_id: 2,
        ..Default::default()
    });
    d.extrusions
        .push(extrusion(8, 7, 3, c.distance, Operation::Cut));
    let mut session = Session::new(d).unwrap();
    let accepted_volume = session.model.info()[0].volume;
    let ids = |model: &Model| {
        if let Geometry::Rust(s) = &model.bodies[0].solid {
            (
                s.topology().edges.iter().map(|e| e.id).collect::<Vec<_>>(),
                s.topology().faces.iter().map(|f| f.id).collect::<Vec<_>>(),
            )
        } else {
            panic!()
        }
    };
    let topology_ids = ids(&session.model);
    let mut edit = session.document.clone();
    edit.parameters.insert("depth".into(), "4 mm".into());
    edit.extrusions[2].distance_expression = Some("-depth".into());
    edit.extrusions[2].distance = -4.;
    session.replace(edit).unwrap();
    assert_eq!(topology_ids, ids(&session.model));
    let edited_volume = session.model.info()[0].volume;
    assert!(edited_volume > accepted_volume);
    session.undo().unwrap();
    close(session.model.info()[0].volume, accepted_volume);
    session.redo().unwrap();
    close(session.model.info()[0].volume, edited_volume);
    let saved = session.document.to_json().unwrap();
    let reopened = Session::new(Document::from_json(&saved).unwrap()).unwrap();
    close(reopened.model.info()[0].volume, edited_volume);
    assert_eq!(session.document, reopened.document);
    let accepted = session.document.clone();
    let mut invalid = accepted.clone();
    invalid.sketches[2].origin[2] = 0.;
    invalid.extrusions[2].distance = 4.;
    invalid.extrusions[2].distance_expression = None;
    assert!(session.replace(invalid).is_err());
    assert_eq!(session.document, accepted);
    close(session.model.info()[0].volume, edited_volume);
    let mut invalid = accepted.clone();
    invalid.sketches[2].origin[0] = 29.;
    assert!(session.replace(invalid).is_err());
    assert_eq!(session.document, accepted);
}

#[test]
fn mixed_line_arc_pockets_with_an_existing_through_hole() {
    let mut a = target();
    a.holes = vec![Profile::Circle {
        center: [-20., 0.],
        radius: 2.,
    }];
    a.loop_ids.push(2);
    let mut b = tool(0., 3., 5);
    b.outer = rect(-10., -5., 20., 10.);
    let mut c = tool(8., 5., 8);
    c.outer = disk(8., 4.);
    let result = cut(
        &cut(&NativeSolid::extrude(a.clone()).unwrap(), &b, 5),
        &c,
        8,
    );
    let overlap = 32. * PI / 3. + 4. * 3f64.sqrt();
    let expected = 36000. - 40. * PI - 600. - 80. * PI + 3. * overlap;
    close(result.volume(), expected);
    close(result.area(), 9780. - 12. * 3f64.sqrt() + 56. * PI);
    let mut oracle = Backend::Occt.extrude(&a).unwrap();
    for t in [&b, &c] {
        let cutter = Backend::Occt.extrude(t).unwrap();
        oracle = Backend::Occt
            .boolean(&oracle, &cutter, Operation::Cut, t.feature_id)
            .unwrap()
            .remove(0);
    }
    close(oracle.volume(), result.volume());
    close(oracle.area(), result.area());
    result.topology().validate().unwrap();
    mesh_volume(&result, 96);
    let mut cover = input(rect(-40., -40., 80., 80.), vec![], 15);
    cover.distance = 10.;
    assert!(
        native::boolean(
            &result,
            &NativeSolid::extrude(cover).unwrap(),
            Operation::Cut,
            15
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn sub_resolution_floors_tangent_footprints_and_nonparallel_tools_fail_explicitly() {
    let a = NativeSolid::extrude(target()).unwrap();
    let first = cut(&a, &tool(0., 3., 5), 5);
    for (x, depth) in [(10., 3. + 1e-6), (20., 3.), (10., 3. + 1e-8)] {
        let result = native::boolean(
            &first,
            &NativeSolid::extrude(tool(x, depth, 8)).unwrap(),
            Operation::Cut,
            8,
        );
        if depth == 3. + 1e-8 {
            assert!(result.is_ok());
        } else {
            assert!(result.is_err(), "{x} {depth}");
        }
    }
    let mut nonparallel = tool(0., 5., 8);
    nonparallel.plane = Plane::XZ;
    assert!(
        native::boolean(
            &first,
            &NativeSolid::extrude(nonparallel).unwrap(),
            Operation::Cut,
            8
        )
        .is_err()
    );
}
