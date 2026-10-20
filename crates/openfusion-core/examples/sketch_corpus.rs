//! Retained entity-edit fixture, saved/reopened on each explicit backend.
use fission180_core::{
    Document, Extrusion, Operation, Profile, Session, Sketch, kernel::Backend, persistence,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Supply an output directory")?,
    );
    std::fs::create_dir_all(&dir)?;
    let parameters = std::collections::BTreeMap::from([
        ("width".into(), "40 mm".into()),
        ("bore".into(), "10 mm".into()),
    ]);
    let mut s = Sketch {
        id: 1,
        name: "Constrained line region".into(),
        ..Default::default()
    };
    for (a, b) in [
        ([-20., -15.], [20., -15.]),
        ([20., 15.], [20., -15.]),
        ([20., 15.], [-20., 15.]),
        ([-20., -15.], [-20., 15.]),
    ] {
        s = s.with_line(a, b, &parameters)?;
    }
    let first = s.graph.as_ref().unwrap().profile_entities[&1][0];
    s = s.with_entity_dimension(first, "width * 2".into(), &parameters)?;
    s = s.with_line([-60., -30.], [-50., -20.], &parameters)?;
    let construction = *s.graph.as_ref().unwrap().entities.keys().last().unwrap();
    s = s.with_construction(&[construction], true, &parameters)?;
    let circle = s.add_profile(Profile::Circle {
        center: [55., 0.],
        radius: 5.,
    })?;
    let circle = s.graph.as_ref().unwrap().profile_entities[&circle][0];
    s = s.with_entity_dimension(circle, "bore / 2".into(), &parameters)?;
    let mut records = vec![];
    for backend in [Backend::Occt, Backend::Rust] {
        let d = Document {
            name: format!("Entity edit {}", backend.label()),
            backend,
            parameters: parameters.clone(),
            sketches: vec![s.clone()],
            extrusions: vec![Extrusion {
                id: 2,
                name: "Line region extrusion".into(),
                sketch_id: 1,
                profile: 0,
                region_id: Some(1),
                distance: 5.,
                distance_expression: None,
                operation: Operation::NewBody,
                body_id: 3,
                result_body_ids: vec![],
                options: Default::default(),
            }],
            ..Default::default()
        };
        let accepted = Session::new(d)?;
        let path = dir.join(format!(
            "entity-edit-{}.ofrust",
            if backend == Backend::Rust {
                "rust"
            } else {
                "occt"
            }
        ));
        persistence::save(&accepted.document, &path)?;
        let reopened = Session::new(Document::from_json(&std::fs::read_to_string(&path)?)?)?;
        assert_eq!(reopened.document, accepted.document);
        assert!((reopened.model.info()[0].volume - 12000.).abs() < 1e-4);
        records.push(serde_json::json!({"path":path,"backend":backend,"volume_mm3":reopened.model.info()[0].volume,"region_ids":s.profile_ids,"saved_reopened":true}));
    }
    std::fs::write(
        dir.join("measurements.json"),
        serde_json::to_string_pretty(&records)?,
    )?;
    println!("Saved and reopened 2 entity-edit fixtures");
    Ok(())
}
