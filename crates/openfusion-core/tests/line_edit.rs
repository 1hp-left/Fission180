use fission180_core::{
    Document, Sketch,
    sketch::{Constraint, Entity},
};
fn lines() -> Sketch {
    let mut s = Sketch {
        id: 1,
        ..Default::default()
    };
    for (a, b) in [
        ([0., 0.], [10., 0.]),
        ([3., -5.], [3., 5.]),
        ([7., -5.], [7., 5.]),
    ] {
        s = s.with_line(a, b, &Default::default()).unwrap();
    }
    s
}
#[test]
fn trim_middle_retires_source_and_dimension_and_keeps_intersection_dependencies() {
    let s = lines();
    let source = *s.graph.as_ref().unwrap().entities.keys().next().unwrap();
    let s = s
        .with_entity_dimension(source, "10 mm".into(), &Default::default())
        .unwrap();
    let before = s.clone();
    let trimmed = s
        .with_trimmed_line(source, [5., 0.], &Default::default())
        .unwrap();
    assert_eq!(s, before);
    let g = trimmed.graph.as_ref().unwrap();
    assert!(!g.entities.contains_key(&source));
    assert_eq!(g.entities.len(), 4);
    assert!(g.dimension_ids.is_empty());
    assert_eq!(
        g.constraints
            .values()
            .filter(|c| matches!(c, Constraint::PointOnLine { .. }))
            .count(),
        2
    );
    for (id, e) in &g.entities {
        if let Entity::Line { a, b } = e {
            if *id
                > *before
                    .graph
                    .as_ref()
                    .unwrap()
                    .entities
                    .keys()
                    .last()
                    .unwrap()
            {
                let length =
                    (g.points[a][0] - g.points[b][0]).hypot(g.points[a][1] - g.points[b][1]);
                assert!((length - 3.).abs() < 1e-5);
            }
        }
    }
    let d = Document {
        sketches: vec![trimmed],
        ..Default::default()
    };
    assert_eq!(Document::from_json(&d.to_json().unwrap()).unwrap(), d);
    assert!(
        s.with_trimmed_line(source, [3., 0.], &Default::default())
            .is_err()
    );
    assert_eq!(s, before);
}
#[test]
fn extend_chooses_nearest_finite_boundary_in_clicked_direction_and_tracks_it() {
    let mut s = Sketch {
        id: 1,
        ..Default::default()
    };
    s = s
        .with_line([0., 0.], [5., 0.], &Default::default())
        .unwrap();
    let source = *s.graph.as_ref().unwrap().entities.keys().next().unwrap();
    for x in [-3., 10., 15.] {
        s = s.with_line([x, -5.], [x, 5.], &Default::default()).unwrap();
    }
    let extended = s
        .with_extended_line(source, [4., 0.], &Default::default())
        .unwrap();
    let g = extended.graph.as_ref().unwrap();
    let new = *g.entities.keys().last().unwrap();
    let Entity::Line { a, b } = g.entities[&new] else {
        panic!()
    };
    assert_eq!(g.points[&a], [0., 0.]);
    assert_eq!(g.points[&b], [10., 0.]);
    let boundary = g
        .constraints
        .values()
        .find_map(|c| {
            if let Constraint::PointOnLine { line, .. } = c {
                Some(*line)
            } else {
                None
            }
        })
        .unwrap();
    let mut graph = g.clone();
    let Entity::Line { a: ba, b: bb } = graph.entities[&boundary] else {
        panic!()
    };
    graph.points.get_mut(&ba).unwrap()[0] = 12.;
    graph.points.get_mut(&bb).unwrap()[0] = 12.;
    for p in [ba, bb] {
        graph
            .constrain(Constraint::Fixed {
                point: p,
                at: graph.points[&p],
            })
            .unwrap();
    }
    let moved = extended.with_graph(graph, &Default::default()).unwrap();
    assert!((moved.graph.as_ref().unwrap().points[&b][0] - 12.).abs() < 1e-5);
    let left = s
        .with_extended_line(source, [1., 0.], &Default::default())
        .unwrap();
    let g = left.graph.as_ref().unwrap();
    let Entity::Line { a, b } = g.entities[g.entities.keys().last().unwrap()] else {
        panic!()
    };
    assert_eq!(g.points[&a], [-3., 0.]);
    assert_eq!(g.points[&b], [5., 0.]);
}
#[test]
fn absent_extension_and_invalid_trim_preserve_the_accepted_sketch() {
    let s = Sketch::default()
        .with_line([0., 0.], [5., 0.], &Default::default())
        .unwrap();
    let before = s.clone();
    let source = *s.graph.as_ref().unwrap().entities.keys().next().unwrap();
    assert!(
        s.with_extended_line(source, [4., 0.], &Default::default())
            .is_err()
    );
    assert!(
        s.with_trimmed_line(source, [f64::NAN, 0.], &Default::default())
            .is_err()
    );
    assert_eq!(s, before);
    let empty = s
        .with_trimmed_line(source, [2., 0.], &Default::default())
        .unwrap();
    assert!(empty.graph.as_ref().unwrap().entities.is_empty());
}
