//! Independent analytic, bounded circular sketch arcs. Sampling is display-only.
use crate::{
    EPS, Result, Sketch,
    expression::Parameters,
    invalid,
    sketch::{Entity, SketchGraph},
};
use std::f64::consts::TAU;
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArcGeometry {
    pub center: [f64; 2],
    pub radius: f64,
    pub start_angle: f64,
    /// Signed radians; positive counterclockwise, negative clockwise.
    pub sweep: f64,
}
fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
impl ArcGeometry {
    pub fn through(start: [f64; 2], through: [f64; 2], end: [f64; 2]) -> Result<Self> {
        for p in [start, through, end] {
            for n in p {
                crate::number(n)?;
            }
        }
        let u = [through[0] - start[0], through[1] - start[1]];
        let v = [end[0] - start[0], end[1] - start[1]];
        let cross = u[0] * v[1] - u[1] * v[0];
        if distance(start, through) < EPS
            || distance(through, end) < EPS
            || distance(start, end) < EPS
            || cross.abs() / distance(start, end) < EPS
        {
            return invalid("Arc needs three distinct, non-collinear points");
        }
        let uu = u[0] * u[0] + u[1] * u[1];
        let vv = v[0] * v[0] + v[1] * v[1];
        let center = [
            start[0] + (uu * v[1] - vv * u[1]) / (2. * cross),
            start[1] + (u[0] * vv - v[0] * uu) / (2. * cross),
        ];
        Self::from_endpoints(center, start, end, cross < 0.)
    }
    pub fn from_endpoints(
        center: [f64; 2],
        start: [f64; 2],
        end: [f64; 2],
        clockwise: bool,
    ) -> Result<Self> {
        for p in [center, start, end] {
            for n in p {
                crate::number(n)?;
            }
        }
        let radius = distance(center, start);
        crate::number(radius)?;
        if radius < EPS
            || distance(start, end) < EPS
            || (distance(center, end) - radius).abs() > 1e-6
        {
            return invalid("Arc endpoints must be distinct and lie on the same nonzero circle");
        }
        let start_angle = (start[1] - center[1]).atan2(start[0] - center[0]);
        let end_angle = (end[1] - center[1]).atan2(end[0] - center[0]);
        let sweep = if clockwise {
            -((start_angle - end_angle).rem_euclid(TAU))
        } else {
            (end_angle - start_angle).rem_euclid(TAU)
        };
        if sweep.abs() * radius < EPS || (TAU - sweep.abs()) * radius < EPS {
            return invalid("Arc sweep is degenerate; use a circle for a full turn");
        }
        Ok(Self {
            center,
            radius,
            start_angle,
            sweep,
        })
    }
    pub fn point(self, t: f64) -> [f64; 2] {
        let angle = self.start_angle + self.sweep * t;
        [
            self.center[0] + self.radius * angle.cos(),
            self.center[1] + self.radius * angle.sin(),
        ]
    }
    pub fn length(self) -> f64 {
        self.radius * self.sweep.abs()
    }
    pub fn signed_segment_area(self) -> f64 {
        0.5 * self.radius * self.radius * (self.sweep - self.sweep.sin())
    }
    pub fn contains(self, p: [f64; 2], tolerance: f64) -> bool {
        if (distance(p, self.center) - self.radius).abs() > tolerance {
            return false;
        }
        if distance(p, self.point(0.)) <= tolerance || distance(p, self.point(1.)) <= tolerance {
            return true;
        }
        let angle = (p[1] - self.center[1]).atan2(p[0] - self.center[0]);
        let progress = if self.sweep > 0. {
            (angle - self.start_angle).rem_euclid(TAU)
        } else {
            (self.start_angle - angle).rem_euclid(TAU)
        };
        progress <= self.sweep.abs() + tolerance / self.radius
    }
    pub fn distance_to(self, p: [f64; 2]) -> f64 {
        let d = distance(p, self.center);
        if d > 0. {
            let radial = [
                self.center[0] + (p[0] - self.center[0]) * self.radius / d,
                self.center[1] + (p[1] - self.center[1]) * self.radius / d,
            ];
            if self.contains(radial, 1e-7) {
                return (d - self.radius).abs();
            }
        }
        distance(p, self.point(0.)).min(distance(p, self.point(1.)))
    }
    pub fn sample(self, segments: usize) -> Vec<[f64; 2]> {
        (0..=segments.max(1))
            .map(|i| self.point(i as f64 / segments.max(1) as f64))
            .collect()
    }
}
impl SketchGraph {
    pub fn arc_geometry(&self, id: u64) -> Result<ArcGeometry> {
        let Some(Entity::Arc {
            center,
            start,
            end,
            clockwise,
        }) = self.entities.get(&id)
        else {
            return invalid("Missing sketch arc");
        };
        let point = |id: &u64| {
            self.points
                .get(id)
                .copied()
                .ok_or_else(|| crate::Error::Invalid("Missing arc point dependency".into()))
        };
        ArcGeometry::from_endpoints(point(center)?, point(start)?, point(end)?, *clockwise)
    }
    pub(crate) fn validate_curve_relations(&self) -> Result<()> {
        for (id, e) in &self.entities {
            if matches!(e, Entity::Arc { .. }) {
                self.arc_geometry(*id)?;
            }
        }
        for c in self.constraints.values() {
            if let crate::sketch::Constraint::Tangent { line, circle } = c {
                if matches!(self.entities.get(circle), Some(Entity::Arc { .. })) {
                    let arc = self.arc_geometry(*circle)?;
                    let Some(Entity::Line { a, b }) = self.entities.get(line) else {
                        return invalid("Missing tangent line");
                    };
                    let a = self.points[a];
                    let b = self.points[b];
                    let d = [b[0] - a[0], b[1] - a[1]];
                    let squared = d[0] * d[0] + d[1] * d[1];
                    let t =
                        ((arc.center[0] - a[0]) * d[0] + (arc.center[1] - a[1]) * d[1]) / squared;
                    let p = [a[0] + t * d[0], a[1] + t * d[1]];
                    if t < -1e-6 / squared.sqrt()
                        || t > 1. + 1e-6 / squared.sqrt()
                        || !arc.contains(p, 1e-6)
                    {
                        return invalid("Tangency falls outside the selected finite arc or line");
                    }
                }
            }
        }
        Ok(())
    }
}
impl Sketch {
    pub fn with_three_point_arc(
        &self,
        start: [f64; 2],
        through: [f64; 2],
        end: [f64; 2],
        parameters: &Parameters,
    ) -> Result<Self> {
        let arc = ArcGeometry::through(start, through, end)?;
        let mut graph = self.graph_for_edit()?;
        let mut endpoint = |p: [f64; 2]| -> Result<u64> {
            if let Some((id, _)) = graph.points.iter().find(|(_, q)| distance(p, **q) < EPS) {
                return Ok(*id);
            }
            graph.point(p)
        };
        let start = endpoint(start)?;
        let end = endpoint(end)?;
        let center = graph.point(arc.center)?;
        graph.entity(Entity::Arc {
            center,
            start,
            end,
            clockwise: arc.sweep < 0.,
        })?;
        self.with_graph(graph, parameters)
    }
}
