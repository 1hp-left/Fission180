//! Saved/reopened same-axis section Boolean acceptance corpus. No Rust-to-OCCT conversion.
#[path = "support/axial_fixtures.rs"]
mod fixtures;
use fission180_core::{
    Document, Model, Plane, Session,
    kernel::{Backend, Geometry},
    persistence,
};
use fixtures::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Supply an external evidence directory")?,
    );
    std::fs::create_dir_all(&dir)?;
    let source = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()?;
    let mut records = vec![];
    for plane in [Plane::XY, Plane::XZ, Plane::YZ] {
        for negative in [false, true] {
            for reverse in [false, true] {
                for mut case in cases() {
                    reframe(&mut case, plane, negative, reverse);
                    let mut measured = vec![];
                    for backend in [Backend::Rust, Backend::Occt] {
                        let label = format!(
                            "{}-{plane:?}-negative{negative}-reverse{reverse}-{backend:?}",
                            case.name
                        );
                        let session = Session::new(document(&case, backend))?;
                        let path = dir.join(format!("{label}.ofrust"));
                        persistence::save(&session.document, &path)?;
                        let reopened =
                            Session::new(Document::from_json(&std::fs::read_to_string(&path)?)?)?;
                        assert_eq!(session.document, reopened.document);
                        let info = reopened.model.info();
                        assert_eq!(info.len(), case.volumes.len());
                        let volume = info.iter().map(|b| b.volume).sum::<f64>();
                        let area = info.iter().map(|b| b.surface_area).sum::<f64>();
                        let expected = case.volumes.iter().sum::<f64>();
                        assert!((volume - expected).abs() < 1e-5);
                        if let Some(expected) = case.area {
                            assert!((area - expected).abs() < 1e-5);
                        }
                        let bodies=reopened.model.bodies.iter().zip(&info).map(|(body,info)| {
                    let topology=if let Geometry::Rust(s)=&body.solid {
                        s.topology().validate().unwrap();
                        serde_json::json!({"closed_manifold":true,"vertices":s.topology().vertices.len(),"edges":s.topology().edges.len(),"faces":s.topology().faces.len(),"sections":s.occupied_sections().unwrap().iter().map(|(z,r)|serde_json::json!({"interval_mm":z,"area_mm2":r.area(),"regions":r.regions.len()})).collect::<Vec<_>>()})
                    } else {serde_json::Value::Null};
                    serde_json::json!({"id":info.id,"backend":info.backend,"volume_mm3":info.volume,"area_mm2":info.surface_area,"bounds_mm":info.bounds,"topology":topology})
                }).collect::<Vec<_>>();
                        let step = if backend == Backend::Occt && !info.is_empty() {
                            let bytes = reopened.model.step_bytes()?;
                            std::fs::write(dir.join(format!("{label}.step")), &bytes)?;
                            let v = Model::step_volume(&bytes)?;
                            assert!((v - volume).abs() < 1e-5);
                            Some(v)
                        } else {
                            None
                        };
                        records.push(serde_json::json!({"fixture":path,"source_base":String::from_utf8_lossy(&source.stdout).trim(),"backend":backend,"case":case.name,"parameters":serde_json::from_str::<serde_json::Value>(&reopened.document.to_json()?)?,"plane":plane,"negative":negative,"reverse":reverse,"body_count":info.len(),"bodies":bodies,"expected_volume_mm3":expected,"volume_mm3":volume,"expected_area_mm2":case.area,"area_mm2":area,"saved_reopened":true,"display_triangles":reopened.model.display_mesh()?.indices.len()/3,"occt_oracle_step_volume_mm3":step,"measurement_tolerance":1e-5,"coincidence_mm":fission180_core::planar::COINCIDENCE_MM,"resolution_mm":fission180_core::planar::RESOLUTION_MM}));
                        measured.push((volume, area, info));
                    }
                    assert!((measured[0].0 - measured[1].0).abs() < 1e-5);
                    assert!((measured[0].1 - measured[1].1).abs() < 1e-5);
                    for body in &measured[0].2 {
                        assert!(measured[1].2.iter().any(|o| {
                            o.bounds
                                .iter()
                                .flatten()
                                .zip(body.bounds.iter().flatten())
                                .all(|(a, b)| (a - b).abs() < 2e-5)
                        }));
                    }
                }
            }
        }
    }
    std::fs::write(
        dir.join("measurements.json"),
        serde_json::to_vec_pretty(&records)?,
    )?;
    println!(
        "Saved/reopened {} backend-tagged axial Boolean fixtures; analytic and independent OCCT checks passed",
        records.len()
    );
    Ok(())
}
