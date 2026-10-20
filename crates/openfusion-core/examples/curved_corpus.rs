//! Save/reopen exact line/arc extrusion fixtures outside the repository.
use fission180_core::{
    Document, Extrusion, Operation, Plane, Profile, Session, Sketch,
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
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Supply an external evidence directory")?,
    );
    std::fs::create_dir_all(&dir)?;
    let profiles = [
        (
            "semicircle",
            Profile::Curved {
                segments: vec![arc([0.; 2], 10., 0., PI), line([-10., 0.], [10., 0.])],
            },
            250. * PI,
        ),
        (
            "quarter",
            Profile::Curved {
                segments: vec![
                    arc([0.; 2], 10., 0., FRAC_PI_2),
                    line([0., 10.], [0.; 2]),
                    line([0.; 2], [10., 0.]),
                ],
            },
            125. * PI,
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
            1000. + 125. * PI,
        ),
    ];
    let mut records = vec![];
    for (name, profile, volume) in profiles {
        for plane in [Plane::XY, Plane::XZ, Plane::YZ] {
            for backend in [Backend::Rust, Backend::Occt] {
                let label = format!("{name}-{plane:?}-{backend:?}");
                let mut sketch = Sketch {
                    id: 1,
                    name: name.into(),
                    plane,
                    origin: [17., -23., 11.],
                    ..Default::default()
                };
                let region = sketch.add_profile(profile.clone())?;
                let d = Document {
                    name: label.clone(),
                    backend,
                    sketches: vec![sketch],
                    extrusions: vec![Extrusion {
                        id: 2,
                        name: "Exact curved extrusion".into(),
                        sketch_id: 1,
                        profile: 0,
                        region_id: Some(region),
                        distance: -5.,
                        distance_expression: Some("-5 mm".into()),
                        operation: Operation::NewBody,
                        body_id: 3,
                        result_body_ids: vec![],
                        boundary_dependencies: None,
                        options: Default::default(),
                    }],
                    ..Default::default()
                };
                let session = Session::new(d)?;
                let path = dir.join(format!("{label}.ofrust"));
                persistence::save(&session.document, &path)?;
                let reopened =
                    Session::new(Document::from_json(&std::fs::read_to_string(&path)?)?)?;
                assert_eq!(session.document, reopened.document);
                let info = &reopened.model.info()[0];
                assert!((info.volume - volume).abs() < 1e-5);
                let topology = if let Geometry::Rust(s) = &reopened.model.bodies[0].solid {
                    s.topology().validate()?;
                    serde_json::json!({"vertices":s.topology().vertices.len(),"edges":s.topology().edges.len(),"faces":s.topology().faces.len(),"source_entities":s.topology().edges.iter().map(|e|e.source_entity).collect::<Vec<_>>()})
                } else {
                    serde_json::Value::Null
                };
                let step = if backend == Backend::Occt {
                    let bytes = reopened.model.step_bytes()?;
                    std::fs::write(dir.join(format!("{label}.step")), &bytes)?;
                    Some(fission180_core::Model::step_volume(&bytes)?)
                } else {
                    None
                };
                records.push(serde_json::json!({"fixture":path,"backend":info.backend,"saved_reopened":true,"volume_mm3":info.volume,"area_mm2":info.surface_area,"bounds_mm":info.bounds,"expected_volume_mm3":volume,"topology":topology,"step_reimport_volume_mm3":step}));
            }
        }
    }
    std::fs::write(
        dir.join("measurements.json"),
        serde_json::to_string_pretty(&records)?,
    )?;
    println!(
        "Saved/reopened {} exact curved fixtures; Rust/OCCT backends identified per record",
        records.len()
    );
    Ok(())
}
