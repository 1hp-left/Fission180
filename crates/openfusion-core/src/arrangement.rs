//! Independent exact planar line/arc arrangement. Virtual spans retain source entities.
use crate::{
    EPS, Profile, Result, Sketch,
    boundary::{BoundarySegment, PlanarCurve, distance, intersections},
    invalid,
    sketch::{Entity, SketchGraph},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A directed source span and its two boundary junctions. Branch is the ordinal
/// finite intersection along the lower entity ID, never a coordinate or mesh index.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundaryUse {
    pub entity: u64,
    pub previous: u64,
    pub next: u64,
    pub start_branch: u8,
    pub end_branch: u8,
    pub forward: bool,
}
#[derive(Debug)]
pub(crate) struct Cell {
    pub cycle: Vec<u64>,
    pub points: Vec<[f64; 2]>,
    pub profile: Profile,
    pub key: Vec<BoundaryUse>,
}
pub(crate) fn canonical_cycle(ids: &[u64]) -> Vec<u64> {
    (0..ids.len())
        .flat_map(|i| {
            [
                (0..ids.len())
                    .map(|j| ids[(i + j) % ids.len()])
                    .collect::<Vec<_>>(),
                (0..ids.len())
                    .map(|j| ids[(i + ids.len() - j) % ids.len()])
                    .collect(),
            ]
        })
        .min()
        .unwrap_or_default()
}
pub(crate) fn canonical_boundary(key: &[BoundaryUse]) -> Vec<BoundaryUse> {
    (0..key.len())
        .map(|i| {
            (0..key.len())
                .map(|j| key[(i + j) % key.len()].clone())
                .collect::<Vec<_>>()
        })
        .min()
        .unwrap_or_default()
}
impl SketchGraph {
    pub(crate) fn line_cells(&self) -> Result<Vec<Cell>> {
        let mut curves = vec![];
        for (id, e) in &self.entities {
            if self.construction.contains(id) {
                continue;
            }
            let curve = match e {
                Entity::Line { a, b } => PlanarCurve::Line {
                    start: self.points[a],
                    end: self.points[b],
                },
                Entity::Arc { .. } => PlanarCurve::Arc {
                    geometry: self.arc_geometry(*id)?,
                },
                Entity::Circle { .. } => continue,
            };
            curve.validate()?;
            curves.push((*id, curve));
        }
        if curves.len() > 2000 {
            return invalid("Arrangement exceeds 2000 entities");
        }
        // Full circles retain their legacy primitive identities. Until their
        // seam-free arrangement is implemented, never ignore a crossing source.
        for (id, entity) in &self.entities {
            if self.construction.contains(id) {
                continue;
            }
            if let Entity::Circle { center, radius } = entity {
                let boundary = Profile::Circle {
                    center: self.points[center],
                    radius: *radius,
                }
                .boundary();
                for (_, curve) in &curves {
                    if boundary.iter().any(|s| {
                        intersections(s.curve, *curve).map_or(true, |hits| !hits.is_empty())
                    }) {
                        return invalid(
                            "Full-circle intersections with line/arc arrangements are unsupported; no boundary was ignored",
                        );
                    }
                }
            }
        }
        let mut cuts = vec![vec![(0., None), (1., None)]; curves.len()];
        // Pair/branch records remain associated with the same analytic intersection
        // even when several source entities meet at one arrangement vertex.
        let mut events = vec![];
        for i in 0..curves.len() {
            for j in i + 1..curves.len() {
                let hits = intersections(curves[i].1, curves[j].1)?;
                for (branch, (t, u)) in hits.into_iter().enumerate() {
                    let event = events.len();
                    events.push(((curves[i].0, curves[j].0), branch as u8));
                    cuts[i].push((t, Some(event)));
                    cuts[j].push((u, Some(event)));
                }
            }
        }
        let mut vertices: Vec<[f64; 2]> = vec![];
        let mut marks: Vec<BTreeMap<(u64, u64), u8>> = vec![];
        let mut edges = vec![];
        for ((id, curve), mut split) in curves.into_iter().zip(cuts) {
            split.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut nodes: Vec<(f64, usize)> = vec![];
            for (t, event) in split {
                let p = curve.point(t);
                let node = if let Some(i) = vertices.iter().position(|q| distance(*q, p) < EPS) {
                    i
                } else {
                    vertices.push(p);
                    marks.push(BTreeMap::new());
                    vertices.len() - 1
                };
                if let Some(e) = event {
                    marks[node].insert(events[e].0, events[e].1);
                }
                if nodes.last().is_none_or(|(_, v)| *v != node) {
                    nodes.push((t, node));
                }
            }
            for w in nodes.windows(2) {
                if w[0].1 != w[1].1 {
                    edges.push((w[0].1, w[1].1, id, curve.portion(w[0].0, w[1].0)));
                }
            }
        }
        if edges.len() > 100_000 {
            return invalid("Arrangement exceeds intersection limit");
        }
        if vertices.len() > 8192 {
            return invalid("Arrangement exceeds the supported vertex traversal limit");
        }
        let mut adjacent = vec![vec![]; vertices.len()];
        for (i, (a, b, _, _)) in edges.iter().enumerate() {
            adjacent[*a].push((i, *b));
            adjacent[*b].push((i, *a));
        }
        fn bridges(
            v: usize,
            parent: usize,
            adjacent: &[Vec<(usize, usize)>],
            clock: &mut usize,
            seen: &mut [usize],
            low: &mut [usize],
            removed: &mut BTreeSet<usize>,
        ) {
            *clock += 1;
            seen[v] = *clock;
            low[v] = *clock;
            for &(edge, to) in &adjacent[v] {
                if edge == parent {
                    continue;
                }
                if seen[to] == 0 {
                    bridges(to, edge, adjacent, clock, seen, low, removed);
                    low[v] = low[v].min(low[to]);
                    if low[to] > seen[v] {
                        removed.insert(edge);
                    }
                } else {
                    low[v] = low[v].min(seen[to]);
                }
            }
        }
        let mut seen = vec![0; vertices.len()];
        let mut low = seen.clone();
        let mut removed = BTreeSet::new();
        let mut clock = 0;
        for v in 0..vertices.len() {
            if seen[v] == 0 {
                bridges(
                    v,
                    usize::MAX,
                    &adjacent,
                    &mut clock,
                    &mut seen,
                    &mut low,
                    &mut removed,
                );
            }
        }
        let tangent = |edge: usize, v: usize| {
            let (a, _, _, curve) = edges[edge];
            if a == v {
                curve.tangent(0.)
            } else {
                curve.reversed().tangent(0.)
            }
        };
        for (v, list) in adjacent.iter_mut().enumerate() {
            list.retain(|(edge, _)| !removed.contains(edge));
            list.sort_by(|(a, _), (b, _)| {
                let a = tangent(*a, v);
                let b = tangent(*b, v);
                a[1].atan2(a[0]).total_cmp(&b[1].atan2(b[0]))
            });
            if list.len() > 2 {
                for i in 0..list.len() {
                    let a = tangent(list[i].0, v);
                    let b = tangent(list[(i + 1) % list.len()].0, v);
                    if crate::boundary::cross(a, b).abs()
                        < 1e-10 * a[0].hypot(a[1]) * b[0].hypot(b[1])
                        && a[0] * b[0] + a[1] * b[1] > 0.
                    {
                        return invalid(
                            "Tangent junction with multiple region branches is ambiguous",
                        );
                    }
                }
            }
        }
        let mut visited = BTreeSet::new();
        let mut cells = vec![];
        for (first, (a, b, _, _)) in edges.iter().enumerate() {
            if removed.contains(&first) {
                continue;
            }
            for (from, to) in [(*a, *b), (*b, *a)] {
                if visited.contains(&(first, from)) {
                    continue;
                }
                let mut walk = vec![];
                let mut edge = first;
                let mut v = from;
                let mut w = to;
                loop {
                    if !visited.insert((edge, v)) {
                        return invalid("Arrangement face walk did not close");
                    }
                    let (_, _, id, curve) = edges[edge];
                    let forward = edges[edge].0 == v;
                    walk.push((
                        id,
                        v,
                        w,
                        if forward { curve } else { curve.reversed() },
                        forward,
                    ));
                    let list = &adjacent[w];
                    let index = list.iter().position(|(e, _)| *e == edge).unwrap();
                    let (next, next_to) = list[(index + list.len() - 1) % list.len()];
                    edge = next;
                    v = w;
                    w = next_to;
                    if edge == first && v == from {
                        break;
                    }
                }
                let anchor = vertices[from];
                let area: f64 = walk.iter().map(|x| x.3.area_term(anchor)).sum();
                if area <= EPS * EPS {
                    continue;
                }
                let Some(offset) = (0..walk.len())
                    .find(|i| walk[*i].0 != walk[(*i + walk.len() - 1) % walk.len()].0)
                else {
                    return invalid("Single-entity closed bounded arc is invalid");
                };
                walk.rotate_left(offset);
                let mut compact: Vec<(u64, usize, usize, PlanarCurve, bool)> = vec![];
                for span in walk {
                    if let Some(last) = compact.last_mut().filter(|last| last.0 == span.0) {
                        last.2 = span.2;
                        last.3 = match (last.3, span.3) {
                            (PlanarCurve::Line { start, .. }, PlanarCurve::Line { end, .. }) => {
                                PlanarCurve::Line { start, end }
                            }
                            (
                                PlanarCurve::Arc { geometry: mut a },
                                PlanarCurve::Arc { geometry: b },
                            ) => {
                                a.sweep += b.sweep;
                                PlanarCurve::Arc { geometry: a }
                            }
                            _ => return invalid("Boundary source changed curve type"),
                        };
                    } else {
                        compact.push(span);
                    }
                }
                let mut key = vec![];
                let mut segments = vec![];
                for i in 0..compact.len() {
                    let (id, v, w, curve, forward) = compact[i];
                    let previous = compact[(i + compact.len() - 1) % compact.len()].0;
                    let next = compact[(i + 1) % compact.len()].0;
                    let branch = |node: usize, other: u64| -> Result<u8> {
                        marks[node]
                            .get(&(id.min(other), id.max(other)))
                            .copied()
                            .ok_or_else(|| {
                                crate::Error::Invalid("Missing analytic junction provenance".into())
                            })
                    };
                    key.push(BoundaryUse {
                        entity: id,
                        previous,
                        next,
                        start_branch: branch(v, previous)?,
                        end_branch: branch(w, next)?,
                        forward,
                    });
                    segments.push(BoundarySegment {
                        source: Some(id),
                        curve,
                    });
                }
                let canonical = canonical_boundary(&key);
                // Align boundary geometry to the same persistent directed source order.
                let start = (0..compact.len())
                    .find(|i| (0..key.len()).all(|j| key[(*i + j) % key.len()] == canonical[j]))
                    .unwrap();
                segments.rotate_left(start);
                compact.rotate_left(start);
                let key = canonical;
                let ids: Vec<_> = compact.iter().map(|x| x.0).collect();
                let points: Vec<_> = segments.iter().map(|s| s.curve.point(0.)).collect();
                let curved = segments
                    .iter()
                    .any(|s| matches!(s.curve, PlanarCurve::Arc { .. }));
                let profile = if curved {
                    Profile::Curved { segments }
                } else {
                    Profile::Polygon {
                        points: points.clone(),
                    }
                };
                profile.validate()?;
                cells.push(Cell {
                    cycle: canonical_cycle(&ids),
                    points,
                    profile,
                    key: if curved { key } else { vec![] },
                });
            }
        }
        cells.sort_by(|a, b| a.cycle.cmp(&b.cycle).then(a.key.cmp(&b.key)));
        if cells
            .windows(2)
            .any(|c| c[0].cycle == c[1].cycle && c[0].key == c[1].key)
        {
            return invalid("Ambiguous sketch boundary provenance");
        }
        Ok(cells)
    }
}
impl Sketch {
    pub(crate) fn refresh_line_regions(&self, mut graph: SketchGraph) -> Result<Self> {
        graph.validate()?;
        let mut candidate = self.clone();
        // Seed pre-arrangement files and primitive tools without changing their IDs.
        for (id, entities) in &graph.profile_entities {
            if !graph.region_cycles.contains_key(id)
                && entities
                    .iter()
                    .all(|e| matches!(graph.entities.get(e), Some(Entity::Line { .. })))
            {
                graph.region_cycles.insert(*id, canonical_cycle(entities));
            }
        }
        let cells = graph.line_cells()?;
        // Bind a newly supplied analytic loop once. Subsequent discovery uses
        // the full directed junction key, including intersection branch.
        for (id, p) in self.profile_ids.iter().zip(&self.profiles) {
            if matches!(p, Profile::Curved { .. }) && !graph.curve_regions.contains_key(id) {
                if let Some(entities) = graph.profile_entities.get(id) {
                    let cycle = canonical_cycle(entities);
                    let matching: Vec<_> = cells
                        .iter()
                        .filter(|c| c.cycle == cycle && !c.key.is_empty())
                        .collect();
                    if matching.len() != 1 {
                        return invalid("Supplied curved loop has ambiguous source provenance");
                    }
                    graph.curve_regions.insert(*id, matching[0].key.clone());
                }
            }
        }
        let mut registry = BTreeMap::new();
        for (id, cycle) in &graph.region_cycles {
            if registry.insert(cycle.clone(), *id).is_some() {
                return invalid("Duplicate sketch region provenance");
            }
        }
        let mut profiles = BTreeMap::new();
        for (id, p) in self.profile_ids.iter().zip(&self.profiles) {
            if matches!(p, Profile::Circle { .. }) {
                profiles.insert(*id, graph.profile(*id, p)?);
            } else if graph
                .profile_entities
                .get(id)
                .is_some_and(|edges| edges.iter().any(|e| graph.construction.contains(e)))
            {
                // Keep whole primitive construction loops editable as primitives.
                // A partition with virtual intersections is rebuilt when active.
                if let Ok(p) = graph.primitive_profile(*id, p) {
                    if p.validate().is_ok() {
                        profiles.insert(*id, p);
                    }
                }
            }
        }
        for cell in cells {
            let existing = if cell.key.is_empty() {
                registry.get(&cell.cycle).copied()
            } else {
                graph
                    .curve_regions
                    .iter()
                    .find(|(_, k)| **k == cell.key)
                    .map(|(id, _)| *id)
            };
            let id = if let Some(id) = existing {
                id
            } else {
                let id = candidate.next_profile_id;
                candidate.next_profile_id = id
                    .checked_add(1)
                    .ok_or_else(|| crate::Error::Invalid("Sketch profile IDs exhausted".into()))?;
                if cell.key.is_empty() {
                    graph.region_cycles.insert(id, cell.cycle.clone());
                } else {
                    graph.curve_regions.insert(id, cell.key.clone());
                }
                graph.profile_entities.insert(id, cell.cycle.clone());
                id
            };
            let original = self
                .profile_ids
                .iter()
                .position(|i| *i == id)
                .map(|i| &self.profiles[i]);
            let profile = if !cell.key.is_empty() {
                cell.profile
            } else if let Some(p @ Profile::Rectangle { .. }) = original {
                graph.primitive_profile(id, p)?
            } else {
                Profile::Polygon {
                    points: cell.points,
                }
            };
            profile.validate()?;
            profiles.insert(id, profile);
        }
        (candidate.profile_ids, candidate.profiles) = profiles.into_iter().unzip();
        candidate.graph = Some(graph);
        candidate.regions()?;
        Ok(candidate)
    }
}
