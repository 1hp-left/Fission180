//! Analytic occupied-section Booleans for parallel principal extrusion axes.
//! Connectivity and measurements use exact planar regions, never display meshes.
use super::*;
use crate::planar::{self, Boolean, Region, RegionSet};
use std::collections::{BTreeSet, HashSet};

pub(super) const MAX_LEVELS: usize = 64;
const MAX_CELLS: usize = 512;
pub(super) type Level = BTreeSet<AxialSource>;
#[derive(Debug, Clone, Serialize)]
enum Kind {
    Sides(Level, Level),
    Cap(Level, bool),
}
#[derive(Debug, Clone)]
struct Patch {
    region: Region,
    lo: f64,
    hi: f64,
    kind: Kind,
}
#[derive(Debug, Clone)]
struct Part {
    input: ExtrudeInput,
    faces: Vec<(TopologyId, TopologyId)>,
}
#[derive(Debug, Clone)]
pub(super) struct Slab {
    pub interval: [f64; 2],
    pub levels: [Level; 2],
    pub occupied: RegionSet,
}
#[derive(Debug, Clone)]
pub(super) struct Stepped {
    pub volume: f64,
    pub area: f64,
    pub slabs: Vec<Slab>,
    pub signature: Vec<SectionSignature>,
    parts: Vec<Part>,
}
fn axes(plane: Plane) -> [usize; 2] {
    match plane {
        Plane::XY => [0, 1],
        Plane::XZ => [0, 2],
        Plane::YZ => [1, 2],
    }
}
fn sections(s: &NativeSolid, origin: [f64; 3]) -> Vec<Slab> {
    let delta = axes(s.input.plane).map(|i| s.input.origin[i] - origin[i]);
    let slabs = s.stepped.as_ref().map_or_else(
        || {
            vec![Slab {
                interval: cuts::interval(&s.input),
                levels: s.axial.clone(),
                occupied: RegionSet {
                    regions: vec![s.planar.clone()],
                },
            }]
        },
        |steps| steps.slabs.clone(),
    );
    slabs
        .into_iter()
        .map(|mut slab| {
            slab.occupied.regions = slab
                .occupied
                .regions
                .iter()
                .map(|r| prisms::shifted(r, delta))
                .collect();
            slab
        })
        .collect()
}
fn validate_budget(slabs: &[Slab]) -> Result<()> {
    let mut cells = 0;
    let mut lineage = 0;
    for slab in slabs {
        cells += slab.occupied.regions.len();
        for level in &slab.levels {
            lineage += level.iter().map(|s| 1 + s.occurrences.len()).sum::<usize>();
        }
        for span in slab
            .occupied
            .regions
            .iter()
            .flat_map(Region::loops)
            .flat_map(|l| &l.spans)
        {
            lineage += span.ancestry.len() + span.start.len() + span.end.len();
        }
        if cells > MAX_CELLS {
            return invalid("Section Boolean exceeds 512 occupied cells");
        }
        if lineage > 32768 {
            return invalid("Section provenance exceeds 32768 records");
        }
    }
    Ok(())
}
/// Partition both operands at their actual occupied axial events, retaining gaps.
pub(super) fn boolean(
    a: &NativeSolid,
    b: &NativeSolid,
    op: Operation,
    feature: u64,
) -> Result<Vec<NativeSolid>> {
    if a.input.plane != b.input.plane {
        return invalid(
            "Rust section Boolean requires parallel principal axes; no OCCT fallback occurred",
        );
    }
    a.topology.validate()?;
    b.topology.validate()?;
    let operation = match op {
        Operation::Join => Boolean::Union,
        Operation::Cut => Boolean::Difference,
        Operation::Intersect => Boolean::Intersection,
        _ => return invalid("New Body is not a Boolean operation"),
    };
    let left = sections(a, a.input.origin);
    let right = sections(b, a.input.origin);
    let mut events: Vec<_> = left
        .iter()
        .chain(&right)
        .flat_map(|s| (0..2).map(|i| (s.interval[i], s.levels[i].clone())))
        .collect();
    events.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut levels: Vec<(f64, Level)> = vec![];
    for (z, sources) in events {
        if let Some(last) = levels.last_mut() {
            let gap = z - last.0;
            if gap <= POLICY.linear_mm {
                last.1.extend(sources);
                continue;
            }
            if gap < planar::RESOLUTION_MM {
                return invalid("Distinct axial events are closer than 0.00001 mm");
            }
        }
        levels.push((z, sources));
        if levels.len() > MAX_LEVELS {
            return invalid("Section Boolean exceeds 64 axial levels");
        }
    }
    if levels.iter().map(|(_, s)| s.len()).sum::<usize>() > 32768 {
        return invalid("Axial provenance exceeds 32768 records");
    }
    // A cut cannot move its target's exterior caps. A tool ending exactly on
    // either cap has the same opening as one extending beyond it; its redundant
    // endpoint must not rename an otherwise unchanged pocket.
    if operation == Boolean::Difference {
        for (z, sources) in &mut levels {
            for (slab, end) in [(&left[0], 0), (left.last().unwrap(), 1)] {
                if (*z - slab.interval[end]).abs() <= POLICY.linear_mm {
                    *sources = slab.levels[end].clone();
                }
            }
        }
    }
    let empty = RegionSet::default();
    let at = |slabs: &[Slab], z: f64| {
        slabs
            .iter()
            .position(|s| z > s.interval[0] && z < s.interval[1])
    };
    let mut slabs = vec![];
    for w in levels.windows(2) {
        let mid = (w[0].0 + w[1].0) * 0.5;
        let l = at(&left, mid).map_or(&empty, |i| &left[i].occupied);
        let r = at(&right, mid).map_or(&empty, |i| &right[i].occupied);
        // Empty operands need no new arrangement or artificial provenance.
        let occupied = if l.regions.is_empty() {
            if operation == Boolean::Union {
                r.clone()
            } else {
                empty.clone()
            }
        } else if r.regions.is_empty() {
            if operation == Boolean::Intersection {
                empty.clone()
            } else {
                l.clone()
            }
        } else {
            l.boolean(r, operation)?
        };
        slabs.push(Slab {
            interval: [w[0].0, w[1].0],
            levels: [w[0].1.clone(), w[1].1.clone()],
            occupied,
        });
    }
    validate_budget(&slabs)?;
    let mut result = components(a, slabs, feature)?;
    // Preserve v5 box split/disjoint Join body ordering, even across axial gaps.
    let aa = a.bounds();
    let bb = b.bounds();
    let legacy = (op == Operation::Join
        && (0..3).any(|i| {
            aa[1][i] < bb[0][i] - POLICY.linear_mm || bb[1][i] < aa[0][i] - POLICY.linear_mm
        }))
        || (a.stepped.is_none()
            && b.stepped.is_none()
            && a.input.holes.is_empty()
            && b.input.holes.is_empty()
            && prisms::rectangular(&a.input.outer)
            && prisms::rectangular(&b.input.outer)
            && result.iter().all(|s| {
                let sections = &s.stepped.as_ref().unwrap().slabs;
                let bounds = sections[0].occupied.regions[0]
                    .outer
                    .profile()
                    .planar_bounds();
                sections.iter().all(|slab| {
                    slab.occupied.regions.len() == 1
                        && slab.occupied.regions[0].holes.is_empty()
                        && prisms::rectangular(&slab.occupied.regions[0].outer.profile())
                        && slab.occupied.regions[0]
                            .outer
                            .profile()
                            .planar_bounds()
                            .iter()
                            .flatten()
                            .zip(bounds.iter().flatten())
                            .all(|(a, b)| (a - b).abs() <= POLICY.linear_mm)
                })
            }));
    let mut signatures = BTreeSet::new();
    for s in &mut result {
        s.legacy_output_order = legacy;
        if !signatures.insert(s.output_signature()) {
            return invalid("Section output identity collision; repair dependencies explicitly");
        }
    }
    result.sort_by_key(NativeSolid::output_signature);
    Ok(result)
}
fn components(base: &NativeSolid, slabs: Vec<Slab>, feature: u64) -> Result<Vec<NativeSolid>> {
    let cells: Vec<_> = slabs
        .iter()
        .enumerate()
        .flat_map(|(i, s)| (0..s.occupied.regions.len()).map(move |j| (i, j)))
        .collect();
    if cells.len() > MAX_CELLS {
        return invalid("Section Boolean exceeds 512 occupied cells");
    }
    let mut adjacency = vec![vec![]; cells.len()];
    for (u, &(i, j)) in cells.iter().enumerate() {
        for (v, &(k, l)) in cells.iter().enumerate().skip(u + 1) {
            if k != i + 1 {
                continue;
            }
            let a = RegionSet {
                regions: vec![slabs[i].occupied.regions[j].clone()],
            };
            let b = RegionSet {
                regions: vec![slabs[k].occupied.regions[l].clone()],
            };
            let overlap = a.boolean(&b, Boolean::Intersection)?;
            if !overlap.regions.is_empty() {
                adjacency[u].push(v);
                adjacency[v].push(u);
            } else {
                // At a shared height, boundary-only attachment is nonmanifold.
                // A regular planar union has two islands iff they are separated.
                if a.boolean(&b, Boolean::Union)?.regions.len() != 2 {
                    return invalid(
                        "Section solids have only point/edge contact at an axial event",
                    );
                }
            }
        }
    }
    let mut seen = vec![false; cells.len()];
    let mut result = vec![];
    for first in 0..cells.len() {
        if seen[first] {
            continue;
        }
        let mut members = BTreeSet::new();
        let mut pending = vec![first];
        while let Some(u) = pending.pop() {
            if !seen[u] {
                seen[u] = true;
                members.insert(cells[u]);
                pending.extend(&adjacency[u]);
            }
        }
        let mut component = vec![];
        for (i, slab) in slabs.iter().enumerate() {
            let mut slab = slab.clone();
            slab.occupied.regions = slab
                .occupied
                .regions
                .into_iter()
                .enumerate()
                .filter_map(|(j, r)| members.contains(&(i, j)).then_some(r))
                .collect();
            if !slab.occupied.regions.is_empty() {
                component.push(slab);
            }
        }
        result.push(build(base, component, feature)?);
    }
    Ok(result)
}
pub(super) fn build(base: &NativeSolid, slabs: Vec<Slab>, feature: u64) -> Result<NativeSolid> {
    if slabs.is_empty() || slabs.len() >= MAX_LEVELS {
        return invalid("Invalid occupied section count");
    }
    validate_budget(&slabs)?;
    let mut patches = vec![];
    let mut volume = 0.;
    let mut area = 0.;
    let mut signature = vec![];
    for slab in &slabs {
        slab.occupied.validate()?;
        let length = slab.interval[1] - slab.interval[0];
        if length < planar::RESOLUTION_MM {
            return invalid("Unresolved axial section");
        }
        volume += slab.occupied.area() * length;
        let mut regions: Vec<_> = slab.occupied.regions.iter().map(Region::key).collect();
        regions.sort();
        signature.push(SectionSignature {
            low: slab.levels[0].clone(),
            high: slab.levels[1].clone(),
            regions,
        });
        for region in &slab.occupied.regions {
            area += region.perimeter() * length;
            patches.push(Patch {
                region: region.clone(),
                lo: slab.interval[0],
                hi: slab.interval[1],
                kind: Kind::Sides(slab.levels[0].clone(), slab.levels[1].clone()),
            });
        }
    }
    let mut cap = |set: RegionSet, height: f64, level: Level, down: bool| {
        area += set.area();
        for region in set.regions {
            patches.push(Patch {
                region,
                lo: height,
                hi: height,
                kind: Kind::Cap(level.clone(), down),
            });
        }
    };
    cap(
        slabs[0].occupied.clone(),
        slabs[0].interval[0],
        slabs[0].levels[0].clone(),
        true,
    );
    let last = slabs.last().unwrap();
    cap(
        last.occupied.clone(),
        last.interval[1],
        last.levels[1].clone(),
        false,
    );
    for pair in slabs.windows(2) {
        if (pair[0].interval[1] - pair[1].interval[0]).abs() > POLICY.linear_mm {
            return invalid("Disconnected axial intervals in one solid");
        }
        let below = &pair[0].occupied;
        let above = &pair[1].occupied;
        let height = pair[0].interval[1];
        let level = pair[0].levels[1].clone();
        cap(
            below.boolean(above, Boolean::Difference)?,
            height,
            level.clone(),
            false,
        );
        cap(
            above.boolean(below, Boolean::Difference)?,
            height,
            level,
            true,
        );
    }
    let mut regions: Vec<_> = patches.iter().map(|p| p.region.clone()).collect();
    planar::refine_sections(&mut regions)?;
    // Refinement may add junction ancestry; apply the same retained provenance
    // policy to the exposed patches before serializing their topology keys.
    let mut lineage = 0;
    for region in &regions {
        for span in region.loops().flat_map(|l| &l.spans) {
            if span.start.len() > planar::MAX_CURVES || span.end.len() > planar::MAX_CURVES {
                return invalid("Section junction ancestry exceeds 512 source pairs");
            }
            lineage += span.ancestry.len() + span.start.len() + span.end.len();
        }
        if lineage > 32768 {
            return invalid("Exposed section provenance exceeds 32768 records");
        }
    }
    for (patch, region) in patches.iter_mut().zip(regions) {
        patch.region = region;
    }
    let (topology, parts) = assemble(base, &patches, feature)?;
    // Reject multiple boundary shells (including enclosed cavities), pinches,
    // orientation errors and unmatched interfaces before publishing any result.
    topology.validate().map_err(|error| {
        crate::Error::Invalid(format!("Rust section boundary unsupported: {error}"))
    })?;
    if !volume.is_finite() || volume <= 0. || !area.is_finite() || area <= 0. {
        return invalid("Invalid stepped analytic measurements");
    }
    let mut result = base.clone();
    result.topology = topology;
    result.axial = [slabs[0].levels[0].clone(), last.levels[1].clone()];
    result.stepped = Some(Stepped {
        volume,
        area,
        slabs,
        signature,
        parts,
    });
    result.planar_named = false;
    result.legacy_output_order = false;
    Ok(result)
}
impl Stepped {
    pub(super) fn bounds(&self, input: &ExtrudeInput) -> [[f64; 3]; 2] {
        let normal = input.plane.normal().to_array();
        let start = dot(input.origin, normal);
        let mut bounds = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
        for slab in &self.slabs {
            for r in &slab.occupied.regions {
                for p in r.outer.spans.iter().flat_map(|s| s.curve.extrema()) {
                    let p = input.plane.point(p, input.origin).to_array();
                    for z in slab.interval {
                        for i in 0..3 {
                            let v = p[i] + normal[i] * (z - start);
                            bounds[0][i] = bounds[0][i].min(v);
                            bounds[1][i] = bounds[1][i].max(v);
                        }
                    }
                }
            }
        }
        bounds
    }
}
fn assemble(base: &NativeSolid, patches: &[Patch], feature: u64) -> Result<(Brep, Vec<Part>)> {
    let mut brep = Brep {
        vertices: vec![],
        edges: vec![],
        faces: vec![],
    };
    let mut parts = vec![];
    let mut edge_keys: Vec<BTreeSet<u64>> = vec![];
    let mut face_ids = HashSet::new();
    for patch in patches {
        let mut input = base.input.clone();
        input.feature_id = feature;
        input.outer = patch.region.outer.profile();
        input.holes = patch.region.holes.iter().map(|h| h.profile()).collect();
        input.loop_ids = (1..=input.holes.len() as u64 + 1).collect();
        let down = matches!(patch.kind, Kind::Cap(_, true));
        let is_cap = matches!(patch.kind, Kind::Cap(..));
        let height = if is_cap && !down {
            patch.lo - 1.
        } else {
            patch.lo
        };
        let normal = input.plane.normal().to_array();
        let start = dot(input.origin, normal);
        input.origin = std::array::from_fn(|i| input.origin[i] + normal[i] * (height - start));
        input.distance = if is_cap { 1. } else { patch.hi - patch.lo };
        let proto = NativeSolid::extrude(input.clone())?;
        let mut proto_edge_keys = vec![0; proto.topology.edges.len()];
        let mut proto_face_keys = vec![0; proto.topology.faces.len()];
        proto_face_keys[0] = planar::identity(&("cap", &patch.kind, patch.region.key()))?;
        proto_face_keys[1] = proto_face_keys[0];
        let mut eb = 0;
        let mut fb = 2;
        for l in patch.region.loops() {
            let n = l.spans.len();
            for (i, s) in l.spans.iter().enumerate() {
                let i = if l.signed_area() < 0. { n - 1 - i } else { i };
                for role in 0..3u8 {
                    proto_edge_keys[eb + role as usize * n + i] =
                        planar::identity(&(&patch.kind, s.key(), role))?;
                }
                proto_face_keys[fb + i] = planar::identity(&("side", &patch.kind, s.key()))?;
            }
            eb += 3 * n;
            fb += n;
        }
        let selected: Vec<_> = if is_cap {
            vec![if down { 0 } else { 1 }]
        } else {
            (2..proto.topology.faces.len()).collect()
        };
        let mut faces = vec![];
        for index in selected {
            let mut face = proto.topology.faces[index].clone();
            let old = face.id;
            face.id = TopologyId {
                body: base.body_id,
                feature,
                region: base.input.region_id,
                local: proto_face_keys[index],
            };
            if !face_ids.insert(face.id) {
                return invalid("Stepped face identity collision");
            }
            for boundary in &mut face.loops {
                for co in boundary {
                    let original = &proto.topology.edges[co.edge];
                    let mut edge = original.clone();
                    for vertex in &mut edge.vertices {
                        let point = proto.topology.vertices[*vertex].point;
                        if let Some(i) = brep
                            .vertices
                            .iter()
                            .position(|v| dist(v.point, point) <= POLICY.linear_mm)
                        {
                            *vertex = i;
                        } else {
                            if brep
                                .vertices
                                .iter()
                                .any(|v| dist(v.point, point) < planar::RESOLUTION_MM)
                            {
                                return invalid("Stepped vertices are below resolution");
                            }
                            *vertex = brep.vertices.len();
                            brep.vertices.push(Vertex { point });
                        }
                    }
                    let midpoint = curve_midpoint(&edge, &brep.vertices);
                    let found = brep.edges.iter().position(|e| {
                        let same = e.vertices == edge.vertices
                            || e.vertices == [edge.vertices[1], edge.vertices[0]];
                        same && std::mem::discriminant(&e.curve)
                            == std::mem::discriminant(&edge.curve)
                            && dist(curve_midpoint(e, &brep.vertices), midpoint) <= POLICY.linear_mm
                    });
                    let candidate = proto_edge_keys[co.edge];
                    co.edge = if let Some(i) = found {
                        if brep.edges[i].vertices != edge.vertices {
                            co.reversed = !co.reversed;
                        }
                        edge_keys[i].insert(candidate);
                        i
                    } else {
                        edge.id = TopologyId {
                            body: base.body_id,
                            feature,
                            region: base.input.region_id,
                            local: 0,
                        };
                        let i = brep.edges.len();
                        brep.edges.push(edge);
                        edge_keys.push(BTreeSet::from([candidate]));
                        i
                    };
                    if brep.edges.len() > 16_384 || brep.vertices.len() > 16_384 {
                        return invalid("Stepped B-rep exceeds 16384 edges/vertices");
                    }
                }
            }
            faces.push((old, face.id));
            brep.faces.push(face);
        }
        parts.push(Part { input, faces });
    }
    let mut edge_ids = HashSet::new();
    for (edge, keys) in brep.edges.iter_mut().zip(edge_keys) {
        edge.id.local = planar::identity(&("edge", keys))?;
        if !edge_ids.insert(edge.id) {
            return invalid("Stepped edge identity collision");
        }
    }
    Ok((brep, parts))
}
fn curve_midpoint(edge: &Edge, vertices: &[Vertex]) -> [f64; 3] {
    match edge.curve {
        Curve::Line => std::array::from_fn(|i| {
            (vertices[edge.vertices[0]].point[i] + vertices[edge.vertices[1]].point[i]) * 0.5
        }),
        Curve::Arc {
            center,
            normal,
            x_axis,
            radius,
            start_angle,
            sweep,
        } => arc_point(center, normal, x_axis, radius, start_angle + sweep * 0.5),
        Curve::Circle { .. } => vertices[edge.vertices[0]].point,
    }
}
fn arc_point(center: [f64; 3], normal: [f64; 3], x: [f64; 3], radius: f64, angle: f64) -> [f64; 3] {
    let y = cross3(normal, x);
    std::array::from_fn(|i| center[i] + radius * (x[i] * angle.cos() + y[i] * angle.sin()))
}
impl Stepped {
    pub fn mesh(&self, topology: &Brep, segments: usize) -> Result<DisplayMesh> {
        if !(8..=4096).contains(&segments) {
            return invalid("Rust display tessellation requires 8..4096 segments");
        }
        topology.validate()?;
        let mut result = DisplayMesh::default();
        let body = topology.faces[0].id.body;
        for part in &self.parts {
            let shape = NativeSolid::extrude(part.input.clone())?;
            let mut mesh = shape.mesh_prism(segments)?;
            for id in &mut mesh.face_ids {
                *id = id.and_then(|id| {
                    part.faces
                        .iter()
                        .find(|(old, _)| *old == id)
                        .map(|(_, new)| TopologyId { body, ..*new })
                });
            }
            cuts::append_mesh(&mut result, mesh, |_| true, false, false);
        }
        for edge in &topology.edges {
            let points = match edge.curve {
                Curve::Line => edge
                    .vertices
                    .iter()
                    .map(|i| topology.vertices[*i].point)
                    .collect(),
                Curve::Arc {
                    center,
                    normal,
                    x_axis,
                    radius,
                    start_angle,
                    sweep,
                } => {
                    let count = (segments as f64 * sweep.abs() / std::f64::consts::TAU)
                        .ceil()
                        .max(1.) as usize;
                    (0..=count)
                        .map(|i| {
                            arc_point(
                                center,
                                normal,
                                x_axis,
                                radius,
                                start_angle + sweep * i as f64 / count as f64,
                            )
                        })
                        .collect::<Vec<_>>()
                }
                Curve::Circle { .. } => return invalid("Unsplit circle in stepped topology"),
            };
            for w in points.windows(2) {
                result.edges.extend([w[0], w[1]]);
                result.edge_ids.push(edge.id);
            }
        }
        Ok(result)
    }
}
