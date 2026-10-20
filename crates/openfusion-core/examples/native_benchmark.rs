use fission180_core::{Document, Extrusion, Model, Operation, Plane, Profile, Sketch};
use std::time::Instant;
fn main() {
    let mut doc = Document::default();
    doc.sketches.push(Sketch {
        id: 1,
        name: "Rectangle".into(),
        plane: Plane::XY,
        origin: [0.; 3],
        profiles: vec![Profile::Rectangle {
            a: [0., 0.],
            b: [40., 30.],
        }],

        profile_ids: vec![1],
        next_profile_id: 2,
        dimension_expressions: Default::default(),
        graph: None,
        support: None,
    });
    doc.extrusions.push(Extrusion {
        id: 2,
        name: "Extrude".into(),
        sketch_id: 1,
        profile: 0,
        distance: 5.,
        operation: Operation::NewBody,
        body_id: 3,

        region_id: Some(1),
        distance_expression: None,
        result_body_ids: vec![],
    });
    doc.sketches.push(Sketch {
        id: 4,
        name: "Circle".into(),
        plane: Plane::XY,
        origin: [0.; 3],
        profiles: vec![Profile::Circle {
            center: [20., 15.],
            radius: 5.,
        }],

        profile_ids: vec![1],
        next_profile_id: 2,
        dimension_expressions: Default::default(),
        graph: None,
        support: None,
    });
    doc.extrusions.push(Extrusion {
        id: 5,
        name: "Cut".into(),
        sketch_id: 4,
        profile: 0,
        distance: 5.,
        operation: Operation::Cut,
        body_id: 3,

        region_id: Some(1),
        distance_expression: None,
        result_body_ids: vec![],
    });
    let expected = 6000. - 125. * std::f64::consts::PI;
    let mut timings = vec![];
    for _ in 0..21 {
        let start = Instant::now();
        let model = Model::build(&doc).expect("build model");
        let mesh = model.display_mesh().expect("mesh");
        assert!(!mesh.indices.is_empty());
        assert!((model.info()[0].volume - expected).abs() < 1e-5);
        timings.push(start.elapsed().as_secs_f64() * 1000.);
    }
    let cold = timings.remove(0);
    timings.sort_by(f64::total_cmp);
    let model = Model::build(&doc).unwrap();
    let step = model.step_bytes().unwrap();
    assert!((Model::step_volume(&step).unwrap() - expected).abs() < 1e-5);
    println!(
        "{}",
        serde_json::json!({"build":"native Rust + OpenCascade 8.0.1","workload":"40 x 30 x 5 mm extrusion, 10 mm bore cut, tessellation","warm_iterations":20,"cold_ms":cold,"median_ms":timings[10],"min_ms":timings[0],"max_ms":timings[19],"volume_mm3":expected,"step_roundtrip":true})
    );
}
