//! Extent semantics resolved against analytic body bounds before kernel execution.
use crate::{
    Body, Document, Extrusion, Operation, Result, Sketch, expression, invalid,
    kernel::{ExtrudeInput, Geometry, SolidKernel},
};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Direction {
    #[default]
    OneSide,
    Symmetric,
    TwoSides,
}
impl Direction {
    pub fn label(self) -> &'static str {
        match self {
            Self::OneSide => "One Side",
            Self::Symmetric => "Symmetric",
            Self::TwoSides => "Two Sides",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Extent {
    #[default]
    Distance,
    ThroughAll,
}
impl Extent {
    pub fn label(self) -> &'static str {
        match self {
            Self::Distance => "Distance",
            Self::ThroughAll => "All",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExtrudeOptions {
    pub offset_start: bool,
    pub start_offset: String,
    pub direction: Direction,
    pub second_distance: String,
    pub extent: Extent,
}
impl Default for ExtrudeOptions {
    fn default() -> Self {
        Self {
            offset_start: false,
            start_offset: "0 mm".into(),
            direction: Direction::OneSide,
            second_distance: "10 mm".into(),
            extent: Extent::Distance,
        }
    }
}
impl ExtrudeOptions {
    pub fn validate(&self, d: &Document, distance: f64, operation: Operation) -> Result<()> {
        if self.offset_start {
            expression::length(&self.start_offset, &d.parameters)?;
        }
        if self.extent == Extent::ThroughAll && operation == Operation::NewBody {
            return invalid("Through All requires an explicit target body");
        }
        if self.extent == Extent::Distance
            && self.direction != Direction::OneSide
            && distance <= crate::EPS
        {
            return invalid("Symmetric/two-sided distance must be positive");
        }
        if self.extent == Extent::Distance
            && self.direction == Direction::TwoSides
            && expression::length(&self.second_distance, &d.parameters)? <= crate::EPS
        {
            return invalid("Second-side distance must be positive");
        }
        Ok(())
    }
}
impl Extrusion {
    pub(crate) fn tool(&self, d: &Document, s: &Sketch, bodies: &[Body]) -> Result<Geometry> {
        self.options.validate(d, self.distance, self.operation)?;
        let region = s.region(self.region_id.ok_or_else(|| {
            crate::Error::Invalid("Extrusion has no stable region reference".into())
        })?)?;
        let normal = s.plane.normal();
        let offset = if self.options.offset_start {
            expression::length(&self.options.start_offset, &d.parameters)?
        } else {
            0.
        };
        let (a, b) = if self.options.extent == Extent::ThroughAll {
            let target = bodies
                .iter()
                .find(|b| b.id == self.body_id)
                .ok_or_else(|| crate::Error::Invalid("Through All target is missing".into()))?;
            let bounds = target.solid.bounds();
            let axis = (0..3)
                .max_by(|a, b| normal[*a].abs().total_cmp(&normal[*b].abs()))
                .unwrap();
            let mut ends = [
                (bounds[0][axis] - s.origin[axis]) * normal[axis],
                (bounds[1][axis] - s.origin[axis]) * normal[axis],
            ];
            ends.sort_by(f64::total_cmp);
            // Avoid a coplanar terminal face. The authoritative result is still a B-rep Boolean.
            let margin = (ends[1] - ends[0]).abs().max(1.) * 1e-6 + crate::EPS * 10.;
            match self.options.direction {
                Direction::OneSide if self.distance > 0. => {
                    if ends[1] <= offset {
                        return invalid("Through All target is behind the extrusion direction");
                    }
                    (offset, ends[1] + margin)
                }
                Direction::OneSide => {
                    if ends[0] >= offset {
                        return invalid("Through All target is behind the extrusion direction");
                    }
                    (offset, ends[0] - margin)
                }
                _ => (offset.min(ends[0]) - margin, offset.max(ends[1]) + margin),
            }
        } else {
            match self.options.direction {
                Direction::OneSide => (offset, offset + self.distance),
                Direction::Symmetric => {
                    (offset - self.distance * 0.5, offset + self.distance * 0.5)
                }
                Direction::TwoSides => (
                    offset - expression::length(&self.options.second_distance, &d.parameters)?,
                    offset + self.distance,
                ),
            }
        };
        let origin = (cadrum::DVec3::from_array(s.origin) + normal * a).to_array();
        d.backend.extrude(&ExtrudeInput {
            feature_id: self.id,
            region_id: region.id,
            loop_ids: std::iter::once(region.outer)
                .chain(region.inner.iter().copied())
                .collect(),
            outer: s.profile_by_id(region.outer)?.clone(),
            holes: region
                .inner
                .iter()
                .map(|id| s.profile_by_id(*id).cloned())
                .collect::<Result<_>>()?,
            plane: s.plane,
            origin,
            distance: b - a,
        })
    }
}
