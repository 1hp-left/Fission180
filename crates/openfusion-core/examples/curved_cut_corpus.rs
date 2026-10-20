//! Retained blind-cut fixtures. No GUI or reference runtime is controlled.
use fission180_core::{
    Document, Extrusion, Model, Operation, Plane, Profile, Session, Sketch,
    arc::ArcGeometry,
    boundary::{BoundarySegment, PlanarCurve},
    kernel::{Backend, Geometry},
    persistence,
};
use std::f64::consts::{FRAC_PI_2, PI};
fn line(start: [f64; 2], end: [f64; 2]) -> BoundarySegment {
    BoundarySegment {
        source: None,
        curve: PlanarCurve::Line { start, end },
    }
}
fn arc(center: [f64; 2], radius: f64, start_angle: f64, sweep: f64) -> BoundarySegment {
    BoundarySegment {
        source: None,
        curve: PlanarCurve::Arc {
            geometry: ArcGeometry {
                center,
                radius,
                start_angle,
                sweep,
            },
        },
    }
}
fn extrusion(id: u64, sketch: u64, distance: f64, operation: Operation) -> Extrusion {
    Extrusion {
        id,
        name: operation.label().into(),
        sketch_id: sketch,
        profile: 0,
        region_id: Some(1),
        distance,
        distance_expression: None,
        operation,
        body_id: 3,
        result_body_ids: vec![],
        boundary_dependencies: None,
        options: Default::default(),
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Supply an external evidence directory")?,
    );
    std::fs::create_dir_all(&dir)?;
    let profiles = [
        (
            "circle",
            Profile::Circle {
                center: [0.; 2],
                radius: 3.,
            },
        ),
        (
            "semicircle",
            Profile::Curved {
                segments: vec![arc([0.; 2], 10., 0., PI), line([-10., 0.], [10., 0.])],
            },
        ),
        (
            "capsule",
            Profile::Curved {
                segments: vec![
                    line([-10., -5.], [10., -5.]),
                    arc([10., 0.], 5., -FRAC_PI_2, PI),
                    line([10., 5.], [-10., 5.]),
                    arc([-10., 0.], 5., FRAC_PI_2, PI),
                ],
            },
        ),
    ];
    let mut records = vec![];
    for (name, profile) in profiles {
        for plane in [Plane::XY, Plane::XZ, Plane::YZ] {
            for backend in [Backend::Rust, Backend::Occt] {
                let label = format!("blind-{name}-{plane:?}-{backend:?}");
                let origin = [17., -23., 11.];
                let normal = plane.normal().to_array();
                let mut base = Sketch {
                    id: 1,
                    plane,
                    origin,
                    name: "Target".into(),
                    ..Default::default()
                };
                base.add_profile(Profile::Rectangle {
                    a: [-30.; 2],
                    b: [30.; 2],
                })?;
                let mut tool = Sketch {
                    id: 4,
                    plane,
                    origin: std::array::from_fn(|i| origin[i] + normal[i] * 10.),
                    name: "Blind curved cut".into(),
                    ..Default::default()
                };
                tool.add_profile(profile.clone())?;
                let d = Document {
                    name: label.clone(),
                    backend,
                    sketches: vec![base, tool],
                    extrusions: vec![
                        extrusion(2, 1, 10., Operation::NewBody),
                        extrusion(5, 4, -3., Operation::Cut),
                    ],
                    ..Default::default()
                };
                let session = Session::new(d)?;
                let path = dir.join(format!("{label}.ofrust"));
                persistence::save(&session.document, &path)?;
                let reopened =
                    Session::new(Document::from_json(&std::fs::read_to_string(&path)?)?)?;
                assert_eq!(session.document, reopened.document);
                let info = &reopened.model.info()[0];
                let expected = 36000. - profile.signed_area().abs() * 3.;
                let area = 9600. + profile.perimeter() * 3.;
                assert!((info.volume - expected).abs() < 1e-5);
                assert!((info.surface_area - area).abs() < 1e-5);
                let topology = if let Geometry::Rust(s) = &reopened.model.bodies[0].solid {
                    s.topology().validate()?;
                    serde_json::json!({"vertices":s.topology().vertices.len(),"edges":s.topology().edges.len(),"faces":s.topology().faces.len(),"closed_manifold":true,"last_operation":s.provenance.last().map(|p|p.operation.label())})
                } else {
                    serde_json::Value::Null
                };
                let step = if backend == Backend::Occt {
                    let bytes = reopened.model.step_bytes()?;
                    std::fs::write(dir.join(format!("{label}.step")), &bytes)?;
                    Some(Model::step_volume(&bytes)?)
                } else {
                    None
                };
                records.push(serde_json::json!({"fixture":path,"backend":info.backend,"saved_reopened":true,"volume_mm3":info.volume,"expected_volume_mm3":expected,"area_mm2":info.surface_area,"expected_area_mm2":area,"bounds_mm":info.bounds,"topology":topology,"step_reimport_volume_mm3":step}));
            }
        }
    }
    std::fs::write(
        dir.join("measurements.json"),
        serde_json::to_string_pretty(&records)?,
    )?;
    println!(
        "Saved/reopened {} blind-cut fixtures with identified backends",
        records.len()
    );
    Ok(())
}
