//! Stable regions for disjoint or strictly nested analytic loops.
//! Intersecting/touching arrangements are rejected until an arrangement solver exists.
use crate::{EPS, Profile, Result, Sketch, invalid};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    pub id: u64,
    pub outer: u64,
    pub inner: Vec<u64>,
}
#[derive(Debug, PartialEq)]
enum Relation {
    Contains,
    Inside,
    Disjoint,
    Ambiguous,
}
fn rectangle(p: &Profile) -> Option<([f64; 2], [f64; 2])> {
    if let Profile::Rectangle { a, b } = p {
        Some((
            [a[0].min(b[0]), a[1].min(b[1])],
            [a[0].max(b[0]), a[1].max(b[1])],
        ))
    } else {
        None
    }
}
fn relation(a: &Profile, b: &Profile) -> Relation {
    if let (Some((lo, hi)), Some((blo, bhi))) = (rectangle(a), rectangle(b)) {
        if (0..2).any(|i| hi[i] < blo[i] - EPS || bhi[i] < lo[i] - EPS) {
            return Relation::Disjoint;
        }
        if (0..2).all(|i| lo[i] < blo[i] - EPS && hi[i] > bhi[i] + EPS) {
            return Relation::Contains;
        }
        if (0..2).all(|i| blo[i] < lo[i] - EPS && bhi[i] > hi[i] + EPS) {
            return Relation::Inside;
        }
        return Relation::Ambiguous;
    }
    if let (
        Profile::Circle {
            center: a,
            radius: ar,
        },
        Profile::Circle {
            center: b,
            radius: br,
        },
    ) = (a, b)
    {
        let d = (a[0] - b[0]).hypot(a[1] - b[1]);
        return if d > ar + br + EPS {
            Relation::Disjoint
        } else if d + br < ar - EPS {
            Relation::Contains
        } else if d + ar < br - EPS {
            Relation::Inside
        } else {
            Relation::Ambiguous
        };
    }
    if let (Some((lo, hi)), Profile::Circle { center, radius }) = (rectangle(a), b) {
        if (0..2).all(|i| center[i] - radius > lo[i] + EPS && center[i] + radius < hi[i] - EPS) {
            return Relation::Contains;
        }
        if [lo, [hi[0], lo[1]], hi, [lo[0], hi[1]]]
            .iter()
            .all(|p| (p[0] - center[0]).hypot(p[1] - center[1]) < radius - EPS)
        {
            return Relation::Inside;
        }
        let nearest = [center[0].clamp(lo[0], hi[0]), center[1].clamp(lo[1], hi[1])];
        if (center[0] - nearest[0]).hypot(center[1] - nearest[1]) > radius + EPS {
            return Relation::Disjoint;
        }
        return Relation::Ambiguous;
    }
    if matches!(a, Profile::Circle { .. }) && rectangle(b).is_some() {
        return match relation(b, a) {
            Relation::Contains => Relation::Inside,
            Relation::Inside => Relation::Contains,
            r => r,
        };
    }
    if let Profile::Circle { center, radius } = b {
        let points = a.outline(0);
        let mut clearance = f64::INFINITY;
        for i in 0..points.len() {
            let p = points[i];
            let q = points[(i + 1) % points.len()];
            let dx = q[0] - p[0];
            let dy = q[1] - p[1];
            let t = (((center[0] - p[0]) * dx + (center[1] - p[1]) * dy) / (dx * dx + dy * dy))
                .clamp(0., 1.);
            let nearest = (p[0] + dx * t - center[0]).hypot(p[1] + dy * t - center[1]);
            let farthest = (p[0] - center[0])
                .hypot(p[1] - center[1])
                .max((q[0] - center[0]).hypot(q[1] - center[1]));
            if nearest <= radius + EPS && farthest >= radius - EPS {
                return Relation::Ambiguous;
            }
            clearance = clearance.min(nearest);
        }
        if point_inside(*center, &points) && clearance > radius + EPS {
            return Relation::Contains;
        }
        if points
            .iter()
            .all(|p| (p[0] - center[0]).hypot(p[1] - center[1]) < radius - EPS)
        {
            return Relation::Inside;
        }
        return Relation::Disjoint;
    }
    if matches!(a, Profile::Circle { .. }) {
        return match relation(b, a) {
            Relation::Contains => Relation::Inside,
            Relation::Inside => Relation::Contains,
            r => r,
        };
    }
    let a = a.outline(0);
    let b = b.outline(0);
    for i in 0..a.len() {
        for j in 0..b.len() {
            if segments_touch(a[i], a[(i + 1) % a.len()], b[j], b[(j + 1) % b.len()]) {
                return Relation::Ambiguous;
            }
        }
    }
    if point_inside(b[0], &a) {
        Relation::Contains
    } else if point_inside(a[0], &b) {
        Relation::Inside
    } else {
        Relation::Disjoint
    }
}
fn point_inside(p: [f64; 2], polygon: &[[f64; 2]]) -> bool {
    let mut inside = false;
    for i in 0..polygon.len() {
        let a = polygon[i];
        let b = polygon[(i + 1) % polygon.len()];
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
    }
    inside
}
fn segments_touch(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    }
    fn on(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> bool {
        cross(a, b, p).abs() <= EPS * (b[0] - a[0]).hypot(b[1] - a[1])
            && (0..2).all(|i| p[i] >= a[i].min(b[i]) - EPS && p[i] <= a[i].max(b[i]) + EPS)
    }
    (cross(a, b, c) * cross(a, b, d) < 0. && cross(c, d, a) * cross(c, d, b) < 0.)
        || on(a, b, c)
        || on(a, b, d)
        || on(c, d, a)
        || on(c, d, b)
}
impl Sketch {
    pub fn profile_by_id(&self, id: u64) -> Result<&Profile> {
        self.profile_ids
            .iter()
            .position(|p| *p == id)
            .and_then(|i| self.profiles.get(i))
            .ok_or_else(|| crate::Error::Invalid("Missing stable profile reference".into()))
    }
    pub fn add_profile(&mut self, profile: Profile) -> Result<u64> {
        profile.validate()?;
        let id = self.next_profile_id;
        let next = id
            .checked_add(1)
            .ok_or_else(|| crate::Error::Invalid("Sketch entity ID space is exhausted".into()))?;
        let mut graph = self.graph.clone().unwrap_or_default();
        if self.graph.is_none() {
            for (id, p) in self.profile_ids.iter().zip(&self.profiles) {
                graph.add_profile(*id, p)?;
            }
        }
        graph.add_profile(id, &profile)?;
        self.graph = Some(graph);
        self.profiles.push(profile);
        self.profile_ids.push(id);
        self.next_profile_id = next;
        Ok(id)
    }
    pub fn regions(&self) -> Result<Vec<Region>> {
        if self.profile_ids.len() != self.profiles.len() {
            return invalid("Profile identity array does not match geometry");
        }
        let n = self.profiles.len();
        let mut contains = vec![vec![false; n]; n];
        for i in 0..n {
            for j in i + 1..n {
                match relation(&self.profiles[i], &self.profiles[j]) {
                    Relation::Contains => contains[i][j] = true,
                    Relation::Inside => contains[j][i] = true,
                    Relation::Disjoint => {}
                    Relation::Ambiguous => {
                        return invalid(
                            "Touching/intersecting or polygon loop arrangements are unsupported; selection is ambiguous",
                        );
                    }
                }
            }
        }
        Ok((0..n)
            .map(|i| Region {
                id: self.profile_ids[i],
                outer: self.profile_ids[i],
                inner: (0..n)
                    .filter(|j| {
                        contains[i][*j] && !(0..n).any(|k| contains[i][k] && contains[k][*j])
                    })
                    .map(|j| self.profile_ids[j])
                    .collect(),
            })
            .collect())
    }
    pub fn region(&self, id: u64) -> Result<Region> {
        self.regions()?
            .into_iter()
            .find(|r| r.id == id)
            .ok_or_else(|| {
                crate::Error::Invalid("Extrusion references a missing stable region".into())
            })
    }
}
