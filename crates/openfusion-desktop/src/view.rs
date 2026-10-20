use eframe::egui::{self, Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke, Vec2, pos2, vec2};
use fission180_core::kernel::native::TopologyId;
use fission180_core::{DisplayMesh, Plane};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    Face(TopologyId),
    Edge(TopologyId),
    Body(u64),
}
pub fn planar_surface(
    camera: Camera,
    r: Rect,
    source: &DisplayMesh,
    at: Pos2,
) -> Option<(Plane, [f64; 3], u64)> {
    let mut result: Option<(f64, Plane, [f64; 3], u64)> = None;
    let mut front_depth = f64::NEG_INFINITY;
    for (index, ids) in source.indices.chunks_exact(3).enumerate() {
        let points = [ids[0], ids[1], ids[2]].map(|i| camera.project(source.vertices[i], r));
        let v0 = points[1].0 - points[0].0;
        let v1 = points[2].0 - points[0].0;
        let v2 = at - points[0].0;
        let det = v0.x * v1.y - v1.x * v0.y;
        if det.abs() < 1e-8 {
            continue;
        }
        let u = (v2.x * v1.y - v1.x * v2.y) / det;
        let v = (v0.x * v2.y - v2.x * v0.y) / det;
        if u < 0. || v < 0. || u + v > 1. {
            continue;
        }
        let depth =
            points[0].1 * (1. - u - v) as f64 + points[1].1 * u as f64 + points[2].1 * v as f64;
        if depth < front_depth {
            continue;
        }
        front_depth = depth;
        result = None;
        let normal = source.normals.get(ids[0])?;
        let axis = (0..3).max_by(|a, b| normal[*a].abs().total_cmp(&normal[*b].abs()))?;
        if normal[axis].abs() < 1. - 1e-8 {
            continue;
        }
        let coordinate = source.vertices[ids[0]][axis];
        if ids
            .iter()
            .any(|id| (source.vertices[*id][axis] - coordinate).abs() > 1e-5)
        {
            continue;
        }
        let plane = match axis {
            0 => Plane::YZ,
            1 => Plane::XZ,
            _ => Plane::XY,
        };
        let mut origin = [0.; 3];
        origin[axis] = coordinate;
        if result.as_ref().is_none_or(|t| depth > t.0) {
            result = Some((depth, plane, origin, *source.body_ids.get(index)?));
        }
    }
    result.map(|(_, plane, origin, body)| (plane, origin, body))
}

pub fn pick(camera: Camera, r: Rect, source: &DisplayMesh, at: Pos2) -> Option<Pick> {
    let vertices: Vec<_> = source
        .vertices
        .iter()
        .map(|p| camera.project(*p, r))
        .collect();
    let mut nearest: Option<(f64, Option<TopologyId>, u64)> = None;
    for (index, ids) in source.indices.chunks_exact(3).enumerate() {
        let [a, b, c] = [vertices[ids[0]], vertices[ids[1]], vertices[ids[2]]];
        let v0 = b.0 - a.0;
        let v1 = c.0 - a.0;
        let v2 = at - a.0;
        let det = v0.x * v1.y - v1.x * v0.y;
        if det.abs() < 1e-8 {
            continue;
        }
        let u = (v2.x * v1.y - v1.x * v2.y) / det;
        let v = (v0.x * v2.y - v2.x * v0.y) / det;
        if u >= 0. && v >= 0. && u + v <= 1. {
            let depth = a.1 * (1. - u - v) as f64 + b.1 * u as f64 + c.1 * v as f64;
            if nearest.as_ref().is_none_or(|n| depth > n.0) {
                nearest = Some((
                    depth,
                    source.face_ids.get(index).copied().flatten(),
                    source.body_ids.get(index).copied().unwrap_or(0),
                ));
            }
        }
    }
    let mut edge_hit: Option<(f32, TopologyId)> = None;
    if source.edge_ids.len() * 2 == source.edges.len() {
        for (segment, id) in source.edges.chunks_exact(2).zip(&source.edge_ids) {
            let a = camera.project(segment[0], r);
            let b = camera.project(segment[1], r);
            let delta = b.0 - a.0;
            let t = ((at - a.0).dot(delta) / delta.length_sq().max(1e-12)).clamp(0., 1.);
            let distance = (at - (a.0 + delta * t)).length();
            let depth = a.1 + (b.1 - a.1) * t as f64;
            if distance <= 5.
                && nearest.as_ref().is_none_or(|n| depth >= n.0 - 0.05)
                && edge_hit.as_ref().is_none_or(|e| distance < e.0)
            {
                edge_hit = Some((distance, *id));
            }
        }
    }
    edge_hit.map(|(_, id)| Pick::Edge(id)).or_else(|| {
        nearest.and_then(|(_, id, body)| {
            id.map(Pick::Face)
                .or_else(|| (body != 0).then_some(Pick::Body(body)))
        })
    })
}
#[derive(Clone, Copy, PartialEq)]
pub struct Camera {
    pub yaw: f64,
    pub elevation: f64,
    pub scale: f32,
    pub pan: Vec2,
    pub top: bool,
    pub plane: Plane,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            yaw: std::f64::consts::FRAC_PI_4,
            elevation: 0.61547970867,
            scale: 3.8,
            pan: Vec2::ZERO,
            top: false,
            plane: Plane::XY,
        }
    }
}
impl Camera {
    pub fn origin(&self, r: Rect) -> Pos2 {
        r.center() + self.pan
    }
    pub fn project(&self, p: [f64; 3], r: Rect) -> (Pos2, f64) {
        let (x, y, z) = (p[0], p[1], p[2]);
        if self.top {
            let (a, b, d) = match self.plane {
                Plane::XY => (x, y, z),
                Plane::XZ => (x, z, -y),
                Plane::YZ => (y, z, x),
            };
            return (
                self.origin(r) + vec2(a as f32 * self.scale, -b as f32 * self.scale),
                d,
            );
        }
        let (s, c) = self.yaw.sin_cos();
        let (se, ce) = self.elevation.sin_cos();
        let right = x * c + y * s;
        let up = -x * s * se + y * c * se + z * ce;
        let depth = x * s * ce - y * c * ce + z * se;
        (
            self.origin(r) + vec2(right as f32 * self.scale, -up as f32 * self.scale),
            depth,
        )
    }
    pub fn unproject(&self, p: Pos2, r: Rect) -> [f64; 2] {
        let delta = (p - self.origin(r)) / self.scale;
        [delta.x as f64, -delta.y as f64]
    }
    pub fn grid(&self, p: &Painter, r: Rect) {
        let p = p.with_clip_rect(r);
        let extent = (r.width().max(r.height()) / self.scale * 2.).max(500.) as f64;
        let step = if self.scale > 2. { 10. } else { 50. };
        let count = (extent / step) as i32;
        for i in -count..=count {
            let value = i as f64 * step;
            let major = i % 5 == 0;
            let col = if major {
                Color32::from_rgb(98, 105, 116)
            } else {
                Color32::from_rgb(78, 85, 97)
            };
            let w = if major { 0.9 } else { 0.75 };
            for (a, b) in [
                ([-extent, value], [extent, value]),
                ([value, -extent], [value, extent]),
            ] {
                let a = self.plane.point(a, [0.; 3]).to_array();
                let b = self.plane.point(b, [0.; 3]).to_array();
                p.line_segment(
                    [self.project(a, r).0, self.project(b, r).0],
                    Stroke::new(w, col),
                );
            }
        }
        let x = self.plane.point([extent, 0.], [0.; 3]).to_array();
        let nx = self.plane.point([-extent, 0.], [0.; 3]).to_array();
        let y = self.plane.point([0., extent], [0.; 3]).to_array();
        let ny = self.plane.point([0., -extent], [0.; 3]).to_array();
        p.line_segment(
            [self.project(nx, r).0, self.project(x, r).0],
            Stroke::new(1., Color32::from_rgb(119, 51, 70)),
        );
        p.line_segment(
            [self.project(ny, r).0, self.project(y, r).0],
            Stroke::new(1., Color32::from_rgb(50, 104, 59)),
        );
        if self.top {
            for i in -20..=20 {
                if i == 0 {
                    continue;
                }
                let n = i as f64 * 50.;
                let a = self
                    .project(self.plane.point([n, 0.], [0.; 3]).to_array(), r)
                    .0;
                let b = self
                    .project(self.plane.point([0., n], [0.; 3]).to_array(), r)
                    .0;
                for at in [a + vec2(4., 7.), b + vec2(14., -14.)] {
                    if r.contains(at) {
                        let galley = p.layout_no_wrap(
                            format!("{n:.0}"),
                            egui::FontId::proportional(14.),
                            Color32::from_gray(178),
                        );
                        p.add(
                            egui::epaint::TextShape::new(at, galley, Color32::from_gray(178))
                                .with_angle(std::f32::consts::FRAC_PI_2),
                        );
                    }
                }
            }
        }
    }
}

pub struct RenderCache {
    key: Option<(Camera, Rect, u64, Option<Pick>, Option<Pick>)>,
    mesh: Mesh,
    edges: Vec<[Pos2; 2]>,
}
impl Default for RenderCache {
    fn default() -> Self {
        Self {
            key: None,
            mesh: Mesh::default(),
            edges: vec![],
        }
    }
}
impl RenderCache {
    pub fn paint(
        &mut self,
        p: &Painter,
        r: Rect,
        camera: Camera,
        source: &DisplayMesh,
        revision: u64,
        preview: bool,
        selected: Option<Pick>,
        hovered: Option<Pick>,
    ) {
        if self.key != Some((camera, r, revision, selected, hovered)) {
            self.key = Some((camera, r, revision, selected, hovered));
            self.mesh = Mesh::default();
            self.edges.clear();
            let vertices: Vec<_> = source
                .vertices
                .iter()
                .map(|&v| camera.project(v, r))
                .collect();
            let mut triangles: Vec<_> = source
                .indices
                .chunks_exact(3)
                .enumerate()
                .map(|(index, t)| {
                    (
                        [t[0], t[1], t[2]],
                        (vertices[t[0]].1 + vertices[t[1]].1 + vertices[t[2]].1) / 3.,
                        source.face_ids.get(index).copied().flatten(),
                    )
                })
                .collect();
            triangles.sort_by(|a, b| a.1.total_cmp(&b.1));
            let mut visible = vec![];
            for (ids, _, face) in triangles {
                let a = vertices[ids[0]].0;
                let b = vertices[ids[1]].0;
                let c = vertices[ids[2]].0;
                let cross = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
                if cross >= 0. {
                    continue;
                }
                let n = source.normals.get(ids[0]).copied().unwrap_or([0., 0., 1.]);
                let light = (n[0] * 0.3 - n[1] * 0.4 + n[2] * 0.85).max(-0.3);
                let shade = (155. + light * 65.) as u8;
                let color = if face.is_some_and(|id| selected == Some(Pick::Face(id))) {
                    Color32::from_rgb(57, 137, 181)
                } else if face.is_some_and(|id| hovered == Some(Pick::Face(id))) {
                    Color32::from_rgb(135, 186, 210)
                } else if preview {
                    Color32::from_rgb(shade / 2, shade.saturating_sub(20), shade)
                } else {
                    Color32::from_rgb(shade, shade.saturating_add(3), shade.saturating_add(8))
                };
                let offset = self.mesh.vertices.len() as u32;
                for point in [a, b, c] {
                    self.mesh.colored_vertex(point, color);
                }
                self.mesh.indices.extend([offset, offset + 1, offset + 2]);
                visible.push((ids, [a, b, c]));
            }
            let segments: Vec<_> =
                if !source.edge_ids.is_empty() && source.edges.len() == source.edge_ids.len() * 2 {
                    source.edges.chunks_exact(2).map(|s| [s[0], s[1]]).collect()
                } else {
                    source.edges.windows(2).map(|s| [s[0], s[1]]).collect()
                };
            for segment in segments {
                if !segment[0]
                    .iter()
                    .chain(segment[1].iter())
                    .all(|n| n.is_finite())
                {
                    continue;
                }
                let a = camera.project(segment[0], r);
                let b = camera.project(segment[1], r);
                let mid = pos2((a.0.x + b.0.x) * 0.5, (a.0.y + b.0.y) * 0.5);
                let depth = (a.1 + b.1) * 0.5;
                let hidden = visible.iter().any(|(ids, ps)| {
                    let v0 = ps[1] - ps[0];
                    let v1 = ps[2] - ps[0];
                    let v2 = mid - ps[0];
                    let det = v0.x * v1.y - v1.x * v0.y;
                    if det.abs() < 0.001 {
                        return false;
                    }
                    let u = (v2.x * v1.y - v1.x * v2.y) / det;
                    let v = (v0.x * v2.y - v2.x * v0.y) / det;
                    u >= -0.0001
                        && v >= -0.0001
                        && u + v <= 1.0001
                        && vertices[ids[0]].1 * (1. - u - v) as f64
                            + vertices[ids[1]].1 * u as f64
                            + vertices[ids[2]].1 * v as f64
                            > depth + 0.02
                });
                if !hidden {
                    self.edges.push([a.0, b.0]);
                }
            }
        }
        p.add(Shape::mesh(self.mesh.clone()));
        for e in &self.edges {
            p.line_segment(*e, Stroke::new(1., Color32::from_rgb(47, 55, 64)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nearest_rust_face_and_edge_pick_preserves_stable_body_identity() {
        use fission180_core::{
            Profile,
            kernel::{ExtrudeInput, native::NativeSolid},
        };
        let mut solid = NativeSolid::extrude(ExtrudeInput {
            feature_id: 2,
            region_id: 1,
            loop_ids: vec![1],
            outer: Profile::Rectangle {
                a: [0., 0.],
                b: [40., 30.],
            },
            holes: vec![],
            plane: Plane::XY,
            origin: [0.; 3],
            distance: 5.,
        })
        .unwrap();
        solid.bind_body(3);
        let mesh = solid.mesh(32).unwrap();
        let r = Rect::from_min_size(Pos2::ZERO, vec2(1536., 674.));
        let camera = Camera {
            top: true,
            ..Default::default()
        };
        let at = camera.project([20., 15., 5.], r).0;
        let Some(Pick::Face(face)) = pick(camera, r, &mesh, at) else {
            panic!("Expected top face")
        };
        assert_eq!(face.local, 1);
        assert_eq!(face.body, 3);
        let at = camera.project([20., 0., 5.], r).0;
        let Some(Pick::Edge(edge)) = pick(camera, r, &mesh, at) else {
            panic!("Expected visible edge")
        };
        assert_eq!(edge.body, 3);
        assert!(pick(camera, r, &mesh, pos2(1., 1.)).is_none());
    }
    #[test]
    fn plane_projection_round_trip() {
        let r = Rect::from_min_size(Pos2::ZERO, vec2(1536., 674.));
        for plane in [Plane::XY, Plane::XZ, Plane::YZ] {
            let c = Camera {
                top: true,
                plane,
                ..Default::default()
            };
            let local = [25., -13.];
            let p = c.project(plane.point(local, [0.; 3]).to_array(), r).0;
            let result = c.unproject(p, r);
            assert!((result[0] - local[0]).abs() < 1e-5);
            assert!((result[1] - local[1]).abs() < 1e-5);
        }
    }
}
