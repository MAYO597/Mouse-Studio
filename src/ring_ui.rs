//! Actions Ring drawing, the in-app editor with live preview, and the overlay process
//! (`mouse-studio --ring <profile-id> <device>`). The overlay never runs inside the settings
//! window: it is a separate frameless, transparent window that exits on focus loss.
use crate::{
    model::{ActionKind, Binding, Settings},
    ring::{self, Hit, MAX_LABEL_CHARS, MAX_SLICES, Outcome, Ring, Session},
    theme::*,
};
use eframe::egui::*;
use std::{cell::Cell, collections::HashMap, rc::Rc, sync::mpsc, time::Duration, time::Instant};

const BACKDROP: Color32 = Color32::from_rgba_premultiplied(17, 19, 19, 235);
const HOVER: Color32 = Color32::from_rgba_premultiplied(166, 227, 204, 240);

fn rim_point(center: Pos2, angle: f32, r: f32) -> Pos2 {
    center + vec2(r * angle.sin(), -r * angle.cos())
}

/// Draws the ring. `scale` shrinks it for the editor preview.
pub fn paint_ring(painter: &Painter, center: Pos2, ring: &Ring, hover: Option<usize>, scale: f32) {
    let radius = ring.radius * scale;
    let dead = ring.dead_zone * scale;
    let n = ring.slices.len();
    let stroke = Stroke::new(1.0_f32, LINE);
    if n < 2 {
        painter.circle(center, radius, BACKDROP, stroke);
        painter.text(
            center,
            Align2::CENTER_CENTER,
            "Add two or more slices",
            FontId::proportional(12.0),
            MUTED,
        );
        return;
    }
    let step = std::f32::consts::TAU / n as f32;
    for i in 0..n {
        let centre_angle = ring::slice_angle(i, n);
        let mut points = vec![center];
        for k in 0..=14 {
            let a = centre_angle - step / 2.0 + step * k as f32 / 14.0;
            points.push(rim_point(center, a, radius));
        }
        let hovered = hover == Some(i);
        painter.add(Shape::convex_polygon(
            points,
            if hovered { HOVER } else { BACKDROP },
            stroke,
        ));
    }
    let middle = (radius + dead) / 2.0;
    let wrap = (step * middle * 0.85).clamp(48.0 * scale, 120.0 * scale);
    for (i, slice) in ring.slices.iter().enumerate() {
        let (dx, dy) = ring::slice_offset(i, n, middle);
        let color = if hover == Some(i) { BG } else { TEXT };
        let galley = painter.layout(
            slice.label.clone(),
            FontId::proportional((13.0 * scale.max(0.75)).min(14.0)),
            color,
            wrap,
        );
        painter.galley(center + vec2(dx, dy) - galley.size() / 2.0, galley, color);
    }
    painter.circle(center, dead, BG, stroke);
    let caption = hover
        .and_then(|i| ring.slices.get(i))
        .map_or("Esc", |s| s.action.kind.label());
    painter.text(
        center,
        Align2::CENTER_CENTER,
        caption,
        FontId::proportional(10.0),
        MUTED,
    );
}

// ---- editor ----------------------------------------------------------------------------------

#[derive(Default)]
pub struct Editor {
    pub open: bool,
    macro_drafts: HashMap<usize, String>,
    macro_error: String,
}

enum Edit {
    Remove(usize),
    Move(usize, isize),
}

impl Editor {
    pub fn open(&mut self) {
        self.open = true;
        self.macro_drafts.clear();
        self.macro_error.clear();
    }

    pub fn show(&mut self, ctx: &Context, settings: &mut Settings) {
        if !self.open {
            return;
        }
        let profile = settings.active_mut();
        let name = profile.name.clone();
        let used = profile.uses_ring();
        let ring = &mut profile.advanced.ring;
        let mut edit = None;
        let mut close = false;
        Modal::new(Id::new("actions-ring-editor")).show(ctx, |ui| {
            ui.set_width(640.0_f32.min(ctx.content_rect().width() - 48.0));
            ui.heading("Actions Ring");
            muted(
                ui,
                format!(
                    "Profile ‘{name}’. Up to {MAX_SLICES} slices, clockwise from the top. Changes stay in your draft until Save profiles."
                ),
            );
            ScrollArea::vertical()
                .id_salt("ring-body")
                .max_height((ctx.content_rect().height() - 180.0).clamp(120.0, 600.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let side = (ring.radius * 2.0 * 0.85 + 20.0).min(ui.available_width());
                    let (rect, response) =
                        ui.allocate_exact_size(vec2(ui.available_width(), side), Sense::hover());
                    let scale = (side - 20.0) / 2.0 / ring.radius;
                    let center = rect.center();
                    let hover = response.hover_pos().and_then(|p| {
                        match ring::hit(
                            p.x - center.x,
                            p.y - center.y,
                            ring.slices.len(),
                            ring.dead_zone * scale,
                            ring.radius * scale,
                        ) {
                            Hit::Slice(i) => Some(i),
                            _ => None,
                        }
                    });
                    paint_ring(&ui.painter_at(rect), center, ring, hover, scale);
                    muted(
                        ui,
                        "Preview. Hover a slice to highlight it; nothing runs from here.",
                    );
                    ui.add(Slider::new(&mut ring.radius, 90.0..=220.0).text("Ring radius"));
                    ui.add(Slider::new(&mut ring.dead_zone, 10.0..=80.0).text("Centre (no selection)"));
                    toggle(
                        ui,
                        &mut ring.release_to_select,
                        "Choose by releasing the button over a slice",
                    );
                    ui.separator();
                    let count = ring.slices.len();
                    for (i, slice) in ring.slices.iter_mut().enumerate() {
                        ui.push_id(i, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(format!("{}", i + 1));
                                ui.add(
                                    TextEdit::singleline(&mut slice.label)
                                        .desired_width(110.0)
                                        .char_limit(MAX_LABEL_CHARS)
                                        .hint_text("Label"),
                                );
                                let old = slice.action.kind;
                                ComboBox::from_id_salt("ring-kind")
                                    .width(150.0)
                                    .selected_text(slice.action.kind.label())
                                    .show_ui(ui, |ui| {
                                        for kind in ActionKind::ALL.into_iter().filter(|k| {
                                            !matches!(k, ActionKind::Default | ActionKind::ActionsRing)
                                        }) {
                                            ui.selectable_value(
                                                &mut slice.action.kind,
                                                kind,
                                                kind.label(),
                                            );
                                        }
                                    });
                                if old != slice.action.kind {
                                    slice.action.value.clear();
                                    self.macro_drafts.remove(&i);
                                }
                                match slice.action.kind {
                                    ActionKind::CycleDpi => {}
                                    ActionKind::Macro => {
                                        let text = self.macro_drafts.entry(i).or_insert_with(|| {
                                            crate::software::macro_text(&slice.action)
                                        });
                                        if ui
                                            .add(
                                                TextEdit::multiline(text)
                                                    .desired_rows(2)
                                                    .desired_width(170.0)
                                                    .hint_text("CTRL+C\nwait 250"),
                                            )
                                            .changed()
                                        {
                                            match crate::software::compose_macro(text) {
                                                Ok(value) => {
                                                    slice.action.value = value;
                                                    self.macro_error.clear();
                                                }
                                                Err(e) => self.macro_error = e,
                                            }
                                        }
                                    }
                                    kind => {
                                        let hint = match kind {
                                            ActionKind::Shortcut => "CTRL+C",
                                            ActionKind::Workspace => "1–99",
                                            ActionKind::Application => "code",
                                            _ => "kitty",
                                        };
                                        ui.add(
                                            TextEdit::singleline(&mut slice.action.value)
                                                .desired_width(170.0)
                                                .hint_text(hint),
                                        );
                                    }
                                }
                                if i > 0 && ui.small_button("Up").clicked() {
                                    edit = Some(Edit::Move(i, -1));
                                }
                                if i + 1 < count && ui.small_button("Down").clicked() {
                                    edit = Some(Edit::Move(i, 1));
                                }
                                if ui.small_button("Remove").clicked() {
                                    edit = Some(Edit::Remove(i));
                                }
                            });
                        });
                    }
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add_enabled(count < MAX_SLICES, Button::new("Add slice"))
                            .clicked()
                        {
                            ring.slices.push(ring::Slice {
                                label: format!("Slice {}", count + 1),
                                action: Binding {
                                    kind: ActionKind::Shortcut,
                                    value: String::new(),
                                },
                            });
                        }
                        if ui.button("Load starter ring").clicked() {
                            *ring = Ring::starter();
                            self.macro_drafts.clear();
                        }
                        if ui.button("Clear").clicked() {
                            ring.slices.clear();
                            self.macro_drafts.clear();
                        }
                    });
                    if !self.macro_error.is_empty() {
                        ui.colored_label(ERROR, &self.macro_error);
                    }
                    match ring.validate() {
                        Err(e) => {
                            ui.colored_label(ERROR, e);
                        }
                        Ok(()) if used => muted(ui, "This profile has a button set to Actions Ring."),
                        Ok(()) => muted(
                            ui,
                            "No button opens this ring yet. On Buttons, choose Action type → Actions Ring.",
                        ),
                    }
                    muted(
                        ui,
                        "Saving installs the background helper that opens the ring at the cursor. Hold the button, move to a slice and release, or tap and click a slice. Esc or a click outside closes it without running anything.",
                    );
                });
            if ui.button("Done").clicked() {
                close = true;
            }
        });
        match edit {
            Some(Edit::Remove(i)) => {
                ring.slices.remove(i);
                self.macro_drafts.clear();
            }
            Some(Edit::Move(i, d)) => {
                let j = (i as isize + d) as usize;
                ring.slices.swap(i, j);
                self.macro_drafts.clear();
            }
            None => {}
        }
        if close {
            self.open = false;
        }
    }
}

// ---- overlay process -----------------------------------------------------------------------------

struct Overlay {
    ring: Ring,
    session: Session,
    chosen: Rc<Cell<Option<usize>>>,
    held: mpsc::Receiver<bool>,
    started: Instant,
    focused_once: bool,
}

impl Overlay {
    fn finish(&self, ctx: &Context, choice: Option<usize>) {
        self.chosen.set(choice);
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }
}

impl eframe::App for Overlay {
    fn clear_color(&self, _: &Visuals) -> [f32; 4] {
        [0.0; 4]
    }
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        let (focused, escape, pressed, pointer) = ctx.input(|i| {
            (
                i.viewport().focused,
                i.key_pressed(Key::Escape),
                i.pointer.any_pressed(),
                i.pointer.hover_pos(),
            )
        });
        let center = ctx.content_rect().center();
        let hit = pointer.map(|p| {
            ring::hit(
                p.x - center.x,
                p.y - center.y,
                self.ring.slices.len(),
                self.ring.dead_zone,
                self.ring.radius,
            )
        });
        let mut outcome = Outcome::Pending;
        while let Ok(held) = self.held.try_recv() {
            if outcome == Outcome::Pending {
                outcome = self.session.poll(held, hit);
            }
        }
        if escape {
            outcome = Outcome::Dismiss;
        } else if pressed && outcome == Outcome::Pending {
            outcome = self.session.click(hit);
        }
        if focused == Some(true) {
            self.focused_once = true;
        }
        let elapsed = self.started.elapsed();
        let lost_focus = (self.focused_once && focused == Some(false))
            || (!self.focused_once && elapsed > Duration::from_secs(3))
            || elapsed > Duration::from_secs(60);
        match outcome {
            Outcome::Choose(i) => self.finish(ctx, Some(i)),
            Outcome::Dismiss => self.finish(ctx, None),
            Outcome::Pending if lost_focus => self.finish(ctx, None),
            Outcome::Pending => {}
        }
        CentralPanel::default().frame(Frame::NONE).show(ctx, |ui| {
            let hover = match hit {
                Some(Hit::Slice(i)) => Some(i),
                _ => None,
            };
            paint_ring(ui.painter(), center, &self.ring, hover, 1.0);
        });
        ctx.request_repaint_after(Duration::from_millis(50));
    }
}

/// Entry point for `mouse-studio --ring <profile-id> <device>`. Returns the process exit code.
pub fn run_overlay(args: &[String]) -> i32 {
    let parsed = match ring::parse_args(args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };
    // A second press while a ring is open must not stack another window.
    let Some(_lock) = ring::acquire_lock() else {
        return 0;
    };
    let loaded = crate::model::settings_path()
        .map(|p| parsed.model.settings_path(&p))
        .and_then(|p| ring::load(&p, &parsed));
    let ring = match loaded {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Actions Ring: {e}");
            return 1;
        }
    };
    let model = parsed.model;
    let size = ring.window_size();
    let (tx, held) = mpsc::channel();
    // Place the window once it is mapped; failures only cost positioning, never safety.
    std::thread::spawn(move || {
        let cursor = ring::cursor(model);
        for _ in 0..120 {
            if ring::window_mapped(model) {
                for _ in 0..2 {
                    match ring::place(model, size, cursor) {
                        Ok(errors) if errors.trim().is_empty() => {}
                        Ok(errors) => eprintln!("Actions Ring placement: {errors}"),
                        Err(e) => eprintln!("Actions Ring placement: {e}"),
                    }
                    std::thread::sleep(Duration::from_millis(120));
                }
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    });
    // Watch the opening button so releasing over a slice can choose it.
    std::thread::spawn(move || {
        while let Ok(held) = ring::buttons_held(model) {
            if tx.send(held).is_err() {
                return;
            }
            std::thread::sleep(Duration::from_millis(40));
        }
    });
    let chosen = Rc::new(Cell::new(None));
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("Mouse Studio Actions Ring")
            .with_app_id(ring::WINDOW_CLASS)
            .with_inner_size([size, size])
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_always_on_top()
            .with_taskbar(false),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    let state = Overlay {
        session: Session::new(ring.release_to_select),
        ring: ring.clone(),
        chosen: chosen.clone(),
        held,
        started: Instant::now(),
        focused_once: false,
    };
    if let Err(e) = eframe::run_native(
        "Mouse Studio Actions Ring",
        options,
        Box::new(move |_| Ok(Box::new(state))),
    ) {
        eprintln!("Actions Ring window failed: {e}");
        return 1;
    }
    // The overlay has closed; let focus return to the previous window before sending input.
    if let Some(slice) = chosen.get().and_then(|i| ring.slices.get(i)) {
        std::thread::sleep(Duration::from_millis(150));
        if let Err(e) = ring::execute(model, &slice.action) {
            eprintln!("Actions Ring action failed: {e}");
            return 1;
        }
    }
    0
}
