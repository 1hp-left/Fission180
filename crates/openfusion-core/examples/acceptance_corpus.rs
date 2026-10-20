//! Reproducible native documents and backend measurements for desktop verification.
use fission180_core::{
    Document, Extrusion, Operation, Profile, Session, Sketch, kernel::Backend, persistence,
};
use std::{collections::BTreeMap, path::PathBuf};
fn sketch(id: u64, profiles: Vec<Profile>) -> Sketch {
    let mut sketch = Sketch {
        id,
        name: format!("Sketch{id}"),
        ..Default::default()
    };
    for p in profiles {
        sketch.add_profile(p).unwrap();
    }
    sketch
}
fn extrusion(
    id: u64,
    sketch_id: u64,
    region_id: u64,
    body_id: u64,
    distance: f64,
    operation: Operation,
) -> Extrusion {
    Extrusion {
        id,
        name: format!("Extrude{id}"),
        sketch_id,
        profile: 0,
        region_id: Some(region_id),
        distance,
        distance_expression: None,
        operation,
        body_id,
        result_body_ids: vec![],
        options: Default::default(),
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Supply an output directory")?,
    );
    std::fs::create_dir_all(&directory)?;
    let rectangle = Profile::Rectangle {
        a: [0., 0.],
        b: [40., 30.],
    };
    let mut records = vec![];
    for backend in [Backend::Occt, Backend::Rust] {
        for scenario in [
            "rectangle",
            "annular-region",
            "split-cut",
            "parameter-edit",
            "offset-symmetric",
            "two-sided",
            "through-all",
        ] {
            let mut d = Document {
                name: format!("{scenario}-{}", backend.label()),
                backend,
                ..Default::default()
            };
            d.sketches.push(sketch(1, vec![rectangle.clone()]));
            d.extrusions
                .push(extrusion(2, 1, 1, 3, 6., Operation::NewBody));
            d.extrusions[0].distance_expression = Some("2 * 3 mm".into());
            match scenario {
                "offset-symmetric" | "two-sided" => {
                    d.extrusions[0].distance = if scenario == "two-sided" { 4. } else { 10. };
                    d.parameters.insert("offset".into(), "3 mm".into());
                    d.extrusions[0].distance_expression = Some(
                        if scenario == "two-sided" {
                            "4 mm"
                        } else {
                            "10 mm"
                        }
                        .into(),
                    );
                    d.extrusions[0].options = fission180_core::extrusion::ExtrudeOptions {
                        offset_start: true,
                        start_offset: "offset".into(),
                        direction: if scenario == "two-sided" {
                            fission180_core::extrusion::Direction::TwoSides
                        } else {
                            fission180_core::extrusion::Direction::Symmetric
                        },
                        second_distance: "6 mm".into(),
                        ..Default::default()
                    };
                }
                "through-all" => {
                    let mut circle = sketch(
                        4,
                        vec![Profile::Circle {
                            center: [20., 15.],
                            radius: 5.,
                        }],
                    );
                    circle.origin[2] = -3.;
                    d.sketches.push(circle);
                    let mut cut = extrusion(5, 4, 1, 3, 1., Operation::Cut);
                    cut.options.extent = fission180_core::extrusion::Extent::ThroughAll;
                    d.extrusions.push(cut);
                }
                "annular-region" => {
                    d.sketches[0].add_profile(Profile::Circle {
                        center: [20., 15.],
                        radius: 5.,
                    })?;
                }
                "split-cut" => {
                    d.sketches.push(sketch(
                        4,
                        vec![Profile::Rectangle {
                            a: [18., -1.],
                            b: [22., 31.],
                        }],
                    ));
                    d.extrusions.push(extrusion(5, 4, 1, 3, 6., Operation::Cut));
                }
                "parameter-edit" => {
                    d.parameters = BTreeMap::from([
                        ("width".into(), "80 mm".into()),
                        ("height".into(), "30 mm".into()),
                    ]);
                    d.sketches[0] = d.sketches[0].with_dimensions(
                        1,
                        BTreeMap::from([
                            ("width".into(), "width".into()),
                            ("height".into(), "height".into()),
                        ]),
                        &d.parameters,
                    )?;
                }
                _ => {}
            }
            let session = Session::new(d)?;
            let path = directory.join(format!(
                "{scenario}-{}.ofrust",
                if backend == Backend::Rust {
                    "rust"
                } else {
                    "occt"
                }
            ));
            persistence::save(&session.document, &path)?;
            let reopened = Session::new(Document::from_json(&std::fs::read_to_string(&path)?)?)?;
            assert_eq!(session.document, reopened.document);
            let info = reopened.model.info();
            records.push(serde_json::json!({"scenario":scenario,"backend":backend,"path":path,"saved_reopened":true,"bodies":info.iter().map(|b|serde_json::json!({"id":b.id,"volume_mm3":b.volume,"area_mm2":b.surface_area,"bounds_mm":b.bounds,"backend":b.backend})).collect::<Vec<_>>(),"display_triangles":reopened.model.display_mesh()?.indices.len()/3}));
        }
    }
    std::fs::write(
        directory.join("measurements.json"),
        serde_json::to_string_pretty(&records)?,
    )?;
    println!(
        "Wrote and reopened {} native fixtures in {}",
        records.len(),
        directory.display()
    );
    Ok(())
}
