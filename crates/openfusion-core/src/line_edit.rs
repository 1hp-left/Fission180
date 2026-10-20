//! Trim/extend finite sketch lines with explicit intersection dependencies.
use crate::{
    EPS, Result, Sketch,
    expression::Parameters,
    invalid,
    sketch::{Constraint, Entity, SketchGraph},
};
fn cross(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
fn delta(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [b[0] - a[0], b[1] - a[1]]
}
fn at(a: [f64; 2], d: [f64; 2], t: f64) -> [f64; 2] {
    [a[0] + d[0] * t, a[1] + d[1] * t]
}
fn line(graph: &SketchGraph, id: u64) -> Result<([f64; 2], [f64; 2])> {
    match graph.entities.get(&id) {
        Some(Entity::Line { a, b }) => Ok((graph.points[a], graph.points[b])),
        _ => invalid("Select a sketch line for Trim or Extend"),
    }
}
fn intersections(graph: &SketchGraph, source: u64) -> Result<Vec<(f64, u64)>> {
    let (a, b) = line(graph, source)?;
    let d = delta(a, b);
    let len = d[0].hypot(d[1]);
    if len < EPS {
        return invalid("Cannot edit a degenerate line");
    }
    let mut hits = vec![];
    for (id, e) in &graph.entities {
        if *id == source || graph.construction.contains(id) {
            continue;
        }
        if let Entity::Line { .. } = e {
            let (c, b) = line(graph, *id)?;
            let e = delta(c, b);
            let other = e[0].hypot(e[1]);
            let denominator = cross(d, e);
            if denominator.abs() < 1e-12 * len * other {
                continue;
            }
            let ac = delta(a, c);
            let t = cross(ac, e) / denominator;
            let u = cross(ac, d) / denominator;
            if u >= -EPS / other && u <= 1. + EPS / other {
                hits.push((t, *id));
            }
        }
    }
    hits.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    Ok(hits)
}
impl Sketch {
    fn replacement_lines(
        &self,
        source: u64,
        segments: &[([f64; 2], [f64; 2])],
        hits: &[(f64, u64)],
        parameters: &Parameters,
    ) -> Result<Self> {
        let g = self.graph_for_edit()?;
        let (start, end) = line(&g, source)?;
        let d = delta(start, end);
        let construction = g.construction.contains(&source);
        let open = self.without_entities(&[source], parameters)?;
        let mut graph = open.graph_for_edit()?;
        for (a, b) in segments {
            let mut point = |p: [f64; 2]| -> Result<u64> {
                if let Some((id, _)) = graph
                    .points
                    .iter()
                    .find(|(_, q)| (q[0] - p[0]).hypot(q[1] - p[1]) < EPS)
                {
                    return Ok(*id);
                }
                graph.point(p)
            };
            let pa = point(*a)?;
            let pb = point(*b)?;
            let entity = graph.entity(Entity::Line { a: pa, b: pb })?;
            if construction {
                graph.construction.insert(entity);
            }
            if (a[0] - b[0]).abs() < EPS {
                graph.constrain(Constraint::Vertical { line: entity })?;
            }
            if (a[1] - b[1]).abs() < EPS {
                graph.constrain(Constraint::Horizontal { line: entity })?;
            }
            for p in [pa, pb] {
                for (t, boundary) in hits {
                    let q = at(start, d, *t);
                    let p_at = graph.points[&p];
                    if (p_at[0] - q[0]).hypot(p_at[1] - q[1]) < EPS
                        && graph.entities.contains_key(boundary)
                    {
                        graph.constrain(Constraint::PointOnLine {
                            point: p,
                            line: *boundary,
                        })?;
                    }
                }
            }
        }
        // Source IDs are retired rather than redirected to an arbitrary remnant.
        // without_entities removes affected dimensions and their expression keys.
        open.with_graph(graph, parameters)
    }
    pub fn with_trimmed_line(
        &self,
        source: u64,
        pick: [f64; 2],
        parameters: &Parameters,
    ) -> Result<Self> {
        let graph = self.graph_for_edit()?;
        let (a, b) = line(&graph, source)?;
        let d = delta(a, b);
        let squared = d[0] * d[0] + d[1] * d[1];
        let len = squared.sqrt();
        let t = ((pick[0] - a[0]) * d[0] + (pick[1] - a[1]) * d[1]) / squared;
        if !t.is_finite() || !(0. ..=1.).contains(&t) {
            return invalid("Click inside the line segment to trim");
        }
        let hits = intersections(&graph, source)?;
        let mut cuts: Vec<_> = hits
            .iter()
            .map(|x| x.0)
            .filter(|t| *t > EPS / len && *t < 1. - EPS / len)
            .collect();
        cuts.extend([0., 1.]);
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() * len < EPS);
        if cuts.iter().any(|c| (t - c).abs() * len < EPS) {
            return invalid("Trim selection is at an intersection; click inside one segment");
        }
        let interval = cuts
            .windows(2)
            .find(|w| t > w[0] && t < w[1])
            .ok_or_else(|| crate::Error::Invalid("Missing trim interval".into()))?;
        let mut segments = vec![];
        if interval[0] * len > EPS {
            segments.push((a, at(a, d, interval[0])));
        }
        if (1. - interval[1]) * len > EPS {
            segments.push((at(a, d, interval[1]), b));
        }
        self.replacement_lines(source, &segments, &hits, parameters)
    }
    pub fn with_extended_line(
        &self,
        source: u64,
        pick: [f64; 2],
        parameters: &Parameters,
    ) -> Result<Self> {
        let graph = self.graph_for_edit()?;
        let (a, b) = line(&graph, source)?;
        let d = delta(a, b);
        let len = d[0].hypot(d[1]);
        let t = ((pick[0] - a[0]) * d[0] + (pick[1] - a[1]) * d[1]) / (len * len);
        if !t.is_finite() {
            return invalid("Invalid Extend selection");
        }
        let end = t >= 0.5;
        let hits = intersections(&graph, source)?;
        let target = hits
            .iter()
            .map(|x| x.0)
            .filter(|t| {
                if end {
                    *t > 1. + EPS / len
                } else {
                    *t < -EPS / len
                }
            })
            .min_by(|a, b| if end { a.total_cmp(b) } else { b.total_cmp(a) })
            .ok_or_else(|| {
                crate::Error::Invalid("No finite sketch line intersects this extension ray".into())
            })?;
        let segment = if end {
            (a, at(a, d, target))
        } else {
            (at(a, d, target), b)
        };
        self.replacement_lines(source, &[segment], &hits, parameters)
    }
}
