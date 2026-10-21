//! Retained analytic planar-face acceptance fixtures and independent OCCT oracles.
#[path = "support/face_fixtures.rs"]
mod fixtures;
use fission180_core::{
    Document, Model, Session,
    kernel::{Backend, Geometry},
    persistence,
};
use fixtures::*;
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-5, "{a} != {b}");
}
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
    for permutation in 0..6 {
        for reverse_distance in [false, true] {
            for reverse_profile in [false, true] {
                for mut case in cases() {
                    reframe(&mut case, permutation, reverse_distance, reverse_profile);
                    let mut measurements = vec![];
                    for combine in [false, true] {
                        for backend in [Backend::Rust, Backend::Occt] {
                            let label = format!(
                                "{}-axes{permutation}-direction{reverse_distance}-profile{reverse_profile}-combine{combine}-{backend:?}",
                                case.name
                            );
                            let session = Session::new(document(&case, backend, combine))
                                .unwrap_or_else(|e| panic!("{label}: {e}"));
                            let path = dir.join(format!("{label}.ofrust"));
                            persistence::save(&session.document, &path)?;
                            let reopened = Session::new(Document::from_json(
                                &std::fs::read_to_string(&path)?,
                            )?)?;
                            assert_eq!(reopened.document, session.document);
                            let info = reopened.model.info();
                            assert_eq!(info.len(), case.volumes.len());
                            let volume: f64 = info.iter().map(|b| b.volume).sum();
                            let area: f64 = info.iter().map(|b| b.surface_area).sum();
                            close(volume, case.volumes.iter().sum());
                            close(area, case.areas.iter().sum());
                            let mut bodies = vec![];
                            for (body, info) in reopened.model.bodies.iter().zip(info) {
                                let i = case
                                    .volumes
                                    .iter()
                                    .position(|v| (*v - info.volume).abs() < 1e-5)
                                    .unwrap();
                                close(info.surface_area, case.areas[i]);
                                for (a, b) in info
                                    .bounds
                                    .iter()
                                    .flatten()
                                    .zip(case.bounds[i].iter().flatten())
                                {
                                    assert!((a - b).abs() < 2e-5);
                                }
                                let topology = if let Geometry::Rust(s) = &body.solid {
                                    s.topology().validate()?;
                                    assert_eq!(s.topology().shell_count(), case.shells[i]);
                                    serde_json::json!({"closed_manifold":true,"shell_count":s.topology().shell_count(),"vertices":s.topology().vertices.len(),"edges":s.topology().edges.len(),"faces":s.topology().faces.len(),"representation":format!("{:?}",s.representation()),"face_ancestry":s.face_ancestry(),"boundary_ancestry":s.boundary_ancestry(),"signature":s.output_signature()})
                                } else {
                                    serde_json::Value::Null
                                };
                                bodies.push(serde_json::json!({"id":info.id,"backend":info.backend,"expected_volume_mm3":case.volumes[i],"volume_mm3":info.volume,"expected_area_mm2":case.areas[i],"area_mm2":info.surface_area,"expected_bounds_mm":case.bounds[i],"bounds_mm":info.bounds,"expected_shells":case.shells[i],"topology":topology}));
                            }
                            let step =
                                if backend == Backend::Occt && !reopened.model.bodies.is_empty() {
                                    let bytes = reopened.model.step_bytes()?;
                                    let step = Model::step_volume(&bytes)?;
                                    close(step, volume);
                                    std::fs::write(dir.join(format!("{label}.step")), bytes)?;
                                    Some(step)
                                } else {
                                    None
                                };
                            records.push(serde_json::json!({"fixture":path,"source_base":String::from_utf8_lossy(&source.stdout).trim(),"case":case.name,"backend":backend,"combine":combine,"axis_permutation":permutation,"reverse_extrusion":reverse_distance,"reverse_profile":reverse_profile,"parameters":serde_json::from_str::<serde_json::Value>(&reopened.document.to_json()?)?,"body_count":reopened.model.bodies.len(),"expected_volume_mm3":case.volumes.iter().sum::<f64>(),"volume_mm3":volume,"expected_area_mm2":case.areas.iter().sum::<f64>(),"area_mm2":area,"bodies":bodies,"saved_reopened":true,"display_triangles":reopened.model.display_mesh()?.indices.len()/3,"independent_occt_step_volume_mm3":step,"measurement_tolerance":1e-5,"coincidence_mm":fission180_core::planar::COINCIDENCE_MM,"resolution_mm":fission180_core::planar::RESOLUTION_MM}));
                            measurements.push((volume, area));
                        }
                    }
                    for m in measurements.windows(2) {
                        close(m[0].0, m[1].0);
                        close(m[0].1, m[1].1);
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
        "Saved/reopened {} analytic planar-face fixtures; all body/shell, area, volume, bounds and independent OCCT checks passed",
        records.len()
    );
    Ok(())
}
