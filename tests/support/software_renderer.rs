//! Test-only CPU rendering of egui's actual tessellated meshes and texture atlas.
//! Uses bilinear sampling and premultiplied sRGBA blending; no native GPU is needed.
use std::collections::HashMap;

use egui::{ColorImage, Pos2, TextureId, TexturesDelta, epaint::Primitive, pos2};
use egui_kittest::TestRenderer;
use image::{Rgba, RgbaImage};

#[derive(Default)]
pub struct SoftwareRenderer {
    textures: HashMap<TextureId, ColorImage>,
    error: Option<String>,
}

impl TestRenderer for SoftwareRenderer {
    fn handle_delta(&mut self, delta: &mut TexturesDelta) {
        for (id, updates) in delta.set.drain() {
            for update in updates {
                let egui::ImageData::Color(image) = update.image;
                if let Some([x, y]) = update.pos {
                    let Some(target) = self.textures.get_mut(&id) else {
                        self.error = Some(format!("Texture patch has no base texture: {id:?}"));
                        continue;
                    };
                    if x + image.size[0] > target.size[0] || y + image.size[1] > target.size[1] {
                        self.error = Some(format!("Texture patch exceeds atlas bounds: {id:?}"));
                        continue;
                    }
                    for row in 0..image.size[1] {
                        let start = (y + row) * target.size[0] + x;
                        target.pixels[start..start + image.size[0]].copy_from_slice(
                            &image.pixels[row * image.size[0]..(row + 1) * image.size[0]],
                        );
                    }
                } else {
                    self.textures.insert(id, (*image).clone());
                }
            }
        }
        for id in delta.free.drain() {
            self.textures.remove(&id);
        }
    }

    fn render(
        &mut self,
        ctx: &egui::Context,
        output: &egui::FullOutput,
    ) -> Result<RgbaImage, String> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        let scale = ctx.pixels_per_point();
        let content = ctx.content_rect();
        let size = content.size() * scale;
        let mut image = RgbaImage::new(size.x.round() as u32, size.y.round() as u32);
        for clipped in ctx.tessellate(output.shapes.clone(), scale) {
            let Primitive::Mesh(mesh) = clipped.primitive else {
                return Err("Software test renderer does not support paint callbacks".into());
            };
            let texture = self
                .textures
                .get(&mesh.texture_id)
                .ok_or_else(|| format!("Missing texture for egui mesh: {:?}", mesh.texture_id))?;
            let clip = egui::Rect::from_min_max(
                pos2(
                    (clipped.clip_rect.min.x - content.min.x) * scale,
                    (clipped.clip_rect.min.y - content.min.y) * scale,
                ),
                pos2(
                    (clipped.clip_rect.max.x - content.min.x) * scale,
                    (clipped.clip_rect.max.y - content.min.y) * scale,
                ),
            );
            for indices in mesh.indices.chunks_exact(3) {
                let mut vertices = [
                    mesh.vertices[indices[0] as usize],
                    mesh.vertices[indices[1] as usize],
                    mesh.vertices[indices[2] as usize],
                ];
                for vertex in &mut vertices {
                    vertex.pos = pos2(
                        (vertex.pos.x - content.min.x) * scale,
                        (vertex.pos.y - content.min.y) * scale,
                    );
                }
                let mut area = edge(vertices[0].pos, vertices[1].pos, vertices[2].pos);
                if area.abs() < 0.0001 {
                    continue;
                }
                if area < 0.0 {
                    vertices.swap(1, 2);
                    area = -area;
                }
                let points = vertices.map(|vertex| vertex.pos);
                let bounds = egui::Rect::from_points(&points).intersect(clip);
                let x0 = bounds.min.x.floor().max(0.0) as u32;
                let y0 = bounds.min.y.floor().max(0.0) as u32;
                let x1 = bounds.max.x.ceil().max(0.0).min(image.width() as f32) as u32;
                let y1 = bounds.max.y.ceil().max(0.0).min(image.height() as f32) as u32;
                for y in y0..y1 {
                    for x in x0..x1 {
                        let sample = pos2(x as f32 + 0.5, y as f32 + 0.5);
                        if !clip.contains(sample) {
                            continue;
                        }
                        let edges = [
                            (points[1], points[2]),
                            (points[2], points[0]),
                            (points[0], points[1]),
                        ];
                        let weights = edges.map(|(a, b)| edge(a, b, sample));
                        // A shared triangle edge belongs to one triangle, preventing blend seams.
                        if edges.iter().zip(weights).any(|(&(a, b), value)| {
                            value < 0.0 || (value == 0.0 && !top_left(a, b))
                        }) {
                            continue;
                        }
                        let weights = weights.map(|weight| weight / area);
                        let uv = pos2(
                            (0..3).map(|i| vertices[i].uv.x * weights[i]).sum(),
                            (0..3).map(|i| vertices[i].uv.y * weights[i]).sum(),
                        );
                        let texel = bilinear(texture, uv);
                        let color: [f32; 4] = std::array::from_fn(|channel| {
                            texel[channel]
                                * (0..3)
                                    .map(|i| {
                                        vertices[i].color.to_array()[channel] as f32 / 255.0
                                            * weights[i]
                                    })
                                    .sum::<f32>()
                        });
                        let destination = image.get_pixel_mut(x, y);
                        for (channel, value) in color.into_iter().enumerate() {
                            destination.0[channel] =
                                ((value + destination.0[channel] as f32 / 255.0 * (1.0 - color[3]))
                                    * 255.0)
                                    .round()
                                    .clamp(0.0, 255.0) as u8;
                        }
                    }
                }
            }
        }
        // PNG RGBA stores straight alpha. All intermediate blending above is premultiplied.
        for Rgba(pixel) in image.pixels_mut() {
            let alpha = pixel[3];
            if alpha > 0 && alpha < 255 {
                for channel in &mut pixel[..3] {
                    *channel = ((*channel as f32 * 255.0 / alpha as f32).round().min(255.0)) as u8;
                }
            }
        }
        Ok(image)
    }
}

fn edge(a: Pos2, b: Pos2, p: Pos2) -> f32 {
    (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
}

fn top_left(a: Pos2, b: Pos2) -> bool {
    b.y < a.y || (b.y == a.y && b.x > a.x)
}

fn bilinear(texture: &ColorImage, uv: Pos2) -> [f32; 4] {
    let x = uv.x * texture.size[0] as f32 - 0.5;
    let y = uv.y * texture.size[1] as f32 - 0.5;
    let fx = x - x.floor();
    let fy = y - y.floor();
    let sample = |dx: usize, dy: usize| {
        let sx =
            (x.floor() + dx as f32).clamp(0.0, texture.size[0].saturating_sub(1) as f32) as usize;
        let sy =
            (y.floor() + dy as f32).clamp(0.0, texture.size[1].saturating_sub(1) as f32) as usize;
        texture.pixels[sy * texture.size[0] + sx]
            .to_array()
            .map(|channel| channel as f32 / 255.0)
    };
    let [a, b, c, d] = [sample(0, 0), sample(1, 0), sample(0, 1), sample(1, 1)];
    std::array::from_fn(|i| {
        a[i] * (1.0 - fx) * (1.0 - fy)
            + b[i] * fx * (1.0 - fy)
            + c[i] * (1.0 - fx) * fy
            + d[i] * fx * fy
    })
}
