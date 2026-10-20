//! Native OpenCascade geometry. No renderer or JavaScript runtime dependencies.
use cadrum::{DVec3, Edge, Solid, Tessellation};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, io::Cursor, path::Path};

pub const EPS: f64 = 1e-5;
pub mod arc;
mod arrangement;
pub mod boundary;
pub mod expression;
pub mod extrusion;
pub mod features;
pub mod kernel;
mod line_edit;
pub mod persistence;
pub mod planar;
pub mod region;
pub mod sketch;
mod sketch_edit;
use kernel::{Backend, Geometry, SolidKernel};
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Kernel(#[from] cadrum::Error),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
}
pub type Result<T> = std::result::Result<T, Error>;
fn invalid<T>(message: &str) -> Result<T> {
    Err(Error::Invalid(message.into()))
}
fn number(n: f64) -> Result<()> {
    if !n.is_finite() || n.abs() > 1_000_000. {
        invalid("Dimensions must be finite and within ±1,000,000 mm")
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum Plane {
    #[default]
    XY,
    XZ,
    YZ,
}
impl Plane {
    pub fn point(self, p: [f64; 2], origin: [f64; 3]) -> DVec3 {
        let local = match self {
            Self::XY => DVec3::new(p[0], p[1], 0.),
            Self::XZ => DVec3::new(p[0], 0., p[1]),
            Self::YZ => DVec3::new(0., p[0], p[1]),
        };
        local + DVec3::from_array(origin)
    }
    pub fn normal(self) -> DVec3 {
        match self {
            Self::XY => DVec3::Z,
            Self::XZ => -DVec3::Y,
            Self::YZ => DVec3::X,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Profile {
    Rectangle {
        a: [f64; 2],
        b: [f64; 2],
    },
    Circle {
        center: [f64; 2],
        radius: f64,
    },
    Polygon {
        points: Vec<[f64; 2]>,
    },
    Curved {
        segments: Vec<boundary::BoundarySegment>,
    },
}
impl Profile {
    pub fn geometrically_equal(&self, other: &Self, tolerance: f64) -> bool {
        if matches!(self, Self::Curved { .. }) || matches!(other, Self::Curved { .. }) {
            let a = self.boundary();
            let b = other.boundary();
            return a.len() == b.len()
                && a.iter().zip(b).all(|(a, b)| match (a.curve, b.curve) {
                    (boundary::PlanarCurve::Line { .. }, boundary::PlanarCurve::Line { .. }) => {
                        [0., 1.].iter().all(|t| {
                            boundary::distance(a.curve.point(*t), b.curve.point(*t)) < tolerance
                        })
                    }
                    (
                        boundary::PlanarCurve::Arc { geometry: x },
                        boundary::PlanarCurve::Arc { geometry: y },
                    ) => {
                        boundary::distance(x.center, y.center) < tolerance
                            && (x.radius - y.radius).abs() < tolerance
                            && (x.sweep - y.sweep).abs() * x.radius < tolerance
                            && boundary::distance(x.point(0.), y.point(0.)) < tolerance
                    }
                    _ => false,
                });
        }
        match (self, other) {
            (
                Self::Circle {
                    center: a,
                    radius: ar,
                },
                Self::Circle {
                    center: b,
                    radius: br,
                },
            ) => {
                (ar - br).abs() < tolerance
                    && a.iter().zip(b).all(|(a, b)| (a - b).abs() < tolerance)
            }
            (Self::Circle { .. }, _) | (_, Self::Circle { .. }) => false,
            _ => {
                let a = self.outline(0);
                let b = other.outline(0);
                a.len() == b.len()
                    && a.iter()
                        .flatten()
                        .zip(b.iter().flatten())
                        .all(|(a, b)| (a - b).abs() < tolerance)
            }
        }
    }
    pub fn outline(&self, segments: usize) -> Vec<[f64; 2]> {
        match self {
            Self::Rectangle { a, b } => vec![*a, [b[0], a[1]], *b, [a[0], b[1]]],
            Self::Circle { center, radius } => (0..segments)
                .map(|i| {
                    let t = i as f64 / segments as f64 * std::f64::consts::TAU;
                    [center[0] + radius * t.cos(), center[1] + radius * t.sin()]
                })
                .collect(),
            Self::Polygon { points } => points.clone(),
            Self::Curved { segments: boundary } => boundary
                .iter()
                .flat_map(|s| {
                    let count = match s.curve {
                        boundary::PlanarCurve::Line { .. } => 1,
                        boundary::PlanarCurve::Arc { geometry: a } => {
                            ((segments.max(8) as f64 * a.sweep.abs() / std::f64::consts::TAU).ceil()
                                as usize)
                                .max(1)
                        }
                    };
                    (0..count).map(move |i| s.curve.point(i as f64 / count as f64))
                })
                .collect(),
        }
    }
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Rectangle { a, b } => {
                for n in a.iter().chain(b) {
                    number(*n)?;
                }
                if (a[0] - b[0]).abs() < EPS || (a[1] - b[1]).abs() < EPS {
                    return invalid("Rectangle width and height must be nonzero");
                }
            }
            Self::Circle { center, radius } => {
                for n in center.iter().chain(std::iter::once(radius)) {
                    number(*n)?;
                }
                if *radius < EPS {
                    return invalid("Circle radius must be positive");
                }
            }
            Self::Polygon { points } => validate_polygon(points)?,
            Self::Curved { segments } => boundary::validate_loop(segments)?,
        }
        Ok(())
    }
    pub fn edges(&self, plane: Plane, origin: [f64; 3]) -> Result<Vec<Edge>> {
        self.validate()?;
        if let Self::Circle { center, radius } = self {
            return Ok(vec![
                Edge::circle(*radius, plane.normal())?.translate(plane.point(*center, origin)),
            ]);
        }
        if let Self::Curved { segments } = self {
            return segments
                .iter()
                .map(|s| match s.curve {
                    boundary::PlanarCurve::Line { start, end } => Ok(Edge::line(
                        plane.point(start, origin),
                        plane.point(end, origin),
                    )?),
                    boundary::PlanarCurve::Arc { geometry: a } => Ok(Edge::arc_3pts(
                        plane.point(a.point(0.), origin),
                        plane.point(a.point(0.5), origin),
                        plane.point(a.point(1.), origin),
                    )?),
                })
                .collect();
        }
        let mut points: Vec<DVec3> = self
            .outline(64)
            .iter()
            .map(|p| plane.point(*p, origin))
            .collect();
        points.push(points[0]);
        Ok(Edge::polygon(&points)?)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Sketch {
    pub id: u64,
    pub name: String,
    pub plane: Plane,
    pub origin: [f64; 3],
    pub profiles: Vec<Profile>,
    #[serde(default)]
    pub profile_ids: Vec<u64>,
    #[serde(default = "first_id")]
    pub next_profile_id: u64,
    #[serde(default)]
    pub dimension_expressions:
        std::collections::BTreeMap<u64, std::collections::BTreeMap<String, String>>,
    #[serde(default)]
    pub graph: Option<sketch::SketchGraph>,
    #[serde(default)]
    pub support: Option<features::PlanarSupport>,
}
fn first_id() -> u64 {
    1
}
impl Default for Sketch {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            plane: Plane::XY,
            origin: [0.; 3],
            profiles: vec![],
            profile_ids: vec![],
            next_profile_id: 1,
            dimension_expressions: Default::default(),
            graph: None,
            support: None,
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub enum Operation {
    #[default]
    NewBody,
    Join,
    Cut,
    Intersect,
}
impl Operation {
    pub fn label(self) -> &'static str {
        match self {
            Self::NewBody => "New Body",
            Self::Join => "Join",
            Self::Cut => "Cut",
            Self::Intersect => "Intersect",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Extrusion {
    pub id: u64,
    pub name: String,
    pub sketch_id: u64,
    pub profile: usize,
    pub distance: f64,
    pub operation: Operation,
    pub body_id: u64,
    #[serde(default)]
    pub region_id: Option<u64>,
    #[serde(default)]
    pub distance_expression: Option<String>,
    /// Materialized Boolean output IDs. Legacy rectangular outputs retain
    /// bounds ordering; new analytic outputs use source relationship ordering.
    #[serde(default)]
    pub result_body_ids: Vec<u64>,
    /// Accepted complete region boundary selection (outer followed by holes).
    /// New/legacy commands acquire this snapshot during preparation.
    #[serde(default)]
    pub boundary_dependencies: Option<Vec<u64>>,
    #[serde(default)]
    pub options: extrusion::ExtrudeOptions,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub units: String,
    pub sketches: Vec<Sketch>,
    pub extrusions: Vec<Extrusion>,
    #[serde(default)]
    pub parameters: expression::Parameters,
    #[serde(default)]
    pub backend: Backend,
    #[serde(default)]
    pub features: Vec<features::Feature>,
    #[serde(default)]
    pub timeline: Vec<u64>,
    #[serde(default)]
    pub suppressed: std::collections::BTreeSet<u64>,
}
impl Default for Document {
    fn default() -> Self {
        Self {
            format: "openfusion-native".into(),
            version: 5,
            name: "Untitled".into(),
            units: "mm".into(),
            sketches: vec![],
            extrusions: vec![],
            parameters: Default::default(),
            backend: Backend::Occt,
            features: vec![],
            timeline: vec![],
            suppressed: Default::default(),
        }
    }
}
impl Document {
    pub fn feature_order(&self) -> Result<Vec<u64>> {
        if self.timeline.is_empty() {
            return Ok(self
                .extrusions
                .iter()
                .map(|e| e.id)
                .chain(self.features.iter().map(|f| f.id))
                .collect());
        }
        let expected: HashSet<_> = self
            .extrusions
            .iter()
            .map(|e| e.id)
            .chain(self.features.iter().map(|f| f.id))
            .collect();
        let actual: HashSet<_> = self.timeline.iter().copied().collect();
        if actual != expected || actual.len() != self.timeline.len() {
            return invalid("Timeline references are missing, duplicate or invalid");
        }
        Ok(self.timeline.clone())
    }
    pub fn push_feature(&mut self, feature: features::Feature) -> Result<()> {
        if self.timeline.is_empty() {
            self.timeline = self.feature_order()?;
        }
        self.timeline.push(feature.id);
        self.features.push(feature);
        Ok(())
    }
    pub fn push_extrusion(&mut self, extrusion: Extrusion) {
        if !self.timeline.is_empty() {
            self.timeline.push(extrusion.id);
        }
        self.extrusions.push(extrusion);
    }
    pub fn recomputed(&self) -> Result<Self> {
        let mut d = self.upgraded()?;
        for s in &mut d.sketches {
            if let Some(graph) = &s.graph {
                *s = s.with_graph(graph.clone(), &d.parameters)?;
            }
        }
        for e in &mut d.extrusions {
            if let Some(text) = &e.distance_expression {
                e.distance = expression::length(text, &d.parameters)?;
            }
        }
        d.validate()?;
        Ok(d)
    }
    pub fn with_parameter(&self, name: String, text: String) -> Result<Self> {
        let mut d = self.clone();
        d.parameters.insert(name, text);
        d.recomputed()
    }
    /// Resolve all v1 positional references before generating any regions.
    pub fn upgraded(&self) -> Result<Self> {
        if self.version < 5
            && (self
                .extrusions
                .iter()
                .any(|e| e.boundary_dependencies.is_some())
                || self.sketches.iter().any(|s| {
                    s.profiles
                        .iter()
                        .any(|p| matches!(p, Profile::Curved { .. }))
                        || s.graph
                            .as_ref()
                            .is_some_and(|g| !g.curve_regions.is_empty())
                }))
        {
            return invalid("Curved semantics require version 5");
        }
        if [2, 3, 4].contains(&self.version) {
            let mut d = self.clone();
            d.version = 5;
            for s in &mut d.sketches {
                if let Some(g) = &s.graph {
                    *s = s.refresh_line_regions(g.clone())?;
                }
            }
            return Ok(d);
        }
        if self.version != 1 {
            return Ok(self.clone());
        }
        self.validate()?;
        let mut d = self.clone();
        for s in &mut d.sketches {
            s.profile_ids = (1..=s.profiles.len() as u64).collect();
            s.next_profile_id = s.profiles.len() as u64 + 1;
            let mut graph = sketch::SketchGraph::default();
            for (id, p) in s.profile_ids.iter().zip(&s.profiles) {
                graph.add_profile(*id, p)?;
            }
            s.graph = Some(graph);
        }
        for e in &mut d.extrusions {
            let s = d.sketches.iter().find(|s| s.id == e.sketch_id).unwrap();
            let original_profile_id = s.profile_ids[e.profile];
            let region = s.region(original_profile_id)?;
            if !region.inner.is_empty() {
                return invalid(
                    "Legacy profile selection is ambiguous after region discovery; original file was not modified",
                );
            }
            e.region_id = Some(region.id);
        }
        d.version = 5;
        Ok(d)
    }
    pub fn next_id(&self) -> Result<u64> {
        self.sketches
            .iter()
            .map(|s| s.id)
            .chain(self.extrusions.iter().map(|e| e.id))
            .chain(self.extrusions.iter().map(|e| e.body_id))
            .chain(self.features.iter().map(|f| f.id))
            .chain(self.features.iter().flat_map(|f| f.outputs.iter().copied()))
            .chain(
                self.extrusions
                    .iter()
                    .flat_map(|e| e.result_body_ids.iter().copied()),
            )
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| Error::Invalid("Document ID space is exhausted".into()))
    }
    pub fn validate(&self) -> Result<()> {
        if self.format != "openfusion-native"
            || ![1, 2, 3, 4, 5].contains(&self.version)
            || self.units != "mm"
        {
            return invalid("Unsupported document format, version, or units");
        }
        if self.sketches.len() > 1000 || self.extrusions.len() > 1000 || self.features.len() > 1000
        {
            return invalid("Document exceeds feature limit");
        }
        if self.parameters.len() > 256 {
            return invalid("Parameter limit exceeded");
        }
        for (name, text) in &self.parameters {
            if name.is_empty()
                || name.len() > 64
                || !name.as_bytes()[0].is_ascii_alphabetic()
                || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
                || ["mm", "cm", "m", "in", "ft"].contains(&name.as_str())
            {
                return invalid("Invalid or reserved parameter name");
            }
            expression::length(text, &self.parameters)?;
        }
        let mut ids = HashSet::new();
        for s in &self.sketches {
            if !ids.insert(s.id) {
                return invalid("Duplicate feature ID");
            }
            if s.profiles.len() > 1000 {
                return invalid("Sketch exceeds profile limit");
            }
            for n in s.origin {
                number(n)?;
            }
            if let Some(support) = &s.support {
                number(support.offset)?;
                let expected = match support.axis {
                    features::Axis::X => Plane::YZ,
                    features::Axis::Y => Plane::XZ,
                    features::Axis::Z => Plane::XY,
                };
                if s.plane != expected {
                    return invalid("Sketch support axis does not match its plane");
                }
            }
            for p in &s.profiles {
                if self.version < 5 && matches!(p, Profile::Curved { .. }) {
                    return invalid("Curved boundaries require document version 5");
                }
                p.validate()?;
            }
            if self.version >= 2 {
                let unique: HashSet<_> = s.profile_ids.iter().copied().collect();
                if unique.len() != s.profile_ids.len()
                    || unique.contains(&0)
                    || s.profile_ids.iter().any(|id| *id >= s.next_profile_id)
                {
                    return invalid("Invalid or exhausted sketch profile identities");
                }
                s.regions()?;
                if let Some(graph) = &s.graph {
                    if self.version < 5 && !graph.curve_regions.is_empty() {
                        return invalid("Curved provenance requires document version 5");
                    }
                    graph.validate()?;
                    if graph
                        .region_cycles
                        .keys()
                        .chain(graph.curve_regions.keys())
                        .any(|id| *id >= s.next_profile_id)
                    {
                        return invalid("Invalid retained sketch region identity");
                    }
                    graph.validate_solution(&self.parameters)?;
                    for (id, p) in s.profile_ids.iter().zip(&s.profiles) {
                        let derived = graph.profile(*id, p)?;
                        if let (Profile::Curved { segments: a }, Profile::Curved { segments: b }) =
                            (p, &derived)
                        {
                            if a.iter().map(|s| s.source).ne(b.iter().map(|s| s.source)) {
                                return invalid(
                                    "Curved boundary source cache differs from its entity dependencies",
                                );
                            }
                        }
                        if !p.geometrically_equal(&derived, EPS) {
                            return invalid(
                                "Sketch geometry cache differs from solved entities; recompute dependencies first",
                            );
                        }
                    }
                }
            }
        }
        for e in &self.extrusions {
            if !ids.insert(e.id) {
                return invalid("Duplicate feature ID");
            }
            number(e.distance)?;
            if e.distance.abs() < EPS {
                return invalid("Extrusion distance must be nonzero");
            }
            e.options.validate(self, e.distance, e.operation)?;
            let s = self
                .sketches
                .iter()
                .find(|s| s.id == e.sketch_id)
                .ok_or_else(|| Error::Invalid(format!("{}: missing sketch", e.name)))?;
            if self.version == 1 && e.profile >= s.profiles.len() {
                return invalid("Extrusion references a missing profile");
            }
            if self.version >= 2 {
                let region = s
                    .region(e.region_id.ok_or_else(|| {
                        Error::Invalid("Missing stable region reference".into())
                    })?)?;
                if let Some(dependencies) = &e.boundary_dependencies {
                    let current: Vec<_> =
                        std::iter::once(region.outer).chain(region.inner).collect();
                    if *dependencies != current {
                        return invalid(
                            "Extrusion region split/merged or its hole dependencies changed; repair selection explicitly",
                        );
                    }
                }
            }
            if let Some(text) = &e.distance_expression {
                if (expression::length(text, &self.parameters)? - e.distance).abs() > EPS {
                    return invalid(
                        "Persisted distance differs from its expression; recompute dependencies first",
                    );
                }
            }
        }
        for f in &self.features {
            if !ids.insert(f.id) {
                return invalid("Duplicate feature ID");
            }
            f.validate(self)?;
        }
        let order = self.feature_order()?;
        if self.suppressed.iter().any(|id| !order.contains(id)) {
            return invalid("Suppression references a missing feature");
        }
        Ok(())
    }
    pub fn from_json(json: &str) -> Result<Self> {
        let d: Self = serde_json::from_str(json)?;
        let d = d.upgraded()?;
        d.validate()?;
        Ok(d)
    }
    pub fn to_json(&self) -> Result<String> {
        self.validate()?;
        if self.version == 1 {
            if !self.features.is_empty()
                || !self.parameters.is_empty()
                || self.backend != Backend::Occt
                || !self.timeline.is_empty()
                || !self.suppressed.is_empty()
                || self.sketches.iter().any(|s| {
                    s.support.is_some() || !s.dimension_expressions.is_empty() || s.graph.is_some()
                })
                || self
                    .extrusions
                    .iter()
                    .any(|e| e.distance_expression.is_some() || e.options != Default::default())
            {
                return invalid(
                    "Legacy v1 cannot encode these feature/parameter/backend extensions",
                );
            }
            let sketches:Vec<_>=self.sketches.iter().map(|s|serde_json::json!({"id":s.id,"name":s.name,"plane":s.plane,"origin":s.origin,"profiles":s.profiles})).collect();
            let extrusions:Vec<_>=self.extrusions.iter().map(|e|serde_json::json!({"id":e.id,"name":e.name,"sketch_id":e.sketch_id,"profile":e.profile,"distance":e.distance,"operation":e.operation,"body_id":e.body_id})).collect();
            return Ok(serde_json::to_string_pretty(
                &serde_json::json!({"format":self.format,"version":1,"name":self.name,"units":self.units,"sketches":sketches,"extrusions":extrusions}),
            )?);
        }
        if [2, 3, 4].contains(&self.version) {
            return self.upgraded()?.to_json();
        }
        Ok(serde_json::to_string_pretty(self)?)
    }
}
pub struct Body {
    pub id: u64,
    pub name: String,
    pub solid: Geometry,
}
pub struct Model {
    pub bodies: Vec<Body>,
}
#[derive(Clone, Debug, Default)]
pub struct DisplayMesh {
    pub vertices: Vec<[f64; 3]>,
    pub normals: Vec<[f64; 3]>,
    pub indices: Vec<usize>,
    pub edges: Vec<[f64; 3]>,
    pub face_ids: Vec<Option<kernel::native::TopologyId>>,
    pub body_ids: Vec<u64>,
    pub edge_ids: Vec<kernel::native::TopologyId>,
}
#[derive(Clone, Debug)]
pub struct BodyInfo {
    pub id: u64,
    pub name: String,
    pub volume: f64,
    /// Kernel-measured surface area in square millimetres.
    pub surface_area: f64,
    pub bounds: [[f64; 3]; 2],
    pub backend: Backend,
}
impl Model {
    pub fn build(document: &Document) -> Result<Self> {
        Self::build_internal(document, false).map(|(_, model)| model)
    }
    /// Materialize output identities once, then persist them with the feature.
    pub fn prepare(document: &Document) -> Result<(Document, Self)> {
        Self::build_internal(document, true)
    }
    fn build_internal(document: &Document, allocate_outputs: bool) -> Result<(Document, Self)> {
        let mut document = document.upgraded()?;
        document.validate()?;
        let mut bodies: Vec<Body> = vec![];
        for id in document.feature_order()? {
            if document.suppressed.contains(&id) {
                continue;
            }
            for i in 0..document.sketches.len() {
                if document.sketches[i]
                    .support
                    .as_ref()
                    .is_some_and(|s| bodies.iter().any(|b| b.id == s.body_id))
                {
                    document.sketches[i] =
                        document.resolved_sketch(document.sketches[i].id, &bodies)?;
                }
            }
            if let Some(i) = document.features.iter().position(|f| f.id == id) {
                let feature = document.features[i].clone();
                let mut outcome = feature.evaluate(&document, &bodies)?;
                if matches!(
                    feature.kind,
                    features::FeatureKind::SplitBody { .. }
                        | features::FeatureKind::Combine { .. }
                        | features::FeatureKind::Hole { .. }
                ) {
                    outcome.solids.sort_by(Geometry::output_cmp);
                }
                let outputs = if feature.outputs.len() != outcome.solids.len() {
                    if !feature.outputs.is_empty()
                        && matches!(
                            feature.kind,
                            features::FeatureKind::Combine { .. }
                                | features::FeatureKind::SplitBody { .. }
                                | features::FeatureKind::Hole { .. }
                        )
                    {
                        return invalid(
                            "Feature result count changed; repair output references explicitly",
                        );
                    }
                    if !allocate_outputs {
                        return invalid(
                            "Feature output identities must be materialized before replay",
                        );
                    }
                    let mut outputs = feature.outputs.clone();
                    outputs.truncate(outcome.solids.len());
                    let mut next = document.next_id()?;
                    if let Some(target) = feature.target.filter(|id| outcome.removed.contains(id)) {
                        if outputs.is_empty() && !outcome.solids.is_empty() {
                            outputs.push(target);
                        }
                    }
                    while outputs.len() < outcome.solids.len() {
                        outputs.push(next);
                        next = next
                            .checked_add(1)
                            .ok_or_else(|| Error::Invalid("Document ID space exhausted".into()))?;
                    }
                    document.features[i].outputs = outputs.clone();
                    outputs
                } else {
                    if feature.outputs.len() != outcome.solids.len() {
                        return invalid(
                            "Feature result count changed; repair output references explicitly",
                        );
                    }
                    feature.outputs.clone()
                };
                bodies.retain(|b| !outcome.removed.contains(&b.id));
                for (mut solid, id) in outcome.solids.into_iter().zip(outputs) {
                    if bodies.iter().any(|b| b.id == id) {
                        return invalid("Feature output body identity collides");
                    }
                    if !solid.volume().is_finite() || solid.volume() <= 0. {
                        return invalid("Feature produced an invalid solid");
                    }
                    if let Geometry::Rust(s) = &mut solid {
                        s.bind_body(id);
                    }
                    bodies.push(Body {
                        id,
                        name: format!("Body{id}"),
                        solid,
                    });
                }
                continue;
            }
            let feature_index = document
                .extrusions
                .iter()
                .position(|e| e.id == id)
                .ok_or_else(|| Error::Invalid("Missing timeline extrusion".into()))?;
            let e = document.extrusions[feature_index].clone();
            let s = document.resolved_sketch(e.sketch_id, &bodies)?;
            if allocate_outputs && e.boundary_dependencies.is_none() {
                let region = s.region(
                    e.region_id
                        .ok_or_else(|| Error::Invalid("Missing stable region".into()))?,
                )?;
                document.extrusions[feature_index].boundary_dependencies =
                    Some(std::iter::once(region.outer).chain(region.inner).collect());
            }
            let tool = e.tool(&document, &s, &bodies)?;
            let index = bodies.iter().position(|b| b.id == e.body_id);
            let mut solids = match e.operation {
                Operation::NewBody => {
                    if index.is_some() {
                        return invalid("Body ID already exists");
                    }
                    vec![tool]
                }
                operation => {
                    let i = index.ok_or_else(|| {
                        Error::Invalid(format!("{}: target body is missing", e.name))
                    })?;
                    let target = &bodies[i].solid;
                    document.backend.boolean(target, &tool, operation, e.id)?
                }
            };
            solids.sort_by(Geometry::output_cmp);
            let outputs = if e.result_body_ids.is_empty() {
                if solids.len() > 1 {
                    if !allocate_outputs {
                        return invalid("Multiple Boolean bodies require explicit result_body_ids");
                    }
                    let mut outputs = vec![e.body_id];
                    let mut next = document.next_id()?;
                    for _ in 1..solids.len() {
                        outputs.push(next);
                        next = next.checked_add(1).ok_or_else(|| {
                            Error::Invalid("Document ID space is exhausted".into())
                        })?;
                    }
                    document.extrusions[feature_index].result_body_ids = outputs.clone();
                    outputs
                } else {
                    vec![e.body_id; solids.len()]
                }
            } else {
                if e.result_body_ids.len() != solids.len() {
                    return invalid(
                        "Boolean result count changed; output references require explicit repair",
                    );
                }
                e.result_body_ids.clone()
            };
            if let Some(i) = index {
                bodies.remove(i);
            }
            for (mut solid, id) in solids.into_iter().zip(outputs) {
                if bodies.iter().any(|b| b.id == id) {
                    return invalid("Result body ID collides with an existing body");
                }
                let volume = solid.volume();
                if !volume.is_finite() || volume <= 0. {
                    return invalid("Operation produced an invalid solid");
                }
                if let Geometry::Rust(s) = &mut solid {
                    s.bind_body(id);
                }
                bodies.push(Body {
                    id,
                    name: format!("Body{id}"),
                    solid,
                });
            }
        }
        document.validate()?;
        Ok((document, Self { bodies }))
    }
    pub fn info(&self) -> Vec<BodyInfo> {
        self.bodies
            .iter()
            .map(|b| BodyInfo {
                id: b.id,
                name: b.name.clone(),
                volume: b.solid.volume(),
                surface_area: b.solid.area(),
                bounds: b.solid.bounds(),
                backend: b.solid.backend(),
            })
            .collect()
    }
    pub fn display_mesh(&self) -> Result<DisplayMesh> {
        if self.bodies.is_empty() {
            return Ok(DisplayMesh::default());
        }
        let mut combined = DisplayMesh::default();
        for b in &self.bodies {
            let m = b.solid.mesh()?;
            let offset = combined.vertices.len();
            combined
                .indices
                .extend(m.indices.iter().map(|i| i + offset));
            combined.vertices.extend(m.vertices);
            combined.normals.extend(m.normals);
            combined.edges.extend(m.edges);
            combined.face_ids.extend(m.face_ids);
            combined
                .body_ids
                .extend(std::iter::repeat_n(b.id, m.indices.len() / 3));
            combined.edge_ids.extend(m.edge_ids);
        }
        Ok(combined)
    }
    pub fn export(&self, path: &Path) -> Result<()> {
        if self.bodies.is_empty() {
            return invalid("There are no solid bodies to export");
        }
        let mut bytes = vec![];
        match path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str()
        {
            "step" | "stp" => bytes = self.step_bytes()?,
            "stl" => {
                let solids = self.occt_solids()?;
                Solid::mesh(solids, Tessellation::default())?.write_stl(&mut bytes)?;
            }
            _ => return invalid("Choose a .step, .stp, or .stl output file"),
        }
        persistence::staged_write(path, |f| {
            use std::io::Write;
            f.write_all(&bytes)?;
            Ok(())
        })?;
        Ok(())
    }
    pub fn step_bytes(&self) -> Result<Vec<u8>> {
        let mut data = vec![];
        let solids = self.occt_solids()?;
        Solid::write_step(solids, &mut data)?;
        Ok(data)
    }
    fn occt_solids(&self) -> Result<Vec<&Solid>> {
        self.bodies.iter().map(|b| match &b.solid {
            Geometry::Occt(s) => Ok(s),
            Geometry::Rust(_) => invalid("Rust neutral-format export is unsupported; choose OCCT explicitly to rebuild/export"),
        }).collect()
    }
    pub fn step_volume(data: &[u8]) -> Result<f64> {
        Ok(Solid::read_step(&mut Cursor::new(data))?
            .iter()
            .map(Solid::volume)
            .sum())
    }
}
pub struct Session {
    pub document: Document,
    pub model: Model,
    undo: Vec<Document>,
    redo: Vec<Document>,
}
impl Session {
    pub fn new(document: Document) -> Result<Self> {
        let (document, model) = Model::prepare(&document)?;
        Ok(Self {
            document,
            model,
            undo: vec![],
            redo: vec![],
        })
    }
    pub fn replace(&mut self, document: Document) -> Result<()> {
        let (document, model) = Model::prepare(&document)?;
        self.check_output_ancestry(&model)?;
        model.display_mesh()?;
        self.commit(document, model);
        Ok(())
    }
    /// Geometry and display must both succeed before publishing any session state.
    pub fn replace_with_display(
        &mut self,
        document: Document,
        mesh: impl FnOnce(&Model) -> Result<DisplayMesh>,
    ) -> Result<DisplayMesh> {
        let (document, model) = Model::prepare(&document)?;
        self.check_output_ancestry(&model)?;
        let display = mesh(&model)?;
        self.commit(document, model);
        Ok(display)
    }
    fn check_output_ancestry(&self, model: &Model) -> Result<()> {
        for old in &self.model.bodies {
            let Some(new) = model.bodies.iter().find(|b| b.id == old.id) else {
                continue;
            };
            if let (Geometry::Rust(a), Geometry::Rust(b)) = (&old.solid, &new.solid) {
                let same_producer = a.provenance.last().map(|p| p.feature)
                    == b.provenance.last().map(|p| p.feature);
                if same_producer
                    && a.output_signature().is_some()
                    && a.output_signature() != b.output_signature()
                {
                    return invalid(
                        "Boolean output ancestry changed by a split/merge or boundary replacement; repair dependencies explicitly",
                    );
                }
            }
        }
        Ok(())
    }
    fn commit(&mut self, document: Document, model: Model) {
        self.undo
            .push(std::mem::replace(&mut self.document, document));
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.model = model;
    }
    pub fn undo(&mut self) -> Result<bool> {
        self.undo_with_display(Model::display_mesh)
            .map(|m| m.is_some())
    }
    pub fn undo_with_display(
        &mut self,
        mesh: impl FnOnce(&Model) -> Result<DisplayMesh>,
    ) -> Result<Option<DisplayMesh>> {
        let Some(d) = self.undo.last() else {
            return Ok(None);
        };
        let model = Model::build(d)?;
        let display = mesh(&model)?;
        self.redo.push(std::mem::replace(
            &mut self.document,
            self.undo.pop().unwrap(),
        ));
        self.model = model;
        Ok(Some(display))
    }
    pub fn redo(&mut self) -> Result<bool> {
        self.redo_with_display(Model::display_mesh)
            .map(|m| m.is_some())
    }
    pub fn redo_with_display(
        &mut self,
        mesh: impl FnOnce(&Model) -> Result<DisplayMesh>,
    ) -> Result<Option<DisplayMesh>> {
        let Some(d) = self.redo.last() else {
            return Ok(None);
        };
        let model = Model::build(d)?;
        let display = mesh(&model)?;
        self.undo.push(std::mem::replace(
            &mut self.document,
            self.redo.pop().unwrap(),
        ));
        self.model = model;
        Ok(Some(display))
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}
fn validate_polygon(points: &[[f64; 2]]) -> Result<()> {
    if !(3..=1000).contains(&points.len()) {
        return invalid("Polygon requires 3–1000 vertices");
    }
    fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    }
    fn on(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> bool {
        cross(a, b, p).abs() < EPS
            && (a[0].min(b[0]) - EPS..=a[0].max(b[0]) + EPS).contains(&p[0])
            && (a[1].min(b[1]) - EPS..=a[1].max(b[1]) + EPS).contains(&p[1])
    }
    let mut area = 0.;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        for n in a {
            number(n)?;
        }
        if (a[0] - b[0]).hypot(a[1] - b[1]) < EPS {
            return invalid("Polygon has a zero length edge");
        }
        let c = points[(i + 2) % points.len()];
        if on(a, b, c) || on(b, c, a) {
            return invalid("Polygon has overlapping adjacent edges");
        }
        area += a[0] * b[1] - a[1] * b[0];
        for j in i + 2..points.len() {
            if i == 0 && j == points.len() - 1 {
                continue;
            }
            let c = points[j];
            let d = points[(j + 1) % points.len()];
            if (cross(a, b, c) * cross(a, b, d) < 0. && cross(c, d, a) * cross(c, d, b) < 0.)
                || on(a, b, c)
                || on(a, b, d)
                || on(c, d, a)
                || on(c, d, b)
            {
                return invalid("Polygon is self intersecting");
            }
        }
    }
    if area.abs() < EPS * EPS {
        return invalid("Polygon has zero area");
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn block() -> Document {
        Document {
            sketches: vec![Sketch {
                id: 1,
                name: "Sketch1".into(),
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
            }],
            extrusions: vec![Extrusion {
                id: 2,
                name: "Extrude1".into(),
                sketch_id: 1,
                profile: 0,
                distance: 5.,
                operation: Operation::NewBody,
                body_id: 3,

                region_id: Some(1),
                distance_expression: None,
                result_body_ids: vec![],
                boundary_dependencies: None,
                options: Default::default(),
            }],
            ..Default::default()
        }
    }
    #[test]
    fn native_extrusion_and_step_roundtrip() {
        let m = Model::build(&block()).unwrap();
        assert!((m.info()[0].volume - 6000.).abs() < 1e-6);
        assert!(!m.display_mesh().unwrap().indices.is_empty());
        assert!((Model::step_volume(&m.step_bytes().unwrap()).unwrap() - 6000.).abs() < 1e-5);
    }
    #[test]
    fn native_cut_join_intersection() {
        for operation in [Operation::Cut, Operation::Join, Operation::Intersect] {
            let mut d = block();
            d.sketches.push(Sketch {
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
            d.extrusions.push(Extrusion {
                id: 5,
                name: "Boolean".into(),
                sketch_id: 4,
                profile: 0,
                distance: 10.,
                operation,
                body_id: 3,

                region_id: Some(1),
                distance_expression: None,
                result_body_ids: vec![],
                boundary_dependencies: None,
                options: Default::default(),
            });
            let expected = match operation {
                Operation::Cut => 6000. - 125. * std::f64::consts::PI,
                Operation::Join => 6000. + 125. * std::f64::consts::PI,
                _ => 125. * std::f64::consts::PI,
            };
            assert!((Model::build(&d).unwrap().info()[0].volume - expected).abs() < 1e-5);
        }
    }
    #[test]
    fn failed_edit_and_undo_preserve_geometry() {
        let mut s = Session::new(block()).unwrap();
        let mut d = s.document.clone();
        d.extrusions[0].distance = 10.;
        s.replace(d).unwrap();
        let mut bad = s.document.clone();
        bad.extrusions[0].distance = 0.;
        assert!(s.replace(bad).is_err());
        assert!((s.model.info()[0].volume - 12000.).abs() < 1e-5);
        s.undo().unwrap();
        assert!((s.model.info()[0].volume - 6000.).abs() < 1e-5);
        s.redo().unwrap();
        assert!((s.model.info()[0].volume - 12000.).abs() < 1e-5);
    }
    #[test]
    fn planes_negative_extrusions_and_validation() {
        for p in [Plane::XY, Plane::XZ, Plane::YZ] {
            let mut d = block();
            d.sketches[0].plane = p;
            d.extrusions[0].distance = -5.;
            assert!((Model::build(&d).unwrap().info()[0].volume - 6000.).abs() < 1e-5);
        }
        assert!(
            Profile::Polygon {
                points: vec![[0., 0.], [10., 10.], [0., 10.], [10., 0.]]
            }
            .validate()
            .is_err()
        );
        assert!(Document::from_json(&block().to_json().unwrap()).is_ok());
        let mut d = block();
        d.version = 99;
        assert!(d.validate().is_err());
    }
}
