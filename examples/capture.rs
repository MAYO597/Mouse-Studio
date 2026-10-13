//! Render only this application's window into PNG files, with isolated temporary settings.
use eframe::egui::{self, Event, ViewportCommand};
use mouse_studio::{
    app::{Page, Studio},
    model::{ActionKind, Settings},
    recovery,
};

struct ReadOnlyTelemetry(mouse_studio::device::MouseModel);
impl mouse_studio::backend::DeviceBackend for ReadOnlyTelemetry {
    fn apply(&self, _: mouse_studio::backend::PointerSettings) -> Result<(), String> {
        Err("Read-only capture".into())
    }
    fn hardware_status(&self) -> mouse_studio::hardware::Snapshot {
        mouse_studio::hardware::snapshot_for(self.0)
    }
}

struct Capture {
    app: Studio,
    frame_count: usize,
    page: usize,
    prepared: usize,
    photo_mode: bool,
    started: std::time::Instant,
    page_started: std::time::Instant,
    _temporary: tempfile::TempDir,
}

const PAGES: [(Page, &str); 9] = [
    (Page::Overview, "overview"),
    (Page::Buttons, "buttons"),
    (Page::Pointer, "pointer"),
    (Page::Scrolling, "scrolling"),
    (Page::Profiles, "profiles"),
    (Page::Settings, "settings"),
    (Page::Overview, "3d"),
    (Page::Buttons, "advanced"),
    (Page::Buttons, "macro"),
];

impl Capture {
    fn photo_labels(&self) -> &'static [&'static str] {
        use mouse_studio::device::MouseModel;
        match self.app.device {
            MouseModel::M650L => &["perspective", "top"],
            MouseModel::MxAnywhere3s => &["perspective", "top", "side", "3d"],
            MouseModel::MxMaster4 => &[
                "perspective",
                "top",
                "side",
                "front",
                "rear-angle",
                "bottom",
            ],
        }
    }
}

impl eframe::App for Capture {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let count = if self.photo_mode {
            self.photo_labels().len()
        } else {
            PAGES.len()
        };
        if self.page >= count {
            ctx.send_viewport_cmd(ViewportCommand::Close);
            return;
        }
        let screenshot = ctx.input(|i| {
            i.events.iter().find_map(|event| {
                if let Event::Screenshot { image, .. } = event {
                    Some(image.clone())
                } else {
                    None
                }
            })
        });
        if let Some(image) = screenshot {
            let directory = std::env::var("MOUSE_CAPTURE_OUTPUT").unwrap_or("output".into());
            std::fs::create_dir_all(&directory).unwrap();
            let label = if self.photo_mode {
                self.photo_labels()[self.page]
            } else {
                PAGES[self.page].1
            };
            let path = format!("{directory}/ui-{label}.png");
            let rgba: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
            image::save_buffer(
                &path,
                &rgba,
                image.width() as u32,
                image.height() as u32,
                image::ColorType::Rgba8,
            )
            .unwrap();
            println!("Saved {path} ({}×{})", image.width(), image.height());
            self.page += 1;
            self.frame_count = 0;
            if self.page == count {
                ctx.send_viewport_cmd(ViewportCommand::Close);
                return;
            }
        }
        if self.prepared != self.page {
            self.prepared = self.page;
            self.page_started = std::time::Instant::now();
            self.app.close_editors();
            self.app.set_preview_view(0);
            match self.page {
                6 => self.app.set_preview_view(3),
                7 => self.app.open_advanced_settings(),
                8 => self.app.open_macro_editor(0, 0),
                _ => {}
            }
        }
        if self.photo_mode {
            self.app.set_preview_view(self.page);
            self.app.page = Page::Overview;
        } else {
            self.app.page = PAGES[self.page].0;
        }
        self.app.show(ctx);
        let delay = std::env::var("MOUSE_CAPTURE_DELAY_MS")
            .ok()
            .and_then(|s| s.parse::<u128>().ok())
            .unwrap_or(3000);
        if self.started.elapsed().as_millis() >= delay
            && self.page_started.elapsed().as_millis() >= 350
        {
            self.frame_count += 1;
        }
        if self.frame_count == 5 {
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
        }
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result {
    std::fs::create_dir_all("output").unwrap();
    eframe::run_native(
        "Mouse Studio UI check",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([
                    std::env::var("MOUSE_CAPTURE_WIDTH")
                        .ok()
                        .and_then(|s| s.parse::<f32>().ok())
                        .unwrap_or(1536.0),
                    std::env::var("MOUSE_CAPTURE_HEIGHT")
                        .ok()
                        .and_then(|s| s.parse::<f32>().ok())
                        .unwrap_or(1024.0),
                ])
                .with_decorations(false)
                .with_app_id("mouse-studio-capture"),
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        },
        Box::new(|cc| {
            let temporary = tempfile::tempdir().unwrap();
            let path = temporary.path().join("settings.json");
            Settings::default().save_to(&path).unwrap();
            let mut sample = Settings::default();
            sample.active_mut().bindings[0].kind = ActionKind::Application;
            sample.active_mut().bindings[0].value = "code".into();
            let hardware_preview = std::env::var_os("MOUSE_CAPTURE_HARDWARE").is_some();
            if hardware_preview {
                let h = &mut sample.active_mut().hardware;
                h.dpi = Some(1600);
                h.wheel_mode = Some(mouse_studio::hardware::WheelMode::Ratchet);
                h.smartshift = Some(20);
            }
            recovery::save_with_history(&sample, &path).unwrap();
            let mut app = if hardware_preview {
                Studio::with_backend(
                    &cc.egui_ctx,
                    path,
                    std::sync::Arc::new(ReadOnlyTelemetry(
                        std::env::var("MOUSE_CAPTURE_DEVICE")
                            .ok()
                            .and_then(|s| mouse_studio::device::MouseModel::parse(&s).ok())
                            .unwrap_or_default(),
                    )),
                )
            } else {
                Studio::new(&cc.egui_ctx, path)
            };
            if let Ok(model) = std::env::var("MOUSE_CAPTURE_DEVICE") {
                app.switch_device(
                    mouse_studio::device::MouseModel::parse(&model).expect("capture device"),
                );
            }
            Ok(Box::new(Capture {
                app,
                frame_count: 0,
                page: 0,
                prepared: usize::MAX,
                photo_mode: std::env::var_os("MOUSE_CAPTURE_PHOTOS").is_some(),
                started: std::time::Instant::now(),
                page_started: std::time::Instant::now(),
                _temporary: temporary,
            }))
        }),
    )
}
