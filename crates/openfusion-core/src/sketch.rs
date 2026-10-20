//! Independent sketch entity graph and damped least-squares constraint solver.
//! This is a Fission implementation, not a recovered Fusion solver.
use crate::{
    EPS, Profile, Result,
    expression::{self, Parameters},
    invalid,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Entity {
    Line { a: u64, b: u64 },
    Circle { center: u64, radius: f64 },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Constraint {
    Fixed {
        point: u64,
        at: [f64; 2],
    },
    Coincident {
        a: u64,
        b: u64,
    },
    Midpoint {
        point: u64,
        line: u64,
    },
    PointOnLine {
        point: u64,
        line: u64,
    },
    Concentric {
        a: u64,
        b: u64,
    },
    Horizontal {
        line: u64,
    },
    Vertical {
        line: u64,
    },
    DistanceX {
        a: u64,
        b: u64,
        expression: String,
        sign: f64,
    },
    DistanceY {
        a: u64,
        b: u64,
        expression: String,
        sign: f64,
    },
    Length {
        line: u64,
        expression: String,
    },
    Radius {
        circle: u64,
        expression: String,
    },
    Equal {
        a: u64,
        b: u64,
    },
    Parallel {
        a: u64,
        b: u64,
    },
    Perpendicular {
        a: u64,
        b: u64,
    },
    Tangent {
        line: u64,
        circle: u64,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SketchGraph {
    pub next_id: u64,
    pub points: BTreeMap<u64, [f64; 2]>,
    pub entities: BTreeMap<u64, Entity>,
    pub constraints: BTreeMap<u64, Constraint>,
    pub profile_entities: BTreeMap<u64, Vec<u64>>,
    pub dimension_ids: BTreeMap<String, u64>,
    #[serde(default)]
    pub construction: std::collections::BTreeSet<u64>,
    /// Retained boundary provenance, including temporarily inactive regions.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub region_cycles: BTreeMap<u64, Vec<u64>>,
}
impl Default for SketchGraph {
    fn default() -> Self {
        Self {
            next_id: 1,
            points: Default::default(),
            entities: Default::default(),
            constraints: Default::default(),
            profile_entities: Default::default(),
            dimension_ids: Default::default(),
            construction: Default::default(),
            region_cycles: Default::default(),
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct SolveReport {
    pub degrees_of_freedom: usize,
    pub redundant_equations: usize,
    pub maximum_residual_mm: f64,
    pub iterations: usize,
}
impl SketchGraph {
    pub fn validate_solution(&self, parameters: &Parameters) -> Result<()> {
        self.validate()?;
        if self.residuals(parameters)?.iter().any(|r| r.abs() > 1e-6) {
            return invalid("Persisted sketch constraints are unsatisfied");
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<()> {
        let mut ids = std::collections::HashSet::new();
        for id in self
            .points
            .keys()
            .chain(self.entities.keys())
            .chain(self.constraints.keys())
        {
            if *id == 0 || *id >= self.next_id || !ids.insert(*id) {
                return invalid("Invalid sketch graph identities");
            }
        }
        for p in self.points.values() {
            for n in p {
                crate::number(*n)?;
            }
        }
        for e in self.entities.values() {
            match e {
                Entity::Line { a, b } => {
                    if a == b || !self.points.contains_key(a) || !self.points.contains_key(b) {
                        return invalid("Line has missing or repeated point dependencies");
                    }
                }
                Entity::Circle { center, radius } => {
                    if !self.points.contains_key(center) || !radius.is_finite() || *radius <= EPS {
                        return invalid("Circle has invalid geometry dependencies");
                    }
                }
            }
        }
        if self
            .construction
            .iter()
            .any(|id| !self.entities.contains_key(id))
        {
            return invalid("Construction state references a missing entity");
        }
        for entities in self.profile_entities.values() {
            if entities.is_empty() || entities.iter().any(|id| !self.entities.contains_key(id)) {
                return invalid("Profile entity dependencies are missing");
            }
        }
        let mut cycles = std::collections::BTreeSet::new();
        for (id, cycle) in &self.region_cycles {
            if *id == 0
                || cycle.len() < 3
                || cycle
                    .iter()
                    .any(|e| !matches!(self.entities.get(e), Some(Entity::Line { .. })))
                || *cycle != crate::arrangement::canonical_cycle(cycle)
                || !cycles.insert(cycle)
            {
                return invalid("Invalid or ambiguous region boundary provenance");
            }
        }
        for id in self.dimension_ids.values() {
            if !self.constraints.contains_key(id) {
                return invalid("Dimension constraint dependency is missing");
            }
        }
        for c in self.constraints.values() {
            if c.references()
                .iter()
                .any(|id| !self.points.contains_key(id) && !self.entities.contains_key(id))
            {
                return invalid("Constraint references missing geometry");
            }
        }
        Ok(())
    }
    fn id(&mut self) -> Result<u64> {
        let id = self.next_id;
        self.next_id = id
            .checked_add(1)
            .ok_or_else(|| crate::Error::Invalid("Sketch graph IDs exhausted".into()))?;
        Ok(id)
    }
    pub fn point(&mut self, at: [f64; 2]) -> Result<u64> {
        for n in at {
            crate::number(n)?;
        }
        let id = self.id()?;
        self.points.insert(id, at);
        Ok(id)
    }
    pub fn entity(&mut self, e: Entity) -> Result<u64> {
        let id = self.id()?;
        self.entities.insert(id, e);
        Ok(id)
    }
    pub fn constrain(&mut self, c: Constraint) -> Result<u64> {
        let id = self.id()?;
        self.constraints.insert(id, c);
        Ok(id)
    }
    pub fn add_profile(&mut self, id: u64, p: &Profile) -> Result<()> {
        if self.profile_entities.contains_key(&id) {
            return invalid("Duplicate profile graph binding");
        }
        let entities = match p {
            Profile::Circle { center, radius } => {
                let center = self.point(*center)?;
                vec![self.entity(Entity::Circle {
                    center,
                    radius: *radius,
                })?]
            }
            _ => {
                let points: Vec<_> = p
                    .outline(0)
                    .into_iter()
                    .map(|p| self.point(p))
                    .collect::<Result<_>>()?;
                let mut entities = vec![];
                for i in 0..points.len() {
                    let line = self.entity(Entity::Line {
                        a: points[i],
                        b: points[(i + 1) % points.len()],
                    })?;
                    if matches!(p, Profile::Rectangle { .. }) {
                        self.constrain(if i % 2 == 0 {
                            Constraint::Horizontal { line }
                        } else {
                            Constraint::Vertical { line }
                        })?;
                    }
                    entities.push(line);
                }
                entities
            }
        };
        self.profile_entities.insert(id, entities);
        Ok(())
    }
    pub fn profile(&self, id: u64, original: &Profile) -> Result<Profile> {
        if let Some(cycle) = self.region_cycles.get(&id) {
            if self
                .profile_entities
                .get(&id)
                .is_some_and(|edges| edges.iter().any(|e| self.construction.contains(e)))
            {
                return self.primitive_profile(id, original);
            }
            let cell = self
                .line_cells()?
                .into_iter()
                .find(|c| &c.cycle == cycle)
                .ok_or_else(|| {
                    crate::Error::Invalid("Missing or changed region boundary".into())
                })?;
            if matches!(original, Profile::Rectangle { .. }) {
                return self.primitive_profile(id, original);
            }
            return Ok(Profile::Polygon {
                points: cell.points,
            });
        }
        self.primitive_profile(id, original)
    }
    pub(crate) fn primitive_profile(&self, id: u64, original: &Profile) -> Result<Profile> {
        let entities = self
            .profile_entities
            .get(&id)
            .ok_or_else(|| crate::Error::Invalid("Profile has no entity dependencies".into()))?;
        let point = |id: &u64| {
            self.points
                .get(id)
                .copied()
                .ok_or_else(|| crate::Error::Invalid("Missing sketch point".into()))
        };
        match original {
            Profile::Circle { .. } => match self.entities.get(&entities[0]) {
                Some(Entity::Circle { center, radius }) => Ok(Profile::Circle {
                    center: point(center)?,
                    radius: *radius,
                }),
                _ => invalid("Circle binding changed type"),
            },
            _ => {
                let points = self.loop_points(entities)?;
                if matches!(original, Profile::Rectangle { .. }) {
                    if points.len() != 4
                        || (points[0][1] - points[1][1]).abs() > EPS
                        || (points[1][0] - points[2][0]).abs() > EPS
                        || (points[2][1] - points[3][1]).abs() > EPS
                        || (points[3][0] - points[0][0]).abs() > EPS
                    {
                        return Ok(Profile::Polygon { points });
                    }
                    Ok(Profile::Rectangle {
                        a: points[0],
                        b: points[2],
                    })
                } else {
                    Ok(Profile::Polygon { points })
                }
            }
        }
    }
    pub fn dimension(&mut self, key: String, constraint: Constraint) -> Result<()> {
        if let Some(id) = self.dimension_ids.get(&key) {
            self.constraints.insert(*id, constraint);
        } else {
            let id = self.constrain(constraint)?;
            self.dimension_ids.insert(key, id);
        }
        Ok(())
    }
    pub fn set_dimensions(
        &mut self,
        id: u64,
        profile: &Profile,
        expressions: &BTreeMap<String, String>,
    ) -> Result<()> {
        let entities = self
            .profile_entities
            .get(&id)
            .ok_or_else(|| crate::Error::Invalid("Dimension references a missing profile".into()))?
            .clone();
        match profile {
            Profile::Rectangle { a, b } => {
                let (p0, p1, p2) = match (
                    self.entities.get(&entities[0]),
                    self.entities.get(&entities[1]),
                ) {
                    (Some(Entity::Line { a, b }), Some(Entity::Line { b: c, .. })) => (*a, *b, *c),
                    _ => return invalid("Rectangle dimension dependencies are invalid"),
                };
                self.dimension(
                    format!("{id}.anchor"),
                    Constraint::Fixed { point: p0, at: *a },
                )?;
                for (name, c) in [
                    (
                        "width",
                        Constraint::DistanceX {
                            a: p0,
                            b: p1,
                            expression: expressions.get("width").cloned().ok_or_else(|| {
                                crate::Error::Invalid("Missing width expression".into())
                            })?,
                            sign: (b[0] - a[0]).signum(),
                        },
                    ),
                    (
                        "height",
                        Constraint::DistanceY {
                            a: p1,
                            b: p2,
                            expression: expressions.get("height").cloned().ok_or_else(|| {
                                crate::Error::Invalid("Missing height expression".into())
                            })?,
                            sign: (b[1] - a[1]).signum(),
                        },
                    ),
                ] {
                    self.dimension(format!("{id}.{name}"), c)?;
                }
            }
            Profile::Circle { .. } => self.dimension(
                format!("{id}.radius"),
                Constraint::Radius {
                    circle: entities[0],
                    expression: expressions
                        .get("radius")
                        .cloned()
                        .ok_or_else(|| crate::Error::Invalid("Missing radius expression".into()))?,
                },
            )?,
            _ => return invalid("Polygon dimension editor is unsupported"),
        }
        Ok(())
    }
    pub fn solve(&self, parameters: &Parameters) -> Result<(Self, SolveReport)> {
        self.validate()?;
        if self.points.len() > 128 || self.constraints.len() > 256 {
            return invalid(
                "Sketch solver currently supports at most 128 points and 256 constraints",
            );
        }
        let mut candidate = self.clone();
        let keys: Vec<_> = candidate.points.keys().copied().collect();
        let circles: Vec<_> = candidate
            .entities
            .iter()
            .filter_map(|(id, e)| matches!(e, Entity::Circle { .. }).then_some(*id))
            .collect();
        let mut x: Vec<f64> = keys
            .iter()
            .flat_map(|id| candidate.points[id])
            .chain(circles.iter().map(|id| match candidate.entities[id] {
                Entity::Circle { radius, .. } => radius,
                _ => unreachable!(),
            }))
            .collect();
        let apply = |graph: &mut Self, x: &[f64]| {
            for (i, id) in keys.iter().enumerate() {
                graph.points.insert(*id, [x[2 * i], x[2 * i + 1]]);
            }
            for (i, id) in circles.iter().enumerate() {
                if let Some(Entity::Circle { radius, .. }) = graph.entities.get_mut(id) {
                    *radius = x[2 * keys.len() + i];
                }
            }
        };
        let mut iterations = 0;
        for iteration in 0..80 {
            iterations = iteration;
            apply(&mut candidate, &x);
            let residual = candidate.residuals(parameters)?;
            if residual.iter().all(|v| v.abs() < 1e-8) {
                break;
            }
            let jac = candidate.jacobian(parameters, &x, &apply)?;
            let n = x.len();
            if n == 0 {
                return invalid("Constraint system has no editable variables");
            }
            let mut normal = vec![vec![0.; n]; n];
            let mut rhs = vec![0.; n];
            for row in 0..residual.len() {
                for i in 0..n {
                    rhs[i] -= jac[row][i] * residual[row];
                    for j in 0..n {
                        normal[i][j] += jac[row][i] * jac[row][j];
                    }
                }
            }
            for (i, row) in normal.iter_mut().enumerate() {
                row[i] += 1e-9;
            }
            let step = linear_solve(normal, rhs)?;
            let before: f64 = residual.iter().map(|r| r * r).sum();
            let mut scale = 1.;
            let mut improved = false;
            for _ in 0..16 {
                let trial: Vec<_> = x.iter().zip(&step).map(|(a, b)| a + scale * b).collect();
                apply(&mut candidate, &trial);
                if let Ok(r) = candidate.residuals(parameters) {
                    if r.iter().map(|r| r * r).sum::<f64>() < before {
                        x = trial;
                        improved = true;
                        break;
                    }
                }
                scale *= 0.5;
            }
            if !improved {
                break;
            }
        }
        apply(&mut candidate, &x);
        let residual = candidate.residuals(parameters)?;
        let maximum = residual.iter().map(|r| r.abs()).fold(0., f64::max);
        if maximum > 1e-6 {
            return invalid(&format!(
                "Sketch constraints conflict or did not converge (residual {maximum:.6} mm); previous geometry retained"
            ));
        }
        for p in candidate.points.values() {
            for n in p {
                crate::number(*n)?;
            }
        }
        let jac = candidate.jacobian(parameters, &x, &apply)?;
        let rank = matrix_rank(jac, 1e-7);
        let report = SolveReport {
            degrees_of_freedom: x.len().saturating_sub(rank),
            redundant_equations: residual.len().saturating_sub(rank),
            maximum_residual_mm: maximum,
            iterations,
        };
        Ok((candidate, report))
    }
    fn jacobian(
        &self,
        p: &Parameters,
        x: &[f64],
        apply: &impl Fn(&mut Self, &[f64]),
    ) -> Result<Vec<Vec<f64>>> {
        let initial = self.residuals(p)?;
        let mut jac = vec![vec![0.; x.len()]; initial.len()];
        for i in 0..x.len() {
            let h = 1e-6 * (1. + x[i].abs());
            let mut values = x.to_vec();
            values[i] += h;
            let mut perturbed = self.clone();
            apply(&mut perturbed, &values);
            let r = perturbed.residuals(p)?;
            values[i] = x[i] - h;
            apply(&mut perturbed, &values);
            let negative = perturbed.residuals(p)?;
            for row in 0..r.len() {
                jac[row][i] = (r[row] - negative[row]) / (2. * h);
            }
        }
        Ok(jac)
    }
    fn residuals(&self, parameters: &Parameters) -> Result<Vec<f64>> {
        let point = |id: u64| {
            self.points
                .get(&id)
                .copied()
                .ok_or_else(|| crate::Error::Invalid("Constraint references missing point".into()))
        };
        let line = |id: u64| match self.entities.get(&id) {
            Some(Entity::Line { a, b }) => Ok((point(*a)?, point(*b)?)),
            _ => invalid("Constraint references missing line"),
        };
        let circle = |id: u64| match self.entities.get(&id) {
            Some(Entity::Circle { center, radius }) if *radius > 0. && radius.is_finite() => {
                Ok((point(*center)?, *radius))
            }
            _ => invalid("Constraint references missing or invalid circle"),
        };
        let delta = |id: u64| -> Result<[f64; 2]> {
            let (a, b) = line(id)?;
            Ok([b[0] - a[0], b[1] - a[1]])
        };
        let length = |id: u64| -> Result<f64> {
            let d = delta(id)?;
            Ok(d[0].hypot(d[1]))
        };
        let mut residual = vec![];
        for c in self.constraints.values() {
            match c {
                Constraint::Fixed { point: id, at } => {
                    let p = point(*id)?;
                    residual.extend([p[0] - at[0], p[1] - at[1]]);
                }
                Constraint::Coincident { a, b } => {
                    let a = point(*a)?;
                    let b = point(*b)?;
                    residual.extend([a[0] - b[0], a[1] - b[1]]);
                }
                Constraint::Midpoint { point: p, line: l } => {
                    let p = point(*p)?;
                    let (a, b) = line(*l)?;
                    residual.extend([p[0] - (a[0] + b[0]) * 0.5, p[1] - (a[1] + b[1]) * 0.5]);
                }
                Constraint::PointOnLine { point: p, line: l } => {
                    let p = point(*p)?;
                    let (a, b) = line(*l)?;
                    let len = length(*l)?;
                    if len < EPS {
                        return invalid("Degenerate point-on-line dependency");
                    }
                    residual.push(
                        ((b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])) / len,
                    );
                }
                Constraint::Concentric { a, b } => {
                    let a = circle(*a)?.0;
                    let b = circle(*b)?.0;
                    residual.extend([a[0] - b[0], a[1] - b[1]]);
                }
                Constraint::Horizontal { line } => residual.push(delta(*line)?[1]),
                Constraint::Vertical { line } => residual.push(delta(*line)?[0]),
                Constraint::DistanceX {
                    a,
                    b,
                    expression,
                    sign,
                }
                | Constraint::DistanceY {
                    a,
                    b,
                    expression,
                    sign,
                } => {
                    let v = expression::length(expression, parameters)?;
                    if v <= EPS || ![-1., 1.].contains(sign) {
                        return invalid(
                            "Driving lengths must be positive; dimension orientation must be ±1",
                        );
                    }
                    let axis = usize::from(matches!(c, Constraint::DistanceY { .. }));
                    residual.push(point(*b)?[axis] - point(*a)?[axis] - v * sign);
                }
                Constraint::Length { line, expression } => {
                    residual.push(length(*line)? - expression::length(expression, parameters)?)
                }
                Constraint::Radius {
                    circle: id,
                    expression,
                } => residual.push(circle(*id)?.1 - expression::length(expression, parameters)?),
                Constraint::Equal { a, b } => {
                    let measure = |id| match self.entities.get(&id) {
                        Some(Entity::Circle { .. }) => Ok(circle(id)?.1),
                        _ => length(id),
                    };
                    if self.entities.get(a).map(std::mem::discriminant)
                        != self.entities.get(b).map(std::mem::discriminant)
                    {
                        return invalid("Equal requires two lines or two circles");
                    }
                    residual.push(measure(*a)? - measure(*b)?);
                }
                Constraint::Parallel { a, b } | Constraint::Perpendicular { a, b } => {
                    let da = delta(*a)?;
                    let db = delta(*b)?;
                    let scale = length(*a)? * length(*b)?;
                    if scale < EPS * EPS {
                        return invalid("Degenerate constrained line");
                    }
                    residual.push(if matches!(c, Constraint::Parallel { .. }) {
                        (da[0] * db[1] - da[1] * db[0]) / scale
                    } else {
                        (da[0] * db[0] + da[1] * db[1]) / scale
                    });
                }
                Constraint::Tangent {
                    line: id,
                    circle: cid,
                } => {
                    let (a, b) = line(*id)?;
                    let (center, radius) = circle(*cid)?;
                    let len = length(*id)?;
                    if len < EPS {
                        return invalid("Degenerate tangent line");
                    }
                    let d = ((b[0] - a[0]) * (center[1] - a[1])
                        - (b[1] - a[1]) * (center[0] - a[0]))
                        .abs()
                        / len;
                    residual.push(d - radius);
                }
            }
        }
        if residual.iter().any(|r| !r.is_finite()) {
            return invalid("Non-finite sketch residual");
        }
        Ok(residual)
    }
}
fn linear_solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Result<Vec<f64>> {
    let n = b.len();
    for col in 0..n {
        let pivot = (col..n)
            .max_by(|i, j| a[*i][col].abs().total_cmp(&a[*j][col].abs()))
            .unwrap();
        if a[pivot][col].abs() < 1e-15 {
            return invalid("Sketch solver singular normal system");
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        let diag = a[col][col];
        for j in col..n {
            a[col][j] /= diag;
        }
        b[col] /= diag;
        for row in 0..n {
            if row == col {
                continue;
            }
            let factor = a[row][col];
            for j in col..n {
                a[row][j] -= factor * a[col][j];
            }
            b[row] -= factor * b[col];
        }
    }
    Ok(b)
}
fn matrix_rank(mut a: Vec<Vec<f64>>, eps: f64) -> usize {
    let cols = a.first().map_or(0, Vec::len);
    let mut rank = 0;
    for col in 0..cols {
        let Some(pivot) =
            (rank..a.len()).max_by(|i, j| a[*i][col].abs().total_cmp(&a[*j][col].abs()))
        else {
            break;
        };
        if a[pivot][col].abs() < eps {
            continue;
        }
        a.swap(rank, pivot);
        let diag = a[rank][col];
        for j in col..cols {
            a[rank][j] /= diag;
        }
        for row in rank + 1..a.len() {
            let f = a[row][col];
            for j in col..cols {
                a[row][j] -= f * a[rank][j];
            }
        }
        rank += 1;
    }
    rank
}
impl crate::Sketch {
    pub fn with_line(&self, a: [f64; 2], b: [f64; 2], parameters: &Parameters) -> Result<Self> {
        if (a[0] - b[0]).hypot(a[1] - b[1]) < EPS {
            return invalid("Line length must be nonzero");
        }
        let mut graph = self.graph.clone().unwrap_or_default();
        if self.graph.is_none() {
            for (id, p) in self.profile_ids.iter().zip(&self.profiles) {
                graph.add_profile(*id, p)?;
            }
        }
        let mut endpoint = |p: [f64; 2]| -> Result<u64> {
            if let Some((id, _)) = graph
                .points
                .iter()
                .find(|(_, at)| (p[0] - at[0]).hypot(p[1] - at[1]) < EPS)
            {
                Ok(*id)
            } else {
                graph.point(p)
            }
        };
        let pa = endpoint(a)?;
        let pb = endpoint(b)?;
        let line = graph.entity(Entity::Line { a: pa, b: pb })?;
        if (a[0] - b[0]).abs() < EPS {
            graph.constrain(Constraint::Vertical { line })?;
        }
        if (a[1] - b[1]).abs() < EPS {
            graph.constrain(Constraint::Horizontal { line })?;
        }
        self.with_graph(graph, parameters)
    }
    pub fn with_dimensions(
        &self,
        profile_id: u64,
        text: BTreeMap<String, String>,
        parameters: &Parameters,
    ) -> Result<Self> {
        let mut candidate = self.clone();
        let mut graph = if let Some(g) = &self.graph {
            g.clone()
        } else {
            let mut graph = SketchGraph::default();
            for (id, p) in self.profile_ids.iter().zip(&self.profiles) {
                graph.add_profile(*id, p)?;
            }
            graph
        };
        graph.set_dimensions(profile_id, self.profile_by_id(profile_id)?, &text)?;
        candidate = candidate.with_graph(graph, parameters)?;
        candidate.dimension_expressions.insert(profile_id, text);
        candidate.regions()?;
        Ok(candidate)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rectangle_entities_dimensions_dof_and_conflict_rollback() {
        let p = Profile::Rectangle {
            a: [0., 0.],
            b: [40., 20.],
        };
        let mut graph = SketchGraph::default();
        graph.add_profile(1, &p).unwrap();
        assert_eq!(
            graph
                .solve(&Parameters::new())
                .unwrap()
                .1
                .degrees_of_freedom,
            4
        );
        graph
            .set_dimensions(
                1,
                &p,
                &BTreeMap::from([
                    ("width".into(), "2 * 40 mm".into()),
                    ("height".into(), "20".into()),
                ]),
            )
            .unwrap();
        let (solved, report) = graph.solve(&Parameters::new()).unwrap();
        assert_eq!(report.degrees_of_freedom, 0);
        let Profile::Rectangle { a, b } = solved.profile(1, &p).unwrap() else {
            panic!()
        };
        assert!((b[0] - a[0] - 80.).abs() < 1e-6);
        let mut conflict = solved.clone();
        let Entity::Line { a, b } = conflict.entities[&conflict.profile_entities[&1][0]] else {
            panic!()
        };
        conflict
            .constrain(Constraint::DistanceX {
                a,
                b,
                expression: "40".into(),
                sign: 1.,
            })
            .unwrap();
        let before = conflict.clone();
        assert!(conflict.solve(&Parameters::new()).is_err());
        assert_eq!(conflict, before);
    }
    #[test]
    fn independent_lines_coincidence_parallel_perpendicular_equal_radius_tangency() {
        let mut g = SketchGraph::default();
        let a = g.point([0., 0.]).unwrap();
        let b = g.point([10., 0.]).unwrap();
        let c = g.point([0., 5.]).unwrap();
        let d = g.point([10., 5.]).unwrap();
        let l1 = g.entity(Entity::Line { a, b }).unwrap();
        let l2 = g.entity(Entity::Line { a: c, b: d }).unwrap();
        let vertical = g.entity(Entity::Line { a, b: c }).unwrap();
        let center = g.point([5., 5.]).unwrap();
        let circle = g.entity(Entity::Circle { center, radius: 5. }).unwrap();
        for constraint in [
            Constraint::Horizontal { line: l1 },
            Constraint::Parallel { a: l1, b: l2 },
            Constraint::Perpendicular { a: l1, b: vertical },
            Constraint::Equal { a: l1, b: l2 },
            Constraint::Radius {
                circle,
                expression: "5 mm".into(),
            },
            Constraint::Tangent { line: l1, circle },
        ] {
            g.constrain(constraint).unwrap();
        }
        let (solved, report) = g.solve(&Parameters::new()).unwrap();
        assert!(report.maximum_residual_mm < 1e-7);
        assert!(report.degrees_of_freedom > 0);
        assert_eq!(g.points, solved.points);
        let duplicate = g.point([0., 0.]).unwrap();
        g.constrain(Constraint::Coincident { a, b: duplicate })
            .unwrap();
        assert!(g.solve(&Parameters::new()).is_ok());
    }
}
