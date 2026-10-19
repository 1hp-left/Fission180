use super::*;

#[test]
fn dimension_undo_redo_retains_rectangle_id_expression_and_constraints() {
    let mut original = Sketch {
        id: 1,
        ..Default::default()
    };
    original
        .add_profile(Profile::Rectangle {
            a: [0., 0.],
            b: [40., 20.],
        })
        .unwrap();
    let mut history = SketchHistory::default();
    history.record(&original);
    let mut draft = original
        .with_dimensions(
            1,
            std::collections::BTreeMap::from([
                ("width".into(), "2 * 40 mm".into()),
                ("height".into(), "20 mm".into()),
            ]),
            &Default::default(),
        )
        .unwrap();
    let changed = draft.clone();
    assert_eq!(draft.profiles.len(), 1);
    assert_eq!(draft.profile_ids, vec![1]);
    history.undo(&mut draft);
    assert_eq!(draft, original);
    assert_eq!(draft.profiles.len(), 1);
    history.redo(&mut draft);
    assert_eq!(draft, changed);
    assert_eq!(draft.dimension_expressions[&1]["width"], "2 * 40 mm");
    assert!(draft.graph.as_ref().unwrap().constraints.len() > 4);
}

#[test]
fn complete_control_chord_is_recognized_after_control_is_released() {
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        events: vec![
            egui::Event::ModifiersChanged(egui::Modifiers::CTRL),
            egui::Event::Key {
                key: Key::Z,
                physical_key: Some(Key::Z),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::CTRL,
            },
            egui::Event::Key {
                key: Key::Z,
                physical_key: Some(Key::Z),
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::CTRL,
            },
            egui::Event::ModifiersChanged(egui::Modifiers::NONE),
        ],
        ..Default::default()
    };
    let mut recognized = false;
    let mut output = ctx.run_pass(input, |ctx| {
        assert!(!ctx.input(|i| i.modifiers.ctrl));
        recognized |= ctrl_shortcut(ctx, Key::Z);
    });
    output.textures_delta.clear();
    assert!(
        recognized,
        "Use the modifiers attached to each event, not the final frame's modifier state"
    );
}
