//! Photo-referenced product mesh shared with the editable Blender and GLB assets.
use crate::theme::{MINT, TEXT};
use eframe::egui::*;
use serde::Deserialize;
use std::sync::{Arc, OnceLock};

#[derive(Clone)]
pub struct Orbit {
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    cache: Option<RenderCache>,
}
impl Default for Orbit {
    fn default() -> Self {
        Self {
            yaw: 0.95,
            pitch: -0.50,
            zoom: 1.0,
            cache: None,
        }
    }
}
#[derive(Deserialize)]
struct Model {
    parts: Vec<Part>,
    hotspots: [[f32; 3]; 5],
}
#[derive(Deserialize)]
struct Part {
    color: [f32; 3],
    metallic: f32,
    vertices: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    triangles: Vec<[usize; 3]>,
}
fn model() -> &'static Model {
    static MODEL: OnceLock<Model> = OnceLock::new();
    MODEL.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/models/mx-anywhere-3s.mesh.json"))
            .expect("validated bundled mouse mesh")
    })
}
fn rotated(p: [f32; 3], o: &Orbit) -> [f32; 3] {
    let (sy, cy) = o.yaw.sin_cos();
    let (sp, cp) = o.pitch.sin_cos();
    let x = p[0] * cy - p[2] * sy;
    let z = p[0] * sy + p[2] * cy;
    [x, p[1] * cp - z * sp, p[1] * sp + z * cp]
}
fn project(p: [f32; 3], rect: Rect, o: &Orbit) -> (Pos2, f32) {
    let p = rotated([p[0], p[1] - 0.05, p[2]], o);
    let scale = rect.height().min(rect.width() * 0.7) * 0.40 * o.zoom;
    // Orthographic product view keeps the silhouette and hit testing consistent.
    (rect.center() + vec2(-p[0] * scale, -p[1] * scale), p[2])
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}
fn lit(color: [f32; 3], normal: [f32; 3], metallic: f32) -> Color32 {
    let normal = [-normal[0], normal[1], normal[2]];
    let key = dot(normal, [-0.40, 0.65, -0.65]).max(0.0);
    let fill = dot(normal, [0.8, 0.2, -0.55]).max(0.0);
    let rim = dot(normal, [0.2, 0.6, 0.77]).max(0.0).powi(3);
    let specular = dot(normal, [-0.22, 0.36, -0.907]).max(0.0).powf(38.0) * (0.06 + metallic * 0.5);
    let rgb = color.map(|c| {
        let linear = c * (0.40 + 1.25 * key + 0.25 * fill + 0.45 * rim) + specular;
        (linear.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8
    });
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}
struct Triangle {
    points: [Pos2; 3],
    depths: [f32; 3],
    colors: [Color32; 3],
}
#[derive(Clone)]
struct RenderCache {
    key: [u32; 5],
    texture: TextureHandle,
    depth: Arc<Vec<f32>>,
    size: [usize; 2],
}

/// Rasterize with per-pixel depth: the center panel overlaps a curved shell,
/// so sorting entire triangles by their average depth is insufficient.
fn rasterize(triangles: &[Triangle], size: [usize; 2]) -> (ColorImage, Vec<f32>) {
    let mut image = ColorImage::filled(size, Color32::TRANSPARENT);
    let mut depth = vec![f32::INFINITY; size[0] * size[1]];
    let cross = |u: Vec2, v: Vec2| u.x * v.y - u.y * v.x;
    for triangle in triangles {
        let [a, b, c] = triangle.points;
        let area = cross(b - a, c - a);
        if area.abs() < 0.0001 {
            continue;
        }
        let min_x = a.x.min(b.x).min(c.x).floor().max(0.0) as usize;
        let min_y = a.y.min(b.y).min(c.y).floor().max(0.0) as usize;
        let max_x = (a.x.max(b.x).max(c.x).ceil().max(0.0) as usize).min(size[0]);
        let max_y = (a.y.max(b.y).max(c.y).ceil().max(0.0) as usize).min(size[1]);
        for y in min_y..max_y {
            for x in min_x..max_x {
                let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
                let u = cross(b - p, c - p) / area;
                let v = cross(c - p, a - p) / area;
                let w = 1.0 - u - v;
                if u < -0.00001 || v < -0.00001 || w < -0.00001 {
                    continue;
                }
                let z = dot([u, v, w], triangle.depths);
                let at = y * size[0] + x;
                if z >= depth[at] {
                    continue;
                }
                depth[at] = z;
                let channel = |i| {
                    (triangle.colors[0][i] as f32 * u
                        + triangle.colors[1][i] as f32 * v
                        + triangle.colors[2][i] as f32 * w)
                        .clamp(0.0, 255.0) as u8
                };
                image.pixels[at] = Color32::from_rgb(channel(0), channel(1), channel(2));
            }
        }
    }
    (image, depth)
}

fn render(ui: &Ui, rect: Rect, orbit: &mut Orbit) {
    let pixels = ui
        .ctx()
        .pixels_per_point()
        .clamp(1.0, 2.0)
        .min(2048.0 / rect.width().max(1.0))
        .min(1536.0 / rect.height().max(1.0));
    let size = [
        (rect.width() * pixels).ceil().clamp(1.0, 2048.0) as usize,
        (rect.height() * pixels).ceil().clamp(1.0, 1536.0) as usize,
    ];
    let key = [
        orbit.yaw.to_bits(),
        orbit.pitch.to_bits(),
        orbit.zoom.to_bits(),
        size[0] as u32,
        size[1] as u32,
    ];
    if orbit.cache.as_ref().is_some_and(|cache| cache.key == key) {
        return;
    }
    let canvas = Rect::from_min_size(Pos2::ZERO, vec2(size[0] as f32, size[1] as f32));
    let mut triangles = Vec::with_capacity(46000);
    for part in &model().parts {
        let points: Vec<_> = part
            .vertices
            .iter()
            .map(|p| project(*p, canvas, orbit))
            .collect();
        let colors: Vec<_> = part
            .normals
            .iter()
            .map(|n| lit(part.color, rotated(*n, orbit), part.metallic))
            .collect();
        for indices in &part.triangles {
            let vertices = indices.map(|i| points[i]);
            triangles.push(Triangle {
                points: vertices.map(|v| v.0),
                depths: vertices.map(|v| v.1),
                colors: indices.map(|i| colors[i]),
            });
        }
    }
    let (image, depth) = rasterize(&triangles, size);
    let texture = if let Some(cache) = &mut orbit.cache {
        cache.texture.set(image, TextureOptions::LINEAR);
        cache.texture.clone()
    } else {
        ui.ctx()
            .load_texture("mouse-model", image, TextureOptions::LINEAR)
    };
    orbit.cache = Some(RenderCache {
        key,
        texture,
        depth: Arc::new(depth),
        size,
    });
}

pub fn draw(ui: &mut Ui, rect: Rect, orbit: &mut Orbit, pressed: &[bool; 5], selected: &mut usize) {
    let drag = ui.interact(rect, ui.id().with("orbit"), Sense::drag());
    if drag.dragged() {
        let delta = ui.input(|i| i.pointer.delta());
        orbit.yaw -= delta.x * 0.012;
        orbit.pitch = (orbit.pitch + delta.y * 0.012).clamp(-1.5, 1.5);
    }
    if drag.hovered() {
        orbit.zoom =
            (orbit.zoom * ui.input(|i| (i.raw_scroll_delta.y * 0.002).exp())).clamp(0.6, 2.0);
        // Zooming must not also scroll the page behind the model.
        ui.input_mut(|i| i.smooth_scroll_delta = Vec2::ZERO);
    }
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    render(ui, rect, orbit);
    let cache = orbit.cache.as_ref().expect("rendered mouse texture");
    painter.image(
        cache.texture.id(),
        rect,
        Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    // Locations lie on the actual button surfaces, in the mesh's Y-up coordinates.
    let hotspots = model().hotspots;
    for (i, p) in hotspots.into_iter().enumerate() {
        let (at, depth) = project(p, rect, orbit);
        if !rect.contains(at) {
            continue;
        }
        let pixel =
            (at - rect.min) / rect.size() * vec2(cache.size[0] as f32, cache.size[1] as f32);
        let x = (pixel.x as usize).min(cache.size[0] - 1);
        let y = (pixel.y as usize).min(cache.size[1] - 1);
        if cache.depth[y * cache.size[0] + x] < depth - 0.045 {
            continue;
        }
        // Primary buttons only need a marker when pressed; numbered controls remain selectable.
        if i >= 3 && !pressed[i] {
            continue;
        }
        let color = if pressed[i] || (i < 3 && *selected == i) {
            MINT
        } else {
            TEXT
        };
        let radius = if pressed[i] { 12.0 } else { 10.0 };
        painter.circle_filled(
            at,
            radius,
            if pressed[i] {
                MINT
            } else {
                Color32::from_black_alpha(225)
            },
        );
        painter.circle_stroke(at, radius, Stroke::new(1.5_f32, color));
        if i < 3 {
            painter.text(
                at,
                Align2::CENTER_CENTER,
                ["3", "2", "1"][i],
                FontId::proportional(11.0),
                if pressed[i] { Color32::BLACK } else { color },
            );
            if ui
                .interact(
                    Rect::from_center_size(at, vec2(26.0, 26.0)).intersect(rect),
                    ui.id().with(("3d-button", i)),
                    Sense::click(),
                )
                .on_hover_text(["Back button", "Forward button", "Middle click"][i])
                .clicked()
            {
                *selected = i;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reference_camera_shows_the_nose_on_the_left() {
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 400.0));
        let orbit = Orbit::default();
        let nose = project([0.0, 0.0, -1.4], rect, &orbit).0;
        let tail = project([0.0, 0.0, 1.4], rect, &orbit).0;
        assert!(nose.x < tail.x);
    }
    #[test]
    fn overlapping_surfaces_use_pixel_depth_in_both_submission_orders() {
        let points = [pos2(0.0, 0.0), pos2(8.0, 0.0), pos2(0.0, 8.0)];
        let flat = || Triangle {
            points,
            depths: [0.0; 3],
            colors: [Color32::RED; 3],
        };
        let tilted = || Triangle {
            points,
            depths: [-1.0, 3.0, 3.0],
            colors: [Color32::BLUE; 3],
        };
        for triangles in [[flat(), tilted()], [tilted(), flat()]] {
            let (image, _) = rasterize(&triangles, [8, 8]);
            assert_eq!(
                image.pixels[0],
                Color32::BLUE,
                "near corner must cover the shell"
            );
            assert_eq!(
                image.pixels[3 * 8 + 3],
                Color32::RED,
                "far corner must be behind the shell"
            );
        }
    }
}
