use fission180_core::{
    Document, Extrusion, Model, Operation, Plane, Profile, Session, Sketch,
    arc::ArcGeometry,
    boundary::{BoundarySegment, PlanarCurve},
    kernel::{
        Backend, ExtrudeInput, Geometry, SolidKernel,
        native::{Curve, NativeSolid},
    },
    sketch::{Constraint, Entity, SketchGraph},
};
use std::f64::consts::{FRAC_PI_2, PI};
fn segment(curve: PlanarCurve) -> BoundarySegment {
    BoundarySegment {
        source: None,
        curve,
    }
}
fn line(a: [f64; 2], b: [f64; 2]) -> BoundarySegment {
    segment(PlanarCurve::Line { start: a, end: b })
}
fn arc(center: [f64; 2], radius: f64, start_angle: f64, sweep: f64) -> BoundarySegment {
    segment(PlanarCurve::Arc {
        geometry: ArcGeometry {
            center,
            radius,
            start_angle,
            sweep,
        },
    })
}
fn semicircle(r: f64) -> Profile {
    Profile::Curved {
        segments: vec![arc([0.; 2], r, 0., PI), line([-r, 0.], [r, 0.])],
    }
}
fn sector(r: f64) -> Profile {
    Profile::Curved {
        segments: vec![
            arc([0.; 2], r, 0., FRAC_PI_2),
            line([0., r], [0.; 2]),
            line([0.; 2], [r, 0.]),
        ],
    }
}
fn capsule() -> Profile {
    Profile::Curved {
        segments: vec![
            line([-10., -5.], [10., -5.]),
            arc([10., 0.], 5., -FRAC_PI_2, PI),
            line([10., 5.], [-10., 5.]),
            arc([-10., 0.], 5., FRAC_PI_2, PI),
        ],
    }
}
fn input(outer: Profile, holes: Vec<Profile>) -> ExtrudeInput {
    ExtrudeInput {
        feature_id: 2,
        region_id: 1,
        loop_ids: (1..=1 + holes.len() as u64).collect(),
        outer,
        holes,
        plane: Plane::XY,
        origin: [0.; 3],
        distance: 5.,
    }
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-5, "{a} != {b}");
}
fn reverse(p: &Profile) -> Profile {
    let mut segments = p.boundary();
    segments.reverse();
    for s in &mut segments {
        s.curve = s.curve.reversed();
    }
    Profile::Curved { segments }
}

/// Explicit test-only OCCT adapter. Reconstructs exact B-rep edge trims; it never
/// reads triangles or becomes part of production Rust execution or export.
fn occt_test_adapter(s: &NativeSolid) -> cadrum::Solid {
    use cadrum::{DVec3, Edge, Solid};
    let brep = s.topology();
    let mut edges = vec![];
    for boundary in &brep.faces[0].loops {
        for co in boundary {
            let edge = &brep.edges[co.edge];
            let mut v = edge.vertices;
            if co.reversed {
                v.swap(0, 1);
            }
            let a = DVec3::from_array(brep.vertices[v[0]].point);
            let b = DVec3::from_array(brep.vertices[v[1]].point);
            edges.push(match edge.curve {
                Curve::Line => Edge::line(a, b).unwrap(),
                Curve::Circle {
                    center,
                    normal,
                    radius,
                } => Edge::circle(radius, DVec3::from_array(normal))
                    .unwrap()
                    .translate(DVec3::from_array(center)),
                Curve::Arc {
                    center,
                    normal,
                    x_axis,
                    radius,
                    start_angle,
                    sweep,
                } => {
                    let x = DVec3::from_array(x_axis);
                    let y = DVec3::from_array(normal).cross(x);
                    let angle = start_angle + sweep * 0.5;
                    let mid =
                        DVec3::from_array(center) + radius * (x * angle.cos() + y * angle.sin());
                    Edge::arc_3pts(a, mid, b).unwrap()
                }
            });
        }
    }
    let fission180_core::kernel::native::Surface::Plane { origin: a, .. } = brep.faces[0].surface
    else {
        panic!()
    };
    let fission180_core::kernel::native::Surface::Plane { origin: b, .. } = brep.faces[1].surface
    else {
        panic!()
    };
    Solid::extrude(&edges, DVec3::from_array(b) - DVec3::from_array(a)).unwrap()
}
#[test]
fn exact_curved_extrusion_measures_and_interoperability_all_frames() {
    for (profile, area, perimeter) in [
        (semicircle(10.), 50. * PI, 10. * PI + 20.),
        (sector(10.), 25. * PI, 5. * PI + 20.),
        (capsule(), 200. + 25. * PI, 40. + 10. * PI),
    ] {
        for reversed in [false, true] {
            for plane in [Plane::XY, Plane::XZ, Plane::YZ] {
                for distance in [5., -5.] {
                    let mut i = input(
                        if reversed {
                            reverse(&profile)
                        } else {
                            profile.clone()
                        },
                        vec![],
                    );
                    i.plane = plane;
                    i.origin = [17., -23., 11.];
                    i.distance = distance;
                    let s = NativeSolid::extrude(i.clone()).unwrap();
                    s.topology().validate().unwrap();
                    close(s.volume(), area * 5.);
                    close(s.area(), area * 2. + perimeter * 5.);
                    let before = (s.volume(), s.area(), s.bounds());
                    for resolution in [8, 31, 128] {
                        let mesh = s.mesh(resolution).unwrap();
                        assert!(!mesh.indices.is_empty());
                        assert_eq!(before, (s.volume(), s.area(), s.bounds()));
                        assert!(
                            mesh.face_ids.iter().flatten().all(|id| s
                                .topology()
                                .faces
                                .iter()
                                .any(|f| f.id == *id))
                        );
                    }
                    let occt = occt_test_adapter(&s);
                    close(occt.volume(), s.volume());
                    close(occt.area(), s.area());
                    let production_occt = Backend::Occt.extrude(&i).unwrap();
                    close(production_occt.volume(), s.volume());
                    let bounds = occt.bounding_box().map(|p| p.to_array());
                    for (a, b) in s.bounds().iter().flatten().zip(bounds.iter().flatten()) {
                        close(*a, *b);
                    }
                    let mut step = vec![];
                    cadrum::Solid::write_step([&occt], &mut step).unwrap();
                    close(Model::step_volume(&step).unwrap(), s.volume());
                }
            }
        }
    }
}
#[test]
fn curved_holes_major_arcs_and_analytic_containment() {
    let circle = Profile::Curved {
        segments: vec![arc([0.; 2], 10., 0., PI), arc([0.; 2], 10., PI, PI)],
    };
    close(
        NativeSolid::extrude(input(circle, vec![]))
            .unwrap()
            .volume(),
        500. * PI,
    );
    let mut i = input(
        capsule(),
        vec![
            Profile::Circle {
                center: [0.; 2],
                radius: 2.,
            },
            sector(2.),
        ],
    );
    assert!(NativeSolid::extrude(i.clone()).is_err()); // these two holes overlap
    i.holes = vec![
        Profile::Circle {
            center: [-5., 0.],
            radius: 2.,
        },
        sector(2.),
    ];
    i.loop_ids = vec![1, 2, 3];
    let s = NativeSolid::extrude(i.clone()).unwrap();
    let o = occt_test_adapter(&s);
    close(s.volume(), (200. + 25. * PI - 4. * PI - PI) * 5.);
    close(s.area(), o.area());
    close(s.volume(), o.volume());
    let major = Profile::Curved {
        segments: vec![arc([0.; 2], 10., 0., 1.5 * PI), line([0., -10.], [10., 0.])],
    };
    close(major.signed_area(), 75. * PI + 50.);
    assert!(major.contains_point([-9., 0.]));
    assert!(!major.contains_point([9., -9.]));
    let s = NativeSolid::extrude(input(major, vec![])).unwrap();
    close(s.volume(), (75. * PI + 50.) * 5.);
    close(s.volume(), occt_test_adapter(&s).volume());
}
pub fn graph_semicircle() -> (Sketch, u64, u64) {
    let mut g = SketchGraph::default();
    let center = g.point([0.; 2]).unwrap();
    let start = g.point([10., 0.]).unwrap();
    let end = g.point([-10., 0.]).unwrap();
    let arc = g
        .entity(Entity::Arc {
            center,
            start,
            end,
            clockwise: false,
        })
        .unwrap();
    let diameter = g.entity(Entity::Line { a: end, b: start }).unwrap();
    g.constrain(Constraint::Fixed {
        point: center,
        at: [0.; 2],
    })
    .unwrap();
    g.constrain(Constraint::Midpoint {
        point: center,
        line: diameter,
    })
    .unwrap();
    g.constrain(Constraint::Horizontal { line: diameter })
        .unwrap();
    let s = Sketch {
        id: 1,
        ..Default::default()
    }
    .with_graph(g, &Default::default())
    .unwrap();
    (s, arc, diameter)
}
fn document(s: Sketch) -> Document {
    let region = s.regions().unwrap()[0].id;
    Document {
        backend: Backend::Rust,
        sketches: vec![s],
        extrusions: vec![Extrusion {
            id: 2,
            name: "Curved extrusion".into(),
            sketch_id: 1,
            profile: 0,
            region_id: Some(region),
            distance: 5.,
            distance_expression: None,
            operation: Operation::NewBody,
            body_id: 3,
            result_body_ids: vec![],
            boundary_dependencies: None,
            options: Default::default(),
        }],
        ..Default::default()
    }
}
#[test]
fn graph_radius_replay_persistence_history_and_dependency_failure_are_transactional() {
    let (s, arc, _) = graph_semicircle();
    let region = s.profile_ids[0];
    let mut d = document(s);
    d.parameters.insert("radius".into(), "10 mm".into());
    d.sketches[0] = d.sketches[0]
        .with_entity_dimension(arc, "radius".into(), &d.parameters)
        .unwrap();
    let mut session = Session::new(d).unwrap();
    close(session.model.info()[0].volume, 250. * PI);
    let Geometry::Rust(s) = &session.model.bodies[0].solid else {
        panic!()
    };
    let old_ids: Vec<_> = s.topology().edges.iter().map(|e| e.id).collect();
    session
        .replace(
            session
                .document
                .with_parameter("radius".into(), "20 mm".into())
                .unwrap(),
        )
        .unwrap();
    close(session.model.info()[0].volume, 1000. * PI);
    assert_eq!(session.document.sketches[0].profile_ids[0], region);
    let Geometry::Rust(s) = &session.model.bodies[0].solid else {
        panic!()
    };
    assert_eq!(
        old_ids,
        s.topology().edges.iter().map(|e| e.id).collect::<Vec<_>>()
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("curved.ofrust");
    fission180_core::persistence::save(&session.document, &path).unwrap();
    let reopened = Document::from_json(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(reopened.version, 5);
    close(
        Model::build(&reopened).unwrap().info()[0].volume,
        1000. * PI,
    );
    session.undo().unwrap();
    close(session.model.info()[0].volume, 250. * PI);
    session.redo().unwrap();
    close(session.model.info()[0].volume, 1000. * PI);
    let accepted = session.document.clone();
    let mut bad = accepted.clone();
    bad.sketches[0] = bad.sketches[0]
        .without_entities(&[arc], &bad.parameters)
        .unwrap();
    assert!(session.replace(bad).is_err());
    assert_eq!(session.document, accepted);
    let mut nested = accepted.clone();
    nested.sketches[0]
        .add_profile(Profile::Circle {
            center: [0., 5.],
            radius: 1.,
        })
        .unwrap();
    assert!(
        session
            .replace(nested)
            .unwrap_err()
            .to_string()
            .contains("dependencies changed")
    );
    assert_eq!(session.document, accepted);
    close(session.model.info()[0].volume, 1000. * PI);
    let mut split = accepted.clone();
    split.sketches[0] = split.sketches[0]
        .with_line([0., -5.], [0., 25.], &split.parameters)
        .unwrap();
    assert!(session.replace(split).is_err());
    assert_eq!(session.document, accepted);
    let mut legacy = accepted;
    legacy.version = 4;
    assert!(Document::from_json(&serde_json::to_string(&legacy).unwrap()).is_err());
}
#[test]
fn line_arc_cells_branches_nested_loops_and_stable_region_keys() {
    let (s, _, _) = graph_semicircle();
    let id = s.profile_ids[0];
    let branch = s
        .with_line([0., 0.], [0., 5.], &Default::default())
        .unwrap();
    assert_eq!(branch.regions().unwrap()[0].id, id);
    let split = s
        .with_line([0., -5.], [0., 15.], &Default::default())
        .unwrap();
    let regions = split.regions().unwrap();
    assert_eq!(regions.len(), 2);
    assert!(split.region(id).is_err());
    for r in &regions {
        let s = NativeSolid::extrude(input(split.profile_by_id(r.outer).unwrap().clone(), vec![]))
            .unwrap();
        close(s.volume(), 125. * PI);
    }
    let mut g = split.graph_for_edit().unwrap();
    let Entity::Line { a, b } = g.entities[g.entities.keys().last().unwrap()] else {
        panic!()
    };
    g.points.get_mut(&a).unwrap()[0] = 2.;
    g.points.get_mut(&b).unwrap()[0] = 2.;
    let moved = split.with_graph(g, &Default::default()).unwrap();
    assert_eq!(moved.profile_ids, split.profile_ids);
    let mut nested = s;
    let inner = nested.add_profile(semicircle(3.)).unwrap_err();
    assert!(inner.to_string().contains("ambiguous") || inner.to_string().contains("Overlapping"));
    let mut nested = Sketch {
        id: 1,
        ..Default::default()
    };
    let outer = nested.add_profile(capsule()).unwrap();
    let inner = nested.add_profile(sector(2.)).unwrap();
    assert_eq!(nested.region(outer).unwrap().inner, vec![inner]);
    let d = document(nested);
    let m = Model::build(&d).unwrap();
    close(m.info()[0].volume, (200. + 25. * PI - PI) * 5.);
}
#[test]
fn arc_arc_intersections_find_lens_and_retire_split_consumers() {
    // Two crossing bounded major arcs joined at their endpoints by two lines.
    // Their proper crossings subdivide the same connected source graph.
    let mut s = Sketch {
        id: 1,
        ..Default::default()
    };
    let y = 24_f64.sqrt();
    s = s
        .with_three_point_arc([-1., -y], [4., 0.], [-1., y], &Default::default())
        .unwrap();
    s = s
        .with_three_point_arc([1., y], [-4., 0.], [1., -y], &Default::default())
        .unwrap();
    s = s.with_line([-1., y], [1., y], &Default::default()).unwrap();
    s = s
        .with_line([1., -y], [-1., -y], &Default::default())
        .unwrap();
    assert!(s.regions().unwrap().len() >= 3);
    for region in s.regions().unwrap() {
        let profile = s.profile_by_id(region.outer).unwrap();
        let native = NativeSolid::extrude(input(profile.clone(), vec![])).unwrap();
        close(native.volume(), occt_test_adapter(&native).volume());
    }
}
#[test]
fn corrupt_topology_and_degenerate_boundaries_are_rejected() {
    let s = NativeSolid::extrude(input(semicircle(10.), vec![])).unwrap();
    let mut corrupt = s.topology().clone();
    if let Curve::Arc { sweep, .. } = &mut corrupt.edges[0].curve {
        *sweep *= 0.5;
    }
    assert!(corrupt.validate().is_err());
    let mut corrupt = s.topology().clone();
    corrupt.faces[0].reversed ^= true;
    assert!(corrupt.validate().is_err());
    let mut corrupt = s.topology().clone();
    corrupt.faces[2].loops[0][0].reversed ^= true;
    assert!(corrupt.validate().is_err());
    let mut corrupt = s.topology().clone();
    corrupt.vertices.push(corrupt.vertices[0].clone());
    assert!(corrupt.validate().is_err());
    let mut corrupt = s.topology().clone();
    corrupt.edges[0].vertices[0] = usize::MAX;
    assert!(corrupt.validate().is_err());
    let mut corrupt = s.topology().clone();
    if let fission180_core::kernel::native::Surface::Plane { normal, .. } =
        &mut corrupt.faces[0].surface
    {
        normal[0] = f64::NAN;
    }
    assert!(corrupt.validate().is_err());
    let mut open = semicircle(10.);
    if let Profile::Curved { segments } = &mut open {
        segments[1] = line([-9., 0.], [10., 0.]);
    }
    assert!(open.validate().is_err());
    let overlap = Profile::Curved {
        segments: vec![arc([0.; 2], 10., 0., PI), arc([0.; 2], 10., PI, -PI)],
    };
    assert!(overlap.validate().is_err());
}
