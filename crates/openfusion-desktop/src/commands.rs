use super::*;
use fission180_core::features::{Axis, Feature, FeatureKind, RegionRef};

pub(super) struct ModelingDialog {
    tool: String,
    editing: Option<u64>,
    target: u64,
    secondary: u64,
    region: Option<RegionRef>,
    other_region: Option<RegionRef>,
    path_sketch: u64,
    axis: Axis,
    count: u32,
    positive: bool,
    copy: bool,
    operation: Operation,
    fields: Vec<(String, String)>,
    last_preview: String,
}
impl ModelingDialog {
    fn value(&self, i: usize) -> String {
        self.fields.get(i).map(|v| v.1.clone()).unwrap_or_default()
    }
    fn scalar(&self, i: usize) -> fission180_core::Result<f64> {
        self.value(i)
            .trim()
            .trim_end_matches("deg")
            .trim()
            .parse::<f64>()
            .map_err(|_| fission180_core::Error::Invalid("Enter an angle in degrees".into()))
    }
    fn region(&self) -> fission180_core::Result<RegionRef> {
        self.region
            .clone()
            .ok_or_else(|| fission180_core::Error::Invalid("Select a sketch region".into()))
    }
    fn feature(&self, d: &Document) -> fission180_core::Result<Feature> {
        let required = |i| fission180_core::expression::length(&self.value(i), &d.parameters);
        let kind = match self.tool.as_str() {
            "Revolve" => FeatureKind::Revolve {
                region: self.region()?,
                axis: self.axis,
                axis_origin: [0.; 3],
                degrees: self.scalar(0)?,
            },
            "Sweep" => {
                let s = d
                    .sketches
                    .iter()
                    .find(|s| s.id == self.path_sketch)
                    .ok_or_else(|| {
                        fission180_core::Error::Invalid(
                            "Create/select a sketch containing an open line path".into(),
                        )
                    })?;
                let graph = s.graph.as_ref().ok_or_else(|| {
                    fission180_core::Error::Invalid("Path sketch has no line entities".into())
                })?;
                let bound: std::collections::HashSet<_> =
                    graph.profile_entities.values().flatten().copied().collect();
                let path_entities = graph
                    .entities
                    .iter()
                    .filter_map(|(id, e)| {
                        (!bound.contains(id)
                            && matches!(e, fission180_core::sketch::Entity::Line { .. }))
                        .then_some(*id)
                    })
                    .collect();
                FeatureKind::Sweep {
                    region: self.region()?,
                    path_sketch: self.path_sketch,
                    path_entities,
                }
            }
            "Loft" => FeatureKind::Loft {
                sections: vec![
                    self.region()?,
                    self.other_region.clone().ok_or_else(|| {
                        fission180_core::Error::Invalid("Select a second sketch section".into())
                    })?,
                ],
                ruled: false,
            },
            "Hole" => FeatureKind::Hole {
                diameter: self.value(0),
                depth: self.value(1),
                center: [required(2)?, required(3)?],
                start_z: required(4)?,
            },
            "Fillet" => FeatureKind::Fillet {
                radius: self.value(0),
            },
            "Chamfer" => FeatureKind::Chamfer {
                distance: self.value(0),
            },
            "Shell" => FeatureKind::Shell {
                thickness: self.value(0),
                open_axis: self.axis,
                positive: self.positive,
            },
            "Press Pull" => FeatureKind::OffsetFace {
                distance: self.value(0),
                axis: self.axis,
                positive: self.positive,
            },
            "Move / Copy" => FeatureKind::Translate {
                offset: [self.value(0), self.value(1), self.value(2)],
                copy: self.copy,
            },
            "Mirror" => FeatureKind::Mirror {
                axis: self.axis,
                origin: [0.; 3],
            },
            "Rectangular Pattern" => FeatureKind::RectangularPattern {
                axis: self.axis,
                spacing: self.value(0),
                count: self.count,
            },
            "Circular Pattern" => FeatureKind::CircularPattern {
                axis: self.axis,
                origin: [0.; 3],
                degrees: self.scalar(0)?,
                count: self.count,
            },
            "Combine" => FeatureKind::Combine {
                tool_body: self.secondary,
                operation: self.operation,
                keep_tool: self.copy,
            },
            "Split Body" => FeatureKind::SplitBody {
                axis: self.axis,
                position: self.value(0),
            },
            _ => {
                return Err(fission180_core::Error::Invalid(
                    "Unknown modeling command".into(),
                ));
            }
        };
        let original = self
            .editing
            .and_then(|id| d.features.iter().find(|f| f.id == id));
        let feature = Feature {
            id: if let Some(id) = self.editing {
                id
            } else {
                d.next_id()?
            },
            name: original
                .map(|f| f.name.clone())
                .unwrap_or_else(|| format!("{}{}", self.tool, d.features.len() + 1)),
            target: if matches!(
                kind,
                FeatureKind::Revolve { .. } | FeatureKind::Sweep { .. } | FeatureKind::Loft { .. }
            ) {
                None
            } else {
                Some(self.target)
            },
            outputs: original.map(|f| f.outputs.clone()).unwrap_or_default(),
            kind,
        };
        feature.validate(d)?;
        Ok(feature)
    }
}
impl FusionApp {
    pub(super) fn start_modeling(&mut self, name: &str) {
        if self.mode == Mode::Sketch && name == "3-Point Arc" {
            self.start_arc();
            return;
        }
        if self.mode == Mode::Sketch && matches!(name, "Trim" | "Extend") {
            self.start_line_edit(name == "Extend");
            return;
        }
        if self.busy {
            return;
        }
        if name == "Extrude" {
            self.start_extrude();
            return;
        }
        if name == "Edit Extrude" {
            if let Some(id) = self.document.extrusions.last().map(|e| e.id) {
                self.edit_extrusion(id);
            }
            return;
        }
        if self.mode == Mode::Sketch && !matches!(name, "Change Parameters" | "Measure") {
            self.pending_modeling = Some(name.to_string());
            self.finish_sketch();
            return;
        }
        if self.extrude.take().is_some() || self.preview {
            self.send(Command::ResetPreview);
        }
        let target = self
            .selected_topology
            .map(|p| match p {
                Pick::Face(id) | Pick::Edge(id) => id.body,
                Pick::Body(id) => id,
            })
            .or_else(|| self.committed_bodies.first().map(|b| b.id))
            .unwrap_or(0);
        let bounds = self
            .committed_bodies
            .iter()
            .find(|b| b.id == target)
            .map(|b| b.bounds)
            .unwrap_or([[0.; 3], [40., 30., 5.]]);
        let region = self
            .selected
            .and_then(|(id, i)| {
                self.document
                    .sketches
                    .iter()
                    .find(|s| s.id == id)
                    .and_then(|s| s.profile_ids.get(i))
                    .map(|r| RegionRef {
                        sketch_id: id,
                        region_id: *r,
                    })
            })
            .or_else(|| {
                self.document.sketches.iter().rev().find_map(|s| {
                    s.profile_ids.first().map(|r| RegionRef {
                        sketch_id: s.id,
                        region_id: *r,
                    })
                })
            });
        let other_region = self
            .document
            .sketches
            .iter()
            .find(|s| {
                Some(s.id) != region.as_ref().map(|r| r.sketch_id) && !s.profile_ids.is_empty()
            })
            .map(|s| RegionRef {
                sketch_id: s.id,
                region_id: s.profile_ids[0],
            });
        let fields: Vec<(&str, String)> = match name {
            "Revolve" => vec![("Angle", "360".into())],
            "Hole" => vec![
                ("Diameter", "10 mm".into()),
                ("Depth", format!("{}", bounds[1][2] - bounds[0][2])),
                (
                    "Center X",
                    format!("{}", (bounds[0][0] + bounds[1][0]) * 0.5),
                ),
                (
                    "Center Y",
                    format!("{}", (bounds[0][1] + bounds[1][1]) * 0.5),
                ),
                ("Start Z", bounds[0][2].to_string()),
            ],
            "Fillet" => vec![("Radius", "1 mm".into())],
            "Chamfer" | "Press Pull" => vec![("Distance", "1 mm".into())],
            "Shell" => vec![("Thickness", "1 mm".into())],
            "Move / Copy" => vec![
                ("X Distance", "0 mm".into()),
                ("Y Distance", "0 mm".into()),
                ("Z Distance", "0 mm".into()),
            ],
            "Rectangular Pattern" => vec![("Spacing", "20 mm".into())],
            "Circular Pattern" => vec![("Angle", "360".into())],
            "Split Body" => vec![(
                "Position",
                format!("{}", (bounds[0][0] + bounds[1][0]) * 0.5),
            )],
            "Change Parameters" => vec![
                (
                    "Name",
                    self.document
                        .parameters
                        .keys()
                        .next()
                        .cloned()
                        .unwrap_or("length".into()),
                ),
                (
                    "Expression",
                    self.document
                        .parameters
                        .values()
                        .next()
                        .cloned()
                        .unwrap_or("10 mm".into()),
                ),
            ],
            _ => vec![],
        };
        self.modeling = Some(ModelingDialog {
            tool: name.into(),
            editing: None,
            target,
            secondary: self
                .committed_bodies
                .iter()
                .find(|b| b.id != target)
                .map(|b| b.id)
                .unwrap_or(0),
            region,
            other_region,
            path_sketch: self
                .document
                .sketches
                .iter()
                .find(|s| {
                    s.graph.as_ref().is_some_and(|g| {
                        g.entities
                            .values()
                            .any(|e| matches!(e, fission180_core::sketch::Entity::Line { .. }))
                    })
                })
                .map(|s| s.id)
                .unwrap_or(0),
            axis: if name == "Revolve" {
                Axis::Y
            } else if matches!(name, "Circular Pattern" | "Shell" | "Press Pull") {
                Axis::Z
            } else {
                Axis::X
            },
            count: 3,
            positive: true,
            copy: false,
            operation: Operation::Cut,
            fields: fields.into_iter().map(|(a, b)| (a.into(), b)).collect(),
            last_preview: String::new(),
        });
        self.error = None;
    }
    pub(super) fn edit_modeling(&mut self, id: u64) {
        let Some(f) = self.document.features.iter().find(|f| f.id == id).cloned() else {
            return;
        };
        self.start_modeling(f.kind.label());
        let Some(dialog) = &mut self.modeling else {
            return;
        };
        dialog.editing = Some(id);
        dialog.target = f.target.unwrap_or(0);
        let values: Vec<String> = match f.kind {
            FeatureKind::Revolve {
                region,
                axis,
                degrees,
                ..
            } => {
                dialog.region = Some(region);
                dialog.axis = axis;
                vec![degrees.to_string()]
            }
            FeatureKind::Sweep {
                region,
                path_sketch,
                ..
            } => {
                dialog.region = Some(region);
                dialog.path_sketch = path_sketch;
                vec![]
            }
            FeatureKind::Loft { sections, .. } => {
                dialog.region = sections.first().cloned();
                dialog.other_region = sections.get(1).cloned();
                vec![]
            }
            FeatureKind::Hole {
                center,
                start_z,
                diameter,
                depth,
            } => vec![
                diameter,
                depth,
                center[0].to_string(),
                center[1].to_string(),
                start_z.to_string(),
            ],
            FeatureKind::Fillet { radius } => vec![radius],
            FeatureKind::Chamfer { distance } => vec![distance],
            FeatureKind::Shell {
                thickness,
                open_axis,
                positive,
            } => {
                dialog.axis = open_axis;
                dialog.positive = positive;
                vec![thickness]
            }
            FeatureKind::OffsetFace {
                distance,
                axis,
                positive,
            } => {
                dialog.axis = axis;
                dialog.positive = positive;
                vec![distance]
            }
            FeatureKind::Translate { offset, copy } => {
                dialog.copy = copy;
                offset.to_vec()
            }
            FeatureKind::Mirror { axis, .. } => {
                dialog.axis = axis;
                vec![]
            }
            FeatureKind::RectangularPattern {
                axis,
                spacing,
                count,
            } => {
                dialog.axis = axis;
                dialog.count = count;
                vec![spacing]
            }
            FeatureKind::CircularPattern {
                axis,
                degrees,
                count,
                ..
            } => {
                dialog.axis = axis;
                dialog.count = count;
                vec![degrees.to_string()]
            }
            FeatureKind::Combine {
                tool_body,
                operation,
                keep_tool,
            } => {
                dialog.secondary = tool_body;
                dialog.operation = operation;
                dialog.copy = keep_tool;
                vec![]
            }
            FeatureKind::SplitBody { axis, position } => {
                dialog.axis = axis;
                vec![position]
            }
        };
        for ((_, value), v) in dialog.fields.iter_mut().zip(values) {
            *value = v;
        }
    }
    pub(super) fn modeling_document(
        &self,
        dialog: &ModelingDialog,
    ) -> fission180_core::Result<Document> {
        if dialog.tool == "Change Parameters" {
            return self
                .document
                .with_parameter(dialog.value(0), dialog.value(1));
        }
        let f = dialog.feature(&self.document)?;
        let mut d = self.document.clone();
        if let Some(id) = dialog.editing {
            let current =
                d.features.iter_mut().find(|f| f.id == id).ok_or_else(|| {
                    fission180_core::Error::Invalid("Missing feature to edit".into())
                })?;
            *current = f;
        } else {
            d.push_feature(f)?;
        }
        Ok(d)
    }
    pub(super) fn modeling_palette(&mut self, ui: &mut egui::Ui, w: f32) {
        let Some(mut dialog) = self.modeling.take() else {
            return;
        };
        let x = w - 254.;
        let y = 258.;
        let p = ui.painter().clone();
        let mut commit = false;
        let mut cancel = false;
        let height = (160.
            + dialog.fields.len() as f32 * 32.
            + if matches!(dialog.tool.as_str(), "Revolve" | "Sweep" | "Loft") {
                70.
            } else {
                100.
            })
        .max(260.);
        p.rect(
            rect(x, y, 250., height),
            0.,
            PANEL,
            Stroke::new(1., BORDER),
            egui::StrokeKind::Inside,
        );
        label(
            &p,
            pos2(x + 12., y + 12.),
            format!("−  {}", dialog.tool.to_uppercase()),
            12.,
            TEXT,
        );
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(rect(x + 12., y + 35., 225., height - 45.)),
            |ui| {
                ui.spacing_mut().item_spacing = vec2(6., 8.);
                if dialog.tool == "Measure" {
                    if let Some(body) = self.committed_bodies.iter().find(|b| b.id == dialog.target)
                    {
                        ui.label(format!("Engine: {}", body.backend.label()));
                        ui.label(format!("Volume: {:.4} mm³", body.volume));
                        ui.label(format!("Area: {:.4} mm²", body.surface_area));
                        for (axis, i) in [("X", 0), ("Y", 1), ("Z", 2)] {
                            ui.label(format!(
                                "{axis}: {:.4} mm",
                                body.bounds[1][i] - body.bounds[0][i]
                            ));
                        }
                    } else {
                        ui.label("Select a body to measure");
                    }
                } else if dialog.tool == "Change Parameters" {
                    if !self.document.parameters.is_empty() {
                        egui::ComboBox::from_id_salt("parameter-name")
                            .selected_text(dialog.value(0))
                            .show_ui(ui, |ui| {
                                for (name, text) in &self.document.parameters {
                                    if ui
                                        .selectable_label(dialog.value(0) == *name, name)
                                        .clicked()
                                    {
                                        dialog.fields[0].1 = name.clone();
                                        dialog.fields[1].1 = text.clone();
                                    }
                                }
                            });
                    }
                } else if matches!(dialog.tool.as_str(), "Revolve" | "Sweep" | "Loft") {
                    region_combo(ui, "Profile", &self.document, &mut dialog.region);
                    if dialog.tool == "Loft" {
                        region_combo(ui, "Section 2", &self.document, &mut dialog.other_region);
                    }
                    if dialog.tool == "Sweep" {
                        egui::ComboBox::from_id_salt("path-sketch")
                            .selected_text(format!("Path: Sketch{}", dialog.path_sketch))
                            .show_ui(ui, |ui| {
                                for s in &self.document.sketches {
                                    ui.selectable_value(&mut dialog.path_sketch, s.id, &s.name);
                                }
                            });
                    }
                } else {
                    egui::ComboBox::from_id_salt("feature-target")
                        .selected_text(format!("Body{}", dialog.target))
                        .show_ui(ui, |ui| {
                            for b in &self.committed_bodies {
                                ui.selectable_value(&mut dialog.target, b.id, &b.name);
                            }
                        });
                }
                if matches!(
                    dialog.tool.as_str(),
                    "Revolve"
                        | "Shell"
                        | "Press Pull"
                        | "Rectangular Pattern"
                        | "Circular Pattern"
                        | "Mirror"
                        | "Split Body"
                ) {
                    egui::ComboBox::from_id_salt("feature-axis")
                        .selected_text(format!("Axis: {:?}", dialog.axis))
                        .show_ui(ui, |ui| {
                            for axis in [Axis::X, Axis::Y, Axis::Z] {
                                ui.selectable_value(&mut dialog.axis, axis, format!("{axis:?}"));
                            }
                        });
                }
                if matches!(dialog.tool.as_str(), "Shell" | "Press Pull") {
                    ui.checkbox(&mut dialog.positive, "Positive side");
                }
                if matches!(dialog.tool.as_str(), "Fillet" | "Chamfer") {
                    ui.label("Edges: All");
                }
                if dialog.tool.ends_with("Pattern") {
                    ui.horizontal(|ui| {
                        ui.label("Quantity");
                        ui.add(egui::DragValue::new(&mut dialog.count).range(2..=100));
                    });
                }
                if dialog.tool == "Move / Copy" {
                    ui.checkbox(&mut dialog.copy, "Create Copy");
                }
                if dialog.tool == "Combine" {
                    egui::ComboBox::from_id_salt("combine-tool")
                        .selected_text(format!("Tool: Body{}", dialog.secondary))
                        .show_ui(ui, |ui| {
                            for b in &self.committed_bodies {
                                ui.selectable_value(&mut dialog.secondary, b.id, &b.name);
                            }
                        });
                    egui::ComboBox::from_id_salt("combine-operation")
                        .selected_text(dialog.operation.label())
                        .show_ui(ui, |ui| {
                            for op in [Operation::Join, Operation::Cut, Operation::Intersect] {
                                ui.selectable_value(&mut dialog.operation, op, op.label());
                            }
                        });
                    ui.checkbox(&mut dialog.copy, "Keep Tools");
                }
                for (label_text, text) in &mut dialog.fields {
                    ui.horizontal(|ui| {
                        ui.label(label_text.as_str());
                        ui.add_sized(vec2(118., 24.), egui::TextEdit::singleline(text));
                    });
                }
                ui.horizontal(|ui| {
                    if dialog.tool != "Measure" {
                        commit = ui.button("OK").clicked();
                    }
                    cancel = ui
                        .button(if dialog.tool == "Measure" {
                            "Close"
                        } else {
                            "Cancel"
                        })
                        .clicked();
                });
            },
        );
        let signature = format!(
            "{:?}:{:?}:{}:{}:{:?}:{:?}:{:?}:{}:{}:{}:{}",
            dialog.fields,
            dialog.axis,
            dialog.target,
            dialog.secondary,
            dialog.region,
            dialog.other_region,
            dialog.operation,
            dialog.count,
            dialog.positive,
            dialog.copy,
            dialog.path_sketch
        );
        let changed = signature != dialog.last_preview;
        dialog.last_preview = signature;
        if cancel {
            self.send(Command::ResetPreview);
            return;
        }
        if commit {
            match self.modeling_document(&dialog) {
                Ok(d) => self.send(Command::Replace(d)),
                Err(e) => {
                    self.error = Some(e.to_string());
                    self.modeling = Some(dialog);
                }
            }
        } else {
            if changed && dialog.tool != "Measure" {
                if let Ok(d) = self.modeling_document(&dialog) {
                    self.send(Command::Preview(d));
                }
            }
            self.modeling = Some(dialog);
        }
    }
}
fn region_combo(ui: &mut egui::Ui, label: &str, d: &Document, current: &mut Option<RegionRef>) {
    let text = current
        .as_ref()
        .map(|r| format!("{label}: Sketch{} / {}", r.sketch_id, r.region_id))
        .unwrap_or_else(|| format!("{label}: Select"));
    egui::ComboBox::from_id_salt(label)
        .selected_text(text)
        .show_ui(ui, |ui| {
            for s in &d.sketches {
                for id in &s.profile_ids {
                    ui.selectable_value(
                        current,
                        Some(RegionRef {
                            sketch_id: s.id,
                            region_id: *id,
                        }),
                        format!("{} / Region {}", s.name, id),
                    );
                }
            }
        });
}
