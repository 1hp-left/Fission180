//! Bounded same-cap overlapping blind cuts as exact occupied axial sections.
//! Each patch is analytic; interfaces are removed and shared edges are welded.
use super::*;
use crate::planar::{self, Boolean, Region, RegionSet};
use std::collections::{BTreeSet, HashSet};

#[derive(Debug, Clone, Serialize)]
enum Level {
    Low,
    High,
    Floors(Vec<u64>),
}
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
    pub occupied: RegionSet,
}
#[derive(Debug, Clone)]
pub(super) struct Stepped {
    pub volume: f64,
    pub area: f64,
    pub slabs: Vec<Slab>,
    parts: Vec<Part>,
}
fn axes(plane: Plane) -> [usize; 2] {
    match plane {
        Plane::XY => [0, 1],
        Plane::XZ => [0, 2],
        Plane::YZ => [1, 2],
    }
}
pub(super) fn cut(
    a: &NativeSolid,
    b: &NativeSolid,
    feature: u64,
) -> Result<Option<Vec<NativeSolid>>> {
    if (a.pockets.is_empty() && b.input.holes.is_empty())
        || !b.pockets.is_empty()
        || a.input.plane != b.input.plane
    {
        return Ok(None);
    }
    let Some(mut tool) = cuts::reframe(&a.input, &b.input) else {
        return Ok(None);
    };
    let target = cuts::interval(&a.input);
    let cutter = cuts::interval(&tool);
    let lo = target[0].max(cutter[0]);
    let hi = target[1].min(cutter[1]);
    if hi - lo <= POLICY.linear_mm {
        return Ok(Some(vec![a.clone()]));
    }
    let through =
        cutter[0] <= target[0] + POLICY.linear_mm && cutter[1] >= target[1] - POLICY.linear_mm;
    let overlaps = a
        .pockets
        .iter()
        .any(|p| !cuts::disjoint(&p.input.outer, &tool.outer));
    if a.stepped.is_none() && ((!overlaps && b.input.holes.is_empty()) || through) {
        return Ok(None);
    }
    if through
        && tool.holes.is_empty()
        && (crate::region::strictly_contains(&tool.outer, &a.input.outer)
            || tool
                .outer
                .geometrically_equal(&a.input.outer, POLICY.linear_mm))
    {
        return Ok(Some(vec![]));
    }
    if !crate::region::strictly_contains(&a.input.outer, &tool.outer) {
        return invalid("Overlapping blind cuts must be strictly inside the target sidewall");
    }
    if a.input
        .holes
        .iter()
        .any(|h| !cuts::disjoint(h, &tool.outer))
    {
        return invalid(
            "Overlapping blind cuts touching/intersecting through-holes are unsupported",
        );
    }
    let delta = axes(a.input.plane).map(|i| b.input.origin[i] - a.input.origin[i]);
    let region = prisms::shifted(&b.planar, delta);
    if through {
        if !tool.holes.is_empty() || overlaps {
            return invalid("Through-cuts interacting with stepped pockets are unsupported");
        }
        let mut result = a.clone();
        result.input.holes.push(tool.outer);
        let next = result
            .input
            .loop_ids
            .iter()
            .copied()
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| crate::Error::Invalid("Topology loop identities exhausted".into()))?;
        result.input.loop_ids.push(next);
        let mut hole = region.outer;
        hole.spans = hole
            .spans
            .iter()
            .rev()
            .map(planar::Span::reversed)
            .collect();
        result.planar.holes.push(hole);
        result.rebuild_steps(feature)?;
        return Ok(Some(vec![result]));
    }
    let enters_low = cutter[0] <= target[0] + POLICY.linear_mm;
    let enters_high = cutter[1] >= target[1] - POLICY.linear_mm;
    if enters_low == enters_high {
        return invalid(
            "Overlapping blind cut must enter one end cap; enclosed cavities are unsupported",
        );
    }
    let (floor, opening) = if enters_high {
        (lo, target[1])
    } else {
        (hi, target[0])
    };
    let normal = a.input.plane.normal().to_array();
    let start = dot(a.input.origin, normal);
    let opening_cap = usize::from((opening - start).abs() > POLICY.linear_mm);
    if a.pockets.iter().any(|p| p.opening_cap != opening_cap) {
        return invalid("Opposite-side overlapping pocket interactions are unsupported");
    }
    if a.pockets.len() >= 32 {
        return invalid("Overlapping blind cuts exceed the 32-pocket section limit");
    }
    tool.feature_id = feature;
    tool.distance = opening - floor;
    tool.origin = std::array::from_fn(|i| a.input.origin[i] + normal[i] * (floor - start));
    let mut result = a.clone();
    result.pockets.push(cuts::Pocket {
        input: tool,
        opening_cap,
        region,
    });
    result.rebuild_steps(feature)?;
    Ok(Some(vec![result]))
}
impl NativeSolid {
    pub(super) fn rebuild_steps(&mut self, feature: u64) -> Result<()> {
        let target = cuts::interval(&self.input);
        let mut levels = vec![target[0], target[1]];
        for p in &self.pockets {
            levels.push(dot(p.input.origin, self.input.plane.normal().to_array()));
        }
        levels.sort_by(f64::total_cmp);
        let mut z: Vec<f64> = vec![];
        for value in levels {
            if let Some(last) = z.last() {
                let gap = (value - last).abs();
                if gap <= POLICY.linear_mm {
                    continue;
                }
                if gap < planar::RESOLUTION_MM {
                    return invalid("Pocket floors are closer than 0.00001 mm");
                }
            }
            z.push(value);
        }
        let level = |height: f64| -> Level {
            if (height - target[0]).abs() <= POLICY.linear_mm {
                return Level::Low;
            }
            if (height - target[1]).abs() <= POLICY.linear_mm {
                return Level::High;
            }
            let mut ids: Vec<_> = self
                .pockets
                .iter()
                .filter(|p| {
                    (dot(p.input.origin, self.input.plane.normal().to_array()) - height).abs()
                        <= POLICY.linear_mm
                })
                .map(|p| p.input.feature_id)
                .collect();
            ids.sort_unstable();
            ids.dedup();
            Level::Floors(ids)
        };
        let base = RegionSet {
            regions: vec![self.planar.clone()],
        };
        base.validate()?;
        let mut slabs = vec![];
        for w in z.windows(2) {
            let mid = (w[0] + w[1]) * 0.5;
            let mut occupied = base.clone();
            for pocket in &self.pockets {
                let interval = cuts::interval(&pocket.input);
                if mid > interval[0] && mid < interval[1] {
                    occupied = occupied.boolean(
                        &RegionSet {
                            regions: vec![pocket.region.clone()],
                        },
                        Boolean::Difference,
                    )?;
                }
            }
            if occupied.regions.is_empty() {
                return invalid("A blind pocket removed an entire axial section");
            }
            slabs.push(Slab {
                interval: [w[0], w[1]],
                occupied,
            });
        }
        let mut patches = vec![];
        let mut volume = 0.;
        let mut area = 0.;
        for slab in &slabs {
            let length = slab.interval[1] - slab.interval[0];
            volume += slab.occupied.area() * length;
            for region in &slab.occupied.regions {
                area += region.perimeter() * length;
                patches.push(Patch {
                    region: region.clone(),
                    lo: slab.interval[0],
                    hi: slab.interval[1],
                    kind: Kind::Sides(level(slab.interval[0]), level(slab.interval[1])),
                });
            }
        }
        let mut cap = |set: RegionSet, height: f64, down: bool| {
            area += set.area();
            for region in set.regions {
                patches.push(Patch {
                    region,
                    lo: height,
                    hi: height,
                    kind: Kind::Cap(level(height), down),
                });
            }
        };
        cap(slabs[0].occupied.clone(), target[0], true);
        cap(slabs.last().unwrap().occupied.clone(), target[1], false);
        for pair in slabs.windows(2) {
            let below = &pair[0].occupied;
            let above = &pair[1].occupied;
            let height = pair[0].interval[1];
            cap(below.boolean(above, Boolean::Difference)?, height, false);
            cap(above.boolean(below, Boolean::Difference)?, height, true);
        }
        let mut regions: Vec<_> = patches.iter().map(|p| p.region.clone()).collect();
        planar::refine_sections(&mut regions)?;
        for (patch, region) in patches.iter_mut().zip(regions) {
            patch.region = region;
        }
        let (topology, parts) = assemble(self, &patches, feature)?;
        topology.validate()?;
        if !volume.is_finite() || volume <= 0. || !area.is_finite() || area <= 0. {
            return invalid("Invalid stepped analytic measurements");
        }
        self.topology = topology;
        self.stepped = Some(Stepped {
            volume,
            area,
            slabs,
            parts,
        });
        self.planar_named = false;
        Ok(())
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
