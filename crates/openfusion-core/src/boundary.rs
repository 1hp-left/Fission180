//! Exact planar line/arc boundaries. Sampling is only used for display.
use crate::{EPS, Profile, Result, arc::ArcGeometry, invalid};
use serde::{Deserialize, Serialize};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlanarCurve {
    Line { start: [f64; 2], end: [f64; 2] },
    Arc { geometry: ArcGeometry },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundarySegment {
    /// Original sketch entity; None for an independently supplied analytic loop.
    pub source: Option<u64>,
    pub curve: PlanarCurve,
}
pub(crate) fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
pub(crate) fn cross(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
pub(crate) fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
impl PlanarCurve {
    pub fn point(self, t: f64) -> [f64; 2] {
        match self {
            Self::Line { start: a, end: b } => [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])],
            Self::Arc { geometry: a } => a.point(t),
        }
    }
    pub fn tangent(self, t: f64) -> [f64; 2] {
        match self {
            Self::Line { start, end } => sub(end, start),
            Self::Arc { geometry: a } => {
                let angle = a.start_angle + a.sweep * t;
                [
                    -angle.sin() * a.radius * a.sweep,
                    angle.cos() * a.radius * a.sweep,
                ]
            }
        }
    }
    pub fn length(self) -> f64 {
        match self {
            Self::Line { start, end } => distance(start, end),
            Self::Arc { geometry } => geometry.length(),
        }
    }
    pub fn portion(self, start: f64, end: f64) -> Self {
        match self {
            Self::Line { .. } => Self::Line {
                start: self.point(start),
                end: self.point(end),
            },
            Self::Arc { geometry: a } => Self::Arc {
                geometry: ArcGeometry {
                    start_angle: a.start_angle + a.sweep * start,
                    sweep: a.sweep * (end - start),
                    ..a
                },
            },
        }
    }
    pub fn reversed(self) -> Self {
        self.portion(1., 0.)
    }
    pub fn parameter(self, p: [f64; 2]) -> Option<f64> {
        let t = match self {
            Self::Line { start: a, end: b } => {
                let d = sub(b, a);
                let v = sub(p, a);
                let l = d[0] * d[0] + d[1] * d[1];
                if cross(d, v).abs() > EPS * l.sqrt() {
                    return None;
                }
                (d[0] * v[0] + d[1] * v[1]) / l
            }
            Self::Arc { geometry: a } => {
                if !a.contains(p, EPS) {
                    return None;
                }
                if distance(p, a.point(0.)) < EPS {
                    return Some(0.);
                }
                if distance(p, a.point(1.)) < EPS {
                    return Some(1.);
                }
                let angle = (p[1] - a.center[1]).atan2(p[0] - a.center[0]);
                if a.sweep > 0. {
                    (angle - a.start_angle).rem_euclid(TAU) / a.sweep
                } else {
                    (a.start_angle - angle).rem_euclid(TAU) / -a.sweep
                }
            }
        };
        (t >= -EPS / self.length() && t <= 1. + EPS / self.length()).then_some(t.clamp(0., 1.))
    }
    pub fn validate(self) -> Result<()> {
        match self {
            Self::Line { start, end } => {
                for n in start.into_iter().chain(end) {
                    crate::number(n)?;
                }
            }
            Self::Arc { geometry: a } => {
                for n in a
                    .center
                    .into_iter()
                    .chain([a.radius, a.start_angle, a.sweep])
                {
                    crate::number(n)?;
                }
                if a.radius < EPS
                    || a.sweep.abs() > TAU + 1e-12
                    || (TAU - a.sweep.abs()).abs() * a.radius < EPS
                {
                    return invalid(
                        "Bounded arc must have a nonzero sweep smaller than a full turn",
                    );
                }
                for n in a.point(0.).into_iter().chain(a.point(1.)) {
                    crate::number(n)?;
                }
            }
        }
        if self.length() < EPS {
            return invalid("Degenerate analytic boundary segment");
        }
        Ok(())
    }
    pub fn area_term(self, anchor: [f64; 2]) -> f64 {
        let chord = cross(sub(self.point(0.), anchor), sub(self.point(1.), anchor)) * 0.5;
        chord
            + match self {
                Self::Arc { geometry: a } => a.signed_segment_area(),
                _ => 0.,
            }
    }
    pub fn extrema(self) -> Vec<[f64; 2]> {
        let mut points = vec![self.point(0.), self.point(1.)];
        if let Self::Arc { geometry: a } = self {
            for angle in [0., FRAC_PI_2, PI, 3. * FRAC_PI_2] {
                let p = [
                    a.center[0] + a.radius * angle.cos(),
                    a.center[1] + a.radius * angle.sin(),
                ];
                if a.contains(p, EPS) {
                    points.push(p);
                }
            }
        }
        points
    }
}

/// Finite analytic intersections; overlapping supports with positive shared length
/// are rejected rather than assigned an arbitrary arrangement ordering.
pub fn intersections(a: PlanarCurve, b: PlanarCurve) -> Result<Vec<(f64, f64)>> {
    let mut points = vec![];
    match (a, b) {
        (PlanarCurve::Line { start: p, end: q }, PlanarCurve::Line { start: r, end: s }) => {
            let u = sub(q, p);
            let v = sub(s, r);
            let delta = sub(r, p);
            let d = cross(u, v);
            if d.abs() <= 1e-12 * a.length() * b.length() {
                if cross(u, delta).abs() <= EPS * a.length() {
                    let l = u[0] * u[0] + u[1] * u[1];
                    let t = (delta[0] * u[0] + delta[1] * u[1]) / l;
                    let ds = sub(s, p);
                    let w = (ds[0] * u[0] + ds[1] * u[1]) / l;
                    if t.max(w).min(1.) - t.min(w).max(0.) > EPS / a.length() {
                        return invalid("Overlapping collinear sketch lines are ambiguous");
                    }
                    points.extend([p, q, r, s]);
                }
            } else {
                points.push(a.point(cross(delta, v) / d));
            }
        }
        (PlanarCurve::Line { start: p, end: q }, PlanarCurve::Arc { geometry: c }) => {
            let v = sub(q, p);
            let l = a.length();
            let m = sub(c.center, p);
            let t = (m[0] * v[0] + m[1] * v[1]) / (l * l);
            let h = cross(v, m) / l;
            let ds = c.radius * c.radius - h * h;
            if ds >= -EPS * (2. * c.radius + EPS) {
                let delta = ds.max(0.).sqrt() / l;
                points.extend([a.point(t - delta), a.point(t + delta)]);
            }
        }
        (PlanarCurve::Arc { .. }, PlanarCurve::Line { .. }) => {
            return Ok(intersections(b, a)?
                .into_iter()
                .map(|(t, u)| (u, t))
                .collect());
        }
        (PlanarCurve::Arc { geometry: c }, PlanarCurve::Arc { geometry: d }) => {
            let v = sub(d.center, c.center);
            let len = distance(c.center, d.center);
            if len < EPS && (c.radius - d.radius).abs() < EPS {
                // Angular interval intersection is exact, including major branches.
                let intervals = |arc: ArcGeometry| {
                    let lo = if arc.sweep > 0. {
                        arc.start_angle
                    } else {
                        arc.start_angle + arc.sweep
                    }
                    .rem_euclid(TAU);
                    let hi = lo + arc.sweep.abs();
                    if hi > TAU {
                        vec![(lo, TAU), (0., hi - TAU)]
                    } else {
                        vec![(lo, hi)]
                    }
                };
                if intervals(c).iter().any(|(lo, hi)| {
                    intervals(d)
                        .iter()
                        .any(|(l, h)| hi.min(*h) - lo.max(*l) > EPS / c.radius)
                }) {
                    return invalid("Overlapping coincident arcs are ambiguous");
                }
                points.extend([a.point(0.), a.point(1.), b.point(0.), b.point(1.)]);
            } else if len >= EPS
                && len <= c.radius + d.radius + EPS
                && len >= (c.radius - d.radius).abs() - EPS
            {
                let x = (c.radius * c.radius - d.radius * d.radius + len * len) / (2. * len);
                let hs = c.radius * c.radius - x * x;
                if hs >= -EPS * (2. * c.radius + EPS) {
                    let h = hs.max(0.).sqrt();
                    for sign in [-1., 1.] {
                        points.push([
                            c.center[0] + (x * v[0] - sign * h * v[1]) / len,
                            c.center[1] + (x * v[1] + sign * h * v[0]) / len,
                        ]);
                    }
                }
            }
        }
    }
    let mut hits: Vec<(f64, f64)> = vec![];
    for p in points {
        if let (Some(t), Some(u)) = (a.parameter(p), b.parameter(p)) {
            if !hits.iter().any(|(s, _)| (t - s).abs() * a.length() < EPS) {
                hits.push((t, u));
            }
        }
    }
    hits.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(hits)
}
impl Profile {
    pub fn boundary(&self) -> Vec<BoundarySegment> {
        match self {
            Self::Curved { segments } => segments.clone(),
            Self::Circle { center, radius } => [0., PI]
                .into_iter()
                .map(|angle| BoundarySegment {
                    source: None,
                    curve: PlanarCurve::Arc {
                        geometry: ArcGeometry {
                            center: *center,
                            radius: *radius,
                            start_angle: angle,
                            sweep: PI,
                        },
                    },
                })
                .collect(),
            _ => {
                let p = self.outline(0);
                (0..p.len())
                    .map(|i| BoundarySegment {
                        source: None,
                        curve: PlanarCurve::Line {
                            start: p[i],
                            end: p[(i + 1) % p.len()],
                        },
                    })
                    .collect()
            }
        }
    }
    pub fn signed_area(&self) -> f64 {
        let boundary = self.boundary();
        let anchor = boundary[0].curve.point(0.);
        boundary.iter().map(|s| s.curve.area_term(anchor)).sum()
    }
    pub fn perimeter(&self) -> f64 {
        self.boundary().iter().map(|s| s.curve.length()).sum()
    }
    pub fn planar_bounds(&self) -> [[f64; 2]; 2] {
        let mut bounds = [[f64::INFINITY; 2], [f64::NEG_INFINITY; 2]];
        for p in self.boundary().iter().flat_map(|s| s.curve.extrema()) {
            for i in 0..2 {
                bounds[0][i] = bounds[0][i].min(p[i]);
                bounds[1][i] = bounds[1][i].max(p[i]);
            }
        }
        bounds
    }
    /// Winding on y-monotone analytic spans, using half-open endpoint ownership.
    pub fn contains_point(&self, p: [f64; 2]) -> bool {
        let mut winding = 0;
        for span in self.boundary() {
            let mut cuts = vec![0., 1.];
            if let PlanarCurve::Arc { geometry: a } = span.curve {
                for angle in [FRAC_PI_2, 3. * FRAC_PI_2] {
                    if let Some(t) = span.curve.parameter([
                        a.center[0] + a.radius * angle.cos(),
                        a.center[1] + a.radius * angle.sin(),
                    ]) {
                        cuts.push(t);
                    }
                }
            }
            cuts.sort_by(f64::total_cmp);
            cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
            for pair in cuts.windows(2) {
                let curve = span.curve.portion(pair[0], pair[1]);
                let a = curve.point(0.);
                let b = curve.point(1.);
                if (a[1] > p[1]) == (b[1] > p[1]) {
                    continue;
                }
                let x = match curve {
                    PlanarCurve::Line { .. } => {
                        a[0] + (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1])
                    }
                    PlanarCurve::Arc { geometry: c } => {
                        c.center[0]
                            + (c.start_angle + c.sweep * 0.5).cos().signum()
                                * (c.radius * c.radius - (p[1] - c.center[1]).powi(2))
                                    .max(0.)
                                    .sqrt()
                    }
                };
                if x > p[0] {
                    winding += if b[1] > a[1] { 1 } else { -1 };
                }
            }
        }
        winding != 0
    }
}
pub(crate) fn validate_loop(segments: &[BoundarySegment]) -> Result<()> {
    if !(2..=1000).contains(&segments.len()) {
        return invalid("Analytic loop requires 2..1000 segments");
    }
    for (i, s) in segments.iter().enumerate() {
        s.curve.validate()?;
        if distance(
            s.curve.point(1.),
            segments[(i + 1) % segments.len()].curve.point(0.),
        ) > 1e-7
        {
            return invalid("Analytic loop is disconnected");
        }
        for (j, t) in segments.iter().enumerate().skip(i + 1) {
            for (u, v) in intersections(s.curve, t.curve)? {
                if !((j == i + 1
                    && (1. - u) * s.curve.length() < EPS
                    && v * t.curve.length() < EPS)
                    || (i == 0
                        && j == segments.len() - 1
                        && u * s.curve.length() < EPS
                        && (1. - v) * t.curve.length() < EPS))
                {
                    return invalid("Analytic loop self-intersects or touches itself");
                }
            }
        }
    }
    let p = Profile::Curved {
        segments: segments.to_vec(),
    };
    if p.signed_area().abs() < EPS * EPS {
        return invalid("Analytic loop has zero area");
    }
    Ok(())
}
