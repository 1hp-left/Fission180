use fission180_core::{Document, Model};

#[test]
fn rectangle_extrusion_matches_measured_fusion_result() {
    // RUNTIME-003 in docs/runtime-extrude-input.md records Fusion's 4800.00 mm³
    // result for a 40 x 20 mm profile and the accepted input `2 * 3 mm`.
    // The fixture stores its evaluated distance; expression-entry parity is separate.
    let document = Document::from_json(include_str!(
        "../../../examples/fusion-reference-extrude.ofrust"
    ))
    .unwrap();
    let model = Model::build(&document).unwrap();
    let bodies = model.info();
    assert_eq!(bodies.len(), 1);
    // Fusion displayed two decimal places; do not infer finer measurement accuracy.
    assert!((bodies[0].volume - 4800.0).abs() < 0.005);
    assert!((bodies[0].surface_area - 2320.0).abs() < 0.005);
    for (axis, expected) in [40.0, 20.0, 6.0].into_iter().enumerate() {
        let extent = bodies[0].bounds[1][axis] - bodies[0].bounds[0][axis];
        assert!((extent - expected).abs() < 1e-5);
    }
    let step_volume = Model::step_volume(&model.step_bytes().unwrap()).unwrap();
    assert!((step_volume - bodies[0].volume).abs() < 1e-5);
}
