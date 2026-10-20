//! Analytic acceptance fixtures; outputs belong outside the source tree.
#[path = "support/prism_fixtures.rs"]
mod fixtures;
use fission180_core::{
    Document, Model, Session,
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
    let revision = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()?;
    let mut records = vec![];
    for c in cases() {
        for combine in [false, true] {
            for backend in [Backend::Rust, Backend::Occt] {
                let label = format!(
                    "{}-{}-{backend:?}",
                    c.name,
                    if combine { "combine" } else { "extrusion" }
                );
                let session = Session::new(document(&c.a, &c.b, c.op, backend, combine))?;
                let path = dir.join(format!("{label}.ofrust"));
                persistence::save(&session.document, &path)?;
                let reopened =
                    Session::new(Document::from_json(&std::fs::read_to_string(&path)?)?)?;
                assert_eq!(session.document, reopened.document);
                let area: f64 = c.areas.iter().sum();
                let expected_volume = 5. * area;
                let expected_area = 2. * area + 5. * c.perimeter;
                let measured_volume: f64 = reopened.model.info().iter().map(|i| i.volume).sum();
                let measured_area: f64 = reopened.model.info().iter().map(|i| i.surface_area).sum();
                assert!((expected_volume - measured_volume).abs() < 1e-5);
                assert!((expected_area - measured_area).abs() < 1e-5);
                assert_eq!(reopened.model.bodies.len(), c.areas.len());
                let mut topology = vec![];
                for body in &reopened.model.bodies {
                    if let Geometry::Rust(s) = &body.solid {
                        s.topology().validate()?;
                        topology.push(serde_json::json!({"body":body.id,"closed_manifold":true,"vertices":s.topology().vertices.len(),"edges":s.topology().edges.len(),"faces":s.topology().faces.len(),"source_boundary_key":s.output_signature()}));
                    }
                }
                let step = if backend == Backend::Occt && !reopened.model.bodies.is_empty() {
                    let bytes = reopened.model.step_bytes()?;
                    std::fs::write(dir.join(format!("{label}.step")), &bytes)?;
                    Some(Model::step_volume(&bytes)?)
                } else {
                    None
                };
                records.push(serde_json::json!({"fixture":path,"source_base":String::from_utf8_lossy(&revision.stdout).trim(),"backend":backend,"operation":c.op,"input_a":{"outer":c.a.outer,"holes":c.a.holes,"distance_mm":c.a.distance},"input_b":{"outer":c.b.outer,"holes":c.b.holes,"distance_mm":c.b.distance},"coincidence_mm":fission180_core::planar::COINCIDENCE_MM,"resolution_mm":fission180_core::planar::RESOLUTION_MM,"measurement_tolerance":1e-5,"expected_volume_mm3":expected_volume,"volume_mm3":measured_volume,"expected_area_mm2":expected_area,"area_mm2":measured_area,"body_count":reopened.model.bodies.len(),"topology":topology,"saved_reopened":true,"display_generated":!reopened.model.display_mesh()?.indices.is_empty(),"occt_oracle_step_reimport_volume_mm3":step}));
            }
        }
    }
    std::fs::write(
        dir.join("measurements.json"),
        serde_json::to_string_pretty(&records)?,
    )?;
    println!(
        "Saved/reopened {} backend-tagged prism fixtures",
        records.len()
    );
    Ok(())
}
