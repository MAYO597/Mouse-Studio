use crate::{
    backend::DeviceBackend,
    device::{self, DeviceStatus, MouseModel},
    model::{ActionKind, Binding, Settings},
    recovery,
    startup::StartupControl,
    theme::*,
};
use eframe::egui::{self, *};
use std::{
    path::PathBuf,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

const BUTTONS: [&str; 6] = device::BUTTON_LABELS;
const BUTTON_NUMBERS: [usize; 6] = [3, 2, 1, 6, 7, 8];
const DEFAULTS: [&str; 6] = device::BUTTON_DEFAULTS;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Overview,
    Buttons,
    Pointer,
    Scrolling,
    Profiles,
    Settings,
}

impl Page {
    const ALL: [Self; 6] = [
        Self::Overview,
        Self::Buttons,
        Self::Pointer,
        Self::Scrolling,
        Self::Profiles,
        Self::Settings,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Buttons => "Buttons",
            Self::Pointer => "Pointer",
            Self::Scrolling => "Scrolling",
            Self::Profiles => "Profiles",
            Self::Settings => "Settings",
        }
    }
    fn icon(self) -> ImageSource<'static> {
        match self {
            Self::Overview => include_image!("../assets/icons/house.svg"),
            Self::Buttons => include_image!("../assets/icons/mouse.svg"),
            Self::Pointer => include_image!("../assets/icons/mouse-pointer-2.svg"),
            Self::Scrolling => include_image!("../assets/icons/arrow-down-up.svg"),
            Self::Profiles => include_image!("../assets/icons/layout-grid.svg"),
            Self::Settings => include_image!("../assets/icons/settings.svg"),
        }
    }
}

#[derive(Clone)]
struct DeviceDraft {
    draft: Settings,
    saved: Settings,
    message: String,
    is_error: bool,
    load_blocked: bool,
    previous_setup: Result<Option<Settings>, String>,
}
pub struct Studio {
    pub device: MouseModel,
    base_path: PathBuf,
    device_drafts: std::collections::HashMap<MouseModel, DeviceDraft>,
    desktop_devices: bool,
    pub draft: Settings,
    pub saved: Settings,
    pub page: Page,
    pub selected_button: usize,
    path: PathBuf,
    images: Vec<TextureHandle>,
    product_photos: crate::product_photos::Gallery,
    view: usize,
    status: DeviceStatus,
    connection: Option<&'static str>,
    last_discovery: Instant,
    message: String,
    is_error: bool,
    load_blocked: bool,
    transfer_path: String,
    confirm: Option<Confirm>,
    allow_close: bool,
    application_picker_open: bool,
    backend: Option<Arc<dyn DeviceBackend>>,
    pending_apply: Option<mpsc::Receiver<Result<(), String>>>,
    close_after_apply: bool,
    startup: Option<Arc<dyn StartupControl>>,
    testing_action: bool,
    trigger: usize,
    software_ui: crate::software_ui::Panels,
    orbit: crate::preview3d::Orbit,
    live: crate::backend::RuntimeStatus,
    pending_live: Option<mpsc::Receiver<Result<crate::backend::RuntimeStatus, String>>>,
    last_live: Instant,
    test_cancel: Arc<std::sync::atomic::AtomicBool>,
    pending_cancel: Option<mpsc::Receiver<Result<(), String>>>,
    previous_setup: Result<Option<Settings>, String>,
    restoring_setup: bool,
    hardware: crate::hardware::Snapshot,
    pending_hardware: Option<mpsc::Receiver<crate::hardware::Snapshot>>,
    pending_haptic: Option<mpsc::Receiver<Result<(), String>>>,
    last_hardware: Instant,
}

#[derive(Clone)]
enum Confirm {
    Close,
    DeleteProfile,
    Import(Settings),
    OverwriteExport(PathBuf),
    Restore(Settings),
}

impl Studio {
    pub fn set_preview_view(&mut self, view: usize) {
        self.view = view.min(match self.device {
            MouseModel::MxMaster4 => 5,
            MouseModel::M650L => 1,
            MouseModel::MxAnywhere3s => 3,
        });
    }
    pub fn open_advanced_settings(&mut self) {
        self.software_ui.advanced_open = true;
    }
    pub fn close_editors(&mut self) {
        self.software_ui.close();
    }
    pub fn open_macro_editor(&mut self, button: usize, trigger: usize) {
        if button < self.device.button_count() && trigger < 3 {
            let binding = crate::software_ui::binding(self.draft.active(), button, trigger).clone();
            self.software_ui
                .open_macro(self.draft.active_id, button, trigger, &binding);
        }
    }
    /// Isolated profile editor: never writes to a desktop session (tests/captures).
    pub fn new(ctx: &Context, path: PathBuf) -> Self {
        install(ctx);
        egui_extras::install_image_loaders(ctx);
        let loaded = if path.exists() {
            Settings::load_from(&path)
        } else {
            Ok(Settings::default())
        };
        let loaded = loaded.and_then(|s| {
            if s.device == MouseModel::MxAnywhere3s {
                Ok(s)
            } else {
                Err("The legacy settings file belongs to another mouse.".into())
            }
        });
        let (settings, message, blocked) = match loaded {
            Ok(s) => (s, "Ready to customize".into(), false),
            Err(e) => (
                Settings::default(),
                format!("Saved settings could not be loaded. {e}"),
                true,
            ),
        };
        let images = [
            include_bytes!("../assets/mouse-3qtr-back.png").as_slice(),
            include_bytes!("../assets/mouse-top-view.png").as_slice(),
            include_bytes!("../assets/mouse-side-left.png").as_slice(),
            include_bytes!("../assets/vscode-stable.png").as_slice(),
        ]
        .iter()
        .enumerate()
        .map(|(i, bytes)| {
            let image = image::load_from_memory(bytes)
                .expect("bundled product image")
                .to_rgba8();
            let size = [image.width() as usize, image.height() as usize];
            ctx.load_texture(
                format!("mouse-{i}"),
                ColorImage::from_rgba_unmultiplied(size, &image),
                TextureOptions::LINEAR,
            )
        })
        .collect();
        let transfer_path = path
            .with_file_name("profiles-export.json")
            .display()
            .to_string();
        let previous_setup = recovery::previous_setup(&path);
        Self {
            device: MouseModel::default(),
            base_path: path.clone(),
            device_drafts: Default::default(),
            desktop_devices: false,
            draft: settings.clone(),
            saved: settings,
            page: Page::Overview,
            selected_button: 0,
            path,
            images,
            product_photos: Default::default(),
            view: 0,
            status: device::discover(),
            connection: device::connection(),
            last_discovery: Instant::now(),
            message,
            is_error: blocked,
            load_blocked: blocked,
            transfer_path,
            confirm: None,
            allow_close: false,
            application_picker_open: false,
            backend: None,
            pending_apply: None,
            close_after_apply: false,
            startup: None,
            testing_action: false,
            trigger: 0,
            software_ui: Default::default(),
            orbit: Default::default(),
            live: Default::default(),
            pending_live: None,
            last_live: Instant::now(),
            test_cancel: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            pending_cancel: None,
            previous_setup,
            restoring_setup: false,
            hardware: Default::default(),
            pending_hardware: None,
            pending_haptic: None,
            last_hardware: Instant::now() - Duration::from_secs(10),
        }
    }

    pub fn with_devices(
        ctx: &Context,
        path: PathBuf,
        model: MouseModel,
        startup: Arc<dyn StartupControl>,
    ) -> Self {
        let mut app = Self::with_services(
            ctx,
            path,
            Arc::new(crate::hardware::desktop::Desktop::default()),
            startup,
        );
        app.desktop_devices = true;
        app.switch_device(model);
        app
    }
    pub fn switch_device(&mut self, model: MouseModel) {
        if model == self.device || self.is_applying() {
            return;
        }
        self.device_drafts.insert(
            self.device,
            DeviceDraft {
                draft: self.draft.clone(),
                saved: self.saved.clone(),
                message: self.message.clone(),
                is_error: self.is_error,
                load_blocked: self.load_blocked,
                previous_setup: self.previous_setup.clone(),
            },
        );
        self.device = model;
        self.path = model.settings_path(&self.base_path);
        let state = self.device_drafts.remove(&model).unwrap_or_else(|| {
            let loaded = if self.path.exists() {
                Settings::load_from(&self.path)
            } else {
                Ok(Settings::for_device(model))
            };
            let loaded = loaded.and_then(|s| {
                if s.device == model {
                    Ok(s)
                } else {
                    Err("This settings file belongs to another mouse.".into())
                }
            });
            let (settings, message, blocked) = match loaded {
                Ok(settings) => (
                    settings,
                    "Save to apply this mouse's profiles.".into(),
                    false,
                ),
                Err(e) => (Settings::for_device(model), e, true),
            };
            DeviceDraft {
                draft: settings.clone(),
                saved: settings,
                message,
                is_error: blocked,
                load_blocked: blocked,
                previous_setup: recovery::previous_setup(&self.path),
            }
        });
        self.draft = state.draft;
        self.saved = state.saved;
        self.message = state.message;
        self.is_error = state.is_error;
        self.load_blocked = state.load_blocked;
        self.previous_setup = state.previous_setup;
        self.transfer_path = self
            .path
            .with_file_name(format!("{}-profiles-export.json", model.slug()))
            .display()
            .to_string();
        if self.desktop_devices {
            self.backend = Some(Arc::new(crate::hardware::desktop::Desktop::for_device(
                model,
            )));
        }
        self.status = device::discover_for(model);
        self.connection = device::connection_for(model);
        self.hardware = Default::default();
        self.live = Default::default();
        self.pending_live = None;
        self.pending_hardware = None;
        self.last_hardware = Instant::now() - Duration::from_secs(10);
        self.last_live = Instant::now() - Duration::from_secs(10);
        self.software_ui = Default::default();
        self.application_picker_open = false;
        self.view = 0;
        self.selected_button = 0;
        self.confirm = None;
    }
    fn other_devices_dirty(&self) -> bool {
        self.device_drafts.values().any(|s| s.draft != s.saved)
    }
    fn device_picker(&mut self, ui: &mut Ui, compact: bool) {
        let mut model = self.device;
        ui.add_enabled_ui(!self.is_applying(), |ui| {
            ComboBox::from_id_salt("mouse-device")
                .width(if compact { 38.0 } else { ui.available_width() })
                .selected_text(if compact { "" } else { model.name() })
                .truncate()
                .show_ui(ui, |ui| {
                    for device in MouseModel::ALL {
                        ui.selectable_value(&mut model, device, device.name());
                    }
                })
                .response
                .on_hover_text(format!("Choose mouse · {}", model.name()));
        });
        if model != self.device {
            self.switch_device(model);
        }
    }

    pub fn with_backend(ctx: &Context, path: PathBuf, backend: Arc<dyn DeviceBackend>) -> Self {
        let mut app = Self::new(ctx, path);
        app.backend = Some(backend);
        if !app.load_blocked {
            app.message = "Save to apply mouse settings and button actions.".into();
        }
        app
    }

    pub fn with_services(
        ctx: &Context,
        path: PathBuf,
        backend: Arc<dyn DeviceBackend>,
        startup: Arc<dyn StartupControl>,
    ) -> Self {
        let mut app = Self::with_backend(ctx, path, backend);
        app.startup = Some(startup);
        app
    }

    fn selected_binding(&self) -> &Binding {
        crate::software_ui::binding(self.draft.active(), self.selected_button, self.trigger)
    }

    fn test_selected_action(&mut self) {
        self.start_test(self.selected_binding().clone());
    }

    fn start_test(&mut self, binding: Binding) {
        if self.is_applying() {
            return;
        }
        if let Err(error) = crate::actions::dispatcher(&binding) {
            self.notify(error, true);
            return;
        }
        let Some(backend) = self.backend.clone() else {
            return;
        };
        let (sender, receiver) = mpsc::channel();
        self.pending_apply = Some(receiver);
        self.testing_action = true;
        self.test_cancel
            .store(false, std::sync::atomic::Ordering::SeqCst);
        let cancel = self.test_cancel.clone();
        let profile = self.draft.active().clone();
        self.notify(
            "Testing in 3 seconds. Switch to the target window now.",
            false,
        );
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(3));
            let result = if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                Err("Test cancelled.".into())
            } else {
                backend.test_profile_action(&binding, &profile)
            };
            let _ = sender.send(result);
        });
    }

    pub fn is_applying(&self) -> bool {
        self.pending_apply.is_some() || self.pending_cancel.is_some()
    }

    fn poll_live(&mut self, ctx: &Context) {
        if let Some(receiver) = &self.pending_live {
            match receiver.try_recv() {
                Ok(Ok(status)) => {
                    self.live = status;
                    self.pending_live = None;
                }
                Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => {
                    self.live = Default::default();
                    self.pending_live = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        // Pressed-button indicators only appear on these pages; poll slowly elsewhere.
        let interval = if matches!(self.page, Page::Overview | Page::Buttons) {
            Duration::from_millis(100)
        } else {
            Duration::from_secs(1)
        };
        if self.last_live.elapsed() >= interval && self.pending_live.is_none() {
            self.last_live = Instant::now();
            if let Some(backend) = self.backend.clone() {
                let (tx, rx) = mpsc::channel();
                self.pending_live = Some(rx);
                std::thread::spawn(move || {
                    let _ = tx.send(backend.runtime_status());
                });
            }
        }
        if self.backend.is_some() {
            ctx.request_repaint_after(interval);
        }
    }

    fn start_haptic_test(&mut self) {
        if self.pending_haptic.is_some() {
            return;
        }
        let Some(backend) = self.backend.clone() else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        self.pending_haptic = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(backend.test_haptic());
        });
    }

    fn poll_haptic(&mut self, ctx: &Context) {
        let Some(rx) = &self.pending_haptic else {
            return;
        };
        match rx.try_recv() {
            Ok(result) => {
                self.pending_haptic = None;
                match result {
                    Ok(()) => self.notify("Haptic effect sent.", false),
                    Err(e) => self.notify(format!("Haptic test failed: {e}"), true),
                }
            }
            Err(mpsc::TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(100)),
            Err(mpsc::TryRecvError::Disconnected) => self.pending_haptic = None,
        }
    }

    fn poll_hardware(&mut self, ctx: &Context) {
        if let Some(rx) = &self.pending_hardware {
            match rx.try_recv() {
                Ok(s) => {
                    if self.desktop_devices {
                        self.status = crate::device::discover_for(self.device);
                        self.connection = crate::device::connection_for(self.device);
                    }
                    self.hardware = s;
                    self.pending_hardware = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.hardware = Default::default();
                    self.pending_hardware = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.pending_hardware.is_none()
            && !self.is_applying()
            && self.last_hardware.elapsed() >= Duration::from_secs(5)
        {
            self.last_hardware = Instant::now();
            if let Some(backend) = self.backend.clone() {
                let (tx, rx) = mpsc::channel();
                self.pending_hardware = Some(rx);
                std::thread::spawn(move || {
                    let _ = tx.send(backend.hardware_status());
                });
            }
        }
        if self.pending_hardware.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn poll_apply(&mut self, ctx: &Context) {
        let Some(receiver) = &self.pending_apply else {
            return;
        };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(50));
                return;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("Apply worker stopped. Save again to retry.".into())
            }
        };
        self.pending_apply = None;
        self.last_hardware = Instant::now() - Duration::from_secs(10);
        let close = std::mem::take(&mut self.close_after_apply);
        if std::mem::take(&mut self.testing_action) {
            match result {
                Ok(()) => self.notify("Test action sent.", false),
                Err(error) => self.notify(format!("Test failed: {error}"), true),
            }
            return;
        }
        let restoring = std::mem::take(&mut self.restoring_setup);
        match result {
            Ok(()) => {
                self.notify(
                    if restoring {
                        "Previous setup restored and applied."
                    } else {
                        "Mouse settings & button actions applied. Profiles saved."
                    },
                    false,
                );
                if close && !self.dirty() {
                    self.allow_close = true;
                    self.confirm = None;
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }
            Err(error) => {
                self.notify(format!("Saved locally; apply failed: {error}"), true);
                // Keep the window open so an apply error is visible, including on Save & close.
                if close {
                    self.confirm = None;
                }
            }
        }
    }

    pub fn dirty(&self) -> bool {
        self.draft != self.saved
    }
    fn notify(&mut self, message: impl Into<String>, is_error: bool) {
        self.message = message.into();
        self.is_error = is_error;
    }
    fn save_settings(&mut self) -> bool {
        self.store_settings(self.draft.clone(), false)
    }

    fn store_settings(&mut self, settings: Settings, restoring: bool) -> bool {
        if self.is_applying() {
            return false;
        }
        if settings.device != self.device {
            self.notify(
                "These profiles belong to a different mouse. Select that mouse to use them.",
                true,
            );
            return false;
        }
        if self.load_blocked {
            self.notify(
                "Resolve the unreadable settings file in Settings first.",
                true,
            );
            return false;
        }
        match recovery::save_with_history(&settings, &self.path) {
            Ok(()) => {
                self.saved = settings;
                if restoring {
                    self.draft = self.saved.clone();
                }
                self.previous_setup = recovery::previous_setup(&self.path);
                if let Some(backend) = self.backend.clone() {
                    let settings = self.saved.clone();
                    let path = self.path.clone();
                    let startup = self.startup.clone();
                    let (sender, receiver) = mpsc::channel();
                    self.pending_apply = Some(receiver);
                    self.restoring_setup = restoring;
                    self.notify(
                        if restoring {
                            "Previous setup saved. Applying mouse settings…"
                        } else {
                            "Profiles saved. Applying mouse settings…"
                        },
                        false,
                    );
                    std::thread::spawn(move || {
                        let result = (|| {
                            if let Some(startup) = startup {
                                startup
                                    .sync(settings.service_required(), &path)
                                    .map_err(|e| format!("Automatic restore: {e}"))?;
                            }
                            backend.apply_settings(&settings)
                        })();
                        let _ = sender.send(result);
                    });
                    false
                } else {
                    self.notify(
                        if restoring {
                            "Previous setup restored locally. Preview mode does not apply mouse settings."
                        } else {
                            "Saved locally. Preview mode does not apply mouse settings."
                        },
                        false,
                    );
                    true
                }
            }
            Err(e) => {
                self.notify(e, true);
                false
            }
        }
    }

    pub fn show(&mut self, ctx: &Context) {
        self.poll_apply(ctx);
        if let Some(receiver) = &self.pending_cancel {
            match receiver.try_recv() {
                Ok(result) => {
                    self.pending_cancel = None;
                    match result {
                        Ok(()) => self.notify("Pending actions and macros cancelled.", false),
                        Err(error) => self.notify(format!("Cancellation failed: {error}"), true),
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.pending_cancel = None;
                    self.notify("Cancellation worker stopped.", true);
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        self.poll_live(ctx);
        self.poll_hardware(ctx);
        self.poll_haptic(ctx);
        if self.last_discovery.elapsed() >= Duration::from_secs(5) {
            self.status = device::discover_for(self.device);
            self.connection = device::connection_for(self.device);
            self.last_discovery = Instant::now();
        }
        ctx.request_repaint_after(Duration::from_secs(5));
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            if self.is_applying() {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            } else if self.dirty() || self.other_devices_dirty() {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                self.confirm = Some(Confirm::Close);
            }
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::S)) {
            self.save_settings();
        }
        self.titlebar(ctx);
        let inspector = matches!(self.page, Page::Overview | Page::Buttons);
        let wide = ctx.content_rect().width() >= 1280.0 && ctx.content_rect().height() >= 800.0;
        if !inspector || !wide {
            self.footer(ctx);
        }
        self.sidebar(ctx);
        if inspector && wide {
            SidePanel::right("inspector")
                .exact_width((ctx.content_rect().width() * 0.277).clamp(310.0, 426.0))
                .resizable(false)
                .frame(egui::Frame::new().fill(PANEL).inner_margin(24))
                .show(ctx, |ui| {
                    ScrollArea::vertical()
                        .id_salt("inspector-scroll")
                        .show(ui, |ui| self.inspector(ui));
                });
        }
        CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(
                if ctx.content_rect().width() < 700.0 {
                    16
                } else {
                    28
                },
            ))
            .show(ctx, |ui| {
                ScrollArea::vertical()
                    .id_salt(("main-scroll", self.page.label(), self.device))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        match self.page {
                            Page::Overview => self.overview(ui),
                            Page::Buttons => self.buttons_page(ui),
                            Page::Pointer => self.pointer_page(ui),
                            Page::Scrolling => self.scrolling_page(ui),
                            Page::Profiles => self.profiles_page(ui),
                            Page::Settings => self.settings_page(ui),
                        }
                        if inspector && !wide {
                            ui.add_space(24.0);
                            ui.separator();
                            self.inspector(ui);
                        }
                    });
            });
        self.confirmation(ctx);
        self.software_panels(ctx);
        if self.is_applying() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }

    fn titlebar(&mut self, ctx: &Context) {
        TopBottomPanel::top("titlebar")
            .exact_height(40.0)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(Margin::symmetric(16, 4)),
            )
            .show(ctx, |ui| {
                let drag = ui.interact(ui.max_rect(), Id::new("titlebar-drag"), Sense::drag());
                if drag.drag_started() {
                    ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                }
                ui.horizontal_centered(|ui| {
                    mouse_mark(ui, 23.0, TEXT);
                    ui.label(RichText::new("Mouse Studio").size(15.0).strong());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(Button::new("×").frame(false).min_size(vec2(32.0, 28.0)))
                            .on_hover_text("Close")
                            .clicked()
                        {
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }
                        if ui
                            .add(Button::new("□").frame(false).min_size(vec2(32.0, 28.0)))
                            .on_hover_text("Maximize / restore")
                            .clicked()
                        {
                            ctx.send_viewport_cmd(ViewportCommand::Maximized(
                                !ctx.input(|i| i.viewport().maximized.unwrap_or(false)),
                            ));
                        }
                        if ui
                            .add(Button::new("−").frame(false).min_size(vec2(32.0, 28.0)))
                            .on_hover_text("Minimize")
                            .clicked()
                        {
                            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                        }
                        if ctx.content_rect().width() >= 650.0 {
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new(if self.backend.is_some() {
                                    "HYPRLAND"
                                } else {
                                    "LOCAL PREVIEW"
                                })
                                .color(MUTED)
                                .size(10.0)
                                .extra_letter_spacing(1.0),
                            )
                            .on_hover_text(
                                "Save profiles applies settings only to the selected mouse.",
                            );
                        }
                    });
                });
            });
    }

    fn footer(&mut self, ctx: &Context) {
        let compact = ctx.content_rect().width() < 700.0;
        TopBottomPanel::bottom("footer")
            .exact_height(if compact { 86.0 } else { 64.0 })
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(Margin::symmetric(22, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add_enabled(
                                !self.load_blocked && !self.is_applying(),
                                Button::new(RichText::new("Save profiles").strong().color(BG))
                                    .fill(MINT)
                                    .min_size(vec2(136.0, 36.0)),
                            )
                            .clicked()
                        {
                            self.save_settings();
                        }
                        if ui
                            .add_enabled(
                                self.dirty(),
                                Button::new("Discard").min_size(vec2(85.0, 36.0)),
                            )
                            .clicked()
                        {
                            self.draft = self.saved.clone();
                            self.notify("Unsaved edits discarded.", false);
                        }
                        if !compact {
                            ui.add_space(10.0);
                            ui.label(
                                RichText::new(if self.dirty() {
                                    "●  Unsaved changes"
                                } else {
                                    "✓  Up to date"
                                })
                                .size(12.0)
                                .color(if self.dirty() {
                                    MINT
                                } else {
                                    MUTED
                                }),
                            );
                            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                ui.add(
                                    Label::new(
                                        RichText::new(&self.message)
                                            .color(if self.is_error { ERROR } else { MUTED })
                                            .size(12.0),
                                    )
                                    .truncate(),
                                )
                                .on_hover_text(&self.message);
                            });
                        }
                    });
                });
                if compact {
                    let status = if self.dirty() && !self.is_error && !self.is_applying() {
                        "●  Unsaved changes"
                    } else {
                        &self.message
                    };
                    ui.add(
                        Label::new(RichText::new(status).size(12.0).color(if self.is_error {
                            ERROR
                        } else {
                            MUTED
                        }))
                        .truncate(),
                    )
                    .on_hover_text(&self.message);
                }
            });
    }

    fn sidebar(&mut self, ctx: &Context) {
        let compact = ctx.content_rect().width() < 1100.0 || ctx.content_rect().height() < 740.0;
        SidePanel::left("navigation")
            .exact_width(if compact {
                64.0
            } else {
                (ctx.content_rect().width() * 0.169).clamp(224.0, 260.0)
            })
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(Margin::symmetric(8, 18)),
            )
            .show(ctx, |ui| {
                if compact {
                    self.device_picker(ui, true);
                    ui.add_space(4.0);
                    // The rail leaves room for controls in narrow and short tiled windows.
                    for page in Page::ALL {
                        if self.navigation_button(ui, page).clicked() {
                            self.page = page;
                        }
                    }
                    return;
                }
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    mouse_mark(ui, 42.0, TEXT);
                    ui.label(
                        RichText::new("Mouse Studio")
                            .size(20.0)
                            .family(FontFamily::Name("display".into())),
                    );
                });
                ui.add_space(25.0);
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    self.device_picker(ui, false);
                });
                ui.add_space(5.0);
                egui::Frame::new()
                    .fill(Color32::from_rgb(29, 33, 32))
                    .stroke(Stroke::new(1.0_f32, LINE))
                    .corner_radius(7)
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let (thumb, _) =
                                ui.allocate_exact_size(vec2(42.0, 66.0), Sense::hover());
                            if self.device == MouseModel::MxAnywhere3s {
                                ui.painter().image(
                                    self.images[1].id(),
                                    thumb,
                                    Rect::from_min_max(pos2(0.24, 0.065), pos2(0.76, 0.95)),
                                    Color32::WHITE,
                                );
                            } else {
                                ui.put(
                                    thumb,
                                    Image::new(include_image!("../assets/icons/mouse.svg"))
                                        .fit_to_exact_size(vec2(36.0, 48.0))
                                        .tint(MUTED),
                                );
                            }
                            ui.vertical(|ui| {
                                ui.add_space(6.0);
                                ui.label(RichText::new(self.device.name()).size(15.0).strong());
                                let (status, color) = match self.status {
                                    DeviceStatus::Connected => ("●  Connected", MINT),
                                    DeviceStatus::Disconnected => ("○  Disconnected", MUTED),
                                    DeviceStatus::Unavailable => ("○  Status unknown", MUTED),
                                };
                                ui.label(RichText::new(status).size(12.0).color(color));
                            });
                        });
                    });
                ui.add_space(18.0);
                for page in Page::ALL.into_iter().filter(|p| *p != Page::Settings) {
                    if self.navigation_button(ui, page).clicked() {
                        self.page = page;
                    }
                    ui.add_space(4.0);
                }
                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    ui.add_space(7.0);
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        muted(ui, "Linux desktop");
                    });
                    ui.add_space(12.0);
                    if self.navigation_button(ui, Page::Settings).clicked() {
                        self.page = Page::Settings;
                    }
                });
            });
    }

    fn navigation_button(&self, ui: &mut Ui, page: Page) -> Response {
        let selected = self.page == page;
        let color = if selected { TEXT } else { MUTED };
        let icon = Image::new(page.icon())
            .fit_to_exact_size(vec2(24.0, 24.0))
            .tint(color);
        let text = RichText::new(page.label()).size(16.0).color(color);
        let button = if ui.available_width() < 100.0 {
            Button::image(icon)
        } else {
            Button::new((icon, text, Atom::grow()))
        }
        .fill(if selected {
            RAISED
        } else {
            Color32::TRANSPARENT
        })
        .stroke(Stroke::NONE)
        .corner_radius(7);
        let height = if ui.available_width() < 100.0 {
            40.0
        } else {
            46.0
        };
        let response = ui.add_sized([ui.available_width(), height], button);
        if selected {
            ui.painter().rect_filled(
                Rect::from_min_size(response.rect.min, vec2(3.0, response.rect.height())),
                0,
                MINT,
            );
        }
        if response.has_focus() {
            ui.painter().rect_stroke(
                response.rect.shrink(1.0),
                6,
                Stroke::new(1.0_f32, MINT),
                StrokeKind::Inside,
            );
        }
        response.widget_info(|| {
            WidgetInfo::selected(WidgetType::SelectableLabel, true, selected, page.label())
        });
        response.on_hover_text(page.label())
    }

    fn profile_picker(&mut self, ui: &mut Ui) {
        eyebrow(ui, "ACTIVE PROFILE");
        let roomy = ui.available_width() >= 700.0;
        let status = self
            .live
            .profile_id
            .and_then(|id| self.saved.profiles.iter().find(|p| p.id == id))
            .map(|p| format!("Applied: {}", p.name))
            .unwrap_or_else(|| {
                if self.draft.auto_switch {
                    "Automatic switching"
                } else {
                    "Manual switching"
                }
                .into()
            });
        ui.horizontal(|ui| {
            ComboBox::from_id_salt("active-profile")
                .selected_text(&self.draft.active().name)
                .width(196.0_f32.min(ui.available_width() - 40.0))
                .truncate()
                .height(200.0)
                .show_ui(ui, |ui| {
                    for p in &self.draft.profiles {
                        ui.selectable_value(&mut self.draft.active_id, p.id, &p.name);
                    }
                });
            if roomy {
                for profile in self
                    .draft
                    .profiles
                    .iter()
                    .filter(|p| p.id != self.draft.active_id)
                    .take(2)
                    .cloned()
                    .collect::<Vec<_>>()
                {
                    if ui
                        .add_sized([100.0, 30.0], Button::new(&profile.name).truncate())
                        .on_hover_text(&profile.name)
                        .clicked()
                    {
                        self.draft.active_id = profile.id;
                    }
                }
            }
            let full = self.draft.profiles.len() >= 100;
            if ui
                .add_enabled(!full, Button::new("+"))
                .on_hover_text(if full {
                    "Profile limit reached (100)"
                } else {
                    "Create a profile"
                })
                .clicked()
            {
                self.draft.add_profile(false);
                self.page = Page::Profiles;
            }
            if roomy {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add(Label::new(RichText::new(&status).color(MUTED).size(13.0)).truncate())
                        .on_hover_text(&status);
                });
            }
        });
        if !roomy {
            ui.add(Label::new(RichText::new(&status).color(MUTED).size(13.0)).truncate())
                .on_hover_text(status);
        }
    }

    fn overview(&mut self, ui: &mut Ui) {
        ui.scope(|ui| {
            ui.spacing_mut().interact_size.y = 0.0;
            ui.horizontal(|ui| {
                eyebrow(ui, "YOUR DEVICE");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let label = match self.status {
                        DeviceStatus::Connected => "●  Connected",
                        DeviceStatus::Disconnected => "○  Disconnected",
                        DeviceStatus::Unavailable => "○  Status unavailable",
                    };
                    ui.label(RichText::new(label).size(13.0).color(MUTED));
                });
            });
        });
        ui.label(
            RichText::new(self.device.name())
                .size(if ui.available_width() < 450.0 {
                    32.0
                } else {
                    40.0
                })
                .family(FontFamily::Name("display".into())),
        );
        muted(
            ui,
            format!(
                "Logitech{}{}",
                self.connection
                    .map(|c| format!(" · {c}"))
                    .unwrap_or_default(),
                self.hardware
                    .battery
                    .as_ref()
                    .map(|b| {
                        let level = b
                            .percent
                            .map_or("Battery".into(), |p| format!("Battery {p}%"));
                        format!(" · {level} · {}", b.status)
                    })
                    .unwrap_or_default()
            ),
        );
        ui.add_space(10.0);
        self.profile_picker(ui);
        let stage_height = (ui.ctx().content_rect().height() * 0.37)
            .clamp(180.0, 390.0)
            .min(ui.available_width() * 0.65);
        self.device_stage(ui, stage_height);
        ui.separator();
        ui.label(
            RichText::new("Pointer & scrolling")
                .size(21.0)
                .family(FontFamily::Name("display".into())),
        );
        ui.add_space(5.0);
        self.sensitivity_control(ui);
        ui.horizontal(|ui| {
            if ui.available_width() >= 500.0 {
                ui.add_space(118.0_f32.min(ui.available_width() * 0.2));
            }
            let width =
                ((ui.available_width() - 2.0 * ui.spacing().item_spacing.x) / 3.0).min(92.0);
            for (label, value) in [("Precise", -0.35), ("Balanced", 0.0), ("Fast", 0.4)] {
                if segment(
                    ui,
                    label,
                    (self.draft.active().sensitivity - value).abs() < 0.01,
                    width,
                )
                .clicked()
                {
                    self.draft.active_mut().sensitivity = value;
                }
            }
        });
        ui.add_space(5.0);
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.add_sized(
                [118.0, 32.0],
                Label::new(RichText::new("Scroll direction").strong()),
            );
            let natural = self.draft.active().natural_scroll;
            if segment(ui, "Normal", !natural, 112.0).clicked() {
                self.draft.active_mut().natural_scroll = false;
            }
            if segment(ui, "Reversed", natural, 112.0).clicked() {
                self.draft.active_mut().natural_scroll = true;
            }
        });
        ui.separator();
        if ui.available_width() >= 600.0 {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Acceleration").strong());
                muted(ui, "Adapt pointer speed to your movement.");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    toggle(
                        ui,
                        &mut self.draft.active_mut().acceleration,
                        "Pointer acceleration",
                    );
                });
            });
        } else {
            toggle_setting(
                ui,
                "Acceleration",
                "Adapt pointer speed to your movement.",
                &mut self.draft.active_mut().acceleration,
                "Pointer acceleration",
            );
        }
        ui.separator();
        ui.vertical_centered(|ui| {
            if ui
                .link(RichText::new("All pointer settings  →").color(MINT))
                .clicked()
            {
                self.page = Page::Pointer;
            }
        });
    }

    fn device_stage(&mut self, ui: &mut Ui, height: f32) {
        if self.device != MouseModel::MxAnywhere3s {
            self.product_photos.draw(
                ui,
                self.device,
                height,
                &mut self.view,
                &self.live.pressed,
                &mut self.selected_button,
            );
            return;
        }
        let width = ui.available_width();
        let (stage, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        if self.view == 3 {
            crate::preview3d::draw(
                ui,
                stage,
                &mut self.orbit,
                &[
                    self.live.pressed[0],
                    self.live.pressed[1],
                    self.live.pressed[2],
                    self.live.pressed[6],
                    self.live.pressed[7],
                ],
                &mut self.selected_button,
            );
        } else {
            let uv = match self.view {
                0 => Rect::from_min_max(pos2(0.12, 0.18), pos2(0.90, 0.82)),
                1 => Rect::from_min_max(pos2(0.24, 0.065), pos2(0.76, 0.95)),
                _ => Rect::from_min_max(pos2(0.09, 0.30), pos2(0.94, 0.69)),
            };
            let ratio = self.images[self.view].aspect_ratio() * uv.width() / uv.height();
            let image_height = (height - 12.0).min(width * 0.78 / ratio);
            let image_rect =
                Rect::from_center_size(stage.center(), vec2(image_height * ratio, image_height));
            ui.painter()
                .image(self.images[self.view].id(), image_rect, uv, Color32::WHITE);
            for (index, at) in [(6, vec2(0.40, 0.18)), (7, vec2(0.57, 0.18))] {
                if self.live.pressed[index] {
                    ui.painter()
                        .circle_filled(image_rect.min + image_rect.size() * at, 9.0, MINT);
                }
            }
            if self.view == 0 {
                let targets = [vec2(0.28, 0.48), vec2(0.18, 0.42), vec2(0.29, 0.07)];
                let labels = [vec2(0.09, 0.70), vec2(0.03, 0.48), vec2(0.65, 0.10)];
                for i in 0..3 {
                    let at = image_rect.min + image_rect.size() * targets[i];
                    let label_at = image_rect.min + image_rect.size() * labels[i];
                    let pressed = self.live.pressed[i];
                    let selected = self.selected_button == i || pressed;
                    let color = if selected {
                        MINT
                    } else {
                        TEXT.gamma_multiply(0.8)
                    };
                    ui.painter()
                        .line_segment([at, label_at], Stroke::new(1.1_f32, color));
                    ui.painter().circle_filled(at, 2.5, color);
                    let response = ui.place(
                        Rect::from_center_size(label_at, vec2(27.0, 27.0)),
                        Button::new(
                            RichText::new(BUTTON_NUMBERS[i].to_string())
                                .size(14.0)
                                .color(if pressed { BG } else { color }),
                        )
                        .fill(if pressed { MINT } else { BG })
                        .stroke(Stroke::new(1.3_f32, color))
                        .corner_radius(14),
                    );
                    let label_pos = if i == 2 {
                        label_at + vec2(23.0, 0.0)
                    } else {
                        label_at + vec2(0.0, 25.0)
                    };
                    let align = if i == 2 {
                        Align2::LEFT_CENTER
                    } else {
                        Align2::CENTER_TOP
                    };
                    if width >= 500.0 {
                        ui.painter().text(
                            label_pos,
                            align,
                            BUTTONS[i],
                            FontId::proportional(13.0),
                            color,
                        );
                    }
                    if response.on_hover_text(BUTTONS[i]).clicked() {
                        self.selected_button = i;
                    }
                }
            }
        }
        ui.horizontal(|ui| {
            ui.add_space((ui.available_width() - 492.0).max(0.0) / 2.0);
            let width = ((ui.available_width() - 44.0) / 4.0).min(112.0);
            ui.spacing_mut().item_spacing.x = 0.0;
            for (i, label) in ["Perspective", "Top", "Side", "3D"].iter().enumerate() {
                let label = if i == 0 && width < 112.0 {
                    "Angle"
                } else {
                    label
                };
                if segment(ui, label, self.view == i, width)
                    .on_hover_text(["Perspective", "Top", "Side", "3D"][i])
                    .clicked()
                {
                    self.view = i;
                }
            }
            ui.add_space(14.0);
            if ui
                .add_sized(
                    [30.0, 30.0],
                    Button::new(
                        Image::new(include_image!("../assets/icons/rotate-ccw.svg"))
                            .fit_to_exact_size(vec2(17.0, 17.0))
                            .tint(MUTED),
                    ),
                )
                .on_hover_text("Reset view")
                .clicked()
            {
                self.view = 0;
                self.orbit = Default::default();
            }
        });
        ui.vertical_centered(|ui| {
            muted(
                ui,
                if self.view == 3 {
                    "3D model · Drag to rotate · Scroll to zoom · Click a hotspot"
                } else {
                    "Click a button to configure · Live presses light up after Save"
                },
            );
        });
        ui.add_space(7.0);
    }

    fn inspector(&mut self, ui: &mut Ui) {
        let number = if self.device != MouseModel::MxAnywhere3s {
            device::BUTTON_MARKERS[self.selected_button].to_string()
        } else {
            BUTTON_NUMBERS[self.selected_button].to_string()
        };
        eyebrow(ui, &format!("BUTTON {number:0>2}"));
        ui.label(
            RichText::new(BUTTONS[self.selected_button])
                .size(28.0)
                .family(FontFamily::Name("display".into())),
        );
        muted(ui, "Choose what this button does.");
        ui.add_space(15.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let width = ui.available_width() / 3.0;
            for (i, label) in ["Click", "Hold", "Double-click"].iter().enumerate() {
                if segment(ui, label, self.trigger == i, width).clicked() {
                    self.trigger = i;
                }
            }
        });
        ui.add_space(14.0);
        ui.label("Action type");
        let native_click_suppressed = self.selected_button >= 4
            && self.trigger == 0
            && self.draft.active().button_managed(self.selected_button);
        let profile_id = self.draft.active_id;
        let binding = crate::software_ui::binding_mut(
            self.draft.active_mut(),
            self.selected_button,
            self.trigger,
        );
        let old_kind = binding.kind;
        ComboBox::from_id_salt("action-type")
            .width(ui.available_width())
            .truncate()
            .selected_text(binding.kind.label())
            .show_ui(ui, |ui| {
                for kind in ActionKind::ALL {
                    ui.selectable_value(&mut binding.kind, kind, kind.label());
                }
            });
        if binding.kind != old_kind {
            binding.value.clear();
        }
        ui.add_space(9.0);
        match binding.kind {
            ActionKind::CycleDpi => {
                muted(
                    ui,
                    "Advance through this profile's DPI presets. Edit them on Pointer, then Save.",
                );
            }
            ActionKind::Default => {
                ui.label("Current action");
                egui::Frame::new()
                    .fill(RAISED)
                    .stroke(Stroke::new(1.0_f32, LINE))
                    .corner_radius(6)
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(if native_click_suppressed {
                            "No click action assigned"
                        } else if self.trigger == 0 {
                            DEFAULTS[self.selected_button]
                        } else {
                            "No action assigned"
                        });
                    });
                if native_click_suppressed {
                    muted(
                        ui,
                        "Custom triggers take over this control. Choose a Click action to run when released without a hold or gesture.",
                    );
                }
            }
            ActionKind::ActionsRing => {
                muted(
                    ui,
                    "Opens a radial menu at the cursor. Hold the button, move to a slice and release, or tap and click a slice.",
                );
                if ui.button("Edit ring").clicked() {
                    self.software_ui.ring.open();
                }
            }
            ActionKind::Macro => {
                muted(ui, "A sequence of shortcuts and delays.");
                if ui.button("Edit macro").clicked() {
                    self.software_ui.open_macro(
                        profile_id,
                        self.selected_button,
                        self.trigger,
                        binding,
                    );
                }
            }
            ActionKind::Application => {
                ui.label("Application");
                let name = match binding.value.as_str() {
                    "code" => "Visual Studio Code",
                    "brave" => "Brave",
                    "blender" => "Blender",
                    "" => "Choose application",
                    other => other,
                };
                let icon = if binding.value == "code" {
                    Image::new(&self.images[3]).fit_to_exact_size(vec2(26.0, 26.0))
                } else {
                    Image::new(include_image!("../assets/icons/app-window.svg"))
                        .fit_to_exact_size(vec2(24.0, 24.0))
                        .tint(MUTED)
                };
                if ui
                    .add_sized(
                        [ui.available_width(), 44.0],
                        Button::new((
                            icon,
                            RichText::new(name).size(15.0),
                            Atom::grow(),
                            RichText::new("Change").size(12.0).color(MINT),
                        ))
                        .fill(PANEL)
                        .truncate(),
                    )
                    .clicked()
                {
                    self.application_picker_open = !self.application_picker_open;
                }
                if self.application_picker_open || binding.value.is_empty() {
                    ui.add(
                        TextEdit::singleline(&mut binding.value)
                            .hint_text("Application command, e.g. code")
                            .desired_width(f32::INFINITY)
                            .margin(vec2(10.0, 8.0)),
                    );
                    ui.horizontal_wrapped(|ui| {
                        for (name, command) in [
                            ("VS Code", "code"),
                            ("Brave", "brave"),
                            ("Blender", "blender"),
                        ] {
                            if ui.small_button(name).clicked() {
                                binding.value = command.into();
                                self.application_picker_open = false;
                            }
                        }
                    });
                }
            }
            kind => {
                let (label, hint) = match kind {
                    ActionKind::Shortcut => ("Keyboard shortcut", "e.g. CTRL+C"),
                    ActionKind::Command => ("Command", "e.g. kitty"),
                    ActionKind::Workspace => ("Workspace number", "1"),
                    ActionKind::Macro => unreachable!(),
                    ActionKind::Default
                    | ActionKind::Application
                    | ActionKind::CycleDpi
                    | ActionKind::ActionsRing => {
                        unreachable!()
                    }
                };
                ui.label(label);
                ui.add(
                    TextEdit::singleline(&mut binding.value)
                        .hint_text(hint)
                        .desired_width(f32::INFINITY)
                        .margin(vec2(12.0, 10.0)),
                );
            }
        }
        ui.add_space(22.0);
        ui.horizontal(|ui| {
            eyebrow(ui, "OTHER ACTIONS");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .link(
                        RichText::new("Advanced button settings")
                            .size(11.0)
                            .color(MINT),
                    )
                    .clicked()
                {
                    self.software_ui.advanced_open = true;
                }
            });
        });
        for kind in ActionKind::ALL.into_iter().filter(|k| {
            *k != old_kind
                && !matches!(
                    *k,
                    ActionKind::Macro | ActionKind::CycleDpi | ActionKind::ActionsRing
                )
        }) {
            let icon = match kind {
                ActionKind::Default => include_image!("../assets/icons/rotate-ccw.svg"),
                ActionKind::Application => include_image!("../assets/icons/app-window.svg"),
                ActionKind::Shortcut => include_image!("../assets/icons/keyboard.svg"),
                ActionKind::Command => include_image!("../assets/icons/terminal.svg"),
                ActionKind::Workspace => include_image!("../assets/icons/layout-grid.svg"),
                ActionKind::Macro => include_image!("../assets/icons/keyboard.svg"),
                ActionKind::CycleDpi | ActionKind::ActionsRing => {
                    include_image!("../assets/icons/mouse-pointer-2.svg")
                }
            };
            let row = Button::new((
                Image::new(icon)
                    .fit_to_exact_size(vec2(23.0, 23.0))
                    .tint(TEXT),
                RichText::new(kind.label()).size(15.0),
                Atom::grow(),
            ))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE);
            if ui.add_sized([ui.available_width(), 38.0], row).clicked() && binding.kind != kind {
                binding.kind = kind;
                binding.value.clear();
            }
            ui.separator();
        }
        // Keep the actions in the same lower-right position as the reference at desktop sizes.
        let remaining = ui.clip_rect().bottom() - ui.cursor().top() - 270.0;
        if remaining.is_finite() && remaining > 0.0 {
            ui.add_space(remaining);
        }
        ui.add_space(9.0);
        ui.label("Applies to");
        ComboBox::from_id_salt("inspector-profile")
            .width(ui.available_width())
            .truncate()
            .selected_text(&self.draft.active().name)
            .show_ui(ui, |ui| {
                for p in &self.draft.profiles {
                    ui.selectable_value(&mut self.draft.active_id, p.id, &p.name);
                }
            });
        muted(ui, "Save to apply this profile’s mouse actions.");
        ui.add_space(14.0);
        if ui
            .add_enabled(
                self.backend.is_some()
                    && !self.is_applying()
                    && self.selected_binding().kind != ActionKind::Default,
                Button::new("Test action").min_size(vec2(ui.available_width(), 36.0)),
            )
            .on_hover_text(
                "Run this draft action in 3 seconds; switch to the target window for shortcuts.",
            )
            .on_disabled_hover_text(
                "Choose a custom action. Default behavior uses the physical button.",
            )
            .clicked()
        {
            self.test_selected_action();
        }
        if ui
            .add_enabled(
                !self.load_blocked && !self.is_applying(),
                Button::new(RichText::new("Save profiles").color(BG).strong())
                    .fill(MINT)
                    .min_size(vec2(ui.available_width(), 38.0)),
            )
            .clicked()
        {
            self.save_settings();
        }
        ui.horizontal(|ui| {
            if ui
                .link(RichText::new("Restore default").color(MINT))
                .clicked()
            {
                *crate::software_ui::binding_mut(
                    self.draft.active_mut(),
                    self.selected_button,
                    self.trigger,
                ) = Binding::default();
            }
            if self.backend.is_some() && ui.link("Cancel actions").clicked() {
                self.cancel_actions();
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_enabled(self.dirty(), Button::new("Discard").frame(false))
                    .clicked()
                {
                    self.draft = self.saved.clone();
                    self.notify("Unsaved edits discarded.", false);
                }
            });
        });
        let status = if self.is_error || self.is_applying() {
            self.message.as_str()
        } else if self.live.macro_running {
            "Macro running · Cancel actions to stop"
        } else if self.dirty() {
            "●  Unsaved changes"
        } else {
            self.message.as_str()
        };
        ui.add(
            Label::new(RichText::new(status).size(12.0).color(if self.is_error {
                ERROR
            } else {
                MUTED
            }))
            .truncate(),
        )
        .on_hover_text(&self.message);
    }

    fn buttons_page(&mut self, ui: &mut Ui) {
        heading(
            ui,
            "Buttons",
            if self.device == MouseModel::MxMaster4 {
                "Six configurable controls. Give each a purpose."
            } else {
                "Three everyday controls. Give each a purpose."
            },
        );
        self.profile_picker(ui);
        self.device_stage(ui, 285.0);
        for (i, label) in BUTTONS[..self.device.button_count()].iter().enumerate() {
            let binding = &self.draft.active().bindings[i];
            let summary = if binding.kind == ActionKind::Macro {
                format!(
                    "Macro · {} steps",
                    crate::software::parse_macro(&binding.value).map_or(0, |s| s.len())
                )
            } else if binding.kind == ActionKind::Default {
                DEFAULTS[i].to_string()
            } else if binding.kind == ActionKind::ActionsRing {
                format!(
                    "Actions Ring · {} slices",
                    self.draft.active().advanced.ring.slices.len()
                )
            } else {
                format!("{}  ·  {}", binding.kind.label(), binding.value)
            };
            let fill = if self.selected_button == i {
                RAISED
            } else {
                PANEL
            };
            let number = if self.device != MouseModel::MxAnywhere3s {
                device::BUTTON_MARKERS[i].to_string()
            } else {
                BUTTON_NUMBERS[i].to_string()
            };
            if ui
                .add_sized(
                    [ui.available_width(), 55.0],
                    Button::new(format!("{}   {}     ·     {}", number, label, summary))
                        .fill(fill)
                        .truncate(),
                )
                .clicked()
            {
                self.selected_button = i;
            }
        }
        ui.add_space(15.0);
        muted(
            ui,
            if self.device == MouseModel::MxMaster4 {
                "Save applies extra button assignments. Unassigned controls keep their native behavior. Primary clicks and Easy-Switch remain native."
            } else {
                "Primary clicks and the physical wheel-mode button keep their normal behavior."
            },
        );
    }

    fn sensitivity_control(&mut self, ui: &mut Ui) {
        let p = self.draft.active_mut();
        ui.horizontal(|ui| {
            ui.label("Pointer speed");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{:+.2}", p.sensitivity))
                        .color(MINT)
                        .monospace(),
                );
            });
        });
        let indent = if self.page == Page::Overview && ui.available_width() >= 500.0 {
            130.0
        } else {
            0.0
        };
        ui.horizontal(|ui| {
            ui.add_space(indent);
            ui.vertical(|ui| {
                mint_slider(ui, &mut p.sensitivity, -1.0..=1.0, 0.01);
                ui.scope(|ui| {
                    ui.spacing_mut().interact_size.y = 0.0;
                    ui.horizontal(|ui| {
                        muted(ui, "Precise");
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            muted(ui, "Fast");
                        });
                    });
                });
            });
        });
    }

    fn pointer_page(&mut self, ui: &mut Ui) {
        heading(ui, "Pointer", "Find a pace that feels right.");
        self.profile_picker(ui);
        ui.add_space(24.0);
        ui.add_enabled_ui(
            self.device == MouseModel::MxAnywhere3s || self.hardware.available,
            |ui| {
                crate::hardware::ui::dpi(ui, &mut self.draft.active_mut().hardware, &self.hardware);
            },
        );
        if self.device != MouseModel::MxAnywhere3s && !self.hardware.available {
            muted(
                ui,
                "Connect this mouse over Bluetooth or a supported Bolt receiver with HID access to discover sensor DPI support.",
            );
        }
        crate::hardware::ui::access(ui, &self.hardware);
        ui.add_space(14.0);
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            self.sensitivity_control(ui);
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                for (label, value) in [("Precise", -0.35), ("Balanced", 0.0), ("Fast", 0.4)] {
                    if ui.button(label).clicked() {
                        self.draft.active_mut().sensitivity = value;
                    }
                }
            });
        });
        ui.add_space(14.0);
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            toggle_setting(
                ui,
                "Pointer acceleration",
                "On: adaptive speed. Off: constant speed.",
                &mut self.draft.active_mut().acceleration,
                "Pointer acceleration",
            );
        });
        ui.add_space(24.0);
        muted(
            ui,
            "Save profiles to apply sensor DPI, pointer speed and acceleration.",
        );
    }

    fn scrolling_page(&mut self, ui: &mut Ui) {
        heading(ui, "Scrolling", "Move through your work at your own speed.");
        self.profile_picker(ui);
        ui.add_space(24.0);
        if self.device == MouseModel::MxMaster4 {
            crate::hardware::ui::thumbwheel(
                ui,
                &mut self.draft.active_mut().hardware,
                &self.hardware,
            );
            ui.add_space(14.0);
            if crate::hardware::ui::haptics(
                ui,
                &mut self.draft.active_mut().hardware,
                &self.hardware,
            ) {
                self.start_haptic_test();
            }
            ui.add_space(14.0);
        }
        if self.device.has_mx_wheel() {
            ui.add_enabled_ui(
                self.device == MouseModel::MxAnywhere3s || self.hardware.available,
                |ui| {
                    crate::hardware::ui::wheel(
                        ui,
                        &mut self.draft.active_mut().hardware,
                        &self.hardware,
                    );
                },
            );
        } else {
            muted(
                ui,
                "M650 L uses SmartWheel. Change scroll speed and direction below; MX wheel-mode and SmartShift settings do not apply.",
            );
        }
        crate::hardware::ui::access(ui, &self.hardware);
        ui.add_space(14.0);
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            let p = self.draft.active_mut();
            ui.horizontal(|ui| {
                ui.label(RichText::new("Scroll speed").strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{:.1}×", p.scroll_speed))
                            .color(MINT)
                            .monospace(),
                    );
                });
            });
            ui.spacing_mut().slider_width = (ui.available_width() - 12.0).max(120.0);
            ui.add(
                Slider::new(&mut p.scroll_speed, 0.1..=5.0)
                    .show_value(false)
                    .step_by(0.1),
            );
            ui.horizontal(|ui| {
                muted(ui, "Gentle · 0.1×");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    muted(ui, "Quick · 5×");
                });
            });
            if ui.small_button("Reset to 1×").clicked() {
                p.scroll_speed = 1.0;
            }
        });
        ui.add_space(14.0);
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            toggle_setting(
                ui,
                "Natural scrolling",
                "Reverse the wheel's scrolling direction.",
                &mut self.draft.active_mut().natural_scroll,
                "Natural scrolling",
            );
        });
        ui.add_space(24.0);
        muted(
            ui,
            "Save profiles to apply scrolling settings and restore native side-wheel scrolling on MX Master 4.",
        );
    }

    fn profiles_page(&mut self, ui: &mut Ui) {
        heading(
            ui,
            "Profiles",
            "A different setup for a different kind of day.",
        );
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    self.draft.profiles.len() < 100,
                    Button::new("+  New profile"),
                )
                .clicked()
            {
                self.draft.add_profile(false);
            }
            if ui
                .add_enabled(
                    self.draft.profiles.len() < 100,
                    Button::new("Duplicate selected"),
                )
                .clicked()
            {
                self.draft.add_profile(true);
            }
        });
        ui.add_space(14.0);
        for profile in &self.draft.profiles {
            let active = self.draft.active_id == profile.id;
            let summary = format!(
                "{}   {}     ·     {}",
                if active { "●" } else { "○" },
                profile.name,
                if active { "Selected" } else { "Select profile" }
            );
            if ui
                .add_sized(
                    [ui.available_width(), 54.0],
                    Button::new(RichText::new(summary).color(if active { MINT } else { TEXT }))
                        .fill(if active { RAISED } else { PANEL })
                        .truncate(),
                )
                .clicked()
            {
                self.draft.active_id = profile.id;
            }
        }
        ui.add_space(20.0);
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Profile name").strong());
            ui.add(
                TextEdit::singleline(&mut self.draft.active_mut().name)
                    .desired_width(360.0_f32.min(ui.available_width() - 20.0))
                    .margin(vec2(10.0, 10.0)),
            );
            muted(
                ui,
                "Switch profiles using the selector on any settings page.",
            );
            ui.add_space(8.0);
            if ui
                .add_enabled(
                    self.draft.profiles.len() > 1,
                    Button::new(RichText::new("Delete selected profile").color(ERROR)),
                )
                .clicked()
            {
                self.confirm = Some(Confirm::DeleteProfile);
            }
        });
        ui.add_space(14.0);
        self.software_ui.profile_editor(ui, &mut self.draft);
        ui.add_space(14.0);
        if ui.button("Reset selected profile to defaults").clicked() {
            crate::software::reset_profile(self.draft.active_mut());
            self.notify(
                "Profile reset in your draft. Save to apply, or Discard to undo.",
                false,
            );
        }
    }

    fn settings_page(&mut self, ui: &mut Ui) {
        heading(ui, "Settings", "Your setup, stored on your desktop.");
        if self
            .hardware
            .error
            .as_ref()
            .is_some_and(|e| e.contains("device-access script"))
        {
            card().show(ui,|ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("Mouse hardware access").strong());
                crate::hardware::ui::access(ui,&self.hardware);
                muted(ui,"One-time setup: run the following in your terminal, then the app retries automatically.");
                let installed="/usr/share/mouse-studio/scripts/install-device-access.sh";
                let script=if std::path::Path::new(installed).exists(){installed}else{concat!(env!("CARGO_MANIFEST_DIR"),"/scripts/install-device-access.sh")};
                let command=&format!("sudo bash \"{script}\"");
                ui.label(RichText::new(command).monospace().size(12.0));
                if ui.button("Copy setup command").clicked(){ui.ctx().copy_text(command.clone());}
            });
            ui.add_space(14.0);
        }
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Local storage").strong());
            ui.add(
                Label::new(
                    RichText::new(self.path.display().to_string())
                        .monospace()
                        .size(12.0),
                )
                .wrap(),
            );
            muted(ui, "Save profiles with the button below or Ctrl+S.");
            if self.load_blocked {
                ui.colored_label(
                    ERROR,
                    "The existing file could not be loaded. Saving is disabled to protect it.",
                );
                if ui.button("Retry loading saved settings").clicked() {
                    match Settings::load_from(&self.path) {
                        Ok(s) if s.device != self.device => self.notify(
                            "The saved file belongs to another mouse. Fix or remove it first.",
                            true,
                        ),
                        Ok(s) => {
                            self.saved = s.clone();
                            self.draft = s;
                            self.load_blocked = false;
                            self.previous_setup = recovery::previous_setup(&self.path);
                            self.notify("Saved settings loaded.", false);
                        }
                        Err(e) => self.notify(e, true),
                    }
                }
            }
        });
        ui.add_space(14.0);
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Previous setup").strong());
            muted(
                ui,
                "Undo your last saved change. Keeps all profiles and button assignments.",
            );
            match &self.previous_setup {
                Ok(Some(previous)) => {
                    ui.label(format!(
                        "{} · {} profile{}",
                        previous.active().name,
                        previous.profiles.len(),
                        if previous.profiles.len() == 1 {
                            ""
                        } else {
                            "s"
                        },
                    ));
                }
                Ok(None) => {
                    muted(
                        ui,
                        "No previous setup yet. Saving a change to an existing setup creates one.",
                    );
                }
                Err(error) => {
                    ui.colored_label(ERROR, error);
                }
            }
            if ui
                .add_enabled(
                    matches!(self.previous_setup, Ok(Some(_)))
                        && !self.load_blocked
                        && !self.is_applying(),
                    Button::new("Restore previous setup"),
                )
                .clicked()
            {
                // Read again before confirmation, in case another window saved meanwhile.
                self.previous_setup = recovery::previous_setup(&self.path);
                match &self.previous_setup {
                    Ok(Some(previous)) => self.confirm = Some(Confirm::Restore(previous.clone())),
                    Ok(None) => self.notify("No previous saved setup is available.", true),
                    Err(error) => self.notify(error.clone(), true),
                }
            }
        });
        ui.add_space(14.0);
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            toggle_setting(ui, "Automatic restore", "Restore the saved profile at login, reconnect and Hyprland reload.", &mut self.draft.restore_on_login, "Restore settings automatically");
            muted(
                ui,
                "Save profiles to enable or disable. Runs quietly while this window is closed.",
            );
            if self.draft.hardware_required() || self.draft.horizontal_required() || self.draft.buttons_required() || self.draft.thumbwheel_required() {
                muted(ui,"Side-wheel restoration, extra buttons, hardware settings and horizontal scrolling keep the helper running while configured.");
            }
        });
        ui.add_space(14.0);
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Import & export").strong());
            muted(ui, "Back up all profiles or bring back a previous setup.");
            ui.label("JSON file path");
            ui.add(
                TextEdit::singleline(&mut self.transfer_path)
                    .desired_width(f32::INFINITY)
                    .margin(vec2(10.0, 10.0)),
            );
            ui.horizontal_wrapped(|ui| {
                if ui.button("Export profiles").clicked() {
                    let path = PathBuf::from(self.transfer_path.trim());
                    if !path.is_absolute() {
                        self.notify("Enter an absolute JSON file path.", true);
                    } else if self.is_settings_file(&path) {
                        self.notify(
                            "That is a live Mouse Studio settings file. Choose a different path.",
                            true,
                        );
                    } else if path.exists() {
                        self.confirm = Some(Confirm::OverwriteExport(path));
                    } else {
                        self.export(path);
                    }
                }
                if ui.button("Import profiles").clicked() {
                    let path = PathBuf::from(self.transfer_path.trim());
                    if !path.is_absolute() {
                        self.notify("Enter an absolute JSON file path.", true);
                    } else {
                        match Settings::load_from(&path) {
                            Ok(s) if s.device == self.device => {
                                self.confirm = Some(Confirm::Import(s))
                            }
                            Ok(_) => self.notify(
                                "Import belongs to another mouse. Select that mouse first.",
                                true,
                            ),
                            Err(e) => self.notify(format!("Import failed: {e}"), true),
                        }
                    }
                }
            });
        });
        ui.add_space(14.0);
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("About Mouse Studio").strong());
            muted(ui, "Mouse Studio 0.1 · MX Anywhere 3S, Signature M650 L & MX Master 4");
            ui.label("Save profiles applies pointer, scrolling, button triggers, gestures, macros and application profiles in Hyprland.");
            muted(ui,"Horizontal scrolling requires the user helper; Save manages it automatically when a profile uses this mode.");
            muted(ui, "Enable Automatic restore to keep your saved profile active across login, reconnect and config reload.");
        });
    }

    /// True when `path` is this or another mouse's live settings file, however spelled.
    fn is_settings_file(&self, path: &std::path::Path) -> bool {
        let canonical = |p: &std::path::Path| {
            let name = p.file_name()?.to_owned();
            Some(p.parent()?.canonicalize().ok()?.join(name))
        };
        let target = canonical(path);
        path == self.path
            || crate::device::MouseModel::ALL.iter().any(|model| {
                let live = model.settings_path(&self.base_path);
                live == path || (target.is_some() && canonical(&live) == target)
            })
    }

    fn export(&mut self, path: PathBuf) {
        match self.draft.save_to(&path) {
            Ok(()) => self.notify(format!("Profiles exported to {}", path.display()), false),
            Err(e) => self.notify(e, true),
        }
    }

    fn software_panels(&mut self, ctx: &Context) {
        let busy = self.is_applying();
        let events = self
            .software_ui
            .show(ctx, &mut self.draft, self.backend.is_some(), busy);
        for event in events {
            match event {
                crate::software_ui::Event::Test(binding) => self.start_test(binding),
                crate::software_ui::Event::Cancel => self.cancel_actions(),
            }
        }
    }
    fn cancel_actions(&mut self) {
        self.test_cancel
            .store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(backend) = self.backend.clone() {
            let (tx, rx) = mpsc::channel();
            self.pending_cancel = Some(rx);
            std::thread::spawn(move || {
                let _ = tx.send(backend.cancel_actions());
            });
            self.notify("Cancelling pending actions…", false);
        }
    }

    fn confirmation(&mut self, ctx: &Context) {
        let Some(confirm) = self.confirm.clone() else {
            return;
        };
        let (title, body, action) = match &confirm {
            Confirm::Close => (
                "Save before closing?",
                "There are unsaved mouse profiles. Discard & close discards edits for all mice. To keep edits on other mice, Cancel and save each mouse first.",
                "Discard & close",
            ),
            Confirm::DeleteProfile => (
                "Delete this profile?",
                "The selected profile will be removed from your draft. You can discard changes before saving.",
                "Delete profile",
            ),
            Confirm::Import(_) => (
                "Replace your profiles?",
                "Import replaces all draft profiles. Save afterward to keep them, or Discard to restore your saved setup.",
                "Import profiles",
            ),
            Confirm::OverwriteExport(_) => (
                "Replace the export file?",
                "A file already exists at this path. Exporting will replace its contents.",
                "Replace file",
            ),
            Confirm::Restore(_) => (
                "Restore this setup?",
                "Replaces all profiles, button assignments and the automatic-restore preference. Unsaved edits will be discarded. Your current saved setup will become the restore point.",
                "Restore & apply",
            ),
        };
        Modal::new(Id::new("confirm")).show(ctx, |ui| {
            ui.set_width(410.0_f32.min(ctx.content_rect().width() - 48.0));
            ui.heading(title);
            ui.label(body);
            if let Confirm::Restore(settings) = &confirm {
                ui.label(format!(
                    "Restore: {} · {} profile(s)",
                    settings.active().name,
                    settings.profiles.len(),
                ));
            }
            ui.add_space(16.0);
            ui.horizontal_wrapped(|ui| {
                if ui.button("Cancel").clicked() {
                    self.close_after_apply = false;
                    self.confirm = None;
                }
                if matches!(confirm, Confirm::Close)
                    && !self.other_devices_dirty()
                    && ui
                        .add_enabled(!self.is_applying(), Button::new("Save & close"))
                        .clicked()
                {
                    if self.save_settings() {
                        self.allow_close = true;
                        self.confirm = None;
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    } else if self.is_applying() {
                        self.close_after_apply = true;
                    }
                }
                if ui
                    .add_enabled(!self.is_applying(), Button::new(action))
                    .clicked()
                {
                    match confirm {
                        Confirm::Close => {
                            self.allow_close = true;
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }
                        Confirm::DeleteProfile => {
                            self.draft.profiles.retain(|p| p.id != self.draft.active_id);
                            self.draft.active_id = self.draft.profiles[0].id;
                        }
                        Confirm::Import(s) => {
                            self.draft = s;
                            self.notify(
                                "Imported into your draft. Save profiles to keep it.",
                                false,
                            );
                        }
                        Confirm::OverwriteExport(path) => self.export(path),
                        Confirm::Restore(settings) => {
                            self.store_settings(settings, true);
                        }
                    }
                    self.confirm = None;
                }
            });
        });
    }
}

impl eframe::App for Studio {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.show(ctx);
    }
}
