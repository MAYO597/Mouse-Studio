//! Bundled Logitech product photos. Coordinates use the local images.
use crate::device::MouseModel;
use crate::theme::{BG, MINT, MUTED, TEXT, muted, segment};
use eframe::egui::*;
use std::collections::HashMap;
struct Photo {
    label: &'static str,
    bytes: &'static [u8],
    crop: [f32; 4],
    spots: &'static [(usize, [f32; 2])],
}
const MASTER_PHOTOS: [Photo; 6] = [
    Photo {
        label: "Perspective",
        bytes: include_bytes!("../assets/mx-master-4/front-angle.png"),
        crop: [0.12, 0.20, 0.88, 0.77],
        spots: &[
            (0, [0.655, 0.420]),
            (1, [0.606, 0.466]),
            (2, [0.322, 0.470]),
            (3, [0.453, 0.340]),
            (4, [0.542, 0.535]),
            (5, [0.690, 0.555]),
        ],
    },
    Photo {
        label: "Top",
        bytes: include_bytes!("../assets/mx-master-4/top.png"),
        crop: [0.21, 0.075, 0.74, 0.925],
        spots: &[(2, [0.525, 0.28]), (3, [0.525, 0.445])],
    },
    Photo {
        label: "Side",
        bytes: include_bytes!("../assets/mx-master-4/side.png"),
        crop: [0.08, 0.26, 0.92, 0.73],
        spots: &[
            (0, [0.574, 0.422]),
            (1, [0.505, 0.465]),
            (4, [0.419, 0.498]),
            (5, [0.608, 0.560]),
        ],
    },
    Photo {
        label: "Front",
        bytes: include_bytes!("../assets/mx-master-4/front.png"),
        crop: [0.19, 0.245, 0.83, 0.73],
        spots: &[(2, [0.447, 0.443])],
    },
    Photo {
        label: "Rear angle",
        bytes: include_bytes!("../assets/mx-master-4/perspective.png"),
        crop: [0.245, 0.16, 0.78, 0.86],
        spots: &[(2, [0.455, 0.238])],
    },
    Photo {
        label: "Bottom",
        bytes: include_bytes!("../assets/mx-master-4/bottom.png"),
        crop: [0.25, 0.075, 0.78, 0.925],
        spots: &[],
    },
];
const M650_PHOTOS: [Photo; 2] = [
    Photo {
        label: "Perspective",
        bytes: include_bytes!("../assets/m650-l/perspective.png"),
        crop: [0.27, 0.21, 0.74, 0.81],
        spots: &[
            (0, [0.590, 0.480]),
            (1, [0.554, 0.543]),
            (2, [0.396, 0.536]),
        ],
    },
    Photo {
        label: "Top",
        bytes: include_bytes!("../assets/m650-l/top.png"),
        crop: [0.36, 0.10, 0.64, 0.90],
        spots: &[(2, [0.501, 0.266])],
    },
];
#[derive(Default)]
pub struct Gallery {
    textures: HashMap<(MouseModel, usize), TextureHandle>,
}
impl Gallery {
    pub fn draw(
        &mut self,
        ui: &mut Ui,
        model: MouseModel,
        height: f32,
        view: &mut usize,
        pressed: &[bool; 8],
        selected: &mut usize,
    ) {
        let photos: &[Photo] = match model {
            MouseModel::M650L => &M650_PHOTOS,
            MouseModel::MxMaster4 => &MASTER_PHOTOS,
            MouseModel::MxAnywhere3s => return,
        };
        *view = (*view).min(photos.len() - 1);
        let photo = &photos[*view];
        let texture = self.textures.entry((model, *view)).or_insert_with(|| {
            let limit = ui.input(|i| i.max_texture_side).min(1536) as u32;
            let image = image::load_from_memory(photo.bytes)
                .expect("bundled Logitech photo")
                .thumbnail(limit, limit)
                .to_rgba8();
            ui.ctx().load_texture(
                format!("{}-{}", model.slug(), photo.label),
                ColorImage::from_rgba_unmultiplied(
                    [image.width() as usize, image.height() as usize],
                    &image,
                ),
                TextureOptions::LINEAR,
            )
        });
        let width = ui.available_width();
        let (stage, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        let [x0, y0, x1, y1] = photo.crop;
        let uv = Rect::from_min_max(pos2(x0, y0), pos2(x1, y1));
        let ratio = texture.aspect_ratio() * uv.width() / uv.height();
        let h = (height - 16.0).min(width * 0.84 / ratio);
        let rect = Rect::from_center_size(stage.center(), vec2(h * ratio, h));
        ui.painter().image(texture.id(), rect, uv, Color32::WHITE);
        for &(index, [x, y]) in photo.spots {
            let at = rect.min + rect.size() * vec2((x - x0) / uv.width(), (y - y0) / uv.height());
            // Separate the adjacent side-button labels without moving their target dots.
            let offset = match index {
                0 => vec2(32.0, -32.0),
                1 => vec2(-8.0, 34.0),
                3 => vec2(0.0, -40.0),
                4 => vec2(-36.0, 34.0),
                5 => vec2(30.0, 38.0),
                _ => vec2(-38.0, -34.0),
            };
            let label = stage.shrink(15.0).clamp(at + offset);
            let color = if *selected == index || pressed[index] {
                MINT
            } else {
                TEXT.gamma_multiply(0.8)
            };
            ui.painter()
                .line_segment([at, label], Stroke::new(1.1_f32, color));
            ui.painter().circle_filled(at, 2.5, color);
            if ui
                .place(
                    Rect::from_center_size(label, vec2(28.0, 28.0)),
                    Button::new(
                        RichText::new(crate::device::BUTTON_MARKERS[index])
                            .color(if pressed[index] { BG } else { color }),
                    )
                    .fill(if pressed[index] { MINT } else { BG })
                    .stroke(Stroke::new(1.3_f32, color))
                    .corner_radius(14),
                )
                .on_hover_text(crate::device::BUTTON_LABELS[index])
                .clicked()
            {
                *selected = index;
            }
        }
        let columns = photos.len().min(if width < 560.0 { 3 } else { 6 });
        let item_width = ((width - 8.0) / columns as f32).min(112.0);
        for row in (0..photos.len()).collect::<Vec<_>>().chunks(columns) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(((width - item_width * row.len() as f32) / 2.0).max(0.0));
                for &i in row {
                    let label = if i == 0 && item_width < 112.0 {
                        "Angle"
                    } else {
                        photos[i].label
                    };
                    if segment(ui, label, *view == i, item_width)
                        .on_hover_text(photos[i].label)
                        .clicked()
                    {
                        *view = i;
                    }
                }
            });
        }
        ui.horizontal_wrapped(|ui| {
            for (i, label) in crate::device::BUTTON_LABELS[..model.button_count()]
                .iter()
                .enumerate()
            {
                if ui
                    .selectable_label(
                        *selected == i,
                        RichText::new(*label).color(if pressed[i] { MINT } else { MUTED }),
                    )
                    .clicked()
                {
                    *selected = i;
                }
            }
        });
        ui.vertical_centered(|ui| muted(ui, "Choose a view · Click a button to configure"));
        ui.add_space(7.0);
    }
}
