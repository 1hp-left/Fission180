//! Regularized planar-face CSG. Finite analytic trims decide all geometry;
//! display triangulation is downstream of the validated boundary representation.
use super::*;
use crate::{
    boundary::PlanarCurve,
    planar::{self, Boolean, Region, RegionSet},
};
use std::collections::{BTreeMap, BTreeSet};

const MAX_FACES: usize = 2048;
const MAX_PAIRS: usize = 262_144;
const MAX_WORK: usize = 4_194_304;
const MAX_TOPOLOGY: usize = 16_384;
const MAX_SHELLS: usize = 64;

#[derive(Debug, Clone)]
pub(super) struct General {
    pub roots: Vec<BTreeSet<FaceSource>>,
    regions: Vec<Region>,
    pub signature: Vec<u64>,
    transforms: usize,
    pub sources: std::sync::Arc<SourceLedger>,
}
#[derive(Debug, Clone, Default)]
pub(super) struct SourceLedger {
    pub entries: BTreeMap<u64, BoundarySource>,
    records: usize,
}
impl SourceLedger {
    fn insert(&mut self, id: u64, origin: BoundarySource) -> Result<()> {
        if let Some(old) = self.entries.get(&id) {
            if old != &origin {
                return invalid("Planar source identity collision");
            }
            return Ok(());
        }
        self.records += match &origin {
            BoundarySource::Edge(_) => 1,
            BoundarySource::PlaneIntersection(a, b) | BoundarySource::FiniteGuard(a, b, _) => {
                1 + a
                    .iter()
                    .chain(b)
                    .map(|r| 1 + r.occurrences.len())
                    .sum::<usize>()
            }
        };
        if self.records > 32768 {
            return invalid("Planar source ledger exceeds 32768 ancestry records");
        }
        self.entries.insert(id, origin);
        Ok(())
    }
}
type V = [f64; 3];
fn sub(a: V, b: V) -> V {
    std::array::from_fn(|i| a[i] - b[i])
}
fn add(a: V, b: V) -> V {
    std::array::from_fn(|i| a[i] + b[i])
}
fn mul(a: V, t: f64) -> V {
    a.map(|x| x * t)
}
fn unit(a: V) -> Result<V> {
    let l = dot(a, a).sqrt();
    if !l.is_finite() || l <= POLICY.angular_rad {
        return invalid("Unresolved planar direction");
    }
    Ok(mul(a, 1. / l))
}
fn budget(work: &mut usize, n: usize) -> Result<()> {
    *work = work
        .checked_add(n)
        .ok_or_else(|| crate::Error::Invalid("Planar work overflow".into()))?;
    if *work > MAX_WORK {
        return invalid("Planar face classification/refinement exceeds 4194304 work units");
    }
    Ok(())
}
#[derive(Debug, Clone, Copy)]
struct Frame {
    o: V,
    n: V,
    u: V,
    v: V,
}
impl Frame {
    fn new(o: V, n: V) -> Result<Self> {
        for x in o {
            crate::number(x)?;
        }
        if (dot(n, n) - 1.).abs() > POLICY.angular_rad {
            return invalid("Invalid planar face normal");
        }
        let k = (0..3)
            .min_by(|a, b| n[*a].abs().total_cmp(&n[*b].abs()))
            .unwrap();
        let mut axis = [0.; 3];
        axis[k] = 1.;
        let u = unit(cross3(axis, n))?;
        let v = cross3(n, u);
        Ok(Self { o, n, u, v })
    }
    fn project(self, p: V) -> [f64; 2] {
        let p = sub(p, self.o);
        [dot(p, self.u), dot(p, self.v)]
    }
    fn world(self, p: [f64; 2]) -> V {
        add(self.o, add(mul(self.u, p[0]), mul(self.v, p[1])))
    }
    fn distance(self, p: V) -> f64 {
        dot(sub(p, self.o), self.n)
    }
}
#[derive(Debug, Clone, Copy)]
enum Relation {
    Parallel,
    Coplanar,
    Line { p: V, d: V },
}
fn relation(a: Frame, b: Frame) -> Result<Relation> {
    let c = cross3(a.n, b.n);
    let c2 = dot(c, c);
    if c2 <= POLICY.angular_rad.powi(2) {
        let gap = a.distance(b.o).abs();
        if gap <= POLICY.linear_mm {
            return Ok(Relation::Coplanar);
        }
        if gap < EPS {
            return invalid("Parallel face separation is below resolution");
        }
        return Ok(Relation::Parallel);
    }
    // Solve in a translated frame, avoiding cancellation of world plane constants.
    let p = add(a.o, mul(cross3(c, a.n), dot(b.n, sub(b.o, a.o)) / c2));
    for x in p {
        if !x.is_finite() {
            return invalid("Nonfinite plane intersection");
        }
    }
    Ok(Relation::Line { p, d: unit(c)? })
}
#[derive(Debug, Clone)]
struct Patch {
    frame: Frame,
    region: Region,
    roots: BTreeSet<FaceSource>,
    source: Option<u64>,
}
fn source(id: u64) -> planar::Source {
    planar::Source {
        occurrences: vec![],
        feature: 0,
        region: 0,
        boundary: id,
        entity: None,
        ordinal: 0,
    }
}
fn span(curve: PlanarCurve, id: u64) -> planar::Span {
    let s = source(id);
    planar::Span {
        curve,
        ancestry: vec![planar::Ancestry {
            source: s.clone(),
            interval: [0., 1.],
        }],
        start: BTreeSet::from([planar::Junction::Endpoint(s.clone(), false)]),
        end: BTreeSet::from([planar::Junction::Endpoint(s, true)]),
    }
}
pub(super) fn supported(b: &Brep) -> bool {
    b.faces
        .iter()
        .all(|f| matches!(f.surface, Surface::Plane { .. }))
        && b.edges.iter().all(|e| matches!(e.curve, Curve::Line))
}
fn patches(b: &Brep, metadata: Option<&General>) -> Result<Vec<Patch>> {
    if !supported(b) {
        return invalid(
            "General 3D Boolean requires planar faces and straight edges; curved intersections are unsupported, no OCCT fallback occurred",
        );
    }
    if b.faces.len() > MAX_FACES || b.edges.len() > MAX_TOPOLOGY || b.vertices.len() > MAX_TOPOLOGY
    {
        return invalid("Planar B-rep exceeds 2048 faces or 16384 edges/vertices");
    }
    if let Some(g) = metadata {
        validate_metadata(g)?;
    }
    b.faces
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let Surface::Plane { origin, normal } = f.surface else {
                unreachable!()
            };
            let frame = Frame::new(origin, mul(normal, if f.reversed { -1. } else { 1. }))?;
            let loops = f
                .loops
                .iter()
                .map(|l| {
                    let spans = l
                        .iter()
                        .map(|co| {
                            let e = &b.edges[co.edge];
                            let mut v = e.vertices;
                            if co.reversed {
                                v.swap(0, 1);
                            }
                            let mut s = span(
                                PlanarCurve::Line {
                                    start: frame.project(b.vertices[v[0]].point),
                                    end: frame.project(b.vertices[v[1]].point),
                                },
                                planar::identity(&("original-edge", e.id))?,
                            );
                            let original = planar::Source {
                                feature: e.id.feature,
                                region: e.id.region,
                                entity: e.source_entity,
                                ..s.ancestry[0].source.clone()
                            };
                            s.ancestry[0].source = original.clone();
                            s.ancestry[0].interval = if co.reversed { [1., 0.] } else { [0., 1.] };
                            s.start = BTreeSet::from([planar::Junction::Endpoint(
                                original.clone(),
                                co.reversed,
                            )]);
                            s.end = BTreeSet::from([planar::Junction::Endpoint(
                                original,
                                !co.reversed,
                            )]);
                            Ok(s)
                        })
                        .collect::<Result<Vec<_>>>()?;
                    Ok(planar::Loop { spans })
                })
                .collect::<Result<Vec<_>>>()?;
            let mut region = Region {
                outer: loops[0].clone(),
                holes: loops[1..].to_vec(),
            };
            RegionSet {
                regions: vec![region.clone()],
            }
            .validate()?;
            let roots = if let Some(g) = metadata {
                // Preserve directed original trim ancestry through repeated CSG.
                region = g.regions[i].clone();
                g.roots[i].clone()
            } else {
                BTreeSet::from([FaceSource {
                    original: f.id,
                    occurrences: vec![],
                }])
            };
            Ok(Patch {
                frame,
                region,
                roots,
                source: f.source_entity,
            })
        })
        .collect()
}
fn set(r: Region) -> RegionSet {
    RegionSet { regions: vec![r] }
}
fn inside(r: &Region, p: [f64; 2]) -> bool {
    r.outer.profile().contains_point(p) && !r.holes.iter().any(|h| h.profile().contains_point(p))
}
fn boundary(r: &Region, p: [f64; 2]) -> bool {
    r.loops()
        .flat_map(|l| &l.spans)
        .any(|s| s.curve.parameter(p).is_some())
}
fn point_segment(p: [f64; 2], s: &planar::Span) -> f64 {
    let a = s.curve.point(0.);
    let b = s.curve.point(1.);
    let d = [b[0] - a[0], b[1] - a[1]];
    let t =
        (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / (d[0] * d[0] + d[1] * d[1])).clamp(0., 1.);
    crate::boundary::distance(p, [a[0] + t * d[0], a[1] + t * d[1]])
}
/// Analytic scanline witness with proven finite-boundary clearance. No mesh query.
fn witness(r: &Region, work: &mut usize) -> Result<[f64; 2]> {
    let spans: Vec<_> = r.loops().flat_map(|l| &l.spans).collect();
    let mut ys: Vec<_> = spans.iter().map(|s| s.curve.point(0.)[1]).collect();
    ys.sort_by(f64::total_cmp);
    ys.dedup_by(|a, b| (*a - *b).abs() <= POLICY.linear_mm);
    let mut best = None;
    let mut clearance = 0.;
    for y in ys.windows(2).map(|w| (w[0] + w[1]) * 0.5) {
        let mut xs = vec![];
        for s in &spans {
            budget(work, 1)?;
            let a = s.curve.point(0.);
            let b = s.curve.point(1.);
            if (a[1] > y) != (b[1] > y) {
                xs.push(a[0] + (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]));
            }
        }
        xs.sort_by(f64::total_cmp);
        for x in xs.windows(2).map(|w| (w[0] + w[1]) * 0.5) {
            budget(work, spans.len())?;
            let p = [x, y];
            if inside(r, p) {
                let d = spans
                    .iter()
                    .map(|s| point_segment(p, s))
                    .fold(f64::INFINITY, f64::min);
                if d > clearance {
                    best = Some(p);
                    clearance = d;
                }
            }
        }
    }
    if clearance < EPS {
        return invalid("Face has no resolved interior witness");
    }
    best.ok_or_else(|| crate::Error::Invalid("No interior face witness".into()))
}
fn projected(p: &Patch, f: Frame) -> Region {
    let make = |l: &planar::Loop| {
        let mut l = l.clone();
        for s in &mut l.spans {
            s.curve = PlanarCurve::Line {
                start: f.project(p.frame.world(s.curve.point(0.))),
                end: f.project(p.frame.world(s.curve.point(1.))),
            };
        }
        if dot(p.frame.n, f.n) < 0. {
            l.spans = l.spans.iter().rev().map(planar::Span::reversed).collect();
        }
        l
    };
    Region {
        outer: make(&p.region.outer),
        holes: p.region.holes.iter().map(make).collect(),
    }
}
/// Finite intervals of the plane intersection line inside a trimmed face.
fn intervals(f: &Patch, p: V, d: V, work: &mut usize) -> Result<Vec<[f64; 2]>> {
    let mut nodes = vec![];
    let across = cross3(f.frame.n, d);
    for s in f.region.loops().flat_map(|l| &l.spans) {
        budget(work, 1)?;
        let a = f.frame.world(s.curve.point(0.));
        let b = f.frame.world(s.curve.point(1.));
        let x = dot(sub(a, p), across);
        let y = dot(sub(b, p), across);
        if x.abs() <= POLICY.linear_mm {
            nodes.push(dot(sub(a, p), d));
        }
        if y.abs() <= POLICY.linear_mm {
            nodes.push(dot(sub(b, p), d));
        }
        if (x > POLICY.linear_mm && y < -POLICY.linear_mm)
            || (y > POLICY.linear_mm && x < -POLICY.linear_mm)
        {
            nodes.push(dot(sub(add(a, mul(sub(b, a), x / (x - y))), p), d));
        }
    }
    nodes.sort_by(f64::total_cmp);
    nodes.dedup_by(|a, b| (*a - *b).abs() <= POLICY.linear_mm);
    let mut out = vec![];
    for w in nodes.windows(2) {
        if w[1] - w[0] < EPS {
            return invalid("Finite intersection events are below resolution");
        }
        let q = f.frame.project(add(p, mul(d, (w[0] + w[1]) * 0.5)));
        if inside(&f.region, q) || boundary(&f.region, q) {
            out.push([w[0], w[1]]);
        }
    }
    Ok(out)
}
fn finite_overlap(a: &Patch, b: &Patch, p: V, d: V, work: &mut usize) -> Result<bool> {
    let aa = intervals(a, p, d, work)?;
    let bb = intervals(b, p, d, work)?;
    Ok(aa.iter().any(|a| {
        bb.iter()
            .any(|b| a[1].min(b[1]) - a[0].max(b[0]) > POLICY.linear_mm)
    }))
}
fn half(patch: &Patch, other: &Patch, sources: &mut SourceLedger) -> Result<Option<Region>> {
    let f = patch.frame;
    let g = [dot(other.frame.n, f.u), dot(other.frame.n, f.v)];
    let len = (g[0] * g[0] + g[1] * g[1]).sqrt();
    let n = g.map(|x| x / len);
    let t = [-n[1], n[0]];
    let h = dot(other.frame.n, sub(other.frame.o, f.o)) / len;
    let points: Vec<_> = patch
        .region
        .loops()
        .flat_map(|l| &l.spans)
        .map(|s| s.curve.point(0.))
        .collect();
    let at = |p: [f64; 2], d: [f64; 2]| p[0] * d[0] + p[1] * d[1];
    let low = points
        .iter()
        .map(|p| at(*p, n))
        .fold(f64::INFINITY, f64::min);
    let high = points
        .iter()
        .map(|p| at(*p, n))
        .fold(f64::NEG_INFINITY, f64::max);
    if low >= h - POLICY.linear_mm || high <= h + POLICY.linear_mm {
        return Ok(None);
    }
    if h - low < EPS || high - h < EPS {
        return invalid("Face split is below resolution");
    }
    let lo = points
        .iter()
        .map(|p| at(*p, t))
        .fold(f64::INFINITY, f64::min)
        - 1.;
    let hi = points
        .iter()
        .map(|p| at(*p, t))
        .fold(f64::NEG_INFINITY, f64::max)
        + 1.;
    let at = |x: f64, y: f64| [n[0] * x + t[0] * y, n[1] * x + t[1] * y];
    let pts = [at(low - 1., lo), at(h, lo), at(h, hi), at(low - 1., hi)];
    let mut spans = vec![];
    for i in 0..4 {
        let id = if i == 1 {
            planar::identity(&("plane-intersection", &patch.roots, &other.roots))?
        } else {
            planar::identity(&("finite-half-space-guard", &patch.roots, &other.roots, i))?
        };
        sources.insert(
            id,
            if i == 1 {
                BoundarySource::PlaneIntersection(patch.roots.clone(), other.roots.clone())
            } else {
                BoundarySource::FiniteGuard(patch.roots.clone(), other.roots.clone(), i)
            },
        )?;
        spans.push(span(
            PlanarCurve::Line {
                start: pts[i],
                end: pts[(i + 1) % 4],
            },
            id,
        ));
    }
    Ok(Some(Region {
        outer: planar::Loop { spans },
        holes: vec![],
    }))
}
fn split(
    input: &[Patch],
    other: &[Patch],
    work: &mut usize,
    sources: &mut SourceLedger,
) -> Result<Vec<Patch>> {
    if input
        .len()
        .checked_mul(other.len())
        .is_none_or(|n| n > MAX_PAIRS)
    {
        return invalid("Planar face pair budget exceeds 262144");
    }
    let mut output = vec![];
    for face in input {
        let mut pieces = vec![face.clone()];
        for cutter in other {
            budget(work, 1)?;
            let rel = relation(face.frame, cutter.frame)?;
            let coplanar = matches!(rel, Relation::Coplanar);
            match rel {
                Relation::Parallel => continue,
                Relation::Line { p, d } => {
                    if !finite_overlap(face, cutter, p, d, work)? {
                        continue;
                    }
                }
                Relation::Coplanar => {}
            }
            let mut next = vec![];
            for part in pieces {
                let mask = if coplanar {
                    Some(projected(cutter, part.frame))
                } else {
                    half(&part, cutter, sources)?
                };
                let Some(mask) = mask else {
                    next.push(part);
                    continue;
                };
                let left = set(part.region.clone());
                let right = set(mask);
                budget(
                    work,
                    left.regions[0]
                        .loops()
                        .map(|l| l.spans.len())
                        .sum::<usize>()
                        * right.regions[0]
                            .loops()
                            .map(|l| l.spans.len())
                            .sum::<usize>(),
                )?;
                for op in [Boolean::Intersection, Boolean::Difference] {
                    for region in left.boolean(&right, op)?.regions {
                        let mut p = part.clone();
                        p.region = region;
                        if coplanar && op == Boolean::Intersection {
                            p.roots.extend(cutter.roots.iter().cloned());
                        }
                        next.push(p);
                    }
                }
                if next.len() + output.len() > MAX_FACES {
                    return invalid("Planar splitting exceeds 2048 face patches");
                }
            }
            pieces = next;
        }
        output.extend(pieces);
        if output.len() > MAX_FACES {
            return invalid("Planar splitting exceeds 2048 face patches");
        }
    }
    Ok(output)
}
#[derive(Debug, Clone, Copy, PartialEq)]
enum Location {
    Outside,
    Inside,
    Boundary(V),
}
fn locate(faces: &[Patch], p: V, work: &mut usize) -> Result<Location> {
    let mut on = None;
    for f in faces {
        budget(work, f.region.loops().map(|l| l.spans.len()).sum::<usize>())?;
        let d = f.frame.distance(p).abs();
        if d < EPS {
            let q = f.frame.project(p);
            if inside(&f.region, q) || boundary(&f.region, q) {
                if d > POLICY.linear_mm {
                    return invalid("Solid classification is below plane resolution");
                }
                if let Some(n) = on {
                    if dot(n, f.frame.n) < 1. - POLICY.angular_rad {
                        return invalid("Classification witness lies on a solid edge");
                    }
                }
                on = Some(f.frame.n);
            }
        }
    }
    if let Some(n) = on {
        return Ok(Location::Boundary(n));
    }
    // Retry analytically degenerate rays only; no occupancy sampling or tessellation.
    for ray in [
        [1., 0.371, 0.529],
        [0.173, 1., 0.683],
        [0.419, 0.233, 1.],
        [1., -0.719, 0.137],
        [-0.257, 1., 0.431],
        [0.613, -0.347, 1.],
    ] {
        let ray = unit(ray)?;
        let mut winding = 0i32;
        let mut degenerate = false;
        for f in faces {
            budget(work, f.region.loops().map(|l| l.spans.len()).sum::<usize>())?;
            let denom = dot(f.frame.n, ray);
            if denom.abs() <= POLICY.angular_rad {
                continue;
            }
            let t = -f.frame.distance(p) / denom;
            if t <= POLICY.linear_mm {
                continue;
            }
            let q = f.frame.project(add(p, mul(ray, t)));
            if boundary(&f.region, q) {
                degenerate = true;
                break;
            }
            if inside(&f.region, q) {
                winding += if denom > 0. { 1 } else { -1 };
            }
        }
        if !degenerate {
            return match winding {
                0 => Ok(Location::Outside),
                1 => Ok(Location::Inside),
                _ => invalid("Invalid analytic solid winding"),
            };
        }
    }
    invalid("All bounded analytic classification rays meet trim junctions")
}
fn apply(op: Operation, a: bool, b: bool) -> bool {
    match op {
        Operation::Join => a || b,
        Operation::Cut => a && !b,
        Operation::Intersect => a && b,
        _ => false,
    }
}
fn flip(p: &mut Patch) -> Result<()> {
    let old = p.clone();
    p.frame = Frame::new(p.frame.o, mul(p.frame.n, -1.))?;
    p.region = projected(&old, p.frame);
    Ok(())
}
pub(super) fn boolean(
    a: &NativeSolid,
    b: &NativeSolid,
    op: Operation,
    feature: u64,
) -> Result<Vec<NativeSolid>> {
    let aa = patches(&a.topology, a.general.as_ref())?;
    let bb = patches(&b.topology, b.general.as_ref())?;
    let mut sources = SourceLedger::default();
    for s in [a, b] {
        if let Some(g) = &s.general {
            for (id, origin) in &g.sources.entries {
                sources.insert(*id, origin.clone())?;
            }
        } else {
            for e in &s.topology.edges {
                sources.insert(
                    planar::identity(&("original-edge", e.id))?,
                    BoundarySource::Edge(e.id),
                )?;
            }
        }
    }
    let mut work = 0;
    let mut selected = vec![];
    for (index, (input, other)) in [(&aa, &bb), (&bb, &aa)].into_iter().enumerate() {
        for mut p in split(input, other, &mut work, &mut sources)? {
            let q = p.frame.world(witness(&p.region, &mut work)?);
            let loc = locate(other, q, &mut work)?;
            // Material lies on the negative side of every source oriented face.
            let sides = match loc {
                Location::Outside => (false, false),
                Location::Inside => (true, true),
                Location::Boundary(n) => {
                    if dot(n, p.frame.n) > 0. {
                        (true, false)
                    } else {
                        (false, true)
                    }
                }
            };
            // Both source passes would otherwise publish a coincident interface twice.
            if index == 1 && matches!(loc, Location::Boundary(_)) {
                continue;
            }
            let (minus, plus) = if index == 0 {
                (apply(op, true, sides.0), apply(op, false, sides.1))
            } else {
                (apply(op, sides.0, true), apply(op, sides.1, false))
            };
            if minus != plus {
                if plus {
                    flip(&mut p)?;
                }
                selected.push(p);
            }
        }
    }
    let mut result = assemble(
        a,
        selected,
        feature,
        &mut work,
        std::sync::Arc::new(sources),
    )?;
    let aa = a.bounds();
    let bb = b.bounds();
    let disjoint_join = op == Operation::Join
        && (0..3).any(|i| {
            aa[1][i] < bb[0][i] - POLICY.linear_mm || bb[1][i] < aa[0][i] - POLICY.linear_mm
        });
    let old_boxes = a.general.is_none()
        && b.general.is_none()
        && a.stepped.is_none()
        && b.stepped.is_none()
        && a.input.holes.is_empty()
        && b.input.holes.is_empty()
        && prisms::rectangular(&a.input.outer)
        && prisms::rectangular(&b.input.outer);
    // Preserve the v5 bounds-order contract of the former different-axis box
    // split/disjoint paths, while retaining semantic topology and edit guards.
    let legacy = disjoint_join
        || (old_boxes
            && result.iter().all(|s| {
                let bounds = s.bounds();
                s.topology.faces.iter().all(|f| {
                    if let Surface::Plane { origin, normal } = f.surface {
                        (0..3).any(|i| {
                            normal[i].abs() > 1. - POLICY.angular_rad
                                && [bounds[0][i], bounds[1][i]]
                                    .iter()
                                    .any(|x| (origin[i] - x).abs() <= POLICY.linear_mm)
                        })
                    } else {
                        false
                    }
                })
            }));
    for s in &mut result {
        s.legacy_output_order = legacy;
        s.general.as_mut().unwrap().transforms = a
            .general
            .as_ref()
            .map_or(0, |g| g.transforms)
            .max(b.general.as_ref().map_or(0, |g| g.transforms));
    }
    Ok(result)
}

fn patch_key(p: &Patch) -> Result<u64> {
    planar::identity(&("planar-face", &p.roots, p.region.key()))
}
fn vertex(b: &mut Brep, p: V) -> Result<usize> {
    for x in p {
        crate::number(x)?;
    }
    let mut found = None;
    for (i, v) in b.vertices.iter().enumerate() {
        let d = dist(p, v.point);
        if d <= POLICY.linear_mm {
            if found.is_some() {
                return invalid("Ambiguous planar vertex cluster");
            }
            found = Some(i);
        } else if d < EPS {
            return invalid("Distinct planar vertices are below resolution");
        }
    }
    if let Some(i) = found {
        return Ok(i);
    }
    if b.vertices.len() >= MAX_TOPOLOGY {
        return invalid("Planar B-rep exceeds 16384 vertices");
    }
    let i = b.vertices.len();
    b.vertices.push(Vertex { point: p });
    Ok(i)
}
fn assemble(
    base: &NativeSolid,
    patches: Vec<Patch>,
    feature: u64,
    work: &mut usize,
    sources: std::sync::Arc<SourceLedger>,
) -> Result<Vec<NativeSolid>> {
    if patches.is_empty() {
        return Ok(vec![]);
    }
    if patches.len() > MAX_FACES {
        return invalid("Planar output exceeds 2048 faces");
    }
    let mut b = Brep {
        vertices: vec![],
        edges: vec![],
        faces: vec![],
    };
    let mut roots = vec![];
    let mut loops = vec![];
    let mut vertex_marks: Vec<BTreeSet<u64>> = vec![];
    let mut ids = BTreeSet::new();
    for p in &patches {
        if p.roots.len() > 512 {
            return invalid("Planar face ancestry exceeds 512 sources");
        }
        for s in p.region.loops().flat_map(|l| &l.spans) {
            if s.ancestry.len() > 512 || s.start.len() > 512 || s.end.len() > 512 {
                return invalid("Planar trim ancestry exceeds 512 sources per junction");
            }
        }
        let key = patch_key(p)?;
        if !ids.insert(key) {
            return invalid("Planar face identity collision");
        }
        let mut face_loops = vec![];
        for l in p.region.loops() {
            let mut vs = vec![];
            for s in &l.spans {
                budget(work, b.vertices.len() + 1)?;
                let i = vertex(&mut b, p.frame.world(s.curve.point(0.)))?;
                while vertex_marks.len() < b.vertices.len() {
                    vertex_marks.push(BTreeSet::new());
                }
                vertex_marks[i].insert(planar::identity(&("trim-junction", s.key().start))?);
                vs.push(i);
            }
            face_loops.push(vs);
        }
        loops.push(face_loops);
        roots.push(p.roots.clone());
        b.faces.push(Face {
            id: TopologyId {
                body: base.body_id,
                feature,
                region: 0,
                local: key,
            },
            surface: Surface::Plane {
                origin: p.frame.o,
                normal: p.frame.n,
            },
            loops: vec![],
            reversed: false,
            source_entity: p.source,
        });
    }
    let mut edge_map = BTreeMap::new();
    let mut edge_faces: Vec<BTreeSet<u64>> = vec![];
    for (fi, ls) in loops.iter().enumerate() {
        for l in ls {
            let mut coedges = vec![];
            for j in 0..l.len() {
                let a = l[j];
                let c = l[(j + 1) % l.len()];
                let p = b.vertices[a].point;
                let d = sub(b.vertices[c].point, p);
                let dd = dot(d, d);
                let len = dd.sqrt();
                if len < EPS {
                    return invalid("Planar edge is below resolution");
                }
                let mut cuts = vec![(0., a), (1., c)];
                for (v, q) in b.vertices.iter().enumerate() {
                    budget(work, 1)?;
                    if v == a || v == c {
                        continue;
                    }
                    let t = dot(sub(q.point, p), d) / dd;
                    if t > 0. && t < 1. {
                        let gap = dist(q.point, add(p, mul(d, t)));
                        if gap <= POLICY.linear_mm {
                            cuts.push((t, v));
                        } else if gap < EPS {
                            return invalid("Planar edge refinement is below resolution");
                        }
                    }
                }
                cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
                for w in cuts.windows(2) {
                    if (w[1].0 - w[0].0) * len < EPS {
                        return invalid("Planar refined edge is below resolution");
                    }
                    let v = [w[0].1, w[1].1];
                    let k = if v[0] < v[1] { v } else { [v[1], v[0]] };
                    let ei = if let Some(i) = edge_map.get(&k) {
                        *i
                    } else {
                        if b.edges.len() >= MAX_TOPOLOGY {
                            return invalid("Planar B-rep exceeds 16384 edges");
                        }
                        let i = b.edges.len();
                        edge_map.insert(k, i);
                        b.edges.push(Edge {
                            id: TopologyId {
                                body: base.body_id,
                                feature,
                                region: 0,
                                local: 0,
                            },
                            vertices: v,
                            curve: Curve::Line,
                            source_entity: None,
                        });
                        edge_faces.push(BTreeSet::new());
                        i
                    };
                    edge_faces[ei].insert(b.faces[fi].id.local);
                    coedges.push(Coedge {
                        edge: ei,
                        reversed: b.edges[ei].vertices != v,
                    });
                }
            }
            b.faces[fi].loops.push(coedges);
        }
    }
    let mut edge_ids = BTreeSet::new();
    let mut lineage = 0;
    for (edge, adj) in b.edges.iter_mut().zip(edge_faces) {
        let mut ends = edge.vertices.map(|i| vertex_marks[i].clone());
        ends.sort();
        lineage += adj.len() + ends.iter().map(BTreeSet::len).sum::<usize>();
        edge.id.local = planar::identity(&("planar-edge", adj, ends))?;
        if !edge_ids.insert(edge.id) {
            return invalid("Planar edge identity collision");
        }
    }
    if lineage > 32768 {
        return invalid("Planar topology ancestry exceeds 32768 records");
    }
    for p in &patches {
        lineage += p
            .roots
            .iter()
            .map(|r| 1 + r.occurrences.len())
            .sum::<usize>();
        for s in p.region.loops().flat_map(|l| &l.spans) {
            lineage += s
                .ancestry
                .iter()
                .map(|a| 1 + a.source.occurrences.len())
                .sum::<usize>()
                + s.start.len()
                + s.end.len();
        }
        if lineage > 32768 {
            return invalid("Planar face/trim provenance exceeds 32768 records");
        }
    }
    // Global validation rejects edge/point pinches even between separate shells.
    // It deliberately precedes splitting into material bodies.
    validate_local(&b)?;
    let groups = shells(&b);
    let parents = shell_relationships(&b, &groups, work)?;
    let mut results = vec![];
    let mut signatures = BTreeSet::new();
    for (i, parent) in parents.iter().enumerate() {
        let vol = signed_volume(&b, &groups[i]);
        if vol <= 0. {
            continue;
        }
        if parent.is_some() {
            return invalid("A material island must be published as a separate solid");
        }
        let mut face_set = groups[i].clone();
        for (j, parent) in parents.iter().enumerate() {
            if *parent == Some(i) {
                face_set.extend(&groups[j]);
            }
        }
        let (topology, selected) = subset(&b, &face_set);
        topology.validate()?;
        let mut signature: Vec<_> = topology.faces.iter().map(|f| f.id.local).collect();
        signature.sort();
        if !signatures.insert(signature.clone()) {
            return invalid("Planar body identity collision");
        }
        let mut s = base.clone();
        s.topology = topology;
        s.stepped = None;
        s.planar_named = false;
        s.legacy_output_order = false;
        s.general = Some(General {
            roots: selected.iter().map(|i| roots[*i].clone()).collect(),
            regions: selected
                .iter()
                .map(|i| patches[*i].region.clone())
                .collect(),
            signature,
            transforms: base.general.as_ref().map_or(0, |g| g.transforms),
            sources: sources.clone(),
        });
        validate_metadata(s.general.as_ref().unwrap())?;
        if s.volume() <= EPS.powi(3) || !s.volume().is_finite() {
            return invalid("Planar solid has unresolved signed volume");
        }
        results.push(s);
    }
    results.sort_by_key(|s| s.output_signature());
    Ok(results)
}
fn subset(b: &Brep, faces: &[usize]) -> (Brep, Vec<usize>) {
    let mut out = Brep {
        vertices: vec![],
        edges: vec![],
        faces: vec![],
    };
    let mut vm = BTreeMap::new();
    let mut em = BTreeMap::new();
    for i in faces {
        let mut f = b.faces[*i].clone();
        for co in f.loops.iter_mut().flatten() {
            co.edge = *em.entry(co.edge).or_insert_with(|| {
                let mut e = b.edges[co.edge].clone();
                for v in &mut e.vertices {
                    *v = *vm.entry(*v).or_insert_with(|| {
                        let i = out.vertices.len();
                        out.vertices.push(b.vertices[*v].clone());
                        i
                    });
                }
                let i = out.edges.len();
                out.edges.push(e);
                i
            });
        }
        out.faces.push(f);
    }
    (out, faces.to_vec())
}
pub(super) fn shells(b: &Brep) -> Vec<Vec<usize>> {
    let mut ef = vec![vec![]; b.edges.len()];
    for (i, f) in b.faces.iter().enumerate() {
        for co in f.loops.iter().flatten() {
            if let Some(e) = ef.get_mut(co.edge) {
                e.push(i);
            }
        }
    }
    let mut seen = BTreeSet::new();
    let mut groups = vec![];
    for i in 0..b.faces.len() {
        if seen.contains(&i) {
            continue;
        }
        let mut group = vec![];
        let mut pending = vec![i];
        while let Some(f) = pending.pop() {
            if seen.insert(f) {
                group.push(f);
                for co in b.faces[f].loops.iter().flatten() {
                    pending.extend(&ef[co.edge]);
                }
            }
        }
        groups.push(group);
    }
    groups
}
fn signed_area(b: &Brep, f: &Face) -> f64 {
    let Surface::Plane { normal, .. } = f.surface else {
        return f64::NAN;
    };
    let co = &f.loops[0][0];
    let origin = b.vertices[b.edges[co.edge].vertices[0]].point;
    let mut area = 0.;
    for co in f.loops.iter().flatten() {
        let e = &b.edges[co.edge];
        let [a, c] = e.vertices.map(|v| sub(b.vertices[v].point, origin));
        area += dot(cross3(a, c), normal) * if co.reversed { -0.5 } else { 0.5 };
    }
    area * if f.reversed { -1. } else { 1. }
}
fn signed_volume(b: &Brep, faces: &[usize]) -> f64 {
    let origin = b.vertices[0].point;
    faces
        .iter()
        .map(|i| {
            let f = &b.faces[*i];
            let Surface::Plane { origin: p, normal } = f.surface else {
                return f64::NAN;
            };
            signed_area(b, f)
                * dot(sub(p, origin), normal)
                * if f.reversed { -1. / 3. } else { 1. / 3. }
        })
        .sum()
}
pub(super) fn measurements(b: &Brep) -> (f64, f64) {
    (
        signed_volume(b, &(0..b.faces.len()).collect::<Vec<_>>()),
        b.faces.iter().map(|f| signed_area(b, f)).sum(),
    )
}
pub(super) fn bounds(b: &Brep) -> [[f64; 3]; 2] {
    let mut bounds = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
    for v in &b.vertices {
        for i in 0..3 {
            bounds[0][i] = bounds[0][i].min(v.point[i]);
            bounds[1][i] = bounds[1][i].max(v.point[i]);
        }
    }
    bounds
}
/// Validate local topology without interpreting separate boundary shells as bodies.
fn validate_local(b: &Brep) -> Result<()> {
    // Each connected shell is passed through the existing incidence, coedge,
    // orientation and vertex-link validator. Shared vertices across shells are
    // separately forbidden, preventing pinches hidden by remapping subsets.
    let groups = shells(b);
    let mut used = BTreeSet::new();
    for group in &groups {
        let mut vertices = BTreeSet::new();
        for i in group {
            for co in b.faces[*i].loops.iter().flatten() {
                vertices.extend(b.edges[co.edge].vertices);
            }
        }
        if vertices.iter().any(|i| !used.insert(*i)) {
            return invalid("Planar shells share a pinched vertex");
        }
        let mut shell = subset(b, group).0;
        // Validate the local manifold of an inner shell in outward orientation;
        // its actual inward orientation is checked against containment below.
        if signed_volume(b, group) < 0. {
            for f in &mut shell.faces {
                f.reversed = !f.reversed;
                for l in &mut f.loops {
                    reverse(l);
                }
            }
        }
        shell.validate()?;
    }
    Ok(())
}
fn shell_relationships(
    b: &Brep,
    groups: &[Vec<usize>],
    work: &mut usize,
) -> Result<Vec<Option<usize>>> {
    if groups.len() > MAX_SHELLS {
        return invalid("Planar solid exceeds 64 boundary shells");
    }
    let pp = patches(b, None)?;
    let mut containers = vec![vec![]; groups.len()];
    for (i, g) in groups.iter().enumerate() {
        if signed_volume(b, g).abs() <= EPS.powi(3) {
            return invalid("Degenerate planar shell volume");
        }
        for (j, h) in groups.iter().enumerate() {
            if i == j {
                continue;
            }
            let mut other: Vec<_> = h.iter().map(|k| pp[*k].clone()).collect();
            if signed_volume(b, h) < 0. {
                for p in &mut other {
                    flip(p)?;
                }
            }
            // Check all vertices, not just one arbitrary containment probe.
            let mut states = BTreeSet::new();
            for f in g {
                for co in b.faces[*f].loops.iter().flatten() {
                    let p = b.vertices[b.edges[co.edge].vertices[0]].point;
                    match locate(&other, p, work)? {
                        Location::Inside => {
                            states.insert(true);
                        }
                        Location::Outside => {
                            states.insert(false);
                        }
                        Location::Boundary(_) => {
                            return invalid("Boundary shells touch or coincide");
                        }
                    }
                }
            }
            if states.len() != 1 {
                return invalid("Boundary shells intersect");
            }
            if states.contains(&true) {
                containers[i].push(j);
            }
            // Vertex containment alone misses transverse crossings through face interiors.
            for f in g {
                for k in h {
                    budget(work, 1)?;
                    if let Relation::Line { p, d } = relation(pp[*f].frame, pp[*k].frame)? {
                        if finite_overlap(&pp[*f], &pp[*k], p, d, work)? {
                            return invalid("Boundary shells intersect along finite face trims");
                        }
                    } else if matches!(relation(pp[*f].frame, pp[*k].frame)?, Relation::Coplanar) {
                        if set(pp[*f].region.clone())
                            .boolean(
                                &set(projected(&pp[*k], pp[*f].frame)),
                                Boolean::Intersection,
                            )?
                            .area()
                            > EPS.powi(2)
                        {
                            return invalid("Boundary shells have coincident face area");
                        }
                    }
                }
            }
        }
    }
    let mut parents = vec![None; groups.len()];
    for i in 0..groups.len() {
        let depth = containers[i].len();
        let positive = signed_volume(b, &groups[i]) > 0.;
        if positive != (depth % 2 == 0) {
            return invalid("Planar shell orientation disagrees with nesting");
        }
        if let Some(parent) = containers[i].iter().max_by_key(|j| containers[**j].len()) {
            if containers[*parent].len() + 1 != depth {
                return invalid("Ambiguous shell containment hierarchy");
            }
            parents[i] = Some(*parent);
        }
    }
    // Positive shells nested inside a void are independent material components.
    for i in 0..parents.len() {
        if signed_volume(b, &groups[i]) > 0. {
            parents[i] = None;
        }
    }
    Ok(parents)
}
pub(super) fn validate_shells(b: &Brep) -> Result<()> {
    if !supported(b) {
        return invalid("Multiple curved boundary shells are unsupported");
    }
    let groups = shells(b);
    let mut work = 0;
    let parents = shell_relationships(b, &groups, &mut work)?;
    if parents.iter().filter(|p| p.is_none()).count() != 1 {
        return invalid("B-rep contains multiple material components");
    }
    Ok(())
}
/// Analytic trim validation also excludes crossings inside a connected shell.
/// Two-use edges alone cannot establish an embedded (non-self-intersecting) shell.
pub(super) fn validate_geometry(b: &Brep) -> Result<()> {
    let pp = patches(b, None)?;
    let mut work = 0;
    let mut pairs = 0;
    let bounds: Vec<_> = pp
        .iter()
        .map(|p| {
            let mut b = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
            for q in p
                .region
                .loops()
                .flat_map(|l| &l.spans)
                .map(|s| p.frame.world(s.curve.point(0.)))
            {
                for i in 0..3 {
                    b[0][i] = b[0][i].min(q[i]);
                    b[1][i] = b[1][i].max(q[i]);
                }
            }
            b
        })
        .collect();
    for i in 0..pp.len() {
        for j in i + 1..pp.len() {
            if (0..3).any(|k| {
                bounds[i][1][k] < bounds[j][0][k] - POLICY.linear_mm
                    || bounds[j][1][k] < bounds[i][0][k] - POLICY.linear_mm
            }) {
                continue;
            }
            pairs += 1;
            if pairs > MAX_PAIRS {
                return invalid("Planar validation exceeds 262144 face pairs");
            }
            match relation(pp[i].frame, pp[j].frame)? {
                Relation::Parallel => {}
                Relation::Coplanar => {
                    let a = set(pp[i].region.clone());
                    let b = set(projected(&pp[j], pp[i].frame));
                    budget(
                        &mut work,
                        a.regions[0].loops().map(|l| l.spans.len()).sum::<usize>()
                            * b.regions[0].loops().map(|l| l.spans.len()).sum::<usize>(),
                    )?;
                    if a.boolean(&b, Boolean::Intersection)?.area() > EPS.powi(2) {
                        return invalid("Planar faces have overlapping coplanar interiors");
                    }
                }
                Relation::Line { p, d } => {
                    let aa = intervals(&pp[i], p, d, &mut work)?;
                    let bb = intervals(&pp[j], p, d, &mut work)?;
                    for a in &aa {
                        for b in &bb {
                            let lo = a[0].max(b[0]);
                            let hi = a[1].min(b[1]);
                            if hi - lo <= POLICY.linear_mm {
                                continue;
                            }
                            if hi - lo < EPS {
                                return invalid("Face incidence is below resolution");
                            }
                            let q = add(p, mul(d, (lo + hi) * 0.5));
                            if !boundary(&pp[i].region, pp[i].frame.project(q))
                                || !boundary(&pp[j].region, pp[j].frame.project(q))
                            {
                                return invalid(
                                    "Planar shell has a transverse face self-intersection",
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
pub(super) fn translated(
    s: &NativeSolid,
    offset: V,
    feature: u64,
    instance: u64,
) -> Result<NativeSolid> {
    let mut result = s.clone();
    let g = result.general.as_mut().unwrap();
    g.transforms += 1;
    if g.transforms > 64 || result.provenance.len() >= 32768 {
        return invalid("Planar transform/provenance resource limit");
    }
    for v in &mut result.topology.vertices {
        v.point = add(v.point, offset);
        for x in v.point {
            crate::number(x)?;
        }
    }
    for f in &mut result.topology.faces {
        if let Surface::Plane { origin, .. } = &mut f.surface {
            *origin = add(*origin, offset);
        }
        f.id.local = planar::identity(&("translated-face", f.id, feature, instance))?;
        f.id.feature = feature;
    }
    for e in &mut result.topology.edges {
        e.id.local = planar::identity(&("translated-edge", e.id, feature, instance))?;
        e.id.feature = feature;
    }
    for roots in &mut g.roots {
        *roots = roots
            .iter()
            .cloned()
            .map(|mut r| {
                r.occurrences.push((feature, instance));
                r
            })
            .collect();
    }
    for region in &mut g.regions {
        prisms::instance(region, feature, instance);
    }
    validate_metadata(g)?;
    g.signature = result.topology.faces.iter().map(|f| f.id.local).collect();
    g.signature.sort();
    result.provenance.push(Provenance {
        feature,
        operation: Operation::NewBody,
        parents: vec![s.last_feature()],
    });
    result.topology.validate()?;
    Ok(result)
}
fn validate_metadata(g: &General) -> Result<()> {
    if g.roots.len() != g.regions.len() {
        return invalid("Planar ancestry/face inventory mismatch");
    }
    let mut records = g.sources.records;
    for (roots, region) in g.roots.iter().zip(&g.regions) {
        if roots.is_empty() || roots.len() > 512 || roots.iter().any(|r| r.occurrences.len() > 64) {
            return invalid("Planar face source/transform resource limit");
        }
        records += roots.iter().map(|r| 1 + r.occurrences.len()).sum::<usize>();
        set(region.clone()).validate()?;
        for s in region.loops().flat_map(|l| &l.spans) {
            let mut refs: Vec<_> = s.ancestry.iter().map(|a| &a.source).collect();
            for j in s.start.iter().chain(&s.end) {
                match j {
                    planar::Junction::Endpoint(a, _) => refs.push(a),
                    planar::Junction::Intersection(a, b, _) => {
                        refs.push(a);
                        refs.push(b);
                    }
                }
            }
            if refs
                .iter()
                .any(|s| !g.sources.entries.contains_key(&s.boundary) || s.occurrences.len() > 64)
            {
                return invalid("Planar trim has missing source ancestry");
            }
            if s.ancestry.iter().any(|a| {
                matches!(
                    g.sources.entries.get(&a.source.boundary),
                    Some(BoundarySource::FiniteGuard(..))
                )
            }) {
                return invalid("A finite clipping guard escaped into the solid boundary");
            }
            records += s
                .ancestry
                .iter()
                .map(|a| 1 + a.source.occurrences.len())
                .sum::<usize>()
                + s.start.len()
                + s.end.len();
        }
        if records > 32768 {
            return invalid("Planar source provenance exceeds 32768 records");
        }
    }
    Ok(())
}
/// Display-only exact polygon vertical decomposition, including holes.
pub(super) fn mesh(b: &Brep, segments: usize) -> Result<DisplayMesh> {
    if !(8..=4096).contains(&segments) {
        return invalid("Rust display tessellation requires 8..4096 segments");
    }
    b.validate()?;
    let pp = patches(b, None)?;
    let mut mesh = DisplayMesh::default();
    for (f, p) in b.faces.iter().zip(pp) {
        let edges: Vec<_> = p
            .region
            .loops()
            .flat_map(|l| &l.spans)
            .map(|s| (s.curve.point(0.), s.curve.point(1.)))
            .collect();
        let mut xs: Vec<_> = edges.iter().flat_map(|(a, c)| [a[0], c[0]]).collect();
        xs.sort_by(f64::total_cmp);
        xs.dedup_by(|a, c| (*a - *c).abs() < POLICY.linear_mm);
        let y = |(a, c): ([f64; 2], [f64; 2]), x: f64| {
            a[1] + (c[1] - a[1]) * (x - a[0]) / (c[0] - a[0])
        };
        for w in xs.windows(2) {
            let mid = (w[0] + w[1]) * 0.5;
            let mut crossing: Vec<_> = edges
                .iter()
                .copied()
                .filter(|(a, c)| mid > a[0].min(c[0]) && mid < a[0].max(c[0]))
                .collect();
            crossing.sort_by(|a, c| y(*a, mid).total_cmp(&y(*c, mid)));
            if crossing.len() % 2 != 0 {
                return invalid("Planar display decomposition has odd crossings");
            }
            for pair in crossing.chunks_exact(2) {
                let a = p.frame.world([w[0], y(pair[0], w[0])]);
                let c = p.frame.world([w[1], y(pair[0], w[1])]);
                let d = p.frame.world([w[1], y(pair[1], w[1])]);
                let e = p.frame.world([w[0], y(pair[1], w[0])]);
                triangle(&mut mesh, [a, c, d], p.frame.n, f.id);
                triangle(&mut mesh, [a, d, e], p.frame.n, f.id);
            }
        }
    }
    for e in &b.edges {
        mesh.edges.extend(e.vertices.map(|i| b.vertices[i].point));
        mesh.edge_ids.push(e.id);
    }
    Ok(mesh)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn append(b: &mut Brep, other: Brep) {
        let v = b.vertices.len();
        let e = b.edges.len();
        b.vertices.extend(other.vertices);
        b.edges.extend(other.edges.into_iter().map(|mut edge| {
            edge.vertices = edge.vertices.map(|i| i + v);
            edge
        }));
        b.faces.extend(other.faces.into_iter().map(|mut face| {
            for co in face.loops.iter_mut().flatten() {
                co.edge += e;
            }
            face
        }));
    }
    #[test]
    fn arbitrary_face_frames_and_analytic_plane_relations() {
        let a = Frame::new([17., -23., 51.], unit([1., 2., 3.]).unwrap()).unwrap();
        let b = Frame::new([-31., 11., 7.], unit([2., -1., 4.]).unwrap()).unwrap();
        let Relation::Line { p, d } = relation(a, b).unwrap() else {
            panic!()
        };
        for t in [-100., 0., 137.] {
            let q = add(p, mul(d, t));
            assert!(a.distance(q).abs() < POLICY.linear_mm);
            assert!(b.distance(q).abs() < POLICY.linear_mm);
        }
        let p = a.world([13., -7.]);
        let q = a.project(p);
        assert!((q[0] - 13.).abs() < POLICY.linear_mm);
        assert!((q[1] + 7.).abs() < POLICY.linear_mm);
        assert!(matches!(
            relation(a, Frame::new(a.o, mul(a.n, -1.)).unwrap()).unwrap(),
            Relation::Coplanar
        ));
        assert!(matches!(
            relation(a, Frame::new(add(a.o, mul(a.n, 2.)), a.n).unwrap()).unwrap(),
            Relation::Parallel
        ));
        assert!(relation(a, Frame::new(add(a.o, mul(a.n, 1e-6)), a.n).unwrap()).is_err());
        assert!(Frame::new([0.; 3], [0.; 3]).is_err());
        let mut ledger = SourceLedger::default();
        let id = TopologyId {
            body: 1,
            feature: 2,
            region: 3,
            local: 4,
        };
        ledger.insert(7, BoundarySource::Edge(id)).unwrap();
        assert!(
            ledger
                .insert(7, BoundarySource::Edge(TopologyId { local: 5, ..id }))
                .is_err()
        );
        assert_eq!(ledger.entries[&7], BoundarySource::Edge(id));
    }
    #[test]
    fn shell_nesting_orientation_intersection_and_shell_budget_are_validated() {
        let outer = NativeSolid::extrude(box_input([[0.; 3], [10.; 3]], 1)).unwrap();
        let inner = NativeSolid::extrude(box_input([[2.; 3], [8.; 3]], 2)).unwrap();
        let mut b = outer.topology.clone();
        append(&mut b, inner.topology.clone());
        assert!(
            b.validate().is_err(),
            "nested outer-oriented shell must fail"
        );
        for f in b.faces.iter_mut().skip(6) {
            f.reversed = !f.reversed;
            for l in &mut f.loops {
                reverse(l);
            }
        }
        b.validate().unwrap();
        assert_eq!(b.shell_count(), 2);
        assert!((measurements(&b).0 - 784.).abs() < EPS);
        let mut inward = outer.topology.clone();
        for f in &mut inward.faces {
            f.reversed = !f.reversed;
            for l in &mut f.loops {
                reverse(l);
            }
        }
        assert!(inward.validate().is_err());
        let cross = NativeSolid::extrude(box_input([[-1., 4., 4.], [11., 6., 6.]], 3)).unwrap();
        let mut intersected = outer.topology.clone();
        append(&mut intersected, cross.topology);
        assert!(intersected.validate().is_err());
        let mut many = Brep {
            vertices: vec![],
            edges: vec![],
            faces: vec![],
        };
        for i in 0..65 {
            let x = i as f64 * 3.;
            append(
                &mut many,
                NativeSolid::extrude(box_input([[x, 0., 0.], [x + 1., 1., 1.]], 100 + i))
                    .unwrap()
                    .topology,
            );
        }
        assert!(
            many.validate()
                .unwrap_err()
                .to_string()
                .contains("64 boundary shells")
        );
    }
}
