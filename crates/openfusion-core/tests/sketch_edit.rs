use fission180_core::{
    Document, Extrusion, Operation, Profile, Session, Sketch,
    expression::Parameters,
    kernel::Backend,
    persistence,
    sketch::{Constraint, Entity, SketchGraph},
};
fn chain() -> Sketch {
    let mut s = Sketch {
        id: 1,
        name: "Line rectangle".into(),
        ..Default::default()
    };
    for (a, b) in [
        ([0., 0.], [40., 0.]),
        ([40., 30.], [40., 0.]),
        ([40., 30.], [0., 30.]),
        ([0., 0.], [0., 30.]),
    ] {
        s = s.with_line(a, b, &Parameters::new()).unwrap();
    }
    s
}
fn solid(s: Sketch, backend: Backend) -> Document {
    Document {
        backend,
        sketches: vec![s],
        extrusions: vec![Extrusion {
            id: 2,
            name: "Closed chain extrusion".into(),
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
#[test]
fn reversed_line_chain_closes_into_stable_region_dimensions_and_real_solids() {
    let s = chain();
    assert_eq!(s.profiles.len(), 1);
    assert_eq!(s.profile_ids, vec![1]);
    let first = s.graph.as_ref().unwrap().profile_entities[&1][0];
    let s = s
        .with_entity_dimension(
            first,
            "2 * width".into(),
            &Parameters::from([("width".into(), "40 mm".into())]),
        )
        .unwrap();
    assert_eq!(s.profile_ids, vec![1]);
    for backend in [Backend::Occt, Backend::Rust] {
        let mut d = solid(s.clone(), backend);
        d.parameters.insert("width".into(), "40 mm".into());
        let accepted = Session::new(d).unwrap();
        assert!(
            (accepted.model.info()[0].volume - 12000.).abs() < 1e-4,
            "{:?}: {} {:?}",
            backend,
            accepted.model.info()[0].volume,
            accepted.document.sketches[0].profiles
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chain.ofrust");
        persistence::save(&accepted.document, &path).unwrap();
        let opened =
            Session::new(Document::from_json(&std::fs::read_to_string(path).unwrap()).unwrap())
                .unwrap();
        assert_eq!(opened.document, accepted.document);
        assert!((opened.model.info()[0].volume - 12000.).abs() < 1e-4);
    }
}
#[test]
fn construction_delete_reclose_and_consumed_region_rollback() {
    let original = chain();
    let graph = original.graph.as_ref().unwrap();
    let edge = graph.profile_entities[&1][0];
    let Entity::Line { a, b } = graph.entities[&edge] else {
        panic!()
    };
    let start = graph.points[&a];
    let end = graph.points[&b];
    let construction = original
        .with_construction(&[edge], true, &Parameters::new())
        .unwrap();
    assert!(construction.regions().unwrap().is_empty());
    let restored = construction
        .with_construction(&[edge], false, &Parameters::new())
        .unwrap();
    assert_eq!(restored.profile_ids, original.profile_ids);
    let open = original
        .without_entities(&[edge], &Parameters::new())
        .unwrap();
    assert!(open.regions().unwrap().is_empty());
    assert_eq!(open.graph.as_ref().unwrap().entities.len(), 3);
    let reclosed = open.with_line(start, end, &Parameters::new()).unwrap();
    assert_eq!(reclosed.profile_ids, vec![2]);
    let mut accepted = Session::new(solid(original, Backend::Rust)).unwrap();
    let before = accepted.document.clone();
    let mut broken = before.clone();
    broken.sketches[0] = open;
    assert!(accepted.replace(broken).is_err());
    assert_eq!(accepted.document, before);
    assert!((accepted.model.info()[0].volume - 6000.).abs() < 1e-5);
}
#[test]
fn dimension_constraint_deletion_preserves_profile_and_graph_dependencies() {
    let s = chain();
    let edge = s.graph.as_ref().unwrap().profile_entities[&1][0];
    let d = s
        .with_entity_dimension(edge, "80 mm".into(), &Parameters::new())
        .unwrap();
    let id = d.graph.as_ref().unwrap().dimension_ids[&format!("entity.{edge}")];
    let removed = d.without_constraint(id, &Parameters::new()).unwrap();
    assert_eq!(removed.profiles, d.profiles);
    assert_eq!(removed.profile_ids, d.profile_ids);
    assert!(
        !removed
            .graph
            .as_ref()
            .unwrap()
            .constraints
            .contains_key(&id)
    );
    assert!(removed.graph.as_ref().unwrap().dimension_ids.is_empty());
    removed.graph.as_ref().unwrap().validate().unwrap();
}
#[test]
fn point_drag_respects_constraints_and_conflicts_preserve_the_input() {
    let s = chain();
    let g = s.graph.as_ref().unwrap();
    let point = *g.points.keys().next().unwrap();
    let moved = s
        .with_dragged_point(point, [-10., -5.], &Parameters::new())
        .unwrap();
    let p = moved.graph.as_ref().unwrap().points[&point];
    assert!((p[0] + 10.).abs() < 1e-6 && (p[1] + 5.).abs() < 1e-6);
    let fixed = moved
        .with_constraints(vec![Constraint::Fixed { point, at: p }], &Parameters::new())
        .unwrap();
    assert!(
        fixed
            .with_dragged_point(point, [30., 30.], &Parameters::new())
            .is_err()
    );
    assert_eq!(fixed.graph.as_ref().unwrap().points[&point], p);
    assert_eq!(fixed.profile_ids, vec![1]);
}
#[test]
fn midpoint_concentric_equal_circles_and_missing_dependencies_are_checked() {
    let mut g = SketchGraph::default();
    let a = g.point([0., 0.]).unwrap();
    let b = g.point([20., 0.]).unwrap();
    let m = g.point([9., 1.]).unwrap();
    let line = g.entity(Entity::Line { a, b }).unwrap();
    g.constrain(Constraint::Fixed {
        point: a,
        at: [0., 0.],
    })
    .unwrap();
    g.constrain(Constraint::Fixed {
        point: b,
        at: [20., 0.],
    })
    .unwrap();
    g.constrain(Constraint::Midpoint { point: m, line })
        .unwrap();
    let (solved, _) = g.solve(&Parameters::new()).unwrap();
    assert!((solved.points[&m][0] - 10.).abs() < 1e-6 && solved.points[&m][1].abs() < 1e-6);
    let mut s = Sketch {
        id: 1,
        ..Default::default()
    };
    s.add_profile(Profile::Circle {
        center: [0., 0.],
        radius: 5.,
    })
    .unwrap();
    s.add_profile(Profile::Circle {
        center: [30., 0.],
        radius: 7.,
    })
    .unwrap();
    let g = s.graph.as_ref().unwrap();
    let c1 = g.profile_entities[&1][0];
    let c2 = g.profile_entities[&2][0];
    let equal = s
        .with_constraints(vec![Constraint::Equal { a: c1, b: c2 }], &Parameters::new())
        .unwrap();
    let [
        Profile::Circle { radius: a, .. },
        Profile::Circle { radius: b, .. },
    ] = equal.profiles.as_slice()
    else {
        panic!()
    };
    assert!((a - b).abs() < 1e-6);
    let concentric = s
        .with_constraints(
            vec![Constraint::Concentric { a: c1, b: c2 }],
            &Parameters::new(),
        )
        .unwrap();
    let [
        Profile::Circle { center: a, .. },
        Profile::Circle { center: b, .. },
    ] = concentric.profiles.as_slice()
    else {
        panic!()
    };
    assert!((a[0] - b[0]).abs() < 1e-6);
    let mut malformed = g.clone();
    malformed
        .constrain(Constraint::Equal { a: c1, b: 9999 })
        .unwrap();
    assert!(malformed.solve(&Parameters::new()).is_err());
}
