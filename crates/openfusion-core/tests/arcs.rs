use fission180_core::{
    Document, Sketch,
    arc::ArcGeometry,
    sketch::{Constraint, Entity},
};
use std::f64::consts::{FRAC_PI_2, PI};
fn arc() -> (Sketch, u64) {
    let s = Sketch {
        id: 1,
        ..Default::default()
    }
    .with_three_point_arc(
        [10., 0.],
        [std::f64::consts::FRAC_1_SQRT_2 * 10.; 2],
        [0., 10.],
        &Default::default(),
    )
    .unwrap();
    let id = *s.graph.as_ref().unwrap().entities.keys().last().unwrap();
    (s, id)
}
#[test]
fn analytic_arc_preserves_minor_major_and_clockwise_branches_and_exact_queries() {
    for (mid, clockwise, sweep) in [
        ([5., 5.], false, PI),
        ([5., -5.], true, -PI),
        ([5., 10.], false, 2. * (2_f64).atan() * 2.),
    ] {
        let a = ArcGeometry::through([10., 0.], mid, [0., 0.]).unwrap();
        assert_eq!(a.sweep < 0., clockwise);
        assert!((a.sweep - sweep).abs() < 1e-12, "{} {sweep}", a.sweep);
        assert!(a.contains(mid, 1e-8));
        assert!((a.distance_to(mid)).abs() < 1e-8);
        assert!((a.length() - a.radius * a.sweep.abs()).abs() < 1e-12);
        let count = a.sample(128).len();
        assert_eq!(count, 129);
    }
    let q = ArcGeometry::through(
        [10., 0.],
        [std::f64::consts::FRAC_1_SQRT_2 * 10.; 2],
        [0., 10.],
    )
    .unwrap();
    assert!((q.sweep - FRAC_PI_2).abs() < 1e-12);
    assert!((q.distance_to([-10., 0.]) - 200_f64.sqrt()).abs() < 1e-10);
    assert!(!q.contains([-10., 0.], 1e-7));
    assert!(ArcGeometry::through([0., 0.], [5., 0.], [10., 0.]).is_err());
    assert!(ArcGeometry::through([0., 0.], [0., 0.], [10., 0.]).is_err());
}
#[test]
fn radius_expression_drives_both_endpoints_and_survives_save_reopen_parameter_edit() {
    let (s, id) = arc();
    let mut g = s.graph_for_edit().unwrap();
    let Entity::Arc {
        center, start, end, ..
    } = g.entities[&id]
    else {
        panic!()
    };
    g.constrain(Constraint::Fixed {
        point: center,
        at: [0., 0.],
    })
    .unwrap();
    let x = g
        .entity(Entity::Line {
            a: center,
            b: start,
        })
        .unwrap();
    g.construction.insert(x);
    g.constrain(Constraint::Horizontal { line: x }).unwrap();
    let y = g.entity(Entity::Line { a: center, b: end }).unwrap();
    g.construction.insert(y);
    g.constrain(Constraint::Vertical { line: y }).unwrap();
    let parameters = std::collections::BTreeMap::from([("radius".into(), "20 mm".into())]);
    let s = s
        .with_graph(g, &Default::default())
        .unwrap()
        .with_entity_dimension(id, "radius".into(), &parameters)
        .unwrap();
    let arc = s.graph.as_ref().unwrap().arc_geometry(id).unwrap();
    assert!((arc.radius - 20.).abs() < 1e-6);
    assert!((arc.sweep - FRAC_PI_2).abs() < 1e-6);
    let d = Document {
        parameters,
        sketches: vec![s],
        ..Default::default()
    };
    let d = Document::from_json(&d.to_json().unwrap()).unwrap();
    let d = d.with_parameter("radius".into(), "30 mm".into()).unwrap();
    let a = d.sketches[0]
        .graph
        .as_ref()
        .unwrap()
        .arc_geometry(id)
        .unwrap();
    assert!((a.radius - 30.).abs() < 1e-6);
    assert!((a.length() - 15. * PI).abs() < 1e-5);
    assert_eq!(d.sketches[0].graph.as_ref().unwrap().dimension_ids.len(), 1);
}
#[test]
fn finite_arc_tangency_and_conflict_paths_reject_without_mutation() {
    let (s, arc) = arc();
    let s = s
        .with_line([5., 10.], [15., 10.], &Default::default())
        .unwrap();
    let mut g = s.graph_for_edit().unwrap();
    let line = *g.entities.keys().last().unwrap();
    let all: Vec<_> = g.points.iter().map(|(id, p)| (*id, *p)).collect();
    for (point, at) in all {
        g.constrain(Constraint::Fixed { point, at }).unwrap();
    }
    let fixed = s.with_graph(g, &Default::default()).unwrap();
    let before = fixed.clone();
    let tangent = fixed
        .with_constraints(
            vec![Constraint::Tangent { line, circle: arc }],
            &Default::default(),
        )
        .unwrap_err();
    assert!(tangent.to_string().contains("outside"));
    assert_eq!(fixed, before);
    assert!(
        fixed
            .with_entity_dimension(arc, "20 mm".into(), &Default::default())
            .is_err()
    );
    let s = fixed
        .without_entities(&[line], &Default::default())
        .unwrap()
        .with_line([-5., 10.], [5., 10.], &Default::default())
        .unwrap();
    let line = *s.graph.as_ref().unwrap().entities.keys().last().unwrap();
    let tangent = s
        .with_constraints(
            vec![Constraint::Tangent { line, circle: arc }],
            &Default::default(),
        )
        .unwrap();
    tangent
        .graph
        .as_ref()
        .unwrap()
        .validate_solution(&Default::default())
        .unwrap();
}
#[test]
fn arcs_support_equal_concentric_construction_deletion_and_shared_endpoints() {
    let (s, id) = arc();
    let s = s
        .with_three_point_arc(
            [20., 0.],
            [std::f64::consts::FRAC_1_SQRT_2 * 20.; 2],
            [0., 20.],
            &Default::default(),
        )
        .unwrap();
    let other = *s.graph.as_ref().unwrap().entities.keys().last().unwrap();
    // Equal/concentric overlapping curves remain valid construction geometry;
    // duplicate active boundaries are intentionally rejected by the arrangement.
    let s = s
        .with_construction(&[other], true, &Default::default())
        .unwrap();
    let s = s
        .with_constraints(
            vec![
                Constraint::Concentric { a: id, b: other },
                Constraint::Equal { a: id, b: other },
            ],
            &Default::default(),
        )
        .unwrap();
    let g = s.graph.as_ref().unwrap();
    assert!(
        (g.arc_geometry(id).unwrap().radius - g.arc_geometry(other).unwrap().radius).abs() < 1e-6
    );
    let construction = s
        .with_construction(&[id], true, &Default::default())
        .unwrap();
    assert!(
        construction
            .graph
            .as_ref()
            .unwrap()
            .construction
            .contains(&id)
    );
    let restored = construction
        .with_construction(&[id], false, &Default::default())
        .unwrap();
    let deleted = restored
        .without_entities(&[id], &Default::default())
        .unwrap();
    assert!(!deleted.graph.as_ref().unwrap().entities.contains_key(&id));
    assert!(deleted.graph.as_ref().unwrap().constraints.is_empty());
    let (s, id) = arc();
    let endpoint = s
        .graph
        .as_ref()
        .unwrap()
        .arc_geometry(id)
        .unwrap()
        .point(1.);
    let s = s
        .with_line(endpoint, [0., 20.], &Default::default())
        .unwrap();
    let g = s.graph.as_ref().unwrap();
    let Entity::Arc { end, .. } = g.entities[&id] else {
        panic!()
    };
    let Entity::Line { a, .. } = g.entities[g.entities.keys().last().unwrap()] else {
        panic!()
    };
    assert_eq!(a, end);
}
#[test]
fn analytic_arc_reconstruction_matches_occt_sector_volume_and_step() {
    use cadrum::{DVec3, Edge, Solid};
    let arc = ArcGeometry::through([-5., 0.], [0., 5.], [5., 0.]).unwrap();
    let p = |t| {
        let p = arc.point(t);
        DVec3::new(p[0], p[1], 0.)
    };
    let edges = vec![
        Edge::arc_3pts(p(0.), p(0.5), p(1.)).unwrap(),
        Edge::line(p(1.), p(0.)).unwrap(),
    ];
    let solid = Solid::extrude(&edges, DVec3::Z * 3.).unwrap();
    let volume = arc.signed_segment_area().abs() * 3.;
    assert!((solid.volume() - volume).abs() < 1e-5);
    let mut bytes = vec![];
    Solid::write_step([&solid], &mut bytes).unwrap();
    assert!((fission180_core::Model::step_volume(&bytes).unwrap() - volume).abs() < 1e-5);
    // This is a test adapter, not a production arc-region extrusion/fallback.
}

#[test]
fn curved_cells_retire_the_old_linear_region_and_reject_ambiguous_tangencies() {
    let mut s = Sketch {
        id: 1,
        ..Default::default()
    };
    s.add_profile(fission180_core::Profile::Rectangle {
        a: [-10., -10.],
        b: [10., 10.],
    })
    .unwrap();
    let region = s.profile_ids[0];
    let outside = s
        .with_three_point_arc([20., 0.], [25., 5.], [30., 0.], &Default::default())
        .unwrap();
    assert_eq!(outside.regions().unwrap().len(), 1);
    assert!(
        s.with_three_point_arc([-10., 0.], [0., 10.], [10., 0.], &Default::default())
            .is_err()
    );
    let intersecting = s
        .with_three_point_arc([-10., 0.], [0., 8.], [10., 0.], &Default::default())
        .unwrap();
    assert_eq!(intersecting.regions().unwrap().len(), 2);
    assert!(intersecting.region(region).is_err());
    let arc = *intersecting
        .graph
        .as_ref()
        .unwrap()
        .entities
        .keys()
        .last()
        .unwrap();
    let construction = intersecting
        .with_construction(&[arc], true, &Default::default())
        .unwrap();
    assert_eq!(construction.regions().unwrap()[0].id, region);
}
