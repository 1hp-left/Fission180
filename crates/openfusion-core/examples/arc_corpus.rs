//! Persisted analytic arc fixtures. Sketch geometry executes independent Rust.
use fission180_core::{
    Document, Session, Sketch,
    arc::ArcGeometry,
    kernel::Backend,
    persistence,
    sketch::{Constraint, Entity},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Supply an output directory")?,
    );
    std::fs::create_dir_all(&dir)?;
    let parameters = std::collections::BTreeMap::from([("radius".into(), "40 mm".into())]);
    let mut s = Sketch {
        id: 1,
        name: "Analytic arcs".into(),
        ..Default::default()
    };
    s = s.with_three_point_arc([-40., 0.], [0., 40.], [40., 0.], &parameters)?;
    let arc = *s.graph.as_ref().unwrap().entities.keys().last().unwrap();
    let mut g = s.graph_for_edit()?;
    let Entity::Arc {
        center, start, end, ..
    } = g.entities[&arc]
    else {
        unreachable!()
    };
    g.constrain(Constraint::Fixed {
        point: center,
        at: [0., 0.],
    })?;
    for end in [start, end] {
        let radial = g.entity(Entity::Line { a: center, b: end })?;
        g.construction.insert(radial);
        g.constrain(Constraint::Horizontal { line: radial })?;
    }
    s = s
        .with_graph(g, &parameters)?
        .with_entity_dimension(arc, "radius".into(), &parameters)?;
    s = s.with_line([-15., 40.], [15., 40.], &parameters)?;
    let tangent = *s.graph.as_ref().unwrap().entities.keys().last().unwrap();
    s = s.with_constraints(
        vec![Constraint::Tangent {
            line: tangent,
            circle: arc,
        }],
        &parameters,
    )?;
    s = s.with_three_point_arc([-20., -10.], [0., -30.], [20., -10.], &parameters)?;
    let construction = *s.graph.as_ref().unwrap().entities.keys().last().unwrap();
    s = s.with_construction(&[construction], true, &parameters)?;
    s = s.with_three_point_arc(
        [70., 0.],
        [
            50. + 20. * std::f64::consts::FRAC_1_SQRT_2,
            20. * std::f64::consts::FRAC_1_SQRT_2,
        ],
        [50., 20.],
        &parameters,
    )?;
    let mut records = vec![];
    for backend in [Backend::Occt, Backend::Rust] {
        let name = format!(
            "arcs-{}",
            if backend == Backend::Rust {
                "rust"
            } else {
                "occt"
            }
        );
        let d = Document {
            name: name.clone(),
            backend,
            sketches: vec![s.clone()],
            parameters: parameters.clone(),
            ..Default::default()
        };
        let accepted = Session::new(d)?;
        let path = dir.join(format!("{name}.ofrust"));
        persistence::save(&accepted.document, &path)?;
        let opened = Session::new(Document::from_json(&std::fs::read_to_string(&path)?)?)?;
        assert_eq!(accepted.document, opened.document);
        let g = opened.document.sketches[0].graph.as_ref().unwrap();
        let curves:Vec<_>=g.entities.keys().filter_map(|id|g.arc_geometry(*id).ok().map(|a:ArcGeometry|serde_json::json!({"entity":id,"radius_mm":a.radius,"sweep_rad":a.sweep,"length_mm":a.length(),"construction":g.construction.contains(id)}))).collect();
        assert_eq!(curves.len(), 3);
        records.push(serde_json::json!({"fixture":path,"document_solid_backend":backend,"executing_sketch_engine":"independent Rust analytic","saved_reopened":true,"arcs":curves}));
    }
    std::fs::write(
        dir.join("measurements.json"),
        serde_json::to_string_pretty(&records)?,
    )?;
    println!(
        "Saved/reopened 2 fixtures with 3 analytic arcs, named radius, tangent line and construction geometry"
    );
    Ok(())
}
