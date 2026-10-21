//! Axial interval and display patch helpers. No modeling classification uses meshes.
use super::*;
pub(super) fn interval(input: &ExtrudeInput) -> [f64; 2] {
    let start = dot(input.origin, input.plane.normal().to_array());
    [
        start.min(start + input.distance),
        start.max(start + input.distance),
    ]
}
pub(super) fn append_mesh(
    mesh: &mut DisplayMesh,
    part: DisplayMesh,
    include: impl Fn(TopologyId) -> bool,
    invert: bool,
    edges: bool,
) {
    for (index, tri) in part.indices.chunks_exact(3).enumerate() {
        if let Some(id) = part.face_ids[index].filter(|id| include(*id)) {
            let normal = part.normals[tri[0]].map(|n| n * if invert { -1. } else { 1. });
            triangle(
                mesh,
                [
                    part.vertices[tri[0]],
                    part.vertices[tri[1]],
                    part.vertices[tri[2]],
                ],
                normal,
                id,
            );
        }
    }
    if edges {
        mesh.edges.extend(part.edges);
        mesh.edge_ids.extend(part.edge_ids);
    }
}
