//! Persistent solid feature commands, independent of desktop widgets.
use crate::kernel::{Backend, ExtrudeInput, Geometry, SolidKernel};
use crate::{Body, Document, Error, Operation, Plane, Profile, Result, invalid};
use cadrum::{DVec3, Edge, ProfileOrient, Solid, SurfaceKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Axis {
    X,
    Y,
    Z,
}
impl Axis {
    pub fn vector(self) -> DVec3 {
        match self {
            Self::X => DVec3::X,
            Self::Y => DVec3::Y,
            Self::Z => DVec3::Z,
        }
    }
    pub fn index(self) -> usize {
        match self {
            Self::X => 0,
            Self::Y => 1,
            Self::Z => 2,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionRef {
    pub sketch_id: u64,
    pub region_id: u64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanarSupport {
    pub body_id: u64,
    pub axis: Axis,
    pub positive: bool,
    pub offset: f64,
}
impl Document {
    pub fn dependent_features(&self, root: u64) -> Result<std::collections::BTreeSet<u64>> {
        use std::collections::{BTreeMap, BTreeSet};
        let mut graph: BTreeMap<u64, BTreeSet<u64>> = BTreeMap::new();
        let mut writer: BTreeMap<u64, u64> = BTreeMap::new();
        let mut producer: BTreeMap<u64, u64> = BTreeMap::new();
        for id in self.feature_order()? {
            let mut dependencies = BTreeSet::new();
            let outputs;
            if let Some(e) = self.extrusions.iter().find(|e| e.id == id) {
                dependencies.insert(e.sketch_id);
                if e.operation != Operation::NewBody {
                    if let Some(p) = writer.get(&e.body_id) {
                        dependencies.insert(*p);
                    }
                }
                outputs = if e.result_body_ids.is_empty() {
                    vec![e.body_id]
                } else {
                    e.result_body_ids.clone()
                };
            } else {
                let f = self.features.iter().find(|f| f.id == id).unwrap();
                for raw in f.dependencies() {
                    if self.sketches.iter().any(|s| s.id == raw) {
                        dependencies.insert(raw);
                    } else if let Some(p) = writer.get(&raw) {
                        dependencies.insert(*p);
                    }
                }
                outputs = f.outputs.clone();
            }
            graph.insert(id, dependencies);
            for output in outputs {
                producer.entry(output).or_insert(id);
                writer.insert(output, id);
            }
        }
        for s in &self.sketches {
            let mut dependencies = BTreeSet::new();
            if let Some(support) = &s.support {
                if let Some(p) = producer.get(&support.body_id) {
                    dependencies.insert(*p);
                }
            }
            graph.insert(s.id, dependencies);
        }
        if !graph.contains_key(&root) {
            return invalid("Missing feature to edit");
        }
        let mut selected = BTreeSet::from([root]);
        loop {
            let before = selected.len();
            for (id, dependencies) in &graph {
                if dependencies.iter().any(|id| selected.contains(id)) {
                    selected.insert(*id);
                }
            }
            if selected.len() == before {
                break;
            }
        }
        Ok(selected)
    }
    pub fn with_suppression(&self, root: u64, suppressed: bool) -> Result<Self> {
        let selected = self.dependent_features(root)?;
        let mut d = self.clone();
        let order = self.feature_order()?;
        for id in selected {
            if order.contains(&id) {
                if suppressed {
                    d.suppressed.insert(id);
                } else {
                    d.suppressed.remove(&id);
                }
            }
        }
        Ok(d)
    }
    pub fn without_feature(&self, root: u64) -> Result<Self> {
        let selected = self.dependent_features(root)?;
        let mut d = self.clone();
        d.sketches.retain(|s| !selected.contains(&s.id));
        d.extrusions.retain(|e| !selected.contains(&e.id));
        d.features.retain(|f| !selected.contains(&f.id));
        d.timeline.retain(|id| !selected.contains(id));
        d.suppressed.retain(|id| !selected.contains(id));
        d.validate()?;
        Ok(d)
    }
    pub fn resolved_sketch(&self, id: u64, bodies: &[Body]) -> Result<crate::Sketch> {
        let mut s = self
            .sketches
            .iter()
            .find(|s| s.id == id)
            .cloned()
            .ok_or_else(|| Error::Invalid("Missing sketch".into()))?;
        if let Some(support) = &s.support {
            let body = bodies
                .iter()
                .find(|b| b.id == support.body_id)
                .ok_or_else(|| {
                    Error::Invalid(
                        "Sketch support body is missing at this timeline position".into(),
                    )
                })?;
            let axis = support.axis.index();
            let expected = match s.plane {
                Plane::XY => 2,
                Plane::XZ => 1,
                Plane::YZ => 0,
            };
            if axis != expected {
                return invalid(
                    "Sketch support plane orientation differs from its stable face selector",
                );
            }
            s.origin[axis] =
                body.solid.bounds()[usize::from(support.positive)][axis] + support.offset;
        }
        Ok(s)
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FeatureKind {
    Revolve {
        region: RegionRef,
        axis: Axis,
        axis_origin: [f64; 3],
        degrees: f64,
    },
    Sweep {
        region: RegionRef,
        path_sketch: u64,
        path_entities: Vec<u64>,
    },
    Loft {
        sections: Vec<RegionRef>,
        ruled: bool,
    },
    Hole {
        center: [f64; 2],
        start_z: f64,
        diameter: String,
        depth: String,
    },
    Fillet {
        radius: String,
    },
    Chamfer {
        distance: String,
    },
    Shell {
        thickness: String,
        open_axis: Axis,
        positive: bool,
    },
    OffsetFace {
        distance: String,
        axis: Axis,
        positive: bool,
    },
    Translate {
        offset: [String; 3],
        copy: bool,
    },
    Mirror {
        axis: Axis,
        origin: [f64; 3],
    },
    RectangularPattern {
        axis: Axis,
        spacing: String,
        count: u32,
    },
    CircularPattern {
        axis: Axis,
        origin: [f64; 3],
        degrees: f64,
        count: u32,
    },
    Combine {
        tool_body: u64,
        operation: Operation,
        keep_tool: bool,
    },
    SplitBody {
        axis: Axis,
        position: String,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Feature {
    pub id: u64,
    pub name: String,
    pub target: Option<u64>,
    pub outputs: Vec<u64>,
    pub kind: FeatureKind,
}
pub(crate) struct Outcome {
    pub removed: Vec<u64>,
    pub solids: Vec<Geometry>,
}
impl FeatureKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Revolve { .. } => "Revolve",
            Self::Sweep { .. } => "Sweep",
            Self::Loft { .. } => "Loft",
            Self::Hole { .. } => "Hole",
            Self::Fillet { .. } => "Fillet",
            Self::Chamfer { .. } => "Chamfer",
            Self::Shell { .. } => "Shell",
            Self::OffsetFace { .. } => "Press Pull",
            Self::Translate { .. } => "Move / Copy",
            Self::Mirror { .. } => "Mirror",
            Self::RectangularPattern { .. } => "Rectangular Pattern",
            Self::CircularPattern { .. } => "Circular Pattern",
            Self::Combine { .. } => "Combine",
            Self::SplitBody { .. } => "Split Body",
        }
    }
}
impl Feature {
    pub fn dependencies(&self) -> Vec<u64> {
        let mut ids = self.target.into_iter().collect::<Vec<_>>();
        match &self.kind {
            FeatureKind::Revolve { region, .. } => ids.push(region.sketch_id),
            FeatureKind::Sweep {
                region,
                path_sketch,
                ..
            } => ids.extend([region.sketch_id, *path_sketch]),
            FeatureKind::Loft { sections, .. } => ids.extend(sections.iter().map(|r| r.sketch_id)),
            FeatureKind::Combine { tool_body, .. } => ids.push(*tool_body),
            _ => {}
        }
        ids
    }
    pub fn validate(&self, d: &Document) -> Result<()> {
        let region = |r: &RegionRef| -> Result<()> {
            let s = d
                .sketches
                .iter()
                .find(|s| s.id == r.sketch_id)
                .ok_or_else(|| Error::Invalid("Feature references a missing sketch".into()))?;
            s.region(r.region_id)?;
            Ok(())
        };
        let length = |s: &str| crate::expression::length(s, &d.parameters);
        let positive = |s: &str| -> Result<()> {
            if length(s)? <= crate::EPS {
                return invalid("Feature dimension must be positive");
            }
            Ok(())
        };
        match &self.kind {
            FeatureKind::Revolve {
                region: r,
                axis_origin,
                degrees,
                ..
            } => {
                region(r)?;
                point(*axis_origin)?;
                angle(*degrees)?;
            }
            FeatureKind::Sweep {
                region: r,
                path_entities,
                ..
            } => {
                region(r)?;
                if path_entities.is_empty() || path_entities.len() > 1000 {
                    return invalid("Sweep needs 1..1000 path entities");
                }
            }
            FeatureKind::Loft { sections, .. } => {
                if !(2..=100).contains(&sections.len()) {
                    return invalid("Loft requires 2..100 sections");
                }
                for r in sections {
                    region(r)?;
                }
            }
            FeatureKind::Hole {
                center,
                start_z,
                diameter,
                depth,
            } => {
                point([center[0], center[1], *start_z])?;
                positive(diameter)?;
                positive(depth)?;
            }
            FeatureKind::Fillet { radius } => positive(radius)?,
            FeatureKind::Chamfer { distance } => positive(distance)?,
            FeatureKind::Shell { thickness, .. } => positive(thickness)?,
            FeatureKind::OffsetFace { distance, .. } => {
                if length(distance)?.abs() < crate::EPS {
                    return invalid("Face offset distance must be nonzero");
                }
            }
            FeatureKind::Translate { offset, .. } => {
                for v in offset {
                    length(v)?;
                }
            }
            FeatureKind::Mirror { origin, .. } => point(*origin)?,
            FeatureKind::RectangularPattern { spacing, count, .. } => {
                positive(spacing)?;
                count_valid(*count)?;
            }
            FeatureKind::CircularPattern {
                origin,
                degrees,
                count,
                ..
            } => {
                point(*origin)?;
                angle(*degrees)?;
                count_valid(*count)?;
            }
            FeatureKind::Combine { operation, .. } => {
                if *operation == Operation::NewBody {
                    return invalid("Combine requires Join, Cut or Intersect");
                }
            }
            FeatureKind::SplitBody { position, .. } => {
                length(position)?;
            }
        }
        if !matches!(
            self.kind,
            FeatureKind::Revolve { .. } | FeatureKind::Sweep { .. } | FeatureKind::Loft { .. }
        ) && self.target.is_none()
        {
            return invalid("Select a target body");
        }
        let unique: std::collections::HashSet<_> = self.outputs.iter().copied().collect();
        if unique.len() != self.outputs.len() || unique.contains(&0) {
            return invalid("Feature output identities are invalid");
        }
        Ok(())
    }
    pub(crate) fn evaluate(&self, d: &Document, bodies: &[Body]) -> Result<Outcome> {
        self.validate(d)?;
        let find = |id: u64| {
            bodies
                .iter()
                .find(|b| b.id == id)
                .ok_or_else(|| Error::Invalid(format!("{}: missing target body {id}", self.name)))
        };
        let target = self.target.map(find).transpose()?;
        let length = |text: &str| crate::expression::length(text, &d.parameters);
        let required = || target.ok_or_else(|| Error::Invalid("Select a target body".into()));
        let occt = || -> Result<&Solid> {
            match &required()?.solid {
                Geometry::Occt(s) => Ok(s),
                Geometry::Rust(_) => invalid(&format!(
                    "{} is unsupported by Rust analytic; no OCCT fallback occurred",
                    self.kind.label()
                )),
            }
        };
        let new_occt = || -> Result<()> {
            if d.backend != Backend::Occt {
                return invalid(&format!(
                    "{} is unsupported by Rust analytic; no OCCT fallback occurred",
                    self.kind.label()
                ));
            }
            Ok(())
        };
        let mut removed = vec![];
        let solids = match &self.kind {
            FeatureKind::Revolve {
                region,
                axis,
                axis_origin,
                degrees,
            } => {
                new_occt()?;
                let edges = region_edges(d, region, bodies)?;
                vec![Geometry::Occt(Solid::revolve(
                    &edges,
                    DVec3::from_array(*axis_origin),
                    axis.vector(),
                    degrees.to_radians(),
                )?)]
            }
            FeatureKind::Sweep {
                region,
                path_sketch,
                path_entities,
            } => {
                new_occt()?;
                let edges = region_edges(d, region, bodies)?;
                let sketch = d.resolved_sketch(*path_sketch, bodies)?;
                let graph = sketch
                    .graph
                    .as_ref()
                    .ok_or_else(|| Error::Invalid("Sweep path has no entity graph".into()))?;
                let mut spine = vec![];
                let mut last = None;
                for id in path_entities {
                    let Some(crate::sketch::Entity::Line { a, b }) = graph.entities.get(id) else {
                        return invalid("Sweep path must reference line entities");
                    };
                    let a = sketch.plane.point(
                        *graph
                            .points
                            .get(a)
                            .ok_or_else(|| Error::Invalid("Missing sweep point".into()))?,
                        sketch.origin,
                    );
                    let b = sketch.plane.point(
                        *graph
                            .points
                            .get(b)
                            .ok_or_else(|| Error::Invalid("Missing sweep point".into()))?,
                        sketch.origin,
                    );
                    if last.is_some_and(|p: DVec3| p.distance(a) > crate::EPS) {
                        return invalid("Sweep path entities are not ordered/connected");
                    }
                    spine.push(Edge::line(a, b)?);
                    last = Some(b);
                }
                vec![Geometry::Occt(Solid::sweep(
                    &edges,
                    &spine,
                    ProfileOrient::Torsion,
                )?)]
            }
            FeatureKind::Loft { sections, ruled } => {
                new_occt()?;
                let mut wires = vec![];
                for r in sections {
                    let s = d.sketches.iter().find(|s| s.id == r.sketch_id).unwrap();
                    if !s.region(r.region_id)?.inner.is_empty() {
                        return invalid("Loft with inner loops is not supported yet");
                    }
                    wires.push(region_edges(d, r, bodies)?);
                }
                vec![Geometry::Occt(Solid::loft(
                    wires.iter().map(|w| w.iter()),
                    *ruled,
                )?)]
            }
            FeatureKind::Hole {
                center,
                start_z,
                diameter,
                depth,
            } => {
                let t = required()?;
                let tool = d.backend.extrude(&ExtrudeInput {
                    feature_id: self.id,
                    region_id: self.id,
                    loop_ids: vec![1],
                    outer: Profile::Circle {
                        center: *center,
                        radius: length(diameter)? * 0.5,
                    },
                    holes: vec![],
                    plane: Plane::XY,
                    origin: [0., 0., *start_z],
                    distance: length(depth)?,
                })?;
                removed.push(t.id);
                d.backend
                    .boolean(&t.solid, &tool, Operation::Cut, self.id)?
            }
            FeatureKind::Fillet { radius } => {
                let s = occt()?;
                removed.push(required()?.id);
                vec![Geometry::Occt(
                    s.fillet_edges(length(radius)?, s.iter_edge())?,
                )]
            }
            FeatureKind::Chamfer { distance } => {
                let s = occt()?;
                removed.push(required()?.id);
                vec![Geometry::Occt(
                    s.chamfer_edges(length(distance)?, s.iter_edge())?,
                )]
            }
            FeatureKind::Shell {
                thickness,
                open_axis,
                positive,
            } => {
                let s = occt()?;
                let faces = extreme_faces(s, *open_axis, *positive)?;
                removed.push(required()?.id);
                vec![Geometry::Occt(s.shell(-length(thickness)?, faces)?)]
            }
            FeatureKind::OffsetFace {
                distance,
                axis,
                positive,
            } => {
                let s = occt()?;
                let faces = extreme_faces(s, *axis, *positive)?;
                removed.push(required()?.id);
                vec![Geometry::Occt(s.offset(
                    length(distance)?,
                    faces,
                    crate::EPS,
                )?)]
            }
            FeatureKind::Translate { offset, copy } => {
                let t = required()?;
                if !copy {
                    removed.push(t.id);
                }
                let vector = [
                    length(&offset[0])?,
                    length(&offset[1])?,
                    length(&offset[2])?,
                ];
                vec![translate(&t.solid, vector, self.id)?]
            }
            FeatureKind::Mirror { axis, origin } => {
                let s = occt()?;
                vec![Geometry::Occt(
                    s.clone().mirror(DVec3::from_array(*origin), axis.vector()),
                )]
            }
            FeatureKind::RectangularPattern {
                axis,
                spacing,
                count,
            } => {
                let t = required()?;
                let spacing = length(spacing)?;
                (1..*count)
                    .map(|i| {
                        let vector = (axis.vector() * (spacing * i as f64)).to_array();
                        if let Geometry::Rust(s) = &t.solid {
                            return Ok(Geometry::Rust(
                                s.translated_instance(vector, self.id, i as u64)?,
                            ));
                        }
                        translate(&t.solid, vector, self.id)
                    })
                    .collect::<Result<_>>()?
            }
            FeatureKind::CircularPattern {
                axis,
                origin,
                degrees,
                count,
            } => {
                let s = occt()?;
                let step = if (degrees.abs() - 360.).abs() < crate::EPS {
                    *degrees / *count as f64
                } else {
                    *degrees / (*count - 1) as f64
                };
                (1..*count)
                    .map(|i| {
                        Geometry::Occt(s.clone().rotate(
                            DVec3::from_array(*origin),
                            axis.vector(),
                            (step * i as f64).to_radians(),
                        ))
                    })
                    .collect()
            }
            FeatureKind::Combine {
                tool_body,
                operation,
                keep_tool,
            } => {
                let t = required()?;
                if t.id == *tool_body {
                    return invalid("Combine needs two different bodies");
                }
                let tool = find(*tool_body)?;
                removed.push(t.id);
                if !keep_tool {
                    removed.push(tool.id);
                }
                d.backend
                    .boolean(&t.solid, &tool.solid, *operation, self.id)?
            }
            FeatureKind::SplitBody { axis, position } => {
                let t = required()?;
                let bounds = t.solid.bounds();
                let axis = axis.index();
                let position = length(position)?;
                if position <= bounds[0][axis] + crate::EPS
                    || position >= bounds[1][axis] - crate::EPS
                {
                    return invalid("Split plane must cross the selected body");
                }
                let mut solids = vec![];
                removed.push(t.id);
                for upper in [false, true] {
                    if d.backend == Backend::Occt {
                        let normal = match axis {
                            0 => DVec3::X,
                            1 => DVec3::Y,
                            _ => DVec3::Z,
                        };
                        let tool = Geometry::Occt(Solid::half_space(
                            normal * position,
                            normal * if upper { 1. } else { -1. },
                        ));
                        solids.extend(d.backend.boolean(
                            &t.solid,
                            &tool,
                            Operation::Intersect,
                            self.id,
                        )?);
                        continue;
                    }
                    let mut bounds = bounds;
                    if upper {
                        bounds[0][axis] = position;
                    } else {
                        bounds[1][axis] = position;
                    }
                    let cutter = d.backend.extrude(&ExtrudeInput {
                        feature_id: self.id,
                        region_id: self.id,
                        loop_ids: vec![1],
                        outer: Profile::Rectangle {
                            a: [bounds[0][0], bounds[0][1]],
                            b: [bounds[1][0], bounds[1][1]],
                        },
                        holes: vec![],
                        plane: Plane::XY,
                        origin: [0., 0., bounds[0][2]],
                        distance: bounds[1][2] - bounds[0][2],
                    })?;
                    solids.extend(d.backend.boolean(
                        &t.solid,
                        &cutter,
                        Operation::Intersect,
                        self.id,
                    )?);
                }
                solids
            }
        };
        Ok(Outcome { removed, solids })
    }
}
fn angle(value: f64) -> Result<()> {
    if !value.is_finite() || value.abs() < 1e-8 || value.abs() > 360. {
        invalid("Angle must be nonzero and within ±360 degrees")
    } else {
        Ok(())
    }
}
fn count_valid(count: u32) -> Result<()> {
    if !(2..=100).contains(&count) {
        invalid("Pattern count must be 2..100")
    } else {
        Ok(())
    }
}
fn point(p: [f64; 3]) -> Result<()> {
    for n in p {
        crate::number(n)?;
    }
    Ok(())
}
fn region_edges(d: &Document, r: &RegionRef, bodies: &[Body]) -> Result<Vec<Edge>> {
    let s = d.resolved_sketch(r.sketch_id, bodies)?;
    let region = s.region(r.region_id)?;
    let mut edges = s.profile_by_id(region.outer)?.edges(s.plane, s.origin)?;
    for hole in region.inner {
        edges.extend(s.profile_by_id(hole)?.edges(s.plane, s.origin)?);
    }
    Ok(edges)
}
fn extreme_faces(s: &Solid, axis: Axis, positive: bool) -> Result<Vec<&cadrum::Face>> {
    let index = axis.index();
    let bound = s.bounding_box()[usize::from(positive)][index];
    let faces: Vec<_> = s
        .iter_face()
        .filter(|f| {
            f.surface().is_some_and(|surface| {
                matches!(surface.kind, SurfaceKind::Plane)
                    && surface.axis_z.dot(axis.vector()).abs() > 1. - 1e-8
            }) && (f.center()[index] - bound).abs() < crate::EPS
        })
        .collect();
    if faces.is_empty() {
        return invalid("Selected body has no planar face at that extreme");
    }
    Ok(faces)
}
fn translate(geometry: &Geometry, offset: [f64; 3], feature: u64) -> Result<Geometry> {
    Ok(match geometry {
        Geometry::Occt(s) => Geometry::Occt(s.clone().translate(DVec3::from_array(offset))),
        Geometry::Rust(s) => Geometry::Rust(s.translated(offset, feature)?),
    })
}
