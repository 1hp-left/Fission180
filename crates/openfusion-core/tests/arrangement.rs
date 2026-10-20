use fission180_core::{
    Document, Extrusion, Model, Operation, Profile, Session, Sketch, kernel::Backend,
    sketch::Entity,
};
fn rectangle() -> Sketch {
    let mut s = Sketch {
        id: 1,
        name: "Arrangement".into(),
        ..Default::default()
    };
    s.add_profile(Profile::Rectangle {
        a: [0., 0.],
        b: [40., 30.],
    })
    .unwrap();
    s
}
fn volume(s: Sketch, id: u64, backend: Backend) -> (Document, f64) {
    let d = Document {
        backend,
        sketches: vec![s],
        extrusions: vec![Extrusion {
            id: 2,
            name: "Cell extrusion".into(),
            sketch_id: 1,
            profile: 0,
            region_id: Some(id),
            distance: 5.,
            operation: Operation::NewBody,
            body_id: 3,
            distance_expression: None,
            result_body_ids: vec![],
            boundary_dependencies: None,
            options: Default::default(),
        }],
        ..Default::default()
    };
    let session = Session::new(d).unwrap();
    let v = session.model.info()[0].volume;
    (session.document, v)
}
#[test]
fn crossing_dividers_create_four_exact_cells_on_both_backends_and_round_trip() {
    let s = rectangle()
        .with_line([20., -10.], [20., 40.], &Default::default())
        .unwrap()
        .with_line([-10., 15.], [50., 15.], &Default::default())
        .unwrap();
    let regions = s.regions().unwrap();
    assert_eq!(regions.len(), 4);
    for backend in [Backend::Occt, Backend::Rust] {
        for region in &regions {
            let (d, v) = volume(s.clone(), region.id, backend);
            assert!((v - 1500.).abs() < 1e-5);
            let loaded = Document::from_json(&d.to_json().unwrap()).unwrap();
            assert_eq!(loaded, d);
            assert!((Model::build(&loaded).unwrap().info()[0].volume - v).abs() < 1e-5);
            if backend == Backend::Occt {
                let step = Model::build(&loaded).unwrap().step_bytes().unwrap();
                assert!((Model::step_volume(&step).unwrap() - v).abs() < 1e-5);
            }
        }
    }
}
#[test]
fn branches_do_not_change_identity_but_partition_and_merge_invalidate_consumers() {
    let s = rectangle();
    let original = s.profile_ids[0];
    let branch = s
        .with_line([20., 0.], [20., -10.], &Default::default())
        .unwrap();
    assert_eq!(branch.profile_ids, vec![original]);
    let inside = branch
        .with_line([20., 0.], [20., 10.], &Default::default())
        .unwrap();
    assert_eq!(inside.profile_ids, vec![original]);
    let split = inside
        .with_line([20., 10.], [20., 30.], &Default::default())
        .unwrap();
    assert_eq!(split.regions().unwrap().len(), 2);
    assert!(!split.profile_ids.contains(&original));
    let (d, _) = volume(s, original, Backend::Rust);
    let mut accepted = Session::new(d).unwrap();
    let before = accepted.document.clone();
    let mut edited = before.clone();
    edited.sketches[0] = split.clone();
    assert!(accepted.replace(edited).is_err());
    assert_eq!(accepted.document, before);
    let child = split.profile_ids[0];
    let (d, _) = volume(split.clone(), child, Backend::Rust);
    let divider = *split
        .graph
        .as_ref()
        .unwrap()
        .entities
        .keys()
        .last()
        .unwrap();
    let merged = split
        .without_entities(&[divider], &Default::default())
        .unwrap();
    assert_eq!(merged.profile_ids, vec![original]);
    let mut accepted = Session::new(d).unwrap();
    let before = accepted.document.clone();
    let mut edited = before.clone();
    edited.sketches[0] = merged;
    assert!(accepted.replace(edited).is_err());
    assert_eq!(accepted.document, before);
}
#[test]
fn source_cycles_stay_stable_through_motion_construction_and_nested_holes() {
    let s = rectangle()
        .with_line([20., 0.], [20., 30.], &Default::default())
        .unwrap();
    let ids = s.profile_ids.clone();
    let mut g = s.graph_for_edit().unwrap();
    let divider = *g.entities.keys().last().unwrap();
    let Entity::Line { a, b } = g.entities[&divider] else {
        panic!()
    };
    g.points.get_mut(&a).unwrap()[0] = 25.;
    g.points.get_mut(&b).unwrap()[0] = 25.;
    let moved = s.with_graph(g, &Default::default()).unwrap();
    assert_eq!(moved.profile_ids, ids);
    let mut volumes: Vec<_> = ids
        .iter()
        .map(|id| volume(moved.clone(), *id, Backend::Rust).1)
        .collect();
    volumes.sort_by(f64::total_cmp);
    assert_eq!(volumes, vec![2250., 3750.]);
    let construction = moved
        .with_construction(&[divider], true, &Default::default())
        .unwrap();
    let restored = construction
        .with_construction(&[divider], false, &Default::default())
        .unwrap();
    assert_eq!(restored.profile_ids, ids);
    let mut nested = rectangle();
    nested
        .add_profile(Profile::Rectangle {
            a: [5., 5.],
            b: [10., 10.],
        })
        .unwrap();
    for backend in [Backend::Occt, Backend::Rust] {
        assert!((volume(nested.clone(), 1, backend).1 - 5875.).abs() < 1e-5);
    }
}
#[test]
fn shared_vertex_loops_and_bridge_between_loops_are_disjoint_regions() {
    let mut s = rectangle();
    s.add_profile(Profile::Rectangle {
        a: [40., 30.],
        b: [50., 40.],
    })
    .unwrap();
    assert_eq!(s.regions().unwrap().len(), 2);
    let mut separated = rectangle();
    separated
        .add_profile(Profile::Rectangle {
            a: [50., 0.],
            b: [60., 10.],
        })
        .unwrap();
    let ids = separated.profile_ids.clone();
    let connected = separated
        .with_line([40., 5.], [50., 5.], &Default::default())
        .unwrap();
    assert_eq!(connected.profile_ids, ids);
}
#[test]
fn duplicate_overlap_and_malformed_persisted_provenance_reject_without_mutation() {
    let s = rectangle();
    let before = s.clone();
    assert!(
        s.with_line([5., 0.], [35., 0.], &Default::default())
            .is_err()
    );
    assert_eq!(s, before);
    let mut d = Document {
        sketches: vec![s],
        ..Default::default()
    };
    d.sketches[0]
        .graph
        .as_mut()
        .unwrap()
        .region_cycles
        .insert(99, vec![999, 998, 997]);
    assert!(d.to_json().is_err());
}
