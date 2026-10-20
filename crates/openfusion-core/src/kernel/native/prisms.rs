//! Same-axis, equal occupied-interval analytic prism Booleans.
use super::*;
use crate::{
    boundary::PlanarCurve,
    planar::{self, Boolean, RegionSet},
};
/// Prove a rectangular silhouette from its actual finite analytic boundary,
/// including retained collinear source subdivisions after a planar Boolean.
pub(super) fn rectangular(p: &Profile) -> bool {
    let b = p.planar_bounds();
    p.boundary().iter().all(|s| match s.curve {
        PlanarCurve::Line { start, end } => (0..2).any(|i| {
            (start[i] - end[i]).abs() <= POLICY.linear_mm
                && [b[0][i], b[1][i]].iter().any(|v| {
                    (start[i] - v).abs() <= POLICY.linear_mm
                        && (end[i] - v).abs() <= POLICY.linear_mm
                })
        }),
        _ => false,
    })
}
pub(super) fn instance(region: &mut planar::Region, feature: u64, index: u64) {
    let source = |mut s: planar::Source| {
        s.occurrences.push((feature, index));
        s
    };
    let junction = |j: planar::Junction| match j {
        planar::Junction::Endpoint(s, end) => planar::Junction::Endpoint(source(s), end),
        planar::Junction::Intersection(a, b, branch) => {
            planar::Junction::Intersection(source(a), source(b), branch)
        }
    };
    for l in std::iter::once(&mut region.outer).chain(&mut region.holes) {
        for span in &mut l.spans {
            for a in &mut span.ancestry {
                a.source = source(a.source.clone());
            }
            span.start = span.start.iter().cloned().map(junction).collect();
            span.end = span.end.iter().cloned().map(junction).collect();
        }
    }
}

pub(super) fn shifted(region: &planar::Region, delta: [f64; 2]) -> planar::Region {
    let mut result = region.clone();
    for l in std::iter::once(&mut result.outer).chain(&mut result.holes) {
        for s in &mut l.spans {
            let at = |p: [f64; 2]| [p[0] + delta[0], p[1] + delta[1]];
            s.curve = match s.curve {
                PlanarCurve::Line { start, end } => PlanarCurve::Line {
                    start: at(start),
                    end: at(end),
                },
                PlanarCurve::Arc { geometry: mut a } => {
                    a.center = at(a.center);
                    PlanarCurve::Arc { geometry: a }
                }
            };
        }
    }
    result
}
pub(super) fn boolean(
    a: &NativeSolid,
    b: &NativeSolid,
    op: Operation,
    feature: u64,
) -> Result<Option<Vec<NativeSolid>>> {
    if !a.pockets.is_empty() || !b.pockets.is_empty() || a.input.plane != b.input.plane {
        return Ok(None);
    }
    let aa = cuts::interval(&a.input);
    let bb = cuts::interval(&b.input);
    let same = (0..2).all(|i| (aa[i] - bb[i]).abs() <= POLICY.linear_mm);
    let covers = op == Operation::Cut
        && bb[0] <= aa[0] + POLICY.linear_mm
        && bb[1] >= aa[1] - POLICY.linear_mm;
    if !same && !covers {
        return Ok(None);
    }
    let axes = match a.input.plane {
        Plane::XY => [0, 1],
        Plane::XZ => [0, 2],
        Plane::YZ => [1, 2],
    };
    let delta = axes.map(|i| b.input.origin[i] - a.input.origin[i]);
    let left = RegionSet {
        regions: vec![a.planar.clone()],
    };
    let right = RegionSet {
        regions: vec![shifted(&b.planar, delta)],
    };
    let op = match op {
        Operation::Join => Boolean::Union,
        Operation::Cut => Boolean::Difference,
        Operation::Intersect => Boolean::Intersection,
        _ => return invalid("New Body is not a Boolean operation"),
    };
    let regions = left.boolean(&right, op)?;
    let aa = a.bounds();
    let bb = b.bounds();
    let legacy_disjoint_join = op == Boolean::Union
        && (0..3).any(|i| {
            aa[1][i] < bb[0][i] - POLICY.linear_mm || bb[1][i] < aa[0][i] - POLICY.linear_mm
        });
    let mut solids = vec![];
    let mut region_keys = std::collections::BTreeMap::new();
    for region in regions.regions {
        let signature = region.key();
        let id = planar::identity(&signature)?;
        if region_keys.insert(id, signature).is_some() {
            return invalid("Planar result identity collision; no alternate body was selected");
        }
        let mut input = a.input.clone();
        input.feature_id = feature;
        input.region_id = id;
        input.outer = region.outer.profile();
        input.holes = region.holes.iter().map(|h| h.profile()).collect();
        // Construction ordinals are transient. Rebind all public topology IDs
        // to source relationships below, after shared-edge shell validation.
        input.loop_ids = (1..=input.holes.len() as u64 + 1).collect();
        let mut solid = NativeSolid::extrude(input)?;
        solid.planar = region;
        // Existing v5 box results persist IDs in bounding-box order. Keep that
        // contract for rectangular outputs; source keys still guard edits and
        // identify every edge/face. Newly supported curved outputs use ancestry.
        solid.legacy_output_order = legacy_disjoint_join
            || (rectangular(&a.input.outer)
                && rectangular(&b.input.outer)
                && a.input.holes.is_empty()
                && b.input.holes.is_empty()
                && solid.input.holes.is_empty()
                && rectangular(&solid.input.outer));
        solid.bind_body(a.body_id);
        solid.name_planar_topology()?;
        solids.push(solid);
    }
    // Source ancestry, rather than moving bounding boxes, orders split outputs.
    solids.sort_by_key(|s| s.planar.key());
    Ok(Some(solids))
}
impl NativeSolid {
    pub(super) fn name_planar_topology(&mut self) -> Result<()> {
        self.planar_named = true;
        let mut local_ids = std::collections::HashSet::from([0, 1]);
        let mut edge_index = 0;
        let mut face_index = 2;
        for l in self.planar.loops() {
            let n = l.spans.len();
            for (i, s) in l.spans.iter().enumerate() {
                let i = if l.signed_area() < 0. { n - 1 - i } else { i };
                for role in 0..4u8 {
                    let local = planar::identity(&(s.key(), role))?;
                    if !local_ids.insert(local) {
                        return invalid(
                            "Planar topology identity collision; no alternate edge was selected",
                        );
                    }
                    if role < 3 {
                        self.topology.edges[edge_index + role as usize * n + i]
                            .id
                            .local = local;
                    } else {
                        self.topology.faces[face_index + i].id.local = local;
                    }
                }
            }
            edge_index += 3 * n;
            face_index += n;
        }
        self.topology.validate()
    }
}
