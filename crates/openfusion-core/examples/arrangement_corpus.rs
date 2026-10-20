//! Saved/reopened line arrangement and editing fixtures; no Autodesk assets.
use fission180_core::{
    Document, Extrusion, Model, Operation, Profile, Session, Sketch, kernel::Backend, persistence,
};
fn save(
    dir: &std::path::Path,
    name: &str,
    d: Document,
) -> Result<Session, Box<dyn std::error::Error>> {
    let accepted = Session::new(d)?;
    let path = dir.join(format!("{name}.ofrust"));
    persistence::save(&accepted.document, &path)?;
    let opened = Session::new(Document::from_json(&std::fs::read_to_string(path)?)?)?;
    assert_eq!(accepted.document, opened.document);
    Ok(opened)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Supply an output directory")?,
    );
    std::fs::create_dir_all(&dir)?;
    let mut s = Sketch {
        id: 1,
        name: "Crossing line cells".into(),
        ..Default::default()
    };
    s.add_profile(Profile::Rectangle {
        a: [-40., -25.],
        b: [40., 25.],
    })?;
    s = s.with_line([0., -35.], [0., 35.], &Default::default())?;
    s = s.with_line([-50., 0.], [50., 0.], &Default::default())?;
    let regions = s.regions()?;
    let mut records = vec![];
    for backend in [Backend::Occt, Backend::Rust] {
        let name = format!(
            "arrangement-{}",
            if backend == Backend::Rust {
                "rust"
            } else {
                "occt"
            }
        );
        let mut d = Document {
            name: name.clone(),
            backend,
            sketches: vec![s.clone()],
            ..Default::default()
        };
        for (i, region) in regions.iter().enumerate() {
            d.push_extrusion(Extrusion {
                id: 2 + i as u64 * 2,
                name: format!("Cell {}", i + 1),
                sketch_id: 1,
                profile: i,
                region_id: Some(region.id),
                distance: 5.,
                distance_expression: None,
                operation: Operation::NewBody,
                body_id: 3 + i as u64 * 2,
                result_body_ids: vec![],
                boundary_dependencies: None,
                options: Default::default(),
            });
        }
        let opened = save(&dir, &name, d)?;
        let volumes: Vec<_> = opened.model.info().iter().map(|b| b.volume).collect();
        assert_eq!(volumes.len(), 4);
        assert!(volumes.iter().all(|v| (v - 5000.).abs() < 1e-5));
        let step = if backend == Backend::Occt {
            let bytes = opened.model.step_bytes()?;
            let volume = Model::step_volume(&bytes)?;
            assert!((volume - 20000.).abs() < 1e-5);
            std::fs::write(dir.join(format!("{name}.step")), bytes)?;
            Some(volume)
        } else {
            None
        };
        records.push(serde_json::json!({"fixture":name,"backend":backend,"saved_reopened":true,"region_ids":s.profile_ids,"body_volumes_mm3":volumes,"step_reimport_volume_mm3":step}));
    }
    let mut s = Sketch {
        id: 1,
        name: "Trim and Extend".into(),
        ..Default::default()
    };
    for (a, b) in [
        ([-40., 0.], [40., 0.]),
        ([-15., -25.], [-15., 25.]),
        ([15., -25.], [15., 25.]),
    ] {
        s = s.with_line(a, b, &Default::default())?;
    }
    let source = *s.graph.as_ref().unwrap().entities.keys().next().unwrap();
    s = s.with_entity_dimension(source, "80 mm".into(), &Default::default())?;
    let trimmed = s.with_trimmed_line(source, [0., 0.], &Default::default())?;
    let remnant = *trimmed
        .graph
        .as_ref()
        .unwrap()
        .entities
        .keys()
        .last()
        .unwrap();
    let extended = trimmed.with_extended_line(remnant, [16., 0.], &Default::default())?;
    for (name, s) in [
        ("line-tools-before", s),
        ("line-tools-trimmed", trimmed),
        ("line-tools-extended", extended),
    ] {
        let opened = save(
            &dir,
            name,
            Document {
                name: name.into(),
                backend: Backend::Rust,
                sketches: vec![s],
                ..Default::default()
            },
        )?;
        records.push(serde_json::json!({"fixture":name,"backend":Backend::Rust,"saved_reopened":true,"entities":opened.document.sketches[0].graph.as_ref().unwrap().entities.len()}));
    }
    std::fs::write(
        dir.join("measurements.json"),
        serde_json::to_string_pretty(&records)?,
    )?;
    println!(
        "Saved/reopened 5 fixtures; four 5000 mm³ cells on each explicit backend; OCCT STEP reimport 20000 mm³"
    );
    Ok(())
}
