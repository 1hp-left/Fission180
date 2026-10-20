//! Stable analytic regions with exact nesting and certified arrangement adjacency.
use crate::{Profile, Result, Sketch, invalid};
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
fn relation(a: &Profile, b: &Profile) -> Relation {
    for x in a.boundary() {
        for y in b.boundary() {
            match crate::boundary::intersections(x.curve, y.curve) {
                Ok(hits) if hits.is_empty() => {}
                _ => return Relation::Ambiguous,
            }
        }
    }
    if a.contains_point(b.boundary()[0].curve.point(0.)) {
        Relation::Contains
    } else if b.contains_point(a.boundary()[0].curve.point(0.)) {
        Relation::Inside
    } else {
        Relation::Disjoint
    }
}
/// Strict analytic containment used by extrusion and contained-cut validation.
pub(crate) fn strictly_contains(a: &Profile, b: &Profile) -> bool {
    relation(a, b) == Relation::Contains
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
        let mut candidate = self.clone();
        candidate.profiles.push(profile);
        candidate.profile_ids.push(id);
        candidate.next_profile_id = next;
        *self = candidate.refresh_line_regions(graph)?;
        Ok(id)
    }
    pub fn regions(&self) -> Result<Vec<Region>> {
        if self.profile_ids.len() != self.profiles.len() {
            return invalid("Profile identity array does not match geometry");
        }
        let active: Vec<_> = (0..self.profiles.len())
            .filter(|i| {
                self.graph.as_ref().is_none_or(|g| {
                    g.profile_entities
                        .get(&self.profile_ids[*i])
                        .is_none_or(|edges| !edges.iter().any(|id| g.construction.contains(id)))
                })
            })
            .collect();
        let n = active.len();
        let mut contains = vec![vec![false; n]; n];
        for i in 0..n {
            for j in i + 1..n {
                match relation(&self.profiles[active[i]], &self.profiles[active[j]]) {
                    Relation::Contains => contains[i][j] = true,
                    Relation::Inside => contains[j][i] = true,
                    Relation::Disjoint => {}
                    Relation::Ambiguous => {
                        // Verified cells of one line arrangement can share edges
                        // or vertices. Their interiors cannot overlap.
                        let certified = self.graph.as_ref().is_some_and(|g| {
                            (g.region_cycles.contains_key(&self.profile_ids[active[i]])
                                || g.curve_regions.contains_key(&self.profile_ids[active[i]]))
                                && (g.region_cycles.contains_key(&self.profile_ids[active[j]])
                                    || g.curve_regions.contains_key(&self.profile_ids[active[j]]))
                        });
                        if certified {
                            continue;
                        }
                        return invalid(
                            "Touching/intersecting or polygon loop arrangements are unsupported; selection is ambiguous",
                        );
                    }
                }
            }
        }
        Ok((0..n)
            .map(|i| Region {
                id: self.profile_ids[active[i]],
                outer: self.profile_ids[active[i]],
                inner: (0..n)
                    .filter(|j| {
                        contains[i][*j] && !(0..n).any(|k| contains[i][k] && contains[k][*j])
                    })
                    .map(|j| self.profile_ids[active[j]])
                    .collect(),
            })
            .collect())
    }
    pub fn region(&self, id: u64) -> Result<Region> {
        self.regions()?
            .into_iter()
            .find(|r| r.id == id)
            .ok_or_else(|| {
                crate::Error::Invalid("Missing stable region or unsupported curved region boundary; no alternate region was selected".into())
            })
    }
}
