//! Independently written analytic extrusion kernel. No cadrum/OCCT calls occur here.
//! Supported Booleans are deliberately bounded; all other cases return errors.
use super::ExtrudeInput;
use crate::{DisplayMesh, EPS, Operation, Plane, Profile, Result, invalid};
use serde::{Deserialize, Serialize};
mod cuts;
mod faces;
mod prisms;
mod steps;

#[derive(Debug, Clone, Copy)]
pub struct Tolerance {
    pub linear_mm: f64,
    pub relative: f64,
    pub angular_rad: f64,
    pub maximum_mm: f64,
}
pub const POLICY: Tolerance = Tolerance {
    linear_mm: 1e-7,
    relative: 1e-10,
    angular_rad: 1e-10,
    maximum_mm: 1_000_000.,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TopologyId {
    pub body: u64,
    pub feature: u64,
    pub region: u64,
    pub local: u64,
}
/// Original face identity and semantic transform instances, never coordinates.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct FaceSource {
    pub original: TopologyId,
    pub occurrences: Vec<(u64, u64)>,
}
/// Inspectable ancestry of a straight trim source. Occurrences on a planar
/// span compose with these original sources through subsequent translations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum BoundarySource {
    Edge(TopologyId),
    PlaneIntersection(
        std::collections::BTreeSet<FaceSource>,
        std::collections::BTreeSet<FaceSource>,
    ),
    FiniteGuard(
        std::collections::BTreeSet<FaceSource>,
        std::collections::BTreeSet<FaceSource>,
        usize,
    ),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Representation {
    UniformExtrusion,
    OccupiedSections,
    PlanarFaces,
}
#[derive(Debug, Clone)]
pub enum Curve {
    Line,
    Circle {
        center: [f64; 3],
        normal: [f64; 3],
        radius: f64,
    },
    Arc {
        center: [f64; 3],
        normal: [f64; 3],
        x_axis: [f64; 3],
        radius: f64,
        start_angle: f64,
        sweep: f64,
    },
}
#[derive(Debug, Clone)]
pub enum Surface {
    Plane {
        origin: [f64; 3],
        normal: [f64; 3],
    },
    Cylinder {
        origin: [f64; 3],
        axis: [f64; 3],
        radius: f64,
    },
}
#[derive(Debug, Clone)]
pub struct Vertex {
    pub point: [f64; 3],
}
#[derive(Debug, Clone)]
pub struct Edge {
    pub id: TopologyId,
    pub vertices: [usize; 2],
    pub curve: Curve,
    pub source_entity: Option<u64>,
}
#[derive(Debug, Clone)]
pub struct Coedge {
    pub edge: usize,
    pub reversed: bool,
}
#[derive(Debug, Clone)]
pub struct Face {
    pub id: TopologyId,
    pub surface: Surface,
    pub loops: Vec<Vec<Coedge>>,
    pub reversed: bool,
    pub source_entity: Option<u64>,
}
#[derive(Debug, Clone)]
pub struct Brep {
    pub vertices: Vec<Vertex>,
    pub edges: Vec<Edge>,
    pub faces: Vec<Face>,
}
fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
impl Brep {
    pub fn validate(&self) -> Result<()> {
        if self.vertices.is_empty() || self.faces.len() < 3 {
            return invalid("Rust B-rep has no closed shell");
        }
        let mut ids = std::collections::HashSet::new();
        for vertex in &self.vertices {
            if vertex
                .point
                .iter()
                .any(|v| !v.is_finite() || v.abs() > POLICY.maximum_mm)
            {
                return invalid("Rust B-rep vertex outside numeric domain");
            }
        }
        for edge in &self.edges {
            if !ids.insert(edge.id) || edge.vertices.iter().any(|i| *i >= self.vertices.len()) {
                return invalid("Rust B-rep has invalid edge identity/endpoints");
            }
            let [a, b] = edge.vertices.map(|i| self.vertices[i].point);
            if let Curve::Arc {
                center,
                normal,
                x_axis,
                radius,
                start_angle,
                sweep,
            } = edge.curve
            {
                if center
                    .into_iter()
                    .chain(normal)
                    .chain(x_axis)
                    .chain([radius, start_angle, sweep])
                    .any(|v| !v.is_finite())
                    || radius <= POLICY.linear_mm
                    || sweep.abs() * radius <= POLICY.linear_mm
                    || sweep.abs() >= std::f64::consts::TAU
                    || (dot(normal, normal) - 1.).abs() > POLICY.angular_rad
                    || (dot(x_axis, x_axis) - 1.).abs() > POLICY.angular_rad
                    || dot(normal, x_axis).abs() > POLICY.angular_rad
                {
                    return invalid("Rust B-rep has invalid bounded arc geometry");
                }
                let y = cross3(normal, x_axis);
                for (p, t) in [(a, start_angle), (b, start_angle + sweep)] {
                    let q = std::array::from_fn(|i| {
                        center[i] + radius * (x_axis[i] * t.cos() + y[i] * t.sin())
                    });
                    if dist(p, q) > POLICY.linear_mm {
                        return invalid("Rust bounded arc parameters do not match endpoints");
                    }
                }
            }
            match edge.curve {
                Curve::Line if dist(a, b) <= POLICY.linear_mm => {
                    return invalid("Rust B-rep has a degenerate line");
                }
                Curve::Circle {
                    center,
                    normal,
                    radius,
                } => {
                    if center.into_iter().chain(normal).any(|v| !v.is_finite())
                        || !radius.is_finite()
                        || radius <= POLICY.linear_mm
                        || (dot(normal, normal) - 1.).abs() > POLICY.angular_rad
                    {
                        return invalid("Rust B-rep has invalid circular geometry");
                    }
                    for p in [a, b] {
                        let v = std::array::from_fn(|i| p[i] - center[i]);
                        if dot(v, normal).abs() > POLICY.linear_mm
                            || (dist(p, center) - radius).abs() > POLICY.linear_mm
                        {
                            return invalid("Rust circle endpoints do not lie on curve");
                        }
                    }
                }
                _ => {}
            }
        }
        let mut uses = vec![(0usize, 0usize); self.edges.len()];
        let mut face_ids = std::collections::HashSet::new();
        for face in &self.faces {
            if !face_ids.insert(face.id) || face.loops.is_empty() {
                return invalid("Rust B-rep face identity or boundary is invalid");
            }
            match face.surface {
                Surface::Plane { origin, normal } => {
                    if origin.iter().chain(&normal).any(|v| !v.is_finite())
                        || (dot(normal, normal) - 1.).abs() > POLICY.angular_rad
                    {
                        return invalid("Rust plane is invalid");
                    }
                }
                Surface::Cylinder {
                    origin,
                    axis,
                    radius,
                } => {
                    if origin.iter().chain(&axis).any(|v| !v.is_finite())
                        || !radius.is_finite()
                        || radius <= POLICY.linear_mm
                        || (dot(axis, axis) - 1.).abs() > POLICY.angular_rad
                    {
                        return invalid("Rust cylinder is invalid");
                    }
                }
            }
            for (loop_index, boundary) in face.loops.iter().enumerate() {
                if boundary.is_empty() {
                    return invalid("Rust B-rep has an empty loop");
                }
                let mut ends = vec![];
                for co in boundary {
                    let edge = self.edges.get(co.edge).ok_or_else(|| {
                        crate::Error::Invalid("Rust coedge references missing edge".into())
                    })?;
                    let mut v = edge.vertices;
                    if co.reversed {
                        v.swap(0, 1);
                        uses[co.edge].1 += 1;
                    } else {
                        uses[co.edge].0 += 1;
                    }
                    ends.push(v);
                    // Exact analytic surface/edge incidence, independent of display triangles.
                    let on = match face.surface {
                        Surface::Plane { origin, normal } => {
                            v.iter().all(|j| {
                                dot(
                                    std::array::from_fn(|i| self.vertices[*j].point[i] - origin[i]),
                                    normal,
                                )
                                .abs()
                                    <= POLICY.linear_mm
                            }) && match edge.curve {
                                Curve::Circle { normal: axis, .. }
                                | Curve::Arc { normal: axis, .. } => {
                                    dot(axis, normal).abs() > 1. - POLICY.angular_rad
                                }
                                _ => true,
                            }
                        }
                        Surface::Cylinder {
                            origin,
                            axis,
                            radius,
                        } => {
                            if let Curve::Circle {
                                radius: r,
                                normal,
                                center,
                            }
                            | Curve::Arc {
                                radius: r,
                                normal,
                                center,
                                ..
                            } = edge.curve
                            {
                                (r - radius).abs() <= POLICY.linear_mm
                                    && dot(axis, normal).abs() > 1. - POLICY.angular_rad
                                    && {
                                        let delta = std::array::from_fn(|i| center[i] - origin[i]);
                                        let z = dot(delta, axis);
                                        (dot(delta, delta) - z * z).abs()
                                            <= POLICY.linear_mm * POLICY.linear_mm
                                    }
                            } else {
                                let delta = std::array::from_fn(|i| {
                                    self.vertices[v[1]].point[i] - self.vertices[v[0]].point[i]
                                });
                                let z = dot(delta, axis);
                                dist(delta, axis.map(|x| x * z)) <= POLICY.linear_mm
                                    && v.iter().all(|j| {
                                        let delta = std::array::from_fn(|i| {
                                            self.vertices[*j].point[i] - origin[i]
                                        });
                                        let z = dot(delta, axis);
                                        (dot(delta, delta) - z * z - radius * radius).abs()
                                            <= POLICY.linear_mm * (1. + radius * radius)
                                    })
                            }
                        }
                    };
                    if !on {
                        return invalid("Rust edge is not incident on its analytic surface");
                    }
                }
                for i in 0..ends.len() {
                    if ends[i][1] != ends[(i + 1) % ends.len()][0] {
                        return invalid("Rust B-rep loop is not closed");
                    }
                }
                if let Surface::Plane { normal, .. } = face.surface {
                    let anchor = self.vertices[ends[0][0]].point;
                    let mut area = [0.; 3];
                    for co in boundary {
                        let edge = &self.edges[co.edge];
                        let a = self.vertices[edge.vertices[0]].point;
                        let b = self.vertices[edge.vertices[1]].point;
                        let chord = cross3(
                            std::array::from_fn(|i| a[i] - anchor[i]),
                            std::array::from_fn(|i| b[i] - anchor[i]),
                        );
                        let (axis, segment) = match edge.curve {
                            Curve::Line => ([0.; 3], 0.),
                            Curve::Circle { normal, radius, .. } => {
                                (normal, std::f64::consts::PI * radius * radius)
                            }
                            Curve::Arc {
                                normal,
                                radius,
                                sweep,
                                ..
                            } => (normal, 0.5 * radius * radius * (sweep - sweep.sin())),
                        };
                        for i in 0..3 {
                            area[i] += (0.5 * chord[i] + axis[i] * segment)
                                * if co.reversed { -1. } else { 1. };
                        }
                    }
                    let signed = dot(area, normal) * if face.reversed { -1. } else { 1. };
                    if signed * if loop_index == 0 { 1. } else { -1. } <= POLICY.linear_mm.powi(2) {
                        return invalid(
                            "Rust planar loop orientation disagrees with face orientation",
                        );
                    }
                }
                if let Surface::Cylinder { origin, axis, .. } = face.surface {
                    // Signed UV area: -integral(v du). Generators have du=0;
                    // circular trims have constant axial v and exact signed du.
                    let mut area = 0.;
                    for co in boundary {
                        let (center, normal, sweep) = match self.edges[co.edge].curve {
                            Curve::Line => continue,
                            Curve::Circle { center, normal, .. } => {
                                (center, normal, std::f64::consts::TAU)
                            }
                            Curve::Arc {
                                center,
                                normal,
                                sweep,
                                ..
                            } => (center, normal, sweep),
                        };
                        let height = dot(std::array::from_fn(|i| center[i] - origin[i]), axis);
                        area -=
                            height * sweep * dot(normal, axis) * if co.reversed { -1. } else { 1. };
                    }
                    let sign = if face.reversed { -1. } else { 1. }
                        * if loop_index == 0 { 1. } else { -1. };
                    if area * sign <= 0. {
                        return invalid(
                            "Rust cylindrical trim orientation disagrees with face orientation",
                        );
                    }
                }
            }
        }
        if uses.iter().any(|u| *u != (1, 1)) {
            return invalid("Rust B-rep is not an oriented two-manifold");
        }
        // A two-use edge count alone admits disconnected shells and vertex pinches.
        // Validate the connected face shell and each vertex's circular link.
        let mut edge_faces = vec![vec![]; self.edges.len()];
        let mut links: Vec<std::collections::BTreeMap<(usize, usize), Vec<(usize, usize)>>> =
            vec![Default::default(); self.vertices.len()];
        for (fi, face) in self.faces.iter().enumerate() {
            for boundary in &face.loops {
                for i in 0..boundary.len() {
                    let a = &boundary[i];
                    let b = &boundary[(i + 1) % boundary.len()];
                    edge_faces[a.edge].push(fi);
                    let port_a = (a.edge, usize::from(!a.reversed));
                    let port_b = (b.edge, usize::from(b.reversed));
                    let vertex = self.edges[a.edge].vertices[port_a.1];
                    links[vertex].entry(port_a).or_default().push(port_b);
                    links[vertex].entry(port_b).or_default().push(port_a);
                }
            }
        }
        for link in links {
            if link.is_empty() || link.values().any(|v| v.len() != 2) {
                return invalid("Rust B-rep has an unused or nonmanifold vertex");
            }
            let mut seen = std::collections::BTreeSet::new();
            let mut pending = vec![*link.keys().next().unwrap()];
            while let Some(p) = pending.pop() {
                if seen.insert(p) {
                    pending.extend(&link[&p]);
                }
            }
            if seen.len() != link.len() {
                return invalid("Rust B-rep has a pinched vertex link");
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut pending = vec![0];
        while let Some(f) = pending.pop() {
            if seen.insert(f) {
                for co in self.faces[f].loops.iter().flatten() {
                    pending.extend(&edge_faces[co.edge]);
                }
            }
        }
        if seen.len() != self.faces.len() {
            faces::validate_shells(self)?;
        }
        if faces::supported(self) {
            faces::validate_geometry(self)?;
            if self.shell_count() == 1 && faces::measurements(self).0 <= EPS.powi(3) {
                return invalid("Rust outer planar shell must have positive signed volume");
            }
        }
        Ok(())
    }
    /// Connected boundary shells. A cavity shell is not another material body.
    pub fn shell_count(&self) -> usize {
        faces::shells(self).len()
    }
}
#[derive(Debug, Clone)]
pub struct Provenance {
    pub feature: u64,
    pub operation: Operation,
    pub parents: Vec<u64>,
}
/// Semantic axial event ancestry. Heights are deliberately absent from identities.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct AxialSource {
    pub occurrences: Vec<(u64, u64)>,
    pub feature: u64,
    pub region: u64,
    pub high: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct SectionSignature {
    pub low: std::collections::BTreeSet<AxialSource>,
    pub high: std::collections::BTreeSet<AxialSource>,
    pub regions: Vec<Vec<Vec<crate::planar::SpanKey>>>,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum OutputSignature {
    Planar(Vec<Vec<crate::planar::SpanKey>>),
    Sections(Vec<SectionSignature>),
    Faces(Vec<u64>),
}
#[derive(Debug, Clone)]
pub struct NativeSolid {
    input: ExtrudeInput,
    body_id: u64,
    topology: Brep,
    axial: [steps::Level; 2],
    planar: crate::planar::Region,
    planar_named: bool,
    legacy_output_order: bool,
    stepped: Option<steps::Stepped>,
    general: Option<faces::General>,
    pub provenance: Vec<Provenance>,
}
fn measure(profile: &Profile) -> (f64, f64) {
    (profile.signed_area().abs(), profile.perimeter())
}
fn reverse(boundary: &mut [Coedge]) {
    boundary.reverse();
    for co in boundary {
        co.reversed = !co.reversed;
    }
}
impl NativeSolid {
    pub fn translated(&self, offset: [f64; 3], feature: u64) -> Result<Self> {
        self.translated_instance(offset, feature, 0)
    }
    pub(crate) fn translated_instance(
        &self,
        offset: [f64; 3],
        feature: u64,
        instance: u64,
    ) -> Result<Self> {
        for n in offset {
            crate::number(n)?;
        }
        if self.general.is_some() {
            return faces::translated(self, offset, feature, instance);
        }
        let mut input = self.input.clone();
        for (v, o) in input.origin.iter_mut().zip(offset) {
            *v += o;
        }
        input.feature_id = feature;
        let mut planar = self.planar.clone();
        prisms::instance(&mut planar, feature, instance);
        let instance_level = |level: &steps::Level| -> Result<steps::Level> {
            level
                .iter()
                .cloned()
                .map(|mut source| {
                    source.occurrences.push((feature, instance));
                    if source.occurrences.len() > 64 {
                        return invalid("Axial transform ancestry exceeds 64 instances");
                    }
                    Ok(source)
                })
                .collect()
        };
        let mut result = if let Some(steps) = &self.stepped {
            let mut base = self.clone();
            base.input = input;
            base.planar = planar;
            let dz = dot(offset, base.input.plane.normal().to_array());
            let mut slabs = steps.slabs.clone();
            for slab in &mut slabs {
                slab.interval = slab.interval.map(|z| z + dz);
                slab.levels = [
                    instance_level(&slab.levels[0])?,
                    instance_level(&slab.levels[1])?,
                ];
                for region in &mut slab.occupied.regions {
                    prisms::instance(region, feature, instance);
                }
            }
            steps::build(&base, slabs, feature)?
        } else {
            let mut result = Self::extrude(input)?;
            result.planar = planar;
            result.axial = [
                instance_level(&self.axial[0])?,
                instance_level(&self.axial[1])?,
            ];
            if self.planar_named {
                result.name_planar_topology()?;
            }
            result
        };
        result.legacy_output_order = self.legacy_output_order;
        result.provenance = self.provenance.clone();
        result.provenance.push(Provenance {
            feature,
            operation: Operation::NewBody,
            parents: vec![self.last_feature()]
                .into_iter()
                .filter(|parent| *parent != feature)
                .collect(),
        });
        if result.provenance.len() > 32768 {
            return invalid("Solid provenance exceeds 32768 operations");
        }
        Ok(result)
    }
    pub fn topology(&self) -> &Brep {
        &self.topology
    }
    pub fn representation(&self) -> Representation {
        if self.general.is_some() {
            Representation::PlanarFaces
        } else if self.stepped.is_some() {
            Representation::OccupiedSections
        } else {
            Representation::UniformExtrusion
        }
    }
    pub fn face_ancestry(&self) -> Option<&[std::collections::BTreeSet<FaceSource>]> {
        self.general.as_ref().map(|g| g.roots.as_slice())
    }
    pub fn boundary_ancestry(&self) -> Option<&std::collections::BTreeMap<u64, BoundarySource>> {
        self.general.as_ref().map(|g| &g.sources.entries)
    }
    /// Uniform solids alone have one authoritative planar region.
    pub fn planar_region(&self) -> Option<&crate::planar::Region> {
        (self.stepped.is_none() && self.general.is_none()).then_some(&self.planar)
    }
    /// Exact occupied sections for stepped solids; axial coordinates are signed
    /// millimetres along the principal plane normal in world coordinates.
    pub fn occupied_sections(&self) -> Option<Vec<([f64; 2], &crate::planar::RegionSet)>> {
        self.stepped.as_ref().map(|steps| {
            steps
                .slabs
                .iter()
                .map(|s| (s.interval, &s.occupied))
                .collect()
        })
    }
    pub fn output_signature(&self) -> Option<OutputSignature> {
        if let Some(general) = &self.general {
            Some(OutputSignature::Faces(general.signature.clone()))
        } else if let Some(steps) = &self.stepped {
            Some(OutputSignature::Sections(steps.signature.clone()))
        } else {
            self.planar_named
                .then(|| OutputSignature::Planar(self.planar.key()))
        }
    }
    pub(crate) fn uses_legacy_output_order(&self) -> bool {
        self.legacy_output_order
    }
    fn last_feature(&self) -> u64 {
        self.provenance
            .last()
            .map_or(self.input.feature_id, |p| p.feature)
    }
    pub fn bind_body(&mut self, id: u64) {
        self.body_id = id;
        for e in &mut self.topology.edges {
            e.id.body = id;
        }
        for f in &mut self.topology.faces {
            f.id.body = id;
        }
    }
    pub fn extrude(input: ExtrudeInput) -> Result<Self> {
        input.outer.validate()?;
        for hole in &input.holes {
            hole.validate()?;
        }
        for n in input.origin.into_iter().chain([input.distance]) {
            crate::number(n)?;
        }
        if input.distance.abs() < EPS {
            return invalid("Rust extrusion distance must be nonzero");
        }
        if input
            .holes
            .iter()
            .any(|h| !crate::region::strictly_contains(&input.outer, h))
        {
            return invalid("Rust extrusion holes must be strictly inside the outer boundary");
        }
        for i in 0..input.holes.len() {
            for j in i + 1..input.holes.len() {
                let a = &input.holes[i];
                let b = &input.holes[j];
                if crate::region::strictly_contains(a, b)
                    || crate::region::strictly_contains(b, a)
                    || a.boundary().iter().any(|x| {
                        b.boundary().iter().any(|y| {
                            crate::boundary::intersections(x.curve, y.curve)
                                .map_or(true, |v| !v.is_empty())
                        })
                    })
                {
                    return invalid("Rust extrusion holes must be mutually disjoint");
                }
            }
        }
        let normal = input.plane.normal().to_array();
        let world = |p: [f64; 2], top: bool| {
            let v = input.plane.point(p, input.origin).to_array();
            std::array::from_fn(|i| v[i] + if top { normal[i] * input.distance } else { 0. })
        };
        for profile in std::iter::once(&input.outer).chain(input.holes.iter()) {
            for p in profile.boundary().iter().flat_map(|s| s.curve.extrema()) {
                for top in [false, true] {
                    for n in world(p, top) {
                        crate::number(n)?;
                    }
                }
            }
        }
        let key = |local| TopologyId {
            body: 0,
            feature: input.feature_id,
            region: input.region_id,
            local,
        };
        let mut brep = Brep {
            vertices: vec![],
            edges: vec![],
            faces: vec![
                Face {
                    id: key(0),
                    surface: Surface::Plane {
                        origin: world([0., 0.], false),
                        normal: normal.map(|n| -n),
                    },
                    loops: vec![],
                    reversed: false,
                    source_entity: None,
                },
                Face {
                    id: key(1),
                    surface: Surface::Plane {
                        origin: world([0., 0.], true),
                        normal,
                    },
                    loops: vec![],
                    reversed: false,
                    source_entity: None,
                },
            ],
        };
        for (loop_index, profile) in std::iter::once(&input.outer)
            .chain(input.holes.iter())
            .enumerate()
        {
            let hole = loop_index > 0;
            let mut segments = profile.boundary();
            if profile.signed_area() < 0. {
                segments.reverse();
                for s in &mut segments {
                    s.curve = s.curve.reversed();
                }
            }
            let points = if let Profile::Circle { center, radius } = profile {
                vec![[center[0] + radius, center[1]]]
            } else {
                segments.iter().map(|s| s.curve.point(0.)).collect()
            };
            let n = points.len();
            let vbase = brep.vertices.len();
            let ebase = brep.edges.len();
            for top in [false, true] {
                for p in &points {
                    brep.vertices.push(Vertex {
                        point: world(*p, top),
                    });
                }
            }
            // Boundary identities are scoped to the stable profile ID and edge ordinal.
            let loop_id = input
                .loop_ids
                .get(loop_index)
                .copied()
                .unwrap_or(loop_index as u64 + 1);
            let local_base = loop_id
                .checked_mul(10_000)
                .and_then(|base| base.checked_add(5000).map(|_| base))
                .ok_or_else(|| crate::Error::Invalid("Topology ID space exhausted".into()))?;
            for top in [false, true] {
                for i in 0..n {
                    let curve = if let Profile::Circle { center, radius } = profile {
                        Curve::Circle {
                            center: world(*center, top),
                            normal,
                            radius: *radius,
                        }
                    } else if let crate::boundary::PlanarCurve::Arc { geometry: a } =
                        segments[i].curve
                    {
                        Curve::Arc {
                            center: world(a.center, top),
                            normal,
                            x_axis: input.plane.point([1., 0.], [0.; 3]).to_array(),
                            radius: a.radius,
                            start_angle: a.start_angle,
                            sweep: a.sweep,
                        }
                    } else {
                        Curve::Line
                    };
                    brep.edges.push(Edge {
                        source_entity: if matches!(profile, Profile::Circle { .. }) {
                            None
                        } else {
                            segments[i].source
                        },
                        id: key(local_base + if top { 2000 } else { 1000 } + i as u64),
                        vertices: [
                            vbase + if top { n } else { 0 } + i,
                            vbase + if top { n } else { 0 } + (i + 1) % n,
                        ],
                        curve,
                    });
                }
            }
            for i in 0..n {
                brep.edges.push(Edge {
                    source_entity: if matches!(profile, Profile::Circle { .. }) {
                        None
                    } else {
                        segments[i].source
                    },
                    id: key(local_base + 3000 + i as u64),
                    vertices: [vbase + i, vbase + n + i],
                    curve: Curve::Line,
                });
            }
            let mut bottom: Vec<_> = (0..n)
                .rev()
                .map(|i| Coedge {
                    edge: ebase + i,
                    reversed: true,
                })
                .collect();
            let mut top: Vec<_> = (0..n)
                .map(|i| Coedge {
                    edge: ebase + n + i,
                    reversed: false,
                })
                .collect();
            if hole {
                reverse(&mut bottom);
                reverse(&mut top);
            }
            brep.faces[0].loops.push(bottom);
            brep.faces[1].loops.push(top);
            for i in 0..n {
                let mut boundary = vec![
                    Coedge {
                        edge: ebase + i,
                        reversed: false,
                    },
                    Coedge {
                        edge: ebase + 2 * n + (i + 1) % n,
                        reversed: false,
                    },
                    Coedge {
                        edge: ebase + n + i,
                        reversed: true,
                    },
                    Coedge {
                        edge: ebase + 2 * n + i,
                        reversed: true,
                    },
                ];
                if hole {
                    reverse(&mut boundary);
                }
                let arc = if let Profile::Circle { center, radius } = profile {
                    Some((*center, *radius, false))
                } else if let crate::boundary::PlanarCurve::Arc { geometry: a } = segments[i].curve
                {
                    Some((a.center, a.radius, a.sweep < 0.))
                } else {
                    None
                };
                let surface = if let Some((center, radius, _)) = arc {
                    Surface::Cylinder {
                        origin: world(center, false),
                        axis: normal,
                        radius,
                    }
                } else {
                    let a = world(points[i], false);
                    let b = world(points[(i + 1) % n], false);
                    let d = cross3(std::array::from_fn(|j| b[j] - a[j]), normal);
                    let len = dot(d, d).sqrt();
                    Surface::Plane {
                        origin: a,
                        normal: d.map(|v| v / len),
                    }
                };
                brep.faces.push(Face {
                    source_entity: if matches!(profile, Profile::Circle { .. }) {
                        None
                    } else {
                        segments[i].source
                    },
                    id: key(local_base + 4000 + i as u64),
                    surface,
                    loops: vec![boundary],
                    reversed: hole ^ arc.is_some_and(|(_, _, reverse)| reverse),
                });
            }
        }
        if input.distance < 0. {
            for (index, face) in brep.faces.iter_mut().enumerate() {
                for boundary in &mut face.loops {
                    reverse(boundary);
                }
                // Cap outward normals swap with extrusion direction. Side
                // radial/planar outward normals do not.
                if index < 2 {
                    face.reversed = !face.reversed;
                }
            }
        }
        brep.validate()?;
        let feature = input.feature_id;
        let ids: Vec<_> = (0..=input.holes.len())
            .map(|i| input.loop_ids.get(i).copied().unwrap_or(i as u64 + 1))
            .collect();
        let planar = crate::planar::Region::from_profiles(
            &input.outer,
            &input.holes,
            feature,
            input.region_id,
            &ids,
        )?;
        let axial = [false, true].map(|high| {
            std::collections::BTreeSet::from([AxialSource {
                occurrences: vec![],
                feature,
                region: input.region_id,
                high,
            }])
        });
        Ok(Self {
            input,
            body_id: 0,
            topology: brep,
            axial,
            planar,
            planar_named: false,
            legacy_output_order: false,
            stepped: None,
            general: None,
            provenance: vec![Provenance {
                feature,
                operation: Operation::NewBody,
                parents: vec![],
            }],
        })
    }
    pub fn volume(&self) -> f64 {
        if self.general.is_some() {
            return faces::measurements(&self.topology).0;
        }
        if let Some(steps) = &self.stepped {
            return steps.volume;
        }
        (measure(&self.input.outer).0 - self.input.holes.iter().map(|p| measure(p).0).sum::<f64>())
            * self.input.distance.abs()
    }
    pub fn area(&self) -> f64 {
        if self.general.is_some() {
            return faces::measurements(&self.topology).1;
        }
        if let Some(steps) = &self.stepped {
            return steps.area;
        }
        let (a, p) = measure(&self.input.outer);
        let holes_area: f64 = self.input.holes.iter().map(|p| measure(p).0).sum();
        let holes_perimeter: f64 = self.input.holes.iter().map(|p| measure(p).1).sum();
        2. * (a - holes_area) + (p + holes_perimeter) * self.input.distance.abs()
    }
    pub fn bounds(&self) -> [[f64; 3]; 2] {
        if self.general.is_some() {
            return faces::bounds(&self.topology);
        }
        if let Some(steps) = &self.stepped {
            return steps.bounds(&self.input);
        }
        let bounds2 = self.input.outer.planar_bounds();
        let points = [
            bounds2[0],
            [bounds2[0][0], bounds2[1][1]],
            bounds2[1],
            [bounds2[1][0], bounds2[0][1]],
        ];
        let mut bounds = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
        for p in points {
            for t in [0., self.input.distance] {
                let v =
                    self.input.plane.point(p, self.input.origin) + self.input.plane.normal() * t;
                for i in 0..3 {
                    bounds[0][i] = bounds[0][i].min(v[i]);
                    bounds[1][i] = bounds[1][i].max(v[i]);
                }
            }
        }
        bounds
    }
    pub fn mesh(&self, segments: usize) -> Result<DisplayMesh> {
        if self.general.is_some() {
            faces::mesh(&self.topology, segments)
        } else if let Some(steps) = &self.stepped {
            steps.mesh(&self.topology, segments)
        } else {
            self.mesh_prism(segments)
        }
    }
    fn mesh_prism(&self, segments: usize) -> Result<DisplayMesh> {
        if !(8..=4096).contains(&segments) {
            return invalid("Rust display tessellation requires 8..4096 segments");
        }
        self.topology.validate()?;
        let mut ordinals = vec![];
        let mut corners = vec![];
        let loops: Vec<_> = std::iter::once(&self.input.outer)
            .chain(self.input.holes.iter())
            .map(|p| {
                if matches!(p, Profile::Circle { .. }) {
                    ordinals.push(vec![0; segments]);
                    corners.push(vec![false; segments]);
                    return p.outline(segments);
                }
                let mut boundary = p.boundary();
                if p.signed_area() < 0. {
                    boundary.reverse();
                    for s in &mut boundary {
                        s.curve = s.curve.reversed();
                    }
                }
                let mut points = vec![];
                let mut ids = vec![];
                let mut corner = vec![];
                for (i, s) in boundary.iter().enumerate() {
                    let count = match s.curve {
                        crate::boundary::PlanarCurve::Line { .. } => 1,
                        crate::boundary::PlanarCurve::Arc { geometry: a } => {
                            ((segments as f64 * a.sweep.abs() / std::f64::consts::TAU).ceil()
                                as usize)
                                .max(1)
                        }
                    };
                    for j in 0..count {
                        points.push(s.curve.point(j as f64 / count as f64));
                        ids.push(i);
                        corner.push(j == 0);
                    }
                }
                ordinals.push(ids);
                corners.push(corner);
                points
            })
            .collect();
        let mut mesh = DisplayMesh::default();
        let normal = self.input.plane.normal().to_array();
        let world = |p: [f64; 2], top: bool| {
            let b = self.input.plane.point(p, self.input.origin).to_array();
            std::array::from_fn(|i| {
                b[i] + if top {
                    normal[i] * self.input.distance
                } else {
                    0.
                }
            })
        };
        // Display-only vertical decomposition of sampled loop boundaries. Exact circles
        // and cylindrical surfaces above remain authoritative at every mesh resolution.
        let edges: Vec<_> = loops
            .iter()
            .flat_map(|p| (0..p.len()).map(|i| (p[i], p[(i + 1) % p.len()])))
            .collect();
        let mut xs: Vec<f64> = edges.iter().flat_map(|(a, b)| [a[0], b[0]]).collect();
        xs.sort_by(f64::total_cmp);
        xs.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        let y_at = |edge: ([f64; 2], [f64; 2]), x: f64| {
            edge.0[1] + (edge.1[1] - edge.0[1]) * (x - edge.0[0]) / (edge.1[0] - edge.0[0])
        };
        for slab in xs.windows(2) {
            let mid = (slab[0] + slab[1]) * 0.5;
            let mut crossing: Vec<_> = edges
                .iter()
                .copied()
                .filter(|(a, b)| mid > a[0].min(b[0]) && mid < a[0].max(b[0]))
                .collect();
            crossing.sort_by(|a, b| y_at(*a, mid).total_cmp(&y_at(*b, mid)));
            if crossing.len() % 2 != 0 {
                return invalid("Rust display loop decomposition failed");
            }
            for pair in crossing.chunks_exact(2) {
                let q = [
                    [slab[0], y_at(pair[0], slab[0])],
                    [slab[1], y_at(pair[0], slab[1])],
                    [slab[1], y_at(pair[1], slab[1])],
                    [slab[0], y_at(pair[1], slab[0])],
                ];
                for top in [false, true] {
                    let norm = normal.map(|v| {
                        v * if top {
                            self.input.distance.signum()
                        } else {
                            -self.input.distance.signum()
                        }
                    });
                    for tri in [[q[0], q[1], q[2]], [q[0], q[2], q[3]]] {
                        triangle(
                            &mut mesh,
                            tri.map(|p| world(p, top)),
                            norm,
                            self.topology.faces[usize::from(top)].id,
                        );
                    }
                }
            }
        }
        let mut edge_base = 0;
        let mut face_base = 2;
        for (li, points) in loops.iter().enumerate() {
            let n = if matches!(
                std::iter::once(&self.input.outer)
                    .chain(&self.input.holes)
                    .nth(li)
                    .unwrap(),
                Profile::Circle { .. }
            ) {
                1
            } else {
                self.planar.loops().nth(li).unwrap().spans.len()
            };
            for i in 0..points.len() {
                let a = world(points[i], false);
                let b = world(points[(i + 1) % points.len()], false);
                let c = world(points[(i + 1) % points.len()], true);
                let d = world(points[i], true);
                let delta = std::array::from_fn(|j| b[j] - a[j]);
                let norm = cross3(delta, normal);
                let len = dot(norm, norm).sqrt();
                let norm = norm.map(|v| v / len * if li > 0 { -1. } else { 1. });
                let ordinal = ordinals[li][i];
                let face = self.topology.faces[face_base + ordinal].id;
                triangle(&mut mesh, [a, b, c], norm, face);
                triangle(&mut mesh, [a, c, d], norm, face);
                mesh.edges.extend([a, b, d, c]);
                mesh.edge_ids.extend([
                    self.topology.edges[edge_base + ordinal].id,
                    self.topology.edges[edge_base + n + ordinal].id,
                ]);
                if corners[li][i] {
                    mesh.edges.extend([a, d]);
                    mesh.edge_ids
                        .push(self.topology.edges[edge_base + 2 * n + ordinal].id);
                }
            }
            edge_base += 3 * n;
            face_base += n;
        }
        Ok(mesh)
    }
}
fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn triangle(mesh: &mut DisplayMesh, mut p: [[f64; 3]; 3], normal: [f64; 3], face: TopologyId) {
    let cross = cross3(
        std::array::from_fn(|i| p[1][i] - p[0][i]),
        std::array::from_fn(|i| p[2][i] - p[0][i]),
    );
    if dot(cross, cross) < 1e-24 {
        return;
    }
    if dot(cross, normal) < 0. {
        p.swap(1, 2);
    }
    let offset = mesh.vertices.len();
    mesh.vertices.extend(p);
    mesh.normals.extend([normal; 3]);
    mesh.indices.extend([offset, offset + 1, offset + 2]);
    mesh.face_ids.push(Some(face));
}
fn box_input(bounds: [[f64; 3]; 2], feature: u64) -> ExtrudeInput {
    ExtrudeInput {
        feature_id: feature,
        region_id: 1,
        loop_ids: vec![1],
        outer: Profile::Rectangle {
            a: [bounds[0][0], bounds[0][1]],
            b: [bounds[1][0], bounds[1][1]],
        },
        holes: vec![],
        plane: Plane::XY,
        origin: [0., 0., bounds[0][2]],
        distance: bounds[1][2] - bounds[0][2],
    }
}
pub fn boolean(
    a: &NativeSolid,
    b: &NativeSolid,
    operation: Operation,
    feature: u64,
) -> Result<Vec<NativeSolid>> {
    if operation == Operation::NewBody {
        return invalid("New Body is not a Boolean operation");
    }
    a.topology.validate()?;
    b.topology.validate()?;
    let aa = a.bounds();
    let bb = b.bounds();
    let disjoint = (0..3)
        .any(|i| aa[1][i] < bb[0][i] - POLICY.linear_mm || bb[1][i] < aa[0][i] - POLICY.linear_mm);
    let planar_faces = faces::supported(&a.topology) && faces::supported(&b.topology);
    let enclosed = (0..3).all(|i| bb[0][i] > aa[0][i] + EPS && bb[1][i] < aa[1][i] - EPS);
    let general = a.general.is_some()
        || b.general.is_some()
        || (planar_faces && (a.input.plane != b.input.plane || enclosed));
    let mut result = if general {
        faces::boolean(a, b, operation, feature)?
    } else if let Some(solids) = prisms::boolean(a, b, operation, feature)? {
        solids
    } else if a.input.plane == b.input.plane {
        steps::boolean(a, b, operation, feature)?
    } else if disjoint {
        match operation {
            Operation::Cut => vec![a.clone()],
            Operation::Intersect => vec![],
            Operation::Join => [a, b]
                .into_iter()
                .map(|s| {
                    let mut s = s.clone();
                    // Historical disjoint Join saves assigned bodies by bounds,
                    // including pocketed operands on different principal axes.
                    s.legacy_output_order = true;
                    s
                })
                .collect(),
            _ => unreachable!(),
        }
    } else {
        box_boolean(a, b, operation, feature)?
    };
    let mut history = a.provenance.clone();
    for p in &b.provenance {
        if p.feature != feature && !history.iter().any(|q| q.feature == p.feature) {
            history.push(p.clone());
        }
    }
    let mut parents = vec![a.last_feature(), b.last_feature()];
    parents.retain(|p| *p != feature);
    parents.sort_unstable();
    parents.dedup();
    if history.len() >= 32768 {
        return invalid("Solid provenance exceeds 32768 operations");
    }
    for s in &mut result {
        s.topology.validate()?;
        s.provenance = history.clone();
        s.provenance.push(Provenance {
            feature,
            operation,
            parents: parents.clone(),
        });
    }
    Ok(result)
}
fn box_boolean(
    a: &NativeSolid,
    b: &NativeSolid,
    operation: Operation,
    feature: u64,
) -> Result<Vec<NativeSolid>> {
    if a.general.is_some()
        || b.general.is_some()
        || !prisms::rectangular(&a.input.outer)
        || a.stepped.is_some()
        || b.stepped.is_some()
        || !a.input.holes.is_empty()
        || !prisms::rectangular(&b.input.outer)
        || !b.input.holes.is_empty()
    {
        return invalid(
            "Rust Boolean unsupported for these curved/trimmed solids; no OCCT fallback occurred",
        );
    }
    let aa = a.bounds();
    let bb = b.bounds();
    let lo = std::array::from_fn(|i| aa[0][i].max(bb[0][i]));
    let hi = std::array::from_fn(|i| aa[1][i].min(bb[1][i]));
    let overlap = (0..3).all(|i| hi[i] - lo[i] > POLICY.linear_mm);
    let same = |i: usize| {
        (aa[0][i] - bb[0][i]).abs() < POLICY.linear_mm
            && (aa[1][i] - bb[1][i]).abs() < POLICY.linear_mm
    };
    let mut bounds = vec![];
    match operation {
        Operation::Intersect => {
            if overlap {
                bounds.push([lo, hi]);
            }
        }
        Operation::Cut => {
            if !overlap {
                return Ok(vec![a.clone()]);
            }
            if (0..3).all(|i| {
                bb[0][i] <= aa[0][i] + POLICY.linear_mm && bb[1][i] >= aa[1][i] - POLICY.linear_mm
            }) {
                return Ok(vec![]);
            }
            let axis = (0..3)
                .find(|axis| {
                    (0..3).filter(|i| i != axis).all(|i| {
                        bb[0][i] <= aa[0][i] + POLICY.linear_mm
                            && bb[1][i] >= aa[1][i] - POLICY.linear_mm
                    })
                })
                .ok_or_else(|| {
                    crate::Error::Invalid(
                        "Rust Cut supports through slabs or full removal; this trim is unsupported"
                            .into(),
                    )
                })?;
            if lo[axis] - aa[0][axis] > POLICY.linear_mm {
                let mut piece = aa;
                piece[1][axis] = lo[axis];
                bounds.push(piece);
            }
            if aa[1][axis] - hi[axis] > POLICY.linear_mm {
                let mut piece = aa;
                piece[0][axis] = hi[axis];
                bounds.push(piece);
            }
        }
        Operation::Join => {
            if let Some(axis) = (0..3).find(|axis| {
                (0..3).filter(|i| i != axis).all(same)
                    && aa[1][*axis] >= bb[0][*axis] - POLICY.linear_mm
                    && bb[1][*axis] >= aa[0][*axis] - POLICY.linear_mm
            }) {
                let mut piece = aa;
                piece[0][axis] = aa[0][axis].min(bb[0][axis]);
                piece[1][axis] = aa[1][axis].max(bb[1][axis]);
                bounds.push(piece);
            } else if (0..3).any(|i| {
                aa[1][i] < bb[0][i] - POLICY.linear_mm || bb[1][i] < aa[0][i] - POLICY.linear_mm
            }) {
                return Ok(vec![a.clone(), b.clone()]);
            } else {
                return invalid(
                    "Rust Join supports collinear boxes or disjoint bodies; this union is unsupported",
                );
            }
        }
        Operation::NewBody => return invalid("New Body is not a Boolean operation"),
    }
    bounds
        .into_iter()
        .map(|bounds| {
            let mut s = NativeSolid::extrude(box_input(bounds, feature))?;
            s.provenance = vec![Provenance {
                feature,
                operation,
                parents: vec![a.input.feature_id, b.input.feature_id],
            }];
            Ok(s)
        })
        .collect()
}
