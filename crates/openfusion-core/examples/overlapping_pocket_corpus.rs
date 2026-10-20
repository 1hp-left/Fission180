//! Saved/reopened overlapping pocket fixtures with explicit independent OCCT oracles.
#[path = "support/prism_fixtures.rs"]
#[allow(dead_code)]
mod fixtures;
use fission180_core::{
    Document, Model, Operation, Plane, Session, Sketch,
    kernel::{Backend, Geometry},
    persistence,
};
use fixtures::*;
use std::f64::consts::PI;
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
    let lens = 200. * PI / 3. - 50. * 3f64.sqrt();
    for (da, db) in [(3f64, 3f64), (3., 5.), (5., 3.)] {
        for plane in [Plane::XY, Plane::XZ, Plane::YZ] {
            for negative in [false, true] {
                for high in [false, true] {
                    for backend in [Backend::Rust, Backend::Occt] {
                        let label = format!(
                            "pockets-{da}-{db}-{plane:?}-negative{negative}-high{high}-{backend:?}"
                        );
                        let mut a = input(rect(-30., -30., 60., 60.), vec![], 2);
                        a.plane = plane;
                        a.origin = [13., -17., 9.];
                        a.distance = if negative { -10. } else { 10. };
                        let n = plane.normal().to_array();
                        let interval = if negative { [-10., 0.] } else { [0., 10.] };
                        let opening = if high { interval[1] } else { interval[0] };
                        let tool = |x: f64, depth: f64, id: u64| {
                            let mut b = input(disk(0., 10.), vec![], id);
                            b.plane = plane;
                            let delta = plane.point([x, 0.], [0.; 3]).to_array();
                            b.origin =
                                std::array::from_fn(|i| a.origin[i] + delta[i] + n[i] * opening);
                            b.distance = if high { -depth } else { depth };
                            b
                        };
                        let b = tool(0., da, 5);
                        let c = tool(10., db, 8);
                        let mut d = document(&a, &b, Operation::Cut, backend, false);
                        let mut sketch = Sketch {
                            id: 7,
                            plane,
                            origin: c.origin,
                            ..Default::default()
                        };
                        sketch.add_profile(c.outer.clone())?;
                        d.sketches.push(sketch);
                        d.extrusions
                            .push(extrusion(8, 7, 3, c.distance, Operation::Cut));
                        let session = Session::new(d)?;
                        let path = dir.join(format!("{label}.ofrust"));
                        persistence::save(&session.document, &path)?;
                        let reopened =
                            Session::new(Document::from_json(&std::fs::read_to_string(&path)?)?)?;
                        assert_eq!(session.document, reopened.document);
                        assert_eq!(reopened.model.bodies.len(), 1);
                        let expected = 36000. - 100. * PI * (da + db) + lens * da.min(db);
                        let area = 9600. + 20. * PI * (da - db).abs() + 80. * PI / 3. * da.min(db);
                        let info = &reopened.model.info()[0];
                        assert!((info.volume - expected).abs() < 1e-5);
                        assert!((info.surface_area - area).abs() < 1e-5);
                        let topology = if let Geometry::Rust(s) = &reopened.model.bodies[0].solid {
                            s.topology().validate()?;
                            serde_json::json!({"closed_manifold":true,"vertices":s.topology().vertices.len(),"edges":s.topology().edges.len(),"faces":s.topology().faces.len(),"sections":s.occupied_sections().unwrap().iter().map(|(z,r)|serde_json::json!({"interval_mm":z,"area_mm2":r.area(),"region_count":r.regions.len(),"boundary_keys":r.regions.iter().map(|r|r.key()).collect::<Vec<_>>()})).collect::<Vec<_>>()})
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
                        records.push(serde_json::json!({"fixture":path,"source_base":String::from_utf8_lossy(&revision.stdout).trim(),"backend":backend,"input":{"plane":plane,"origin_mm":a.origin,"distance_mm":a.distance,"radii_mm":[10.,10.],"center_separation_mm":10.,"depths_mm":[da,db],"enters_high_cap":high},"expected_volume_mm3":expected,"volume_mm3":info.volume,"expected_area_mm2":area,"area_mm2":info.surface_area,"bounds_mm":info.bounds,"body_count":1,"topology":topology,"saved_reopened":true,"display_triangles":reopened.model.display_mesh()?.indices.len()/3,"occt_oracle_step_reimport_volume_mm3":step,"measurement_tolerance":1e-5,"coincidence_mm":fission180_core::planar::COINCIDENCE_MM,"resolution_mm":fission180_core::planar::RESOLUTION_MM}));
                    }
                }
            }
        }
    }
    std::fs::write(
        dir.join("measurements.json"),
        serde_json::to_string_pretty(&records)?,
    )?;
    println!(
        "Saved/reopened {} backend-tagged overlapping-pocket fixtures",
        records.len()
    );
    Ok(())
}
