//! Strictly contained cap-entering cuts. No surface intersection/general Boolean
//! algorithm is implied by this bounded construction. No OCCT calls occur here.
use super::*;
use crate::boundary::{PlanarCurve, intersections};

#[derive(Debug, Clone)]
pub(super) struct Pocket {
    /// Removed prism from floor to opening; its parametric end cap is the opening.
    pub input: ExtrudeInput,
    pub opening_cap: usize,
    pub region: crate::planar::Region,
}
pub(super) fn interval(input: &ExtrudeInput) -> [f64; 2] {
    let start = dot(input.origin, input.plane.normal().to_array());
    [
        start.min(start + input.distance),
        start.max(start + input.distance),
    ]
}
pub(super) fn shifted(profile: &Profile, delta: [f64; 2]) -> Profile {
    let at = |p: [f64; 2]| [p[0] + delta[0], p[1] + delta[1]];
    match profile {
        Profile::Rectangle { a, b } => Profile::Rectangle {
            a: at(*a),
            b: at(*b),
        },
        Profile::Circle { center, radius } => Profile::Circle {
            center: at(*center),
            radius: *radius,
        },
        Profile::Polygon { points } => Profile::Polygon {
            points: points.iter().copied().map(at).collect(),
        },
        Profile::Curved { segments } => Profile::Curved {
            segments: segments
                .iter()
                .map(|s| crate::boundary::BoundarySegment {
                    source: s.source,
                    curve: match s.curve {
                        PlanarCurve::Line { start, end } => PlanarCurve::Line {
                            start: at(start),
                            end: at(end),
                        },
                        PlanarCurve::Arc { geometry: mut a } => {
                            a.center = at(a.center);
                            PlanarCurve::Arc { geometry: a }
                        }
                    },
                })
                .collect(),
        },
    }
}
pub(super) fn reframe(target: &ExtrudeInput, tool: &ExtrudeInput) -> Option<ExtrudeInput> {
    if target.plane != tool.plane {
        return None;
    }
    let mut input = tool.clone();
    let axes = match target.plane {
        Plane::XY => [0, 1],
        Plane::XZ => [0, 2],
        Plane::YZ => [1, 2],
    };
    let delta = axes.map(|i| tool.origin[i] - target.origin[i]);
    input.outer = shifted(&tool.outer, delta);
    input.holes = tool.holes.iter().map(|p| shifted(p, delta)).collect();
    for i in axes {
        input.origin[i] = target.origin[i];
    }
    Some(input)
}
pub(super) fn disjoint(a: &Profile, b: &Profile) -> bool {
    !a.contains_point(b.boundary()[0].curve.point(0.))
        && !b.contains_point(a.boundary()[0].curve.point(0.))
        && a.boundary().iter().all(|x| {
            b.boundary()
                .iter()
                .all(|y| intersections(x.curve, y.curve).is_ok_and(|hits| hits.is_empty()))
        })
}
pub(super) fn pockets_equal(a: &NativeSolid, b: &NativeSolid) -> bool {
    a.pockets.len() == b.pockets.len()
        && a.pockets.iter().zip(&b.pockets).all(|(a, b)| {
            a.opening_cap == b.opening_cap
                && a.input.origin == b.input.origin
                && (a.input.distance - b.input.distance).abs() < POLICY.linear_mm
                && a.input
                    .outer
                    .geometrically_equal(&b.input.outer, POLICY.linear_mm)
                && a.input.holes.len() == b.input.holes.len()
                && a.input
                    .holes
                    .iter()
                    .zip(&b.input.holes)
                    .all(|(a, b)| a.geometrically_equal(b, POLICY.linear_mm))
        })
}
pub(super) fn contained_cut(
    a: &NativeSolid,
    b: &NativeSolid,
    feature: u64,
) -> Result<Option<Vec<NativeSolid>>> {
    if !b.pockets.is_empty() || !b.input.holes.is_empty() {
        return Ok(None);
    }
    let Some(tool) = reframe(&a.input, &b.input) else {
        return Ok(None);
    };
    let axes = match a.input.plane {
        Plane::XY => [0, 1],
        Plane::XZ => [0, 2],
        Plane::YZ => [1, 2],
    };
    let tool_region = super::prisms::shifted(
        &b.planar,
        axes.map(|i| b.input.origin[i] - a.input.origin[i]),
    );
    let target = interval(&a.input);
    let cutter = interval(&tool);
    let lo = target[0].max(cutter[0]);
    let hi = target[1].min(cutter[1]);
    if hi - lo <= POLICY.linear_mm {
        return Ok(Some(vec![a.clone()]));
    }
    let through =
        cutter[0] <= target[0] + POLICY.linear_mm && cutter[1] >= target[1] - POLICY.linear_mm;
    if through
        && (crate::region::strictly_contains(&tool.outer, &a.input.outer)
            || tool
                .outer
                .geometrically_equal(&a.input.outer, POLICY.linear_mm))
    {
        return Ok(Some(vec![]));
    }
    if !crate::region::strictly_contains(&a.input.outer, &tool.outer) {
        return Ok(None);
    }
    if a.input
        .holes
        .iter()
        .chain(a.pockets.iter().map(|p| &p.input.outer))
        .any(|p| !disjoint(p, &tool.outer))
    {
        return invalid(
            "Rust contained cut intersects/touches an existing void; general curved intersections are unsupported",
        );
    }
    if through {
        let mut input = a.input.clone();
        input.feature_id = feature;
        input.holes.push(tool.outer);
        input.loop_ids.push(next_loop(&input.loop_ids)?);
        let mut result = a.rebuilt(input)?;
        result.planar = a.planar.clone();
        let mut hole = tool_region.outer;
        hole.spans = hole
            .spans
            .iter()
            .rev()
            .map(crate::planar::Span::reversed)
            .collect();
        result.planar.holes.push(hole);
        return Ok(Some(vec![result]));
    }
    let enters_low = cutter[0] <= target[0] + POLICY.linear_mm;
    let enters_high = cutter[1] >= target[1] - POLICY.linear_mm;
    if enters_low == enters_high {
        return invalid("Rust blind cut must enter one end cap; enclosed cavities are unsupported");
    }
    if a.pockets.len() >= 64 {
        return invalid("Rust contained cut exceeds 64 disjoint pockets");
    }
    let (floor, opening) = if enters_high {
        (lo, target[1])
    } else {
        (hi, target[0])
    };
    let normal = a.input.plane.normal().to_array();
    let start = dot(a.input.origin, normal);
    let mut input = tool;
    input.feature_id = feature;
    input.distance = opening - floor;
    input.origin = std::array::from_fn(|i| a.input.origin[i] + normal[i] * (floor - start));
    let opening_cap = usize::from((opening - start).abs() > POLICY.linear_mm);
    let mut result = a.clone();
    let region = tool_region;
    result.append_pocket(Pocket {
        input,
        opening_cap,
        region,
    })?;
    Ok(Some(vec![result]))
}
fn next_loop(ids: &[u64]) -> Result<u64> {
    ids.iter()
        .copied()
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| crate::Error::Invalid("Topology loop identities exhausted".into()))
}
impl NativeSolid {
    pub(super) fn rebuilt(&self, input: ExtrudeInput) -> Result<Self> {
        let mut result = Self::extrude(input)?;
        for pocket in &self.pockets {
            result.append_pocket(pocket.clone())?;
        }
        result.bind_body(self.body_id);
        Ok(result)
    }
    pub(super) fn append_pocket(&mut self, pocket: Pocket) -> Result<()> {
        let tool = Self::extrude(pocket.input.clone())?;
        let mut brep = self.topology.clone();
        let vertex_base = brep.vertices.len();
        let edge_base = brep.edges.len();
        brep.vertices.extend(tool.topology.vertices);
        brep.edges
            .extend(tool.topology.edges.into_iter().map(|mut e| {
                e.vertices = e.vertices.map(|i| i + vertex_base);
                e.id.body = self.body_id;
                e
            }));
        for (index, mut face) in tool.topology.faces.into_iter().enumerate() {
            for boundary in &mut face.loops {
                for co in boundary.iter_mut() {
                    co.edge += edge_base;
                }
                reverse(boundary);
            }
            if index == 1 {
                brep.faces[pocket.opening_cap].loops.extend(face.loops);
            } else {
                face.id.body = self.body_id;
                face.reversed = !face.reversed;
                brep.faces.push(face);
            }
        }
        brep.validate()?;
        self.topology = brep;
        self.pockets.push(pocket);
        Ok(())
    }
    pub(super) fn mesh_pockets(&self, segments: usize) -> Result<DisplayMesh> {
        self.topology.validate()?;
        let mut mesh = DisplayMesh::default();
        append_mesh(
            &mut mesh,
            self.mesh_prism(segments)?,
            |id| id.local > 1,
            false,
            true,
        );
        for cap in 0..2 {
            let mut input = self.input.clone();
            for p in self.pockets.iter().filter(|p| p.opening_cap == cap) {
                input.holes.push(p.input.outer.clone());
                input.loop_ids.push(next_loop(&input.loop_ids)?);
            }
            let mut prism = Self::extrude(input)?;
            prism.bind_body(self.body_id);
            append_mesh(
                &mut mesh,
                prism.mesh_prism(segments)?,
                |id| id.local == cap as u64,
                false,
                false,
            );
        }
        for pocket in &self.pockets {
            let mut tool = Self::extrude(pocket.input.clone())?;
            tool.bind_body(self.body_id);
            append_mesh(
                &mut mesh,
                tool.mesh_prism(segments)?,
                |id| id.local != 1,
                true,
                true,
            );
        }
        Ok(mesh)
    }
}
pub(super) fn append_mesh(
    mesh: &mut DisplayMesh,
    part: DisplayMesh,
    include: impl Fn(TopologyId) -> bool,
    invert: bool,
    edges: bool,
) {
    for (index, tri) in part.indices.chunks_exact(3).enumerate() {
        if let Some(id) = part.face_ids[index].filter(|id| include(*id)) {
            let normal = part.normals[tri[0]].map(|n| n * if invert { -1. } else { 1. });
            triangle(
                mesh,
                [
                    part.vertices[tri[0]],
                    part.vertices[tri[1]],
                    part.vertices[tri[2]],
                ],
                normal,
                id,
            );
        }
    }
    if edges {
        mesh.edges.extend(part.edges);
        mesh.edge_ids.extend(part.edge_ids);
    }
}
