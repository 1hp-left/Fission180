use super::*;
use fission180_core::sketch::{Constraint, Entity, SketchGraph};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SketchPick {
    Point(u64),
    Entity(u64),
    Constraint(u64),
}
fn item_position(graph: &SketchGraph, item: SketchPick) -> Option<[f64; 2]> {
    match item {
        SketchPick::Point(id) => graph.points.get(&id).copied(),
        SketchPick::Entity(id) => match graph.entities.get(&id)? {
            Entity::Line { a, b } => Some([
                (graph.points[a][0] + graph.points[b][0]) * 0.5,
                (graph.points[a][1] + graph.points[b][1]) * 0.5,
            ]),
            Entity::Circle { center, radius } => {
                Some([graph.points[center][0] + radius, graph.points[center][1]])
            }
        },
        SketchPick::Constraint(id) => {
            let c = graph.constraints.get(&id)?;
            let id = *c.references().first()?;
            item_position(
                graph,
                if graph.entities.contains_key(&id) {
                    SketchPick::Entity(id)
                } else {
                    SketchPick::Point(id)
                },
            )
        }
    }
}
fn glyph(c: &Constraint) -> &'static str {
    match c {
        Constraint::Fixed { .. } => "▣",
        Constraint::Coincident { .. } => "○",
        Constraint::Midpoint { .. } => "△",
        Constraint::PointOnLine { .. } => "○",
        Constraint::Concentric { .. } => "◎",
        Constraint::Horizontal { .. } => "━",
        Constraint::Vertical { .. } => "┃",
        Constraint::Parallel { .. } => "∥",
        Constraint::Perpendicular { .. } => "⊥",
        Constraint::Equal { .. } => "=",
        Constraint::Tangent { .. } => "⊙",
        Constraint::Length { .. } | Constraint::DistanceX { .. } | Constraint::DistanceY { .. } => {
            "↔"
        }
        Constraint::Radius { .. } => "R",
    }
}
fn glyph_at(
    graph: &SketchGraph,
    id: u64,
    camera: Camera,
    sketch: &Sketch,
    r: Rect,
) -> Option<Pos2> {
    let local = item_position(graph, SketchPick::Constraint(id))?;
    let at = camera
        .project(sketch.plane.point(local, sketch.origin).to_array(), r)
        .0;
    Some(at + vec2(10., -12. - (id % 3) as f32 * 9.))
}
pub(super) fn snap_point(
    sketch: &Sketch,
    local: [f64; 2],
    scale: f32,
    anchor: Option<[f64; 2]>,
) -> ([f64; 2], Option<&'static str>) {
    let tolerance = 7. / scale.max(0.05) as f64;
    let mut candidates = vec![([0., 0.], "Origin")];
    if let Some(g) = &sketch.graph {
        candidates.extend(g.points.values().map(|p| (*p, "Endpoint / Center")));
        for e in g.entities.values() {
            if let Entity::Line { a, b } = e {
                candidates.push((
                    [
                        (g.points[a][0] + g.points[b][0]) * 0.5,
                        (g.points[a][1] + g.points[b][1]) * 0.5,
                    ],
                    "Midpoint",
                ));
            }
        }
    }
    if let Some((p, kind)) = candidates
        .into_iter()
        .filter_map(|(p, k)| {
            let distance = (p[0] - local[0]).hypot(p[1] - local[1]);
            (distance < tolerance).then_some((distance, p, k))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, p, k)| (p, k))
    {
        return (p, Some(kind));
    }
    let step = if scale > 20. {
        0.1
    } else if scale > 2. {
        1.
    } else if scale > 0.2 {
        10.
    } else {
        100.
    };
    let mut p = local.map(|n| (n / step).round() * step);
    let mut kind = None;
    if let Some(a) = anchor {
        if (local[0] - a[0]).abs() < tolerance {
            p[0] = a[0];
            kind = Some("Vertical");
        } else if (local[1] - a[1]).abs() < tolerance {
            p[1] = a[1];
            kind = Some("Horizontal");
        }
    }
    (p, kind)
}
impl FusionApp {
    pub(super) fn start_line_edit(&mut self, extend: bool) {
        self.tool = if extend { Tool::Extend } else { Tool::Trim };
        self.anchor = None;
        self.polyline.clear();
        self.sketch_selection.clear();
        self.status = if extend {
            "Extend: click near a line end"
        } else {
            "Trim: click a segment between intersections (T)"
        }
        .into();
    }
    fn line_edit_hit(&self, at: Pos2, r: Rect) -> Option<u64> {
        let s = self.draft.as_ref()?;
        let g = s.graph.as_ref()?;
        g.entities
            .iter()
            .filter_map(|(id, e)| {
                let Entity::Line { a, b } = e else {
                    return None;
                };
                let a = self
                    .camera
                    .project(s.plane.point(g.points[a], s.origin).to_array(), r)
                    .0;
                let b = self
                    .camera
                    .project(s.plane.point(g.points[b], s.origin).to_array(), r)
                    .0;
                let d = b - a;
                let t = ((at - a).dot(d) / d.length_sq().max(1e-10)).clamp(0., 1.);
                let distance = (at - (a + d * t)).length();
                (distance < 6.).then_some((distance, *id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|v| v.1)
    }
    pub(super) fn paint_line_edit_hover(&self, p: &Painter, at: Pos2, r: Rect) {
        let Some(id) = self.line_edit_hit(at, r) else {
            return;
        };
        let Some(s) = &self.draft else {
            return;
        };
        let Some(g) = &s.graph else {
            return;
        };
        if let Entity::Line { a, b } = g.entities[&id] {
            let points = [g.points[&a], g.points[&b]].map(|v| {
                self.camera
                    .project(s.plane.point(v, s.origin).to_array(), r)
                    .0
            });
            p.line_segment(
                points,
                Stroke::new(
                    2.5,
                    if self.tool == Tool::Trim {
                        C::from_rgb(238, 139, 116)
                    } else {
                        TEXT
                    },
                ),
            );
        }
    }
    pub(super) fn apply_line_edit(&mut self, at: Pos2, r: Rect) {
        let Some(id) = self.line_edit_hit(at, r) else {
            return;
        };
        let Some(s) = &self.draft else {
            return;
        };
        let local = self.sketch_local(at, r);
        let result = if self.tool == Tool::Extend {
            s.with_extended_line(id, local, &self.document.parameters)
        } else {
            s.with_trimmed_line(id, local, &self.document.parameters)
        };
        self.accept_sketch(result);
        self.sketch_selection.clear();
        self.selected = None;
    }
    pub(super) fn edit_sketch(&mut self, s: Sketch) {
        self.sketch_history = Default::default();
        self.sketch_selection.clear();
        self.sketch_drag = None;
        self.entity_dimension = None;
        self.show_dimension = false;
        self.selected = None;
        self.tool = Tool::Select;
        self.anchor = None;
        self.polyline.clear();
        self.mode = Mode::Sketch;
        self.camera.top = true;
        self.camera.plane = s.plane;
        self.tab = "SKETCH";
        if let Ok(dependents) = self.document.dependent_features(s.id) {
            if let Ok(order) = self.document.feature_order() {
                if let Some(first) = order.iter().position(|id| dependents.contains(id)) {
                    let mut rolled_back = self.document.clone();
                    rolled_back
                        .suppressed
                        .extend(order[first..].iter().copied());
                    self.send(Command::Preview(rolled_back));
                }
            }
        }
        self.draft = Some(s);
    }
    pub(super) fn sketch_local(&self, at: Pos2, r: Rect) -> [f64; 2] {
        let p = self.camera.unproject(at, r);
        if let Some(s) = &self.draft {
            let offset = match s.plane {
                Plane::XY => [s.origin[0], s.origin[1]],
                Plane::XZ => [s.origin[0], s.origin[2]],
                Plane::YZ => [s.origin[1], s.origin[2]],
            };
            [p[0] - offset[0], p[1] - offset[1]]
        } else {
            p
        }
    }
    pub(super) fn drag_sketch_point(&mut self, ui: &egui::Ui, response: &egui::Response, r: Rect) {
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(at) = ui.input(|i| i.pointer.press_origin()) {
                self.select_sketch_item(at, r, false);
                if let (Some(base), Some(SketchPick::Point(point))) =
                    (&self.draft, self.sketch_selection.first())
                {
                    self.sketch_drag = Some((base.clone(), *point));
                }
            }
        }
        if response.dragged_by(egui::PointerButton::Primary) {
            if let (Some((base, point)), Some(at)) =
                (&self.sketch_drag, response.interact_pointer_pos())
            {
                let local = self.sketch_local(at, r);
                // A dragged point must not snap back onto itself.
                let mut snap_source = base.clone();
                if let Some(g) = &mut snap_source.graph {
                    g.points.remove(point);
                    g.entities.clear();
                }
                let local = if self.snap {
                    snap_point(&snap_source, local, self.camera.scale, None).0
                } else {
                    local
                };
                match base.with_dragged_point(*point, local, &self.document.parameters) {
                    Ok(candidate) => {
                        self.draft = Some(candidate);
                        self.error = None;
                    }
                    Err(e) => self.error = Some(e.to_string()),
                }
            }
        }
        if response.drag_stopped_by(egui::PointerButton::Primary) {
            if let Some((base, _)) = self.sketch_drag.take() {
                if self.draft.as_ref() != Some(&base) {
                    self.sketch_history.record(&base);
                    self.dirty = true;
                }
            }
        }
    }
    pub(super) fn sketch_snap(&self, local: [f64; 2]) -> ([f64; 2], Option<&'static str>) {
        self.draft
            .as_ref()
            .map(|s| snap_point(s, local, self.camera.scale, self.anchor))
            .unwrap_or((local, None))
    }
    fn accept_sketch(&mut self, result: fission180_core::Result<Sketch>) {
        match result {
            Ok(candidate) => {
                if let Some(old) = &self.draft {
                    self.sketch_history.record(old);
                }
                self.draft = Some(candidate);
                self.dirty = true;
                self.error = None;
                if let Some(g) = self.draft.as_ref().and_then(|s| s.graph.as_ref()) {
                    if let Ok((_, report)) = g.solve(&self.document.parameters) {
                        self.status = format!("{} degrees of freedom", report.degrees_of_freedom);
                    }
                }
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    pub(super) fn select_sketch_item(&mut self, at: Pos2, r: Rect, multiple: bool) -> bool {
        let Some(s) = &self.draft else {
            return false;
        };
        let Some(g) = &s.graph else {
            return false;
        };
        let projected = |p| {
            self.camera
                .project(s.plane.point(p, s.origin).to_array(), r)
                .0
        };
        let mut found = g
            .points
            .iter()
            .filter_map(|(id, p)| {
                let d = (projected(*p) - at).length();
                (d < 6.).then_some((d, SketchPick::Point(*id)))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if found.is_none() {
            found = g
                .entities
                .iter()
                .filter_map(|(id, e)| {
                    let distance = match e {
                        Entity::Line { a, b } => {
                            let a = projected(g.points[a]);
                            let d = projected(g.points[b]) - a;
                            let t = ((at - a).dot(d) / d.length_sq().max(1e-10)).clamp(0., 1.);
                            (at - (a + d * t)).length()
                        }
                        Entity::Circle { center, radius } => ((at - projected(g.points[center]))
                            .length()
                            - (*radius as f32 * self.camera.scale))
                            .abs(),
                    };
                    (distance < 6.).then_some((distance, SketchPick::Entity(*id)))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
        }
        if found.is_none() && (self.show_constraints || self.show_dimensions) {
            found = g
                .constraints
                .keys()
                .filter_map(|id| {
                    let distance = (glyph_at(g, *id, self.camera, s, r)? - at).length();
                    (distance < 7.).then_some((distance, SketchPick::Constraint(*id)))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
        }
        if !multiple {
            self.sketch_selection.clear();
        }
        if let Some((_, item)) = found {
            if let Some(i) = self.sketch_selection.iter().position(|v| *v == item) {
                self.sketch_selection.remove(i);
            } else {
                self.sketch_selection.push(item);
            }
            self.selected = None;
            true
        } else {
            false
        }
    }
    pub(super) fn paint_sketch_selection(&self, p: &Painter, r: Rect) {
        let Some(s) = &self.draft else {
            return;
        };
        let Some(g) = &s.graph else {
            return;
        };
        let projected = |v| {
            self.camera
                .project(s.plane.point(v, s.origin).to_array(), r)
                .0
        };
        for item in &self.sketch_selection {
            match item {
                SketchPick::Point(id) => {
                    if let Some(at) = g.points.get(id) {
                        p.circle_filled(projected(*at), 4., ACCENT);
                    }
                }
                SketchPick::Entity(id) => match g.entities.get(id) {
                    Some(Entity::Line { a, b }) => {
                        p.line_segment(
                            [projected(g.points[a]), projected(g.points[b])],
                            Stroke::new(2.5, ACCENT),
                        );
                    }
                    Some(Entity::Circle { center, radius }) => {
                        p.circle_stroke(
                            projected(g.points[center]),
                            *radius as f32 * self.camera.scale,
                            Stroke::new(2.5, ACCENT),
                        );
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        if self.show_constraints || self.show_dimensions {
            for (id, c) in &g.constraints {
                let dimension = matches!(
                    c,
                    Constraint::Length { .. }
                        | Constraint::Radius { .. }
                        | Constraint::DistanceX { .. }
                        | Constraint::DistanceY { .. }
                );
                if (dimension && !self.show_dimensions) || (!dimension && !self.show_constraints) {
                    continue;
                }
                if let Some(at) = glyph_at(g, *id, self.camera, s, r) {
                    let text = match c {
                        Constraint::Length { expression, .. }
                        | Constraint::DistanceX { expression, .. }
                        | Constraint::DistanceY { expression, .. } => {
                            fission180_core::expression::length(
                                expression,
                                &self.document.parameters,
                            )
                            .map(|v| format!("{v:.2}"))
                            .unwrap_or_else(|_| expression.clone())
                        }
                        Constraint::Radius { expression, .. } => {
                            fission180_core::expression::length(
                                expression,
                                &self.document.parameters,
                            )
                            .map(|v| format!("R {v:.2}"))
                            .unwrap_or_else(|_| expression.clone())
                        }
                        _ => glyph(c).into(),
                    };
                    label(
                        p,
                        at,
                        text,
                        12.,
                        if self.sketch_selection.contains(&SketchPick::Constraint(*id)) {
                            ACCENT
                        } else {
                            MUTED
                        },
                    );
                }
            }
        }
    }
    pub(super) fn start_relation(&mut self, name: &'static str) {
        self.tool = Tool::Relation(name);
        self.anchor = None;
        self.polyline.clear();
        self.apply_relation(name);
    }
    pub(super) fn apply_relation(&mut self, name: &'static str) {
        let Some(s) = &self.draft else {
            return;
        };
        let Ok(mut g) = s.graph_for_edit() else {
            return;
        };
        let items = &self.sketch_selection;
        let required = if matches!(name, "Horizontal / Vertical" | "Fix / UnFix") {
            1
        } else {
            2
        };
        if items.len() < required {
            self.status = format!("{name}: select {required} sketch items (Ctrl adds selection)");
            return;
        }
        let result = (|| -> fission180_core::Result<Sketch> {
            let entity = |i| match items.get(i) {
                Some(SketchPick::Entity(id)) => Ok(*id),
                _ => Err(fission180_core::Error::Invalid(
                    "Select sketch edges for this constraint".into(),
                )),
            };
            let point = |i| match items.get(i) {
                Some(SketchPick::Point(id)) => Ok(*id),
                _ => Err(fission180_core::Error::Invalid(
                    "Select sketch points for this constraint".into(),
                )),
            };
            let c = match name {
                "Horizontal / Vertical" => {
                    let line = entity(0)?;
                    let Some(Entity::Line { a, b }) = g.entities.get(&line) else {
                        return Err(fission180_core::Error::Invalid("Select a line".into()));
                    };
                    if (g.points[a][0] - g.points[b][0]).abs()
                        > (g.points[a][1] - g.points[b][1]).abs()
                    {
                        Constraint::Horizontal { line }
                    } else {
                        Constraint::Vertical { line }
                    }
                }
                "Coincident" => Constraint::Coincident {
                    a: point(0)?,
                    b: point(1)?,
                },
                "Concentric" => Constraint::Concentric {
                    a: entity(0)?,
                    b: entity(1)?,
                },
                "Midpoint" => {
                    let (p, l) = if matches!(items[0], SketchPick::Point(_)) {
                        (point(0)?, entity(1)?)
                    } else {
                        (point(1)?, entity(0)?)
                    };
                    Constraint::Midpoint { point: p, line: l }
                }
                "Parallel" => Constraint::Parallel {
                    a: entity(0)?,
                    b: entity(1)?,
                },
                "Perpendicular" => Constraint::Perpendicular {
                    a: entity(0)?,
                    b: entity(1)?,
                },
                "Equal" => Constraint::Equal {
                    a: entity(0)?,
                    b: entity(1)?,
                },
                "Tangent" => {
                    let a = entity(0)?;
                    let b = entity(1)?;
                    if matches!(g.entities.get(&a), Some(Entity::Line { .. })) {
                        Constraint::Tangent { line: a, circle: b }
                    } else {
                        Constraint::Tangent { line: b, circle: a }
                    }
                }
                "Fix / UnFix" => {
                    let mut points = vec![];
                    let mut circles = vec![];
                    for item in items {
                        match item {
                            SketchPick::Point(id) => points.push(*id),
                            SketchPick::Entity(id) => match g.entities.get(id) {
                                Some(Entity::Line { a, b }) => points.extend([*a, *b]),
                                Some(Entity::Circle { center, radius }) => {
                                    points.push(*center);
                                    circles.push((*id, *radius));
                                }
                                _ => {}
                            },
                            _ => {}
                        }
                    }
                    let fixed = points.iter().all(|id| {
                        g.constraints
                            .values()
                            .any(|c| matches!(c,Constraint::Fixed{point,..} if point==id))
                    });
                    if fixed {
                        g.constraints.retain(|_,c|!matches!(c,Constraint::Fixed{point,..} if points.contains(point)));
                        for (id, _) in circles {
                            if let Some(c) = g.dimension_ids.remove(&format!("fix.{id}")) {
                                g.constraints.remove(&c);
                            }
                        }
                        g.dimension_ids
                            .retain(|_, id| g.constraints.contains_key(id));
                    } else {
                        for point in points {
                            g.constrain(Constraint::Fixed {
                                point,
                                at: g.points[&point],
                            })?;
                        }
                        for (circle, radius) in circles {
                            g.dimension(
                                format!("fix.{circle}"),
                                Constraint::Radius {
                                    circle,
                                    expression: radius.to_string(),
                                },
                            )?;
                        }
                    }
                    return s.with_graph(g, &self.document.parameters);
                }
                _ => {
                    return Err(fission180_core::Error::Invalid(
                        "Unknown sketch constraint".into(),
                    ));
                }
            };
            s.with_constraints(vec![c], &self.document.parameters)
        })();
        let success = result.is_ok();
        self.accept_sketch(result);
        if success {
            self.sketch_selection.clear();
        }
    }
    pub(super) fn toggle_construction(&mut self) {
        let Some(s) = &self.draft else {
            return;
        };
        let Ok(g) = s.graph_for_edit() else {
            return;
        };
        let mut ids: Vec<_> = self
            .sketch_selection
            .iter()
            .filter_map(|i| {
                if let SketchPick::Entity(id) = i {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        if ids.is_empty() {
            if let Some((_, i)) = self.selected {
                if let Some(id) = s.profile_ids.get(i) {
                    ids = g.profile_entities.get(id).cloned().unwrap_or_default();
                }
            }
        }
        if ids.is_empty() {
            self.status = "Select sketch edges, then press X for construction".into();
            return;
        }
        let construction = !ids.iter().all(|id| g.construction.contains(id));
        self.accept_sketch(s.with_construction(&ids, construction, &self.document.parameters));
    }
    pub(super) fn delete_sketch_selection(&mut self) {
        let Some(s) = &self.draft else {
            return;
        };
        let Ok(g) = s.graph_for_edit() else {
            return;
        };
        let result = (|| -> fission180_core::Result<Sketch> {
            let mut candidate = s.clone();
            let mut entities = vec![];
            for item in &self.sketch_selection {
                match item {
                    SketchPick::Constraint(id) => {
                        candidate = candidate.without_constraint(*id, &self.document.parameters)?
                    }
                    SketchPick::Entity(id) => entities.push(*id),
                    SketchPick::Point(p) => {
                        entities.extend(g.entities.iter().filter_map(|(id, e)| match e {
                            Entity::Line { a, b } if a == p || b == p => Some(*id),
                            Entity::Circle { center, .. } if center == p => Some(*id),
                            _ => None,
                        }))
                    }
                }
            }
            if entities.is_empty() && self.sketch_selection.is_empty() {
                if let Some((_, i)) = self.selected {
                    if let Some(id) = s.profile_ids.get(i) {
                        entities = g.profile_entities.get(id).cloned().unwrap_or_default();
                    }
                }
            }
            entities.sort();
            entities.dedup();
            if !entities.is_empty() {
                candidate = candidate.without_entities(&entities, &self.document.parameters)?;
            }
            Ok(candidate)
        })();
        self.accept_sketch(result);
        self.selected = None;
        self.sketch_selection.clear();
    }
    pub(super) fn start_entity_dimension(&mut self) -> bool {
        let Some(s) = &self.draft else {
            return false;
        };
        let Some(g) = &s.graph else {
            return false;
        };
        let id = match self.sketch_selection.last() {
            Some(SketchPick::Entity(id)) => *id,
            Some(SketchPick::Constraint(id)) => match g.constraints.get(id) {
                Some(Constraint::Length { line, .. }) => *line,
                Some(Constraint::Radius { circle, .. }) => *circle,
                _ => return false,
            },
            _ => return false,
        };
        self.dim_radius = match g.entities.get(&id) {
            Some(Entity::Line { a, b }) => (g.points[a][0] - g.points[b][0])
                .hypot(g.points[a][1] - g.points[b][1])
                .to_string(),
            Some(Entity::Circle { radius, .. }) => radius.to_string(),
            _ => return false,
        };
        if let Some(c) = g
            .dimension_ids
            .get(&format!("entity.{id}"))
            .and_then(|id| g.constraints.get(id))
        {
            match c {
                Constraint::Length { expression, .. } | Constraint::Radius { expression, .. } => {
                    self.dim_radius = expression.clone()
                }
                _ => {}
            }
        }
        self.entity_dimension = Some(id);
        self.show_dimension = true;
        true
    }
    pub(super) fn entity_dimension_palette(&mut self, ui: &mut egui::Ui, w: f32) {
        let Some(id) = self.entity_dimension else {
            return;
        };
        let r = rect(w - 540., 286., 260., 160.);
        let p = ui.painter().clone();
        p.rect(
            r,
            2.,
            PANEL,
            Stroke::new(1., BORDER),
            egui::StrokeKind::Inside,
        );
        label(
            &p,
            r.min + vec2(12., 16.),
            "EDIT SKETCH DIMENSION",
            12.,
            TEXT,
        );
        let radius = self
            .draft
            .as_ref()
            .and_then(|s| s.graph.as_ref())
            .and_then(|g| g.entities.get(&id))
            .is_some_and(|e| matches!(e, Entity::Circle { .. }));
        label(
            &p,
            r.min + vec2(12., 55.),
            if radius { "Radius" } else { "Length" },
            12.,
            TEXT,
        );
        let response = ui.put(
            Rect::from_min_size(r.min + vec2(82., 41.), vec2(160., 27.)),
            egui::TextEdit::singleline(&mut self.dim_radius),
        );
        let state = fission180_core::expression::DraftInput::new(
            self.dim_radius.clone(),
            &self.document.parameters,
        );
        let message = match &state.state {
            fission180_core::expression::InputState::Valid(v) => format!("{} mm", v.millimetres),
            fission180_core::expression::InputState::Incomplete => "Incomplete expression".into(),
            fission180_core::expression::InputState::Error(e) => e.clone(),
        };
        label(&p, r.min + vec2(12., 88.), message, 11., MUTED);
        let apply = button(
            ui,
            Rect::from_min_size(r.min + vec2(90., 120.), vec2(65., 26.)),
            "entity-dimension-apply",
            "Apply",
        ) || (response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)));
        if apply {
            if let Some(s) = &self.draft {
                let result =
                    s.with_entity_dimension(id, self.dim_radius.clone(), &self.document.parameters);
                let success = result.is_ok();
                self.accept_sketch(result);
                if success {
                    self.show_dimension = false;
                    self.entity_dimension = None;
                }
            }
        }
        if button(
            ui,
            Rect::from_min_size(r.min + vec2(170., 120.), vec2(65., 26.)),
            "entity-dimension-cancel",
            "Cancel",
        ) {
            self.show_dimension = false;
            self.entity_dimension = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoint_midpoint_and_axis_inference_precede_grid_quantization() {
        let s = Sketch::default()
            .with_line([0.25, 0.5], [40.25, 0.5], &Default::default())
            .unwrap();
        assert_eq!(snap_point(&s, [40.6, 0.7], 4., None).0, [40.25, 0.5]);
        assert_eq!(snap_point(&s, [20.6, 0.7], 4., None).0, [20.25, 0.5]);
        let (point, kind) = snap_point(&s, [35., 10.6], 4., Some([5., 10.25]));
        assert_eq!(point, [35., 10.25]);
        assert_eq!(kind, Some("Horizontal"));
        assert_eq!(snap_point(&s, [35., 50.], 4., None).0, [35., 50.]);
    }
}
