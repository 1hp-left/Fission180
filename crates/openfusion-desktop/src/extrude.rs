use super::*;
use fission180_core::extrusion::{Direction, Extent};
/// The same draft-to-document path is used by preview and OK in the native UI.
pub(super) fn edited_document(document: &Document, dialog: &ExtrudeDialog) -> Option<Document> {
    let distance =
        fission180_core::expression::length(&dialog.distance, &document.parameters).ok()?;
    if distance.abs() < 1e-5 {
        return None;
    }
    let region_id = *document
        .sketches
        .iter()
        .find(|s| s.id == dialog.sketch_id)?
        .profile_ids
        .get(dialog.profile)?;
    let original = dialog
        .editing
        .and_then(|id| document.extrusions.iter().find(|e| e.id == id));
    let feature = Extrusion {
        boundary_dependencies: original
            .filter(|e| e.sketch_id == dialog.sketch_id && e.region_id == Some(region_id))
            .and_then(|e| e.boundary_dependencies.clone()),
        result_body_ids: original
            .map(|e| e.result_body_ids.clone())
            .unwrap_or_default(),
        options: dialog.options.clone(),
        id: dialog.editing.or_else(|| document.next_id().ok())?,
        name: original
            .map(|e| e.name.clone())
            .unwrap_or_else(|| format!("Extrude{}", document.extrusions.len() + 1)),
        sketch_id: dialog.sketch_id,
        profile: dialog.profile,
        distance,
        operation: dialog.operation,
        body_id: dialog.body_id,
        region_id: Some(region_id),
        distance_expression: Some(dialog.distance.clone()),
    };
    let mut d = document.clone();
    if let Some(id) = dialog.editing {
        *d.extrusions.iter_mut().find(|e| e.id == id)? = feature;
    } else {
        d.push_extrusion(feature);
    }
    Some(d)
}
impl FusionApp {
    pub(super) fn edit_extrusion(&mut self, id: u64) {
        if self.busy {
            return;
        }
        let Some(e) = self.document.extrusions.iter().find(|e| e.id == id) else {
            return;
        };
        let distance = e
            .distance_expression
            .clone()
            .unwrap_or_else(|| e.distance.to_string());
        let Some(profile) = self
            .document
            .sketches
            .iter()
            .find(|s| s.id == e.sketch_id)
            .and_then(|s| s.profile_ids.iter().position(|id| Some(*id) == e.region_id))
        else {
            return;
        };
        self.modeling = None;
        self.selected = Some((e.sketch_id, profile));
        self.extrude = Some(ExtrudeDialog {
            options: e.options.clone(),
            editing: Some(e.id),
            sketch_id: e.sketch_id,
            profile,
            distance: distance.clone(),
            operation: e.operation,
            body_id: e.body_id,
            last_preview: String::new(),
            input: fission180_core::expression::DraftInput::new(
                distance,
                &self.document.parameters,
            ),
        });
    }
    pub(super) fn extrude_palette(&mut self, ui: &mut egui::Ui, w: f32) {
        let Some(mut dialog) = self.extrude.take() else {
            return;
        };
        let x = w - 254.;
        let y = 258.;
        let start_extra = if dialog.options.offset_start { 30. } else { 0. };
        let side_extra = if dialog.options.direction == Direction::TwoSides {
            30.
        } else {
            0.
        };
        let extra = start_extra + side_extra;
        let footer = if dialog.operation == Operation::NewBody {
            282.
        } else {
            312.
        } + extra;
        let p = ui.painter().clone();
        p.rect(
            rect(x, y, 250., footer + 32.),
            0.,
            PANEL,
            Stroke::new(1., BORDER),
            egui::StrokeKind::Inside,
        );
        label(&p, pos2(x + 12., y + 12.), "−  EXTRUDE", 12., TEXT);
        p.line_segment(
            [pos2(x, y + 25.), pos2(w - 4., y + 25.)],
            Stroke::new(1., BORDER),
        );
        label(&p, pos2(x + 12., y + 40.), "Type", 12., TEXT);
        tool_button(
            ui,
            rect(x + 92., y + 27., 27., 25.),
            Icon::Extrude,
            "Solid",
            true,
            true,
        );
        tool_button(
            ui,
            rect(x + 123., y + 27., 25., 25.),
            Icon::Shell,
            "Thin Extrude",
            false,
            false,
        );
        label(&p, pos2(x + 12., y + 70.), "Profiles", 12., TEXT);
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(rect(x + 93., y + 57., 145., 27.)),
            |ui| {
                egui::ComboBox::from_id_salt("extrude-profile")
                    .selected_text("1 selected")
                    .width(125.)
                    .show_ui(ui, |ui| {
                        for s in &self.document.sketches {
                            if let Ok(regions) = s.regions() {
                                for region in regions {
                                    if let Some(index) =
                                        s.profile_ids.iter().position(|id| *id == region.id)
                                    {
                                        let selected =
                                            dialog.sketch_id == s.id && dialog.profile == index;
                                        if ui
                                            .selectable_label(
                                                selected,
                                                format!("{} / Region{}", s.name, region.id),
                                            )
                                            .clicked()
                                        {
                                            dialog.sketch_id = s.id;
                                            dialog.profile = index;
                                        }
                                    }
                                }
                            }
                        }
                    });
            },
        );
        label(&p, pos2(x + 12., y + 100.), "Start", 12., TEXT);
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(rect(x + 93., y + 87., 145., 27.)),
            |ui| {
                egui::ComboBox::from_id_salt("extrude-start")
                    .selected_text(if dialog.options.offset_start {
                        "Offset"
                    } else {
                        "Profile Plane"
                    })
                    .width(125.)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut dialog.options.offset_start,
                            false,
                            "Profile Plane",
                        );
                        ui.selectable_value(&mut dialog.options.offset_start, true, "Offset");
                    });
            },
        );
        if dialog.options.offset_start {
            label(&p, pos2(x + 12., y + 130.), "Offset", 12., TEXT);
            ui.put(
                rect(x + 93., y + 117., 125., 27.),
                egui::TextEdit::singleline(&mut dialog.options.start_offset),
            );
        }
        label(
            &p,
            pos2(x + 12., y + 130. + start_extra),
            "Direction",
            12.,
            TEXT,
        );
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(rect(x + 93., y + 117. + start_extra, 145., 27.)),
            |ui| {
                egui::ComboBox::from_id_salt("extrude-direction")
                    .selected_text(dialog.options.direction.label())
                    .width(125.)
                    .show_ui(ui, |ui| {
                        for direction in [
                            Direction::OneSide,
                            Direction::TwoSides,
                            Direction::Symmetric,
                        ] {
                            ui.selectable_value(
                                &mut dialog.options.direction,
                                direction,
                                direction.label(),
                            );
                        }
                    })
                    .response
                    .on_hover_text(
                        "Symmetric uses the entered distance as the whole extrusion length",
                    );
            },
        );
        label(
            &p,
            pos2(x + 12., y + 160. + start_extra),
            "Extent Type",
            12.,
            TEXT,
        );
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(rect(x + 93., y + 147. + start_extra, 145., 27.)),
            |ui| {
                egui::ComboBox::from_id_salt("extrude-extent")
                    .selected_text(dialog.options.extent.label())
                    .width(125.)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut dialog.options.extent,
                            Extent::Distance,
                            "Distance",
                        );
                        if ui
                            .add_enabled(
                                dialog.operation != Operation::NewBody,
                                egui::Button::selectable(
                                    dialog.options.extent == Extent::ThroughAll,
                                    "All",
                                ),
                            )
                            .clicked()
                        {
                            dialog.options.extent = Extent::ThroughAll;
                        }
                    });
            },
        );
        label(
            &p,
            pos2(x + 12., y + 190. + start_extra),
            if dialog.options.extent == Extent::ThroughAll {
                "Direction sign"
            } else {
                "Distance"
            },
            12.,
            TEXT,
        );
        let response = ui.put(
            rect(x + 93., y + 177. + start_extra, 125., 27.),
            egui::TextEdit::singleline(&mut dialog.distance),
        );
        dialog
            .input
            .update(dialog.distance.clone(), &self.document.parameters);
        let feedback = match &dialog.input.state {
            fission180_core::expression::InputState::Incomplete => {
                "Incomplete expression; last valid preview retained".into()
            }
            fission180_core::expression::InputState::Error(e) => e.clone(),
            fission180_core::expression::InputState::Valid(v) => format!("{} mm", v.millimetres),
        };
        response.on_hover_text(feedback);
        if dialog.options.direction == Direction::TwoSides {
            label(
                &p,
                pos2(x + 12., y + 220. + start_extra),
                "Distance 2",
                12.,
                TEXT,
            );
            ui.put(
                rect(x + 93., y + 207. + start_extra, 125., 27.),
                egui::TextEdit::singleline(&mut dialog.options.second_distance),
            );
        }
        label(
            &p,
            pos2(x + 12., y + 220. + extra),
            "Taper Angle",
            12.,
            MUTED,
        );
        ui.put(
            rect(x + 93., y + 207. + extra, 125., 27.),
            egui::Label::new("0.0 deg").sense(Sense::hover()),
        )
        .on_hover_text("Tapered extrusion is not supported yet");
        p.line_segment(
            [
                pos2(x + 10., y + 242. + extra),
                pos2(w - 15., y + 242. + extra),
            ],
            Stroke::new(0.7, BORDER),
        );
        label(&p, pos2(x + 12., y + 260. + extra), "Operation", 12., TEXT);
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(rect(x + 93., y + 247. + extra, 145., 27.)),
            |ui| {
                egui::ComboBox::from_id_salt("operation")
                    .selected_text(dialog.operation.label())
                    .width(125.)
                    .show_ui(ui, |ui| {
                        for operation in [
                            Operation::NewBody,
                            Operation::Join,
                            Operation::Cut,
                            Operation::Intersect,
                        ] {
                            if ui
                                .selectable_value(
                                    &mut dialog.operation,
                                    operation,
                                    operation.label(),
                                )
                                .changed()
                            {
                                dialog.body_id = if operation == Operation::NewBody {
                                    self.document
                                        .next_id()
                                        .ok()
                                        .and_then(|id| id.checked_add(1))
                                        .unwrap_or(0)
                                } else {
                                    self.committed_bodies.first().map(|b| b.id).unwrap_or(0)
                                };
                            }
                        }
                    });
            },
        );
        if dialog.operation != Operation::NewBody {
            label(
                &p,
                pos2(x + 12., y + 290. + extra),
                "Target Body",
                12.,
                TEXT,
            );
            ui.scope_builder(
                egui::UiBuilder::new().max_rect(rect(x + 93., y + 277. + extra, 145., 27.)),
                |ui| {
                    egui::ComboBox::from_id_salt("target-body")
                        .selected_text(format!("Body{}", dialog.body_id))
                        .width(125.)
                        .show_ui(ui, |ui| {
                            for b in &self.committed_bodies {
                                ui.selectable_value(&mut dialog.body_id, b.id, &b.name);
                            }
                        });
                },
            );
        }
        let ok = button(ui, rect(x + 115., y + footer, 45., 24.), "extrude-ok", "OK");
        let cancel = button(
            ui,
            rect(x + 172., y + footer, 64., 24.),
            "extrude-cancel",
            "Cancel",
        );
        let signature = format!(
            "{}:{:?}:{}:{:?}:{}:{}",
            dialog.distance,
            dialog.operation,
            dialog.body_id,
            dialog.options,
            dialog.sketch_id,
            dialog.profile
        );
        let changed = signature != dialog.last_preview;
        dialog.last_preview = signature;
        self.extrude = Some(dialog);
        if cancel {
            self.extrude = None;
            self.send(Command::ResetPreview);
        } else if ok {
            if let Some(d) = self.extrusion_document() {
                self.send(Command::Replace(d));
                self.extrude = None;
                self.selected = None;
            } else {
                self.error = Some("Enter valid extrusion dimensions".into());
            }
        } else if changed {
            if let Some(d) = self.extrusion_document() {
                self.send(Command::Preview(d));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission180_core::{Model, Session};
    #[test]
    fn native_extrude_editor_replays_rust_pockets_without_rebinding_boundary_dependencies() {
        let mut session =
            Session::new(crate::engine_examples::Example::Pockets.document().unwrap()).unwrap();
        let original = session.document.extrusions[2].clone();
        let dialog = ExtrudeDialog {
            editing: Some(original.id),
            sketch_id: original.sketch_id,
            profile: 0,
            distance: "-4 mm".into(),
            operation: Operation::Cut,
            body_id: 3,
            last_preview: String::new(),
            input: fission180_core::expression::DraftInput::new(
                "-4 mm".into(),
                &session.document.parameters,
            ),
            options: original.options.clone(),
        };
        let edited = edited_document(&session.document, &dialog).unwrap();
        let volume = session.model.info()[0].volume;
        assert_eq!(
            edited.extrusions[2].boundary_dependencies,
            original.boundary_dependencies
        );
        assert_eq!(
            edited.extrusions[2].result_body_ids,
            original.result_body_ids
        );
        session
            .replace_with_display(edited, Model::display_mesh)
            .unwrap();
        assert!(session.model.info()[0].volume > volume);
        let accepted = session.document.clone();
        let mut changed = accepted.clone();
        changed.sketches[2]
            .add_profile(Profile::Circle {
                center: [5., 0.],
                radius: 2.,
            })
            .unwrap();
        let candidate = edited_document(&changed, &dialog).unwrap();
        assert!(
            session
                .replace_with_display(candidate, Model::display_mesh)
                .is_err()
        );
        assert_eq!(session.document, accepted);
    }
    #[test]
    fn native_editor_preserves_new_axial_example_references_during_depth_edits() {
        for (example, index, value) in [
            (crate::engine_examples::Example::Breakthrough, 2, "5.5 mm"),
            (crate::engine_examples::Example::UnequalJoin, 1, "9 mm"),
        ] {
            let mut session = Session::new(example.document().unwrap()).unwrap();
            let original = session.document.extrusions[index].clone();
            let dialog = ExtrudeDialog {
                editing: Some(original.id),
                sketch_id: original.sketch_id,
                profile: 0,
                distance: value.into(),
                operation: original.operation,
                body_id: original.body_id,
                last_preview: String::new(),
                input: fission180_core::expression::DraftInput::new(
                    value.into(),
                    &session.document.parameters,
                ),
                options: original.options.clone(),
            };
            let edited = edited_document(&session.document, &dialog).unwrap();
            assert_eq!(
                edited.extrusions[index].boundary_dependencies,
                original.boundary_dependencies
            );
            assert_eq!(
                edited.extrusions[index].result_body_ids,
                original.result_body_ids
            );
            session
                .replace_with_display(edited, Model::display_mesh)
                .unwrap();
            assert_eq!(session.model.bodies.len(), 1);
            session.undo().unwrap();
            session.redo().unwrap();
        }
    }
}
