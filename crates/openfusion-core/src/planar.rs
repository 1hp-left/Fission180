//! Regularized line/circular-arc region Booleans, independently implemented.
//! All decisions use analytic curves. No polygon sampling or OCCT calls.
use crate::{
    Profile, Result,
    boundary::{BoundarySegment, PlanarCurve, cross, distance, sub},
    invalid,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Metric coincidence tolerance, mm. Features separated by less than
/// RESOLUTION_MM but more than this tolerance are rejected, never healed.
pub const COINCIDENCE_MM: f64 = 1e-7;
pub const RESOLUTION_MM: f64 = crate::EPS;
pub const ANGULAR_RAD: f64 = 1e-10;
pub const MAX_CURVES: usize = 512;
pub const MAX_EVENTS: usize = 32_768;
pub const MAX_SPANS: usize = 8192;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boolean {
    Union,
    Difference,
    Intersection,
}
impl Boolean {
    fn apply(self, a: bool, b: bool) -> bool {
        match self {
            Self::Union => a || b,
            Self::Difference => a && !b,
            Self::Intersection => a && b,
        }
    }
}
/// Identity of an original analytic source, independent of coordinates and output order.
/// Ordinal distinguishes a primitive's original edges when it has no entity IDs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Source {
    /// Semantic transform/pattern instances, preserving the original source below.
    pub occurrences: Vec<(u64, u64)>,
    pub feature: u64,
    pub region: u64,
    pub boundary: u64,
    pub entity: Option<u64>,
    pub ordinal: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Junction {
    Endpoint(Source, bool),
    Intersection(Source, Source, u8),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ancestry {
    pub source: Source,
    pub interval: [f64; 2],
}
#[derive(Debug, Clone)]
pub struct Span {
    pub curve: PlanarCurve,
    pub ancestry: Vec<Ancestry>,
    pub start: BTreeSet<Junction>,
    pub end: BTreeSet<Junction>,
}
impl Span {
    pub fn reversed(&self) -> Self {
        Self {
            curve: self.curve.reversed(),
            ancestry: self
                .ancestry
                .iter()
                .map(|a| Ancestry {
                    source: a.source.clone(),
                    interval: [a.interval[1], a.interval[0]],
                })
                .collect(),
            start: self.end.clone(),
            end: self.start.clone(),
        }
    }
    fn portion(&self, a: f64, b: f64, start: BTreeSet<Junction>, end: BTreeSet<Junction>) -> Self {
        Self {
            curve: self.curve.portion(a, b),
            ancestry: self
                .ancestry
                .iter()
                .map(|s| Ancestry {
                    source: s.source.clone(),
                    interval: [a, b].map(|t| s.interval[0] + t * (s.interval[1] - s.interval[0])),
                })
                .collect(),
            start,
            end,
        }
    }
    pub fn key(&self) -> SpanKey {
        SpanKey {
            sources: self.ancestry.iter().map(|a| a.source.clone()).collect(),
            start: self.start.iter().cloned().collect(),
            end: self.end.iter().cloned().collect(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct SpanKey {
    pub sources: Vec<Source>,
    pub start: Vec<Junction>,
    pub end: Vec<Junction>,
}
#[derive(Debug, Clone)]
pub struct Loop {
    pub spans: Vec<Span>,
}
impl Loop {
    pub fn profile(&self) -> Profile {
        Profile::Curved {
            segments: self
                .spans
                .iter()
                .map(|s| BoundarySegment {
                    source: if s.ancestry.len() == 1 {
                        s.ancestry[0].source.entity
                    } else {
                        None
                    },
                    curve: s.curve,
                })
                .collect(),
        }
    }
    pub fn signed_area(&self) -> f64 {
        let anchor = self.spans[0].curve.point(0.);
        self.spans.iter().map(|s| s.curve.area_term(anchor)).sum()
    }
    pub fn perimeter(&self) -> f64 {
        self.spans.iter().map(|s| s.curve.length()).sum()
    }
    pub fn key(&self) -> Vec<SpanKey> {
        let keys: Vec<_> = self.spans.iter().map(Span::key).collect();
        (0..keys.len())
            .map(|i| {
                (0..keys.len())
                    .map(|j| keys[(i + j) % keys.len()].clone())
                    .collect()
            })
            .min()
            .unwrap_or_default()
    }
    fn oriented(mut self, positive: bool) -> Self {
        if (self.signed_area() > 0.) != positive {
            self.spans = self.spans.iter().rev().map(Span::reversed).collect();
        }
        self
    }
}
#[derive(Debug, Clone)]
pub struct Region {
    pub outer: Loop,
    pub holes: Vec<Loop>,
}
impl Region {
    pub fn from_profiles(
        outer: &Profile,
        holes: &[Profile],
        feature: u64,
        region: u64,
        ids: &[u64],
    ) -> Result<Self> {
        if ids.len() != holes.len() + 1 || ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
            return invalid("Planar boundary identities must be complete and unique");
        }
        let make = |p: &Profile, index: usize| -> Result<Loop> {
            p.validate()?;
            let spans = p
                .boundary()
                .into_iter()
                .enumerate()
                .map(|(i, s)| {
                    let source = Source {
                        occurrences: vec![],
                        feature,
                        region,
                        boundary: ids[index],
                        entity: s.source,
                        ordinal: i as u64,
                    };
                    Span {
                        curve: s.curve,
                        ancestry: vec![Ancestry {
                            source: source.clone(),
                            interval: [0., 1.],
                        }],
                        start: BTreeSet::from([Junction::Endpoint(source.clone(), false)]),
                        end: BTreeSet::from([Junction::Endpoint(source, true)]),
                    }
                })
                .collect();
            Ok(Loop { spans }.oriented(index == 0))
        };
        Ok(Self {
            outer: make(outer, 0)?,
            holes: holes
                .iter()
                .enumerate()
                .map(|(i, p)| make(p, i + 1))
                .collect::<Result<_>>()?,
        })
    }
    pub fn area(&self) -> f64 {
        self.outer.signed_area() + self.holes.iter().map(Loop::signed_area).sum::<f64>()
    }
    pub fn perimeter(&self) -> f64 {
        self.outer.perimeter() + self.holes.iter().map(Loop::perimeter).sum::<f64>()
    }
    pub fn key(&self) -> Vec<Vec<SpanKey>> {
        let mut holes: Vec<_> = self.holes.iter().map(Loop::key).collect();
        holes.sort();
        std::iter::once(self.outer.key()).chain(holes).collect()
    }
    pub fn loops(&self) -> impl Iterator<Item = &Loop> {
        std::iter::once(&self.outer).chain(&self.holes)
    }
}
#[derive(Debug, Clone, Default)]
pub struct RegionSet {
    pub regions: Vec<Region>,
}
impl RegionSet {
    pub fn area(&self) -> f64 {
        self.regions.iter().map(Region::area).sum()
    }
    fn spans(&self) -> Vec<&Span> {
        self.regions
            .iter()
            .flat_map(Region::loops)
            .flat_map(|l| &l.spans)
            .collect()
    }
    pub fn validate(&self) -> Result<()> {
        if self.spans().len() > MAX_CURVES {
            return invalid(
                "Planar Boolean exceeds 512 source curves; simplify or partition the operation",
            );
        }
        for r in &self.regions {
            if r.outer.spans.is_empty() || r.holes.iter().any(|l| l.spans.is_empty()) {
                return invalid("Planar region has an empty boundary");
            }
            r.outer.profile().validate()?;
            if r.outer.signed_area() <= RESOLUTION_MM.powi(2) {
                return invalid(
                    "Planar outer boundary must be counterclockwise with positive area",
                );
            }
            for h in &r.holes {
                h.profile().validate()?;
                if h.signed_area() >= -RESOLUTION_MM.powi(2)
                    || !crate::region::strictly_contains(&r.outer.profile(), &h.profile())
                {
                    return invalid("Planar hole must be clockwise and strictly contained");
                }
            }
            for (i, h) in r.holes.iter().enumerate() {
                for g in r.holes.iter().skip(i + 1) {
                    strict_disjoint(&h.profile(), &g.profile())?;
                }
            }
        }
        for (i, r) in self.regions.iter().enumerate() {
            for s in self.regions.iter().skip(i + 1) {
                // Separate islands may sit inside holes, but may not touch or overlap material.
                for l in r.loops() {
                    for m in s.loops() {
                        for a in &l.spans {
                            for b in &m.spans {
                                if !pair_hits(a.curve, b.curve)?.is_empty() {
                                    return invalid("Planar regions touch or intersect");
                                }
                            }
                        }
                    }
                }
                if inside_region(r, s.outer.spans[0].curve.point(0.))
                    || inside_region(s, r.outer.spans[0].curve.point(0.))
                {
                    return invalid("Planar region interiors overlap");
                }
            }
        }
        let mut lineage = 0usize;
        for s in self.spans() {
            lineage += s.ancestry.len() + s.start.len() + s.end.len();
            if lineage > 32_768 {
                return invalid("Planar provenance exceeds 32768 records");
            }
            if s.ancestry.is_empty()
                || s.ancestry.len() > MAX_CURVES
                || s.start.is_empty()
                || s.end.is_empty()
                || s.start.len() > MAX_CURVES
                || s.end.len() > MAX_CURVES
            {
                return invalid("Planar span has missing/excessive provenance");
            }
            for a in &s.ancestry {
                if a.source.occurrences.len() > 64 {
                    return invalid("Planar transform ancestry exceeds 64 instances");
                }
                if a.interval
                    .iter()
                    .any(|t| !t.is_finite() || *t < 0. || *t > 1.)
                    || a.interval[0] == a.interval[1]
                {
                    return invalid("Planar span has invalid source interval");
                }
            }
        }
        Ok(())
    }
    pub fn boolean(&self, other: &Self, operation: Boolean) -> Result<Self> {
        boolean(self, other, operation)
    }
}
fn strict_disjoint(a: &Profile, b: &Profile) -> Result<()> {
    for x in a.boundary() {
        for y in b.boundary() {
            if !pair_hits(x.curve, y.curve)?.is_empty() {
                return invalid("Planar holes touch/intersect");
            }
        }
    }
    if a.contains_point(b.boundary()[0].curve.point(0.))
        || b.contains_point(a.boundary()[0].curve.point(0.))
    {
        return invalid("Planar holes nest or overlap");
    }
    Ok(())
}
fn inside_region(r: &Region, p: [f64; 2]) -> bool {
    r.outer.profile().contains_point(p) && !r.holes.iter().any(|h| h.profile().contains_point(p))
}
fn coincident(a: PlanarCurve, b: PlanarCurve) -> Result<bool> {
    let gap = match (a, b) {
        (PlanarCurve::Line { start, end }, PlanarCurve::Line { start: r, end: s }) => {
            if cross(sub(end, start), sub(s, r)).abs() > ANGULAR_RAD * a.length() * b.length() {
                return Ok(false);
            }
            cross(sub(end, start), sub(r, start)).abs() / a.length()
        }
        (PlanarCurve::Arc { geometry: a }, PlanarCurve::Arc { geometry: b }) => {
            distance(a.center, b.center) + (a.radius - b.radius).abs()
        }
        _ => return Ok(false),
    };
    if gap > COINCIDENCE_MM && gap < RESOLUTION_MM {
        return invalid(
            "Near-coincident supports are below planar resolution; separate the geometry by at least 0.00001 mm",
        );
    }
    Ok(gap <= COINCIDENCE_MM)
}
/// Finite intersections plus endpoints of ordinary coincident intervals.
pub(crate) fn pair_hits(a: PlanarCurve, b: PlanarCurve) -> Result<Vec<(f64, f64)>> {
    // Support coincidence matters only for bounded curves that can meet. Avoid
    // rejecting unrelated, distant spans merely because their infinite lines
    // (or supporting circles) are nearly coincident/tangent.
    let bounds = |c: PlanarCurve| {
        let mut b = [[f64::INFINITY; 2], [f64::NEG_INFINITY; 2]];
        for p in c.extrema() {
            for i in 0..2 {
                b[0][i] = b[0][i].min(p[i]);
                b[1][i] = b[1][i].max(p[i]);
            }
        }
        b
    };
    let aa = bounds(a);
    let bb = bounds(b);
    if (0..2).any(|i| aa[1][i] < bb[0][i] - RESOLUTION_MM || bb[1][i] < aa[0][i] - RESOLUTION_MM) {
        return Ok(vec![]);
    }
    if coincident(a, b)? {
        let mut hits = vec![];
        for p in [a.point(0.), a.point(1.), b.point(0.), b.point(1.)] {
            if let (Some(t), Some(u)) = (a.parameter(p), b.parameter(p)) {
                if !hits
                    .iter()
                    .any(|(s, _): &(f64, f64)| (t - s).abs() * a.length() <= COINCIDENCE_MM)
                {
                    hits.push((t, u));
                }
            }
        }
        hits.sort_by(|a, b| a.0.total_cmp(&b.0));
        return Ok(hits);
    }
    // Do not let the sketch intersection routine turn a small gap into a tangency.
    let tangent_gap = match (a, b) {
        (PlanarCurve::Arc { geometry: a }, PlanarCurve::Arc { geometry: b }) => {
            let d = distance(a.center, b.center);
            Some(
                (d - a.radius - b.radius)
                    .abs()
                    .min((d - (a.radius - b.radius).abs()).abs()),
            )
        }
        (PlanarCurve::Line { start, end }, PlanarCurve::Arc { geometry: c })
        | (PlanarCurve::Arc { geometry: c }, PlanarCurve::Line { start, end }) => Some(
            (cross(sub(end, start), sub(c.center, start)).abs() / distance(start, end) - c.radius)
                .abs(),
        ),
        _ => None,
    };
    if tangent_gap.is_some_and(|gap| gap > COINCIDENCE_MM && gap < RESOLUTION_MM) {
        return invalid("Near-tangent supports are below planar resolution");
    }
    crate::boundary::intersections(a, b)
}
fn crossing(a: &Span, b: &Span, branch: u8) -> Result<BTreeSet<Junction>> {
    let mut marks = BTreeSet::new();
    for x in &a.ancestry {
        for y in &b.ancestry {
            if x.source != y.source {
                let (u, v) = if x.source < y.source {
                    (&x.source, &y.source)
                } else {
                    (&y.source, &x.source)
                };
                marks.insert(Junction::Intersection(u.clone(), v.clone(), branch));
                if marks.len() > MAX_CURVES {
                    return invalid("Analytic junction ancestry exceeds 512 source pairs");
                }
            }
        }
    }
    Ok(marks)
}
fn classification(set: &RegionSet, p: [f64; 2]) -> Result<bool> {
    if set.spans().iter().any(|s| s.curve.parameter(p).is_some()) {
        return invalid("Unresolved boundary classification below planar resolution");
    }
    Ok(set.regions.iter().any(|r| inside_region(r, p)))
}
pub fn boolean(a: &RegionSet, b: &RegionSet, op: Boolean) -> Result<RegionSet> {
    a.validate()?;
    b.validate()?;
    let na = a.spans().len();
    let sources: Vec<_> = a.spans().into_iter().chain(b.spans()).collect();
    if sources.len() > MAX_CURVES {
        return invalid("Planar Boolean exceeds 512 combined curves");
    }
    let mut cuts: Vec<Vec<(f64, BTreeSet<Junction>)>> = sources
        .iter()
        .map(|s| vec![(0., s.start.clone()), (1., s.end.clone())])
        .collect();
    let mut events = 0;
    for i in 0..sources.len() {
        for j in i + 1..sources.len() {
            // Include same-operand seams so every retained vertex has the complete
            // source relationship. Inputs themselves were already validated.
            let hits = pair_hits(sources[i].curve, sources[j].curve)?;
            for (branch, (t, u)) in hits.into_iter().enumerate() {
                events += 1;
                if events > MAX_EVENTS {
                    return invalid("Planar Boolean exceeds 32768 intersection events");
                }
                let marks = crossing(sources[i], sources[j], branch as u8)?;
                cuts[i].push((t, marks.clone()));
                cuts[j].push((u, marks));
            }
        }
    }
    let mut pieces: Vec<(usize, Span)> = vec![];
    for (i, (source, mut nodes)) in sources.iter().zip(cuts).enumerate() {
        nodes.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Vec<(f64, BTreeSet<Junction>)> = vec![];
        for (t, key) in nodes {
            if let Some(last) = merged
                .last_mut()
                .filter(|(s, _)| (t - s).abs() * source.curve.length() <= COINCIDENCE_MM)
            {
                last.1.extend(key);
            } else {
                merged.push((t, key));
            }
        }
        for w in merged.windows(2) {
            if (w[1].0 - w[0].0) * source.curve.length() < RESOLUTION_MM {
                return invalid("Planar split creates a span shorter than 0.00001 mm");
            }
            pieces.push((
                usize::from(i >= na),
                source.portion(w[0].0, w[1].0, w[0].1.clone(), w[1].1.clone()),
            ));
            if pieces.len() > MAX_SPANS {
                return invalid("Planar Boolean exceeds 8192 bounded spans");
            }
        }
    }
    let mut used = vec![false; pieces.len()];
    let mut retained = vec![];
    for i in 0..pieces.len() {
        if used[i] {
            continue;
        }
        used[i] = true;
        let (operand, mut span) = pieces[i].clone();
        let mut sides = [None, None];
        sides[operand] = Some((true, false));
        for j in i + 1..pieces.len() {
            if used[j] {
                continue;
            }
            let s = &pieces[j].1;
            let forward = distance(span.curve.point(0.), s.curve.point(0.)) <= COINCIDENCE_MM
                && distance(span.curve.point(1.), s.curve.point(1.)) <= COINCIDENCE_MM;
            let backward = distance(span.curve.point(0.), s.curve.point(1.)) <= COINCIDENCE_MM
                && distance(span.curve.point(1.), s.curve.point(0.)) <= COINCIDENCE_MM;
            if !(forward || backward)
                || distance(span.curve.point(0.5), s.curve.point(0.5)) > COINCIDENCE_MM
            {
                continue;
            }
            if !coincident(span.curve, s.curve)? {
                continue;
            }
            if sides[pieces[j].0].is_some() {
                return invalid("Duplicate boundary within a planar operand");
            }
            used[j] = true;
            sides[pieces[j].0] = Some(if forward {
                (true, false)
            } else {
                (false, true)
            });
            let s = if forward { s.clone() } else { s.reversed() };
            span.ancestry.extend(s.ancestry);
            span.start.extend(s.start);
            span.end.extend(s.end);
        }
        span.ancestry.sort_by(|a, b| a.source.cmp(&b.source));
        span.ancestry
            .dedup_by(|a, b| a.source == b.source && a.interval == b.interval);
        for (k, set) in [a, b].into_iter().enumerate() {
            if sides[k].is_none() {
                let inside = classification(set, span.curve.point(0.5))?;
                sides[k] = Some((inside, inside));
            }
        }
        let x = sides[0].unwrap();
        let y = sides[1].unwrap();
        let left = op.apply(x.0, y.0);
        let right = op.apply(x.1, y.1);
        if left != right {
            retained.push(if left { span } else { span.reversed() });
        }
    }
    assemble(retained)
}
fn assemble(spans: Vec<Span>) -> Result<RegionSet> {
    let mut vertices: Vec<[f64; 2]> = vec![];
    let mut endpoints = vec![];
    for s in &spans {
        let mut nodes = [0; 2];
        for (i, p) in [s.curve.point(0.), s.curve.point(1.)]
            .into_iter()
            .enumerate()
        {
            let mut node = None;
            for (j, q) in vertices.iter().enumerate() {
                let d = distance(p, *q);
                if d <= COINCIDENCE_MM {
                    if node.is_some() {
                        return invalid("Ambiguous planar vertex cluster");
                    }
                    node = Some(j);
                } else if d < RESOLUTION_MM {
                    return invalid("Distinct planar junctions are below resolution");
                }
            }
            nodes[i] = node.unwrap_or_else(|| {
                vertices.push(p);
                vertices.len() - 1
            });
        }
        if nodes[0] == nodes[1] {
            return invalid("Planar span has coincident endpoints");
        }
        endpoints.push(nodes);
    }
    let mut outgoing = vec![vec![]; vertices.len()];
    let mut incoming = vec![0; vertices.len()];
    for (i, [u, v]) in endpoints.iter().enumerate() {
        outgoing[*u].push(i);
        incoming[*v] += 1;
    }
    if outgoing
        .iter()
        .zip(&incoming)
        .any(|(a, b)| a.len() != 1 || *b != 1)
    {
        return invalid("Planar result has a point contact or ambiguous nonmanifold junction");
    }
    let mut seen = vec![false; spans.len()];
    let mut loops = vec![];
    for first in 0..spans.len() {
        if seen[first] {
            continue;
        }
        let mut cycle = vec![];
        let mut i = first;
        loop {
            if seen[i] {
                if i != first {
                    return invalid("Planar loop traversal joined an existing loop");
                }
                break;
            }
            seen[i] = true;
            cycle.push(spans[i].clone());
            i = outgoing[endpoints[i][1]][0];
            if cycle.len() > MAX_SPANS {
                return invalid("Planar traversal limit exceeded");
            }
        }
        let l = Loop { spans: cycle };
        l.profile().validate()?;
        if l.signed_area().abs() <= RESOLUTION_MM.powi(2) {
            return invalid("Planar Boolean produced an unresolved zero-area loop");
        }
        loops.push(l);
    }
    let mut regions: Vec<_> = loops
        .iter()
        .filter(|l| l.signed_area() > 0.)
        .cloned()
        .map(|outer| Region {
            outer,
            holes: vec![],
        })
        .collect();
    for hole in loops.into_iter().filter(|l| l.signed_area() < 0.) {
        let owner = regions
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                r.outer
                    .profile()
                    .contains_point(hole.spans[0].curve.point(0.))
            })
            .min_by(|(_, a), (_, b)| a.outer.signed_area().total_cmp(&b.outer.signed_area()))
            .map(|(i, _)| i)
            .ok_or_else(|| {
                crate::Error::Invalid("Planar hole has no containing outer boundary".into())
            })?;
        regions[owner].holes.push(hole);
    }
    for r in &mut regions {
        r.holes.sort_by_key(Loop::key);
    }
    regions.sort_by_key(Region::key);
    let result = RegionSet { regions };
    result.validate()?;
    Ok(result)
}
/// Deterministic identity digest. Callers must detect collisions in their domain.
/// The input consists only of source/junction relationships, never coordinates.
pub fn identity(key: &impl Serialize) -> Result<u64> {
    let mut hash = 0xcbf29ce484222325u64;
    for b in serde_json::to_vec(key)? {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Ok(hash)
}

/// Refine several analytic sections to a common set of planar junctions. This
/// changes segmentation only, retaining each section's area and ancestry.
pub(crate) fn refine_sections(regions: &mut [Region]) -> Result<()> {
    let sources: Vec<Span> = regions
        .iter()
        .flat_map(Region::loops)
        .flat_map(|l| l.spans.clone())
        .collect();
    if sources.len() > 2048 {
        return invalid("Stepped section refinement exceeds 2048 curves");
    }
    let mut events = 0;
    let mut span_count = 0;
    for region in regions {
        for l in std::iter::once(&mut region.outer).chain(&mut region.holes) {
            let mut result = vec![];
            for s in &l.spans {
                let mut cuts = vec![(0., s.start.clone()), (1., s.end.clone())];
                for other in &sources {
                    for (branch, (t, _)) in pair_hits(s.curve, other.curve)?.into_iter().enumerate()
                    {
                        events += 1;
                        if events > 262_144 {
                            return invalid("Stepped section refinement exceeds 262144 events");
                        }
                        cuts.push((t, crossing(s, other, branch as u8)?));
                    }
                }
                cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
                let mut merged: Vec<(f64, BTreeSet<Junction>)> = vec![];
                for (t, key) in cuts {
                    if let Some(last) = merged
                        .last_mut()
                        .filter(|(u, _)| (t - u).abs() * s.curve.length() <= COINCIDENCE_MM)
                    {
                        last.1.extend(key);
                    } else {
                        merged.push((t, key));
                    }
                }
                for w in merged.windows(2) {
                    if (w[1].0 - w[0].0) * s.curve.length() < RESOLUTION_MM {
                        return invalid("Stepped section creates a sub-resolution span");
                    }
                    result.push(s.portion(w[0].0, w[1].0, w[0].1.clone(), w[1].1.clone()));
                    span_count += 1;
                    if span_count > MAX_SPANS {
                        return invalid("Stepped refinement exceeds 8192 spans");
                    }
                }
            }
            l.spans = result;
            l.profile().validate()?;
        }
    }
    Ok(())
}
