//! Independent planar line arrangement. Intersections split analysis edges, never
//! the user's entities or constraints. Boundary identities use source entity cycles.
use crate::{
    EPS, Profile, Result, Sketch, invalid,
    sketch::{Entity, SketchGraph},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
pub(crate) struct Cell {
    pub cycle: Vec<u64>,
    pub points: Vec<[f64; 2]>,
}
fn cross(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
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
impl SketchGraph {
    pub(crate) fn line_cells(&self) -> Result<Vec<Cell>> {
        let mut lines = vec![];
        for (id, e) in &self.entities {
            if self.construction.contains(id) {
                continue;
            }
            if let Entity::Line { a, b } = e {
                let a = self.points[a];
                let b = self.points[b];
                if distance(a, b) < EPS {
                    return invalid("Degenerate line in sketch arrangement");
                }
                lines.push((*id, a, b));
            }
        }
        if lines.len() > 2000 {
            return invalid("Line arrangement exceeds 2000 entities");
        }
        let mut cuts = vec![vec![0., 1.]; lines.len()];
        for i in 0..lines.len() {
            for j in i + 1..lines.len() {
                let (_, a, b) = lines[i];
                let (_, c, d) = lines[j];
                let ab = sub(b, a);
                let cd = sub(d, c);
                let ac = sub(c, a);
                let la = distance(a, b);
                let lb = distance(c, d);
                let denom = cross(ab, cd);
                if denom.abs() <= 1e-12 * la * lb {
                    if cross(ab, ac).abs() / la <= EPS {
                        let axis = usize::from(ab[1].abs() > ab[0].abs());
                        let (t, u) = (
                            (c[axis] - a[axis]) / ab[axis],
                            (d[axis] - a[axis]) / ab[axis],
                        );
                        if t.max(u).min(1.) - t.min(u).max(0.) > EPS / la {
                            return invalid(
                                "Overlapping collinear sketch lines are ambiguous; remove the duplicate segment",
                            );
                        }
                    }
                    continue;
                }
                let t = cross(ac, cd) / denom;
                let u = cross(ac, ab) / denom;
                if t >= -EPS / la && t <= 1. + EPS / la && u >= -EPS / lb && u <= 1. + EPS / lb {
                    cuts[i].push(t.clamp(0., 1.));
                    cuts[j].push(u.clamp(0., 1.));
                }
            }
        }
        let mut vertices: Vec<[f64; 2]> = vec![];
        let mut edges = vec![];
        for ((id, a, b), mut split) in lines.into_iter().zip(cuts) {
            split.sort_by(f64::total_cmp);
            let len = distance(a, b);
            split.dedup_by(|a, b| (*a - *b).abs() * len < EPS);
            let mut nodes = vec![];
            for t in split {
                let p = [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])];
                let node = if let Some(i) = vertices.iter().position(|q| distance(*q, p) < EPS) {
                    i
                } else {
                    vertices.push(p);
                    vertices.len() - 1
                };
                nodes.push(node);
            }
            for w in nodes.windows(2) {
                if w[0] != w[1] {
                    edges.push((w[0], w[1], id));
                }
            }
        }
        if edges.len() > 100_000 {
            return invalid("Line arrangement exceeds intersection limit");
        }
        let mut adjacent = vec![vec![]; vertices.len()];
        for (i, (a, b, _)) in edges.iter().enumerate() {
            adjacent[*a].push((i, *b));
            adjacent[*b].push((i, *a));
        }
        // Remove bridges, including bridges between closed components. They do
        // not bound a region and must not produce self-touching face walks.
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
        for (v, list) in adjacent.iter_mut().enumerate() {
            list.retain(|(edge, _)| !removed.contains(edge));
            list.sort_by(|(_, a), (_, b)| {
                let a = sub(vertices[*a], vertices[v]);
                let b = sub(vertices[*b], vertices[v]);
                a[1].atan2(a[0]).total_cmp(&b[1].atan2(b[0]))
            });
        }
        let mut visited = BTreeSet::new();
        let mut cells = vec![];
        for (first, (a, b, _)) in edges.iter().enumerate() {
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
                        break;
                    }
                    walk.push((edges[edge].2, vertices[v]));
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
                let area: f64 = (0..walk.len())
                    .map(|i| {
                        cross(
                            sub(walk[i].1, walk[0].1),
                            sub(walk[(i + 1) % walk.len()].1, walk[0].1),
                        )
                    })
                    .sum();
                if area <= EPS * EPS {
                    continue;
                }
                // Consecutive analysis spans on one original line are one
                // boundary use. Attaching a dangling T does not change identity.
                let mut compact = vec![];
                for i in 0..walk.len() {
                    if walk[i].0 != walk[(i + walk.len() - 1) % walk.len()].0 {
                        compact.push(walk[i]);
                    }
                }
                if compact.len() < 3 {
                    continue;
                }
                let ids: Vec<_> = compact.iter().map(|x| x.0).collect();
                let cycle = canonical_cycle(&ids);
                // Keep CCW geometry, but rotate it deterministically.
                let offset = (0..ids.len())
                    .min_by_key(|i| {
                        (0..ids.len())
                            .map(|j| ids[(i + j) % ids.len()])
                            .collect::<Vec<_>>()
                    })
                    .unwrap();
                compact.rotate_left(offset);
                let points: Vec<_> = compact.into_iter().map(|x| x.1).collect();
                Profile::Polygon {
                    points: points.clone(),
                }
                .validate()?;
                cells.push(Cell { cycle, points });
            }
        }
        cells.sort_by(|a, b| a.cycle.cmp(&b.cycle));
        if cells.windows(2).any(|c| c[0].cycle == c[1].cycle) {
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
                    p.validate()?;
                    profiles.insert(*id, p);
                }
            }
        }
        for cell in cells {
            let id = if let Some(id) = registry.get(&cell.cycle) {
                *id
            } else {
                let id = candidate.next_profile_id;
                candidate.next_profile_id = id
                    .checked_add(1)
                    .ok_or_else(|| crate::Error::Invalid("Sketch profile IDs exhausted".into()))?;
                graph.region_cycles.insert(id, cell.cycle.clone());
                graph.profile_entities.insert(id, cell.cycle.clone());
                id
            };
            let original = self
                .profile_ids
                .iter()
                .position(|i| *i == id)
                .map(|i| &self.profiles[i]);
            let profile = if let Some(p @ Profile::Rectangle { .. }) = original {
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
