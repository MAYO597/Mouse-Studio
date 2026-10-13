use eframe::egui::{self, *};

pub const BG: Color32 = Color32::from_rgb(19, 22, 22);
pub const PANEL: Color32 = Color32::from_rgb(26, 29, 28);
pub const RAISED: Color32 = Color32::from_rgb(37, 43, 40);
pub const MINT: Color32 = Color32::from_rgb(166, 227, 204);
pub const TEXT: Color32 = Color32::from_rgb(237, 242, 240);
pub const MUTED: Color32 = Color32::from_rgb(177, 184, 180);
pub const LINE: Color32 = Color32::from_rgb(51, 60, 56);
pub const ERROR: Color32 = Color32::from_rgb(242, 167, 150);

pub fn install(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    for (name, bytes) in [
        (
            "Inter",
            include_bytes!("../assets/fonts/Inter-Regular.ttf").as_slice(),
        ),
        (
            "Inter Semibold",
            include_bytes!("../assets/fonts/Inter-Semibold.ttf").as_slice(),
        ),
        (
            "Symbols",
            include_bytes!("../assets/fonts/NotoSansSymbols2-Regular.ttf").as_slice(),
        ),
    ] {
        fonts
            .font_data
            .insert(name.into(), FontData::from_static(bytes).into());
    }
    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, "Inter".into());
    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .push("Symbols".into());
    fonts.families.insert(
        FontFamily::Name("display".into()),
        vec!["Inter Semibold".into(), "Inter".into(), "Symbols".into()],
    );
    ctx.set_fonts(fonts);
    let mut style = (*ctx.style()).clone();
    style.visuals = Visuals::dark();
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = PANEL;
    style.visuals.extreme_bg_color = BG;
    style.visuals.faint_bg_color = PANEL;
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.selection.bg_fill = Color32::from_rgb(56, 87, 74);
    style.visuals.selection.stroke = Stroke::new(1.0_f32, MINT);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, LINE);
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, MUTED);
    for visuals in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        visuals.corner_radius = CornerRadius::same(7);
        visuals.bg_stroke = Stroke::new(1.0_f32, LINE);
        visuals.fg_stroke = Stroke::new(1.2_f32, TEXT);
        visuals.bg_fill = RAISED;
        visuals.weak_bg_fill = RAISED;
    }
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, MINT.gamma_multiply(0.65));
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(59, 85, 73);
    style.spacing.item_spacing = vec2(12.0, 6.0);
    style.spacing.button_padding = vec2(14.0, 7.0);
    style.spacing.interact_size.y = 30.0;
    style.spacing.slider_width = 240.0;
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(15.0));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(14.0));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(12.0));
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(23.0, FontFamily::Name("display".into())),
    );
    ctx.set_style(style);
}

pub fn muted(ui: &mut Ui, text: impl Into<String>) {
    ui.label(RichText::new(text).color(MUTED).size(13.0));
}
pub fn eyebrow(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .color(MUTED)
            .size(11.0)
            .extra_letter_spacing(1.5),
    );
}
pub fn heading(ui: &mut Ui, text: &str, description: &str) {
    ui.label(
        RichText::new(text)
            .size(28.0)
            .family(FontFamily::Name("display".into())),
    );
    muted(ui, description);
    ui.add_space(12.0);
}
pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0_f32, LINE))
        .corner_radius(10)
        .inner_margin(18)
}
pub fn primary(ui: &mut Ui, text: &str, size: Vec2) -> Response {
    ui.add_sized(
        size,
        Button::new(RichText::new(text).color(BG).strong())
            .fill(MINT)
            .stroke(Stroke::NONE),
    )
}

pub fn segment(ui: &mut Ui, label: &str, selected: bool, width: f32) -> Response {
    ui.add_sized(
        [width, 31.0],
        Button::new(RichText::new(label).color(if selected { BG } else { MUTED }))
            .truncate()
            .fill(if selected { MINT } else { PANEL })
            .stroke(Stroke::new(1.0_f32, if selected { MINT } else { LINE }))
            .corner_radius(5),
    )
}

pub fn mint_slider(
    ui: &mut Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    step: f64,
) -> Response {
    ui.scope(|ui| {
        ui.spacing_mut().slider_width = (ui.available_width() - 2.0).max(100.0);
        ui.spacing_mut().slider_rail_height = 4.0;
        ui.spacing_mut().interact_size.y = 20.0;
        let visuals = ui.visuals_mut();
        visuals.selection.bg_fill = MINT;
        visuals.widgets.inactive.bg_fill = LINE;
        visuals.widgets.inactive.fg_stroke = Stroke::new(2.0_f32, MINT);
        visuals.widgets.hovered.bg_fill = MINT;
        visuals.widgets.active.bg_fill = MINT;
        ui.add(
            Slider::new(value, range)
                .show_value(false)
                .step_by(step)
                .trailing_fill(true)
                .handle_shape(egui::style::HandleShape::Circle),
        )
    })
    .inner
}
pub fn toggle(ui: &mut Ui, value: &mut bool, label: &str) -> Response {
    let desired = vec2(42.0, 24.0);
    let (rect, mut response) = ui.allocate_exact_size(desired, Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    response
        .widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, ui.is_enabled(), *value, label));
    let fill = if *value { MINT } else { LINE };
    ui.painter().rect_filled(rect, 12, fill);
    let x = if *value {
        rect.right() - 12.0
    } else {
        rect.left() + 12.0
    };
    ui.painter().circle_filled(
        pos2(x, rect.center().y),
        8.0,
        if *value { BG } else { MUTED },
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(3.0),
            14,
            Stroke::new(1.0_f32, MINT),
            StrokeKind::Outside,
        );
    }
    response.on_hover_text(label)
}

pub fn mouse_mark(ui: &mut Ui, size: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let body = Rect::from_center_size(rect.center(), vec2(size * 0.58, size * 0.86));
    ui.painter().rect_stroke(
        body,
        (size * 0.27) as u8,
        Stroke::new(1.5_f32, color),
        StrokeKind::Inside,
    );
    ui.painter().line_segment(
        [
            body.center_top(),
            body.center_top() + vec2(0.0, size * 0.30),
        ],
        Stroke::new(1.5_f32, color),
    );
    ui.painter().rect_filled(
        Rect::from_center_size(
            body.center_top() + vec2(0.0, size * 0.24),
            vec2(3.0, size * 0.13),
        ),
        2,
        color,
    );
}

/// Reserve the switch before laying out text so descriptions wrap beside it.
pub fn toggle_setting(ui: &mut Ui, title: &str, description: &str, value: &mut bool, label: &str) {
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            toggle(ui, value, label);
            ui.vertical(|ui| {
                ui.label(RichText::new(title).strong());
                muted(ui, description);
            });
        });
    });
}
