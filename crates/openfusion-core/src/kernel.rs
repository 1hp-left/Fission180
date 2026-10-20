//! Explicit kernel boundary. Backend selection never triggers a fallback.
use crate::{DisplayMesh, Operation, Plane, Profile, Result, invalid};
use cadrum::{Solid, Tessellation};
use serde::{Deserialize, Serialize};

pub mod native;
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Backend {
    #[default]
    Occt,
    Rust,
}
impl Backend {
    pub fn label(self) -> &'static str {
        match self {
            Self::Occt => "OCCT",
            Self::Rust => "Rust analytic",
        }
    }
}
#[derive(Debug, Clone)]
pub struct ExtrudeInput {
    pub feature_id: u64,
    pub region_id: u64,
    pub outer: Profile,
    pub holes: Vec<Profile>,
    pub loop_ids: Vec<u64>,
    pub plane: Plane,
    pub origin: [f64; 3],
    pub distance: f64,
}
pub enum Geometry {
    Occt(Solid),
    Rust(native::NativeSolid),
}
pub trait SolidKernel {
    fn extrude(&self, input: &ExtrudeInput) -> Result<Geometry>;
    fn boolean(
        &self,
        target: &Geometry,
        tool: &Geometry,
        operation: Operation,
        feature: u64,
    ) -> Result<Vec<Geometry>>;
}
impl SolidKernel for Backend {
    fn extrude(&self, input: &ExtrudeInput) -> Result<Geometry> {
        match self {
            Self::Occt => {
                let mut edges = input.outer.edges(input.plane, input.origin)?;
                for hole in &input.holes {
                    edges.extend(hole.edges(input.plane, input.origin)?);
                }
                Ok(Geometry::Occt(Solid::extrude(
                    &edges,
                    input.plane.normal() * input.distance,
                )?))
            }
            Self::Rust => Ok(Geometry::Rust(native::NativeSolid::extrude(input.clone())?)),
        }
    }
    fn boolean(
        &self,
        target: &Geometry,
        tool: &Geometry,
        operation: Operation,
        feature: u64,
    ) -> Result<Vec<Geometry>> {
        match (self, target, tool) {
            (Self::Occt, Geometry::Occt(a), Geometry::Occt(b)) => {
                let solids = match operation {
                    Operation::Join => (a + b).build_vec(),
                    Operation::Cut => (a - b).build_vec(),
                    Operation::Intersect => (a * b).build_vec(),
                    _ => return invalid("New Body is not a Boolean operation"),
                }?;
                Ok(solids.into_iter().map(Geometry::Occt).collect())
            }
            (Self::Rust, Geometry::Rust(a), Geometry::Rust(b)) => {
                Ok(native::boolean(a, b, operation, feature)?
                    .into_iter()
                    .map(Geometry::Rust)
                    .collect())
            }
            _ => invalid("Kernel/backend mismatch; implicit conversion and fallback are forbidden"),
        }
    }
}
impl Geometry {
    pub(crate) fn output_cmp(&self, other: &Self) -> std::cmp::Ordering {
        if let (Self::Rust(a), Self::Rust(b)) = (self, other) {
            if !a.uses_legacy_output_order() && !b.uses_legacy_output_order() {
                if let (Some(a), Some(b)) = (a.output_signature(), b.output_signature()) {
                    return a.cmp(&b);
                }
            }
        }
        self.bounds()
            .into_iter()
            .flatten()
            .zip(other.bounds().into_iter().flatten())
            .map(|(a, b)| a.total_cmp(&b))
            .find(|o| !o.is_eq())
            .unwrap_or(std::cmp::Ordering::Equal)
    }
    pub fn backend(&self) -> Backend {
        match self {
            Self::Occt(_) => Backend::Occt,
            Self::Rust(_) => Backend::Rust,
        }
    }
    pub fn volume(&self) -> f64 {
        match self {
            Self::Occt(s) => s.volume(),
            Self::Rust(s) => s.volume(),
        }
    }
    pub fn area(&self) -> f64 {
        match self {
            Self::Occt(s) => s.area(),
            Self::Rust(s) => s.area(),
        }
    }
    pub fn bounds(&self) -> [[f64; 3]; 2] {
        match self {
            Self::Occt(s) => s.bounding_box().map(|v| v.to_array()),
            Self::Rust(s) => s.bounds(),
        }
    }
    pub fn mesh(&self) -> Result<DisplayMesh> {
        match self {
            Self::Occt(s) => {
                let mesh = Solid::mesh([s], Tessellation::default())?;
                let count = mesh.indices.len() / 3;
                Ok(DisplayMesh {
                    vertices: mesh.vertices.iter().map(|v| v.to_array()).collect(),
                    normals: mesh.normals.iter().map(|v| v.to_array()).collect(),
                    indices: mesh.indices,
                    edges: mesh.edges.iter().map(|v| v.to_array()).collect(),
                    face_ids: vec![None; count],
                    body_ids: vec![],
                    edge_ids: vec![],
                })
            }
            Self::Rust(s) => s.mesh(96),
        }
    }
}
