use eframe::egui::*;
use mouse_studio::{
    app::{Page, Studio},
    model::{ActionKind, Settings},
};

fn render(app: &mut Studio, ctx: &Context, events: Vec<Event>, size: Vec2) -> FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ctx| app.show(ctx),
    )
}

fn text_position(shapes: &[epaint::ClippedShape], text: &str) -> Pos2 {
    fn find(shape: &epaint::Shape, text: &str) -> Option<Pos2> {
        match shape {
            epaint::Shape::Text(t) if t.galley.text().contains(text) => {
                Some(t.pos + t.galley.rect.size() / 2.0)
            }
            epaint::Shape::Vec(shapes) => shapes.iter().find_map(|s| find(s, text)),
            _ => None,
        }
    }
    shapes
        .iter()
        .find_map(|s| find(&s.shape, text))
        .unwrap_or_else(|| panic!("No visible text: {text}"))
}

fn click(app: &mut Studio, ctx: &Context, label: &str) {
    click_at_size(app, ctx, label, vec2(1360.0, 900.0));
}

fn click_at_size(app: &mut Studio, ctx: &Context, label: &str, size: Vec2) {
    render(app, ctx, vec![], size);
    let out = render(app, ctx, vec![], size);
    let pos = text_position(&out.shapes, label);
    for pressed in [true, false] {
        render(
            app,
            ctx,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
            size,
        );
    }
}

#[test]
fn navigation_profile_creation_and_discard_work_through_clicks() {
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let mut app = Studio::new(&ctx, dir.path().join("settings.json"));
    click(&mut app, &ctx, "Profiles");
    assert_eq!(app.page, Page::Profiles);
    click(&mut app, &ctx, "New profile");
    assert_eq!(app.draft.profiles.len(), 2);
    assert!(app.dirty());
    click(&mut app, &ctx, "Duplicate selected");
    assert_eq!(app.draft.profiles.len(), 3);
    click(&mut app, &ctx, "Discard");
    assert_eq!(app.draft, Settings::default());
    assert!(!app.dirty());
}

#[test]
fn selecting_an_application_and_saving_persists_the_assignment() {
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let mut app = Studio::new(&ctx, path.clone());
    click(&mut app, &ctx, "Launch application");
    click(&mut app, &ctx, "VS Code");
    assert_eq!(app.draft.active().bindings[0].kind, ActionKind::Application);
    assert_eq!(app.draft.active().bindings[0].value, "code");
    click(&mut app, &ctx, "Save profiles");
    assert!(!app.dirty());
    assert_eq!(
        Settings::load_from(&path).unwrap().active().bindings[0].value,
        "code"
    );
    click(&mut app, &ctx, "Change");
    click(&mut app, &ctx, "Blender");
    click(&mut app, &ctx, "Save profiles");
    assert_eq!(
        Settings::load_from(&path).unwrap().active().bindings[0].value,
        "blender"
    );
}

#[test]
fn all_pages_render_at_desktop_and_compact_sizes() {
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let mut app = Studio::new(&ctx, dir.path().join("settings.json"));
    for size in [vec2(1360.0, 900.0), vec2(880.0, 680.0)] {
        for page in [
            Page::Overview,
            Page::Buttons,
            Page::Pointer,
            Page::Scrolling,
            Page::Profiles,
            Page::Settings,
        ] {
            app.page = page;
            let output = render(&mut app, &ctx, vec![], size);
            assert!(!output.shapes.is_empty());
            assert!(
                output.platform_output.commands.is_empty(),
                "UI must not open URLs or execute clipboard commands on render"
            );
        }
    }
}

#[test]
fn view_controls_stay_below_mouse_hotspots() {
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let mut app = Studio::new(&ctx, dir.path().join("settings.json"));
    let size = vec2(1360.0, 900.0);
    render(&mut app, &ctx, vec![], size);
    let output = render(&mut app, &ctx, vec![], size);
    let bottom_hotspot = text_position(&output.shapes, "2");
    let view_controls = text_position(&output.shapes, "Perspective");
    assert!(
        view_controls.y > bottom_hotspot.y + 20.0,
        "View controls overlap the mouse stage: {view_controls:?}, {bottom_hotspot:?}"
    );
}

#[test]
fn overview_controls_fit_the_reference_window() {
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let mut app = Studio::new(&ctx, dir.path().join("settings.json"));
    app.draft.active_mut().bindings[0].kind = ActionKind::Application;
    app.draft.active_mut().bindings[0].value = "code".into();
    let size = vec2(1536.0, 1024.0);
    render(&mut app, &ctx, vec![], size);
    let output = render(&mut app, &ctx, vec![], size);
    for label in ["All pointer settings", "Save profiles", "Restore default"] {
        let position = text_position(&output.shapes, label);
        assert!(
            position.y < size.y - 20.0,
            "{label} is below the reference window: {position:?}"
        );
    }
}

#[derive(Default)]
struct RecordingBackend {
    calls: std::sync::Mutex<Vec<mouse_studio::backend::PointerSettings>>,
    fail: bool,
}
impl mouse_studio::backend::DeviceBackend for RecordingBackend {
    fn apply(&self, settings: mouse_studio::backend::PointerSettings) -> Result<(), String> {
        self.calls.lock().unwrap().push(settings);
        if self.fail {
            Err("Mouse disconnected".into())
        } else {
            Ok(())
        }
    }
}

fn settle(app: &mut Studio, ctx: &Context) -> FullOutput {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.is_applying() {
        assert!(
            std::time::Instant::now() < deadline,
            "apply worker did not finish"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
        render(app, ctx, vec![], vec2(1360.0, 900.0));
    }
    render(app, ctx, vec![], vec2(1360.0, 900.0))
}

#[test]
fn save_applies_active_profiles_four_controls_and_discard_never_applies() {
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let backend = std::sync::Arc::new(RecordingBackend::default());
    let path = dir.path().join("settings.json");
    let mut app = Studio::with_backend(&ctx, path.clone(), backend.clone());
    assert!(
        backend.calls.lock().unwrap().is_empty(),
        "opening must not apply preview-era settings"
    );
    app.draft.add_profile(false);
    app.draft.active_mut().sensitivity = 0.4;
    app.draft.active_mut().natural_scroll = true;
    app.draft.active_mut().acceleration = false;
    app.draft.active_mut().scroll_speed = 2.3;
    click(&mut app, &ctx, "Save profiles");
    let output = settle(&mut app, &ctx);
    text_position(&output.shapes, "Mouse settings & button actions applied");
    assert_eq!(
        backend.calls.lock().unwrap().as_slice(),
        &[mouse_studio::backend::PointerSettings {
            sensitivity: 0.4,
            natural_scroll: true,
            acceleration: false,
            scroll_speed: 2.3,
        }]
    );
    assert_eq!(Settings::load_from(&path).unwrap(), app.saved);
    app.draft.active_mut().sensitivity = -0.5;
    app.draft.active_mut().acceleration = true;
    app.draft.active_mut().scroll_speed = 0.5;
    click(&mut app, &ctx, "Discard");
    assert_eq!(app.draft.active().sensitivity, 0.4);
    assert!(!app.draft.active().acceleration);
    assert_eq!(app.draft.active().scroll_speed, 2.3);
    assert_eq!(backend.calls.lock().unwrap().len(), 1);
}

#[test]
fn failed_apply_keeps_saved_profiles_and_offers_retry_without_claiming_success() {
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let backend = std::sync::Arc::new(RecordingBackend {
        fail: true,
        ..Default::default()
    });
    let path = dir.path().join("settings.json");
    let mut app = Studio::with_backend(&ctx, path.clone(), backend.clone());
    app.draft.active_mut().sensitivity = 0.3;
    click(&mut app, &ctx, "Save profiles");
    let output = settle(&mut app, &ctx);
    text_position(&output.shapes, "Saved locally; apply failed");
    assert_eq!(
        Settings::load_from(&path).unwrap().active().sensitivity,
        0.3
    );
    click(&mut app, &ctx, "Save profiles");
    settle(&mut app, &ctx);
    assert_eq!(backend.calls.lock().unwrap().len(), 2);
}

#[test]
fn invalid_or_unwritable_profiles_never_apply_to_the_mouse() {
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let backend = std::sync::Arc::new(RecordingBackend::default());
    let mut app = Studio::with_backend(&ctx, dir.path().join("settings.json"), backend.clone());
    app.page = Page::Settings; // Do not let a rendered slider normalize this deliberately invalid value.
    app.draft.active_mut().sensitivity = f32::NAN;
    click(&mut app, &ctx, "Save profiles");
    assert!(!app.is_applying());
    assert!(backend.calls.lock().unwrap().is_empty());
    // A regular file as the parent guarantees a write failure even when tests run as root.
    let parent = dir.path().join("not-a-directory");
    std::fs::write(&parent, "keep").unwrap();
    let mut app = Studio::with_backend(&ctx, parent.join("settings.json"), backend.clone());
    click(&mut app, &ctx, "Save profiles");
    assert!(!app.is_applying());
    assert!(backend.calls.lock().unwrap().is_empty());
}

#[derive(Default)]
struct FullBackend {
    profiles: std::sync::Mutex<Vec<mouse_studio::model::Profile>>,
    tests: std::sync::Mutex<Vec<mouse_studio::model::Binding>>,
    startup: std::sync::Mutex<Vec<bool>>,
}
impl mouse_studio::backend::DeviceBackend for FullBackend {
    fn apply(&self, _: mouse_studio::backend::PointerSettings) -> Result<(), String> {
        panic!("UI must apply complete profiles");
    }
    fn apply_profile(&self, profile: &mouse_studio::model::Profile) -> Result<(), String> {
        self.profiles.lock().unwrap().push(profile.clone());
        Ok(())
    }
    fn test_action(&self, binding: &mouse_studio::model::Binding) -> Result<(), String> {
        self.tests.lock().unwrap().push(binding.clone());
        Ok(())
    }
}
impl mouse_studio::startup::StartupControl for FullBackend {
    fn sync(&self, enabled: bool, _: &std::path::Path) -> Result<(), String> {
        self.startup.lock().unwrap().push(enabled);
        Ok(())
    }
}

#[test]
fn action_test_and_save_use_different_paths_and_restore_follows_saved_preference() {
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let backend = std::sync::Arc::new(FullBackend::default());
    let mut app = Studio::with_services(&ctx, path.clone(), backend.clone(), backend.clone());
    app.draft.active_mut().bindings[0] = mouse_studio::model::Binding {
        kind: ActionKind::Application,
        value: "kitty".into(),
    };
    click(&mut app, &ctx, "Test action");
    assert!(app.is_applying());
    settle(&mut app, &ctx);
    assert_eq!(backend.tests.lock().unwrap()[0].value, "kitty");
    assert!(!path.exists(), "testing a draft must not save it");
    assert!(backend.profiles.lock().unwrap().is_empty());
    assert!(backend.startup.lock().unwrap().is_empty());
    app.draft.restore_on_login = true;
    click(&mut app, &ctx, "Save profiles");
    settle(&mut app, &ctx);
    assert_eq!(
        backend.profiles.lock().unwrap()[0].bindings[0].value,
        "kitty"
    );
    assert!(Settings::load_from(&path).unwrap().restore_on_login);
    app.draft.restore_on_login = false;
    click(&mut app, &ctx, "Save profiles");
    settle(&mut app, &ctx);
    assert_eq!(*backend.startup.lock().unwrap(), [true, false]);
    assert_eq!(
        backend.tests.lock().unwrap().len(),
        1,
        "saving must never execute an action"
    );
}

#[test]
fn restoring_requires_confirmation_and_applies_the_complete_previous_setup() {
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let mut original = Settings::default();
    original.add_profile(false);
    original.active_mut().bindings[0] = mouse_studio::model::Binding {
        kind: ActionKind::Shortcut,
        value: "CTRL+C".into(),
    };
    original.restore_on_login = true;
    original.save_to(&path).unwrap();
    let mut changed = Settings::default();
    changed.active_mut().sensitivity = 0.6;
    mouse_studio::recovery::save_with_history(&changed, &path).unwrap();
    let backend = std::sync::Arc::new(FullBackend::default());
    let mut app = Studio::with_services(&ctx, path.clone(), backend.clone(), backend.clone());
    app.page = Page::Settings;
    app.draft.active_mut().name = "Unsaved edit".into();
    let draft = app.draft.clone();
    click(&mut app, &ctx, "Restore previous setup");
    assert!(backend.profiles.lock().unwrap().is_empty());
    click(&mut app, &ctx, "Cancel");
    assert_eq!(app.draft, draft);
    assert_eq!(Settings::load_from(&path).unwrap(), changed);
    click(&mut app, &ctx, "Restore previous setup");
    click(&mut app, &ctx, "Restore & apply");
    let output = settle(&mut app, &ctx);
    text_position(&output.shapes, "Previous setup restored and applied");
    assert_eq!(app.saved, original);
    assert_eq!(app.draft, original);
    assert_eq!(Settings::load_from(&path).unwrap(), original);
    assert_eq!(
        *backend.profiles.lock().unwrap(),
        [original.active().clone()]
    );
    assert_eq!(*backend.startup.lock().unwrap(), [true]);
    assert_eq!(
        mouse_studio::recovery::previous_setup(&path).unwrap(),
        Some(changed)
    );
    assert!(backend.tests.lock().unwrap().is_empty());
}

#[test]
fn failed_restore_write_preserves_unsaved_edits_and_never_applies() {
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    Settings::default().save_to(&path).unwrap();
    let mut changed = Settings::default();
    changed.active_mut().sensitivity = 0.8;
    mouse_studio::recovery::save_with_history(&changed, &path).unwrap();
    let backend = std::sync::Arc::new(FullBackend::default());
    let mut app = Studio::with_services(&ctx, path.clone(), backend.clone(), backend.clone());
    app.page = Page::Settings;
    app.draft.active_mut().name = "Keep my edits".into();
    let draft = app.draft.clone();
    click(&mut app, &ctx, "Restore previous setup");
    // Fail after the modal opens, without depending on filesystem permissions.
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    click(&mut app, &ctx, "Restore & apply");
    assert_eq!(app.draft, draft);
    assert_eq!(app.saved, changed);
    assert!(backend.profiles.lock().unwrap().is_empty());
    assert!(backend.startup.lock().unwrap().is_empty());
}

#[test]
fn disconnected_restore_keeps_the_restore_point_when_save_is_retried() {
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let original = Settings::default();
    original.save_to(&path).unwrap();
    let mut changed = original.clone();
    changed.active_mut().sensitivity = 0.8;
    mouse_studio::recovery::save_with_history(&changed, &path).unwrap();
    let backend = std::sync::Arc::new(RecordingBackend {
        fail: true,
        ..Default::default()
    });
    let mut app = Studio::with_backend(&ctx, path.clone(), backend.clone());
    app.page = Page::Settings;
    click(&mut app, &ctx, "Restore previous setup");
    click(&mut app, &ctx, "Restore & apply");
    let output = settle(&mut app, &ctx);
    text_position(&output.shapes, "Saved locally; apply failed");
    assert_eq!(app.saved, original);
    assert_eq!(Settings::load_from(&path).unwrap(), original);
    click(&mut app, &ctx, "Save profiles");
    settle(&mut app, &ctx);
    assert_eq!(backend.calls.lock().unwrap().len(), 2);
    assert_eq!(
        mouse_studio::recovery::previous_setup(&path).unwrap(),
        Some(changed)
    );
}

#[test]
fn hold_tabs_macro_editor_and_profile_reset_are_usable_draft_controls() {
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let mut app = Studio::new(&ctx, path.clone());
    click(&mut app, &ctx, "Hold");
    click(&mut app, &ctx, "Run command");
    assert_eq!(
        app.draft.active().advanced.hold[0].kind,
        ActionKind::Command
    );
    app.draft.active_mut().advanced.hold[0].value = "kitty".into();
    click(&mut app, &ctx, "Save profiles");
    assert_eq!(
        Settings::load_from(&path).unwrap().active().advanced.hold[0].value,
        "kitty"
    );
    click(&mut app, &ctx, "Double-click");
    app.draft.active_mut().advanced.double[0] = mouse_studio::model::Binding {
        kind: ActionKind::Macro,
        value: mouse_studio::software::compose_macro("CTRL+C\nwait 100\nCTRL+V").unwrap(),
    };
    click(&mut app, &ctx, "Edit macro");
    let output = render(&mut app, &ctx, vec![], vec2(1360.0, 900.0));
    text_position(&output.shapes, "Macro editor");
    click(&mut app, &ctx, "Use macro");
    assert_eq!(
        app.draft.active().advanced.double[0].kind,
        ActionKind::Macro
    );
    click(&mut app, &ctx, "Profiles");
    app.draft.active_mut().sensitivity = 0.8;
    click(&mut app, &ctx, "Reset selected profile to defaults");
    assert_eq!(app.draft.active().sensitivity, 0.0);
    assert_eq!(app.draft.active().advanced, Default::default());
    assert_eq!(
        Settings::load_from(&path).unwrap().active().advanced.hold[0].value,
        "kitty",
        "reset must remain a draft until Save"
    );
}

#[test]
fn model_3d_hotspots_can_select_a_button() {
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let mut app = Studio::new(&ctx, dir.path().join("settings.json"));
    app.set_preview_view(3);
    click(&mut app, &ctx, "2");
    assert_eq!(app.selected_button, 1);
}

#[test]
fn m650_photos_change_views_and_select_its_three_buttons() {
    use mouse_studio::device::MouseModel;
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let mut app = Studio::new(&ctx, dir.path().join("settings.json"));
    app.switch_device(MouseModel::M650L);
    let size = vec2(1360.0, 900.0);
    render(&mut app, &ctx, vec![], size);
    assert!(
        ctx.tex_manager()
            .read()
            .allocated()
            .any(|(_, meta)| { meta.name == "m650-l-Perspective" && meta.size[0] > 600 }),
        "M650 L must upload its own product photo"
    );
    // Match exact marker text: the model name itself contains the digit 5.
    let marker = |app: &mut Studio, label: &str| {
        let out = render(app, &ctx, vec![], size);
        let pos = out
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                epaint::Shape::Text(t) if t.galley.text() == label => {
                    Some(t.pos + t.galley.rect.size() / 2.0)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing photo marker {label}"));
        for pressed in [true, false] {
            render(
                app,
                &ctx,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
                size,
            );
        }
    };
    click(&mut app, &ctx, "Top");
    app.selected_button = 0;
    marker(&mut app, "3");
    assert_eq!(app.selected_button, 2);
    click(&mut app, &ctx, "Perspective");
    marker(&mut app, "5");
    assert_eq!(app.selected_button, 1);
    marker(&mut app, "4");
    assert_eq!(app.selected_button, 0);
    for size in [vec2(480.0, 480.0), vec2(1536.0, 1024.0)] {
        for page in [Page::Overview, Page::Buttons] {
            app.page = page;
            render(&mut app, &ctx, vec![], size);
            let output = render(&mut app, &ctx, vec![], size);
            if size.x > 500.0 {
                text_position(&output.shapes, "Top");
            }
            assert_label_visible(&output, "Save profiles", size);
            assert!(!format!("{:?}", output.shapes).contains("Haptic Sense panel"));
        }
    }
    assert!(!app.dirty());
}

#[test]
fn cancelling_the_three_second_test_prevents_action_execution() {
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let backend = std::sync::Arc::new(FullBackend::default());
    let mut app = Studio::with_backend(&ctx, dir.path().join("settings.json"), backend.clone());
    app.draft.active_mut().bindings[0] = mouse_studio::model::Binding {
        kind: ActionKind::Command,
        value: "kitty".into(),
    };
    click(&mut app, &ctx, "Test action");
    assert!(app.is_applying());
    click(&mut app, &ctx, "Cancel actions");
    settle(&mut app, &ctx);
    assert!(backend.tests.lock().unwrap().is_empty());
}

#[test]
fn hardware_drafts_save_and_discard_with_the_profile() {
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let mut app = Studio::new(&ctx, path.clone());
    click(&mut app, &ctx, "Pointer");
    click(&mut app, &ctx, "Manage sensor DPI");
    click(&mut app, &ctx, "Balanced · 1600");
    assert_eq!(app.draft.active().hardware.dpi, Some(1600));
    assert!(!path.exists());
    click(&mut app, &ctx, "Scrolling");
    click(&mut app, &ctx, "Manage wheel mode");
    click(&mut app, &ctx, "Free-spin");
    click(&mut app, &ctx, "Manage SmartShift");
    assert_eq!(
        app.draft.active().hardware.wheel_mode,
        Some(mouse_studio::hardware::WheelMode::FreeSpin)
    );
    assert_eq!(app.draft.active().hardware.smartshift, Some(20));
    click(&mut app, &ctx, "Save profiles");
    assert_eq!(
        Settings::load_from(&path).unwrap().active().hardware,
        app.draft.active().hardware
    );
    app.draft.active_mut().hardware.dpi = Some(3200);
    click(&mut app, &ctx, "Discard");
    assert_eq!(app.draft.active().hardware.dpi, Some(1600));
}

// Unlike text_position, this checks the complete label against both its clip and viewport.
fn assert_label_visible(output: &FullOutput, label: &str, size: Vec2) {
    fn visit(shape: &epaint::Shape, clip: Rect, label: &str, viewport: Rect) -> bool {
        match shape {
            epaint::Shape::Text(t) if t.galley.text() == label => {
                let rect = t.galley.rect.translate(t.pos.to_vec2());
                clip.expand(1.0).contains_rect(rect) && viewport.contains_rect(rect)
            }
            epaint::Shape::Vec(shapes) => shapes.iter().any(|s| visit(s, clip, label, viewport)),
            _ => false,
        }
    }
    let viewport = Rect::from_min_size(Pos2::ZERO, size);
    assert!(
        output
            .shapes
            .iter()
            .any(|s| visit(&s.shape, s.clip_rect, label, viewport)),
        "{label:?} is missing or clipped at {size:?}"
    );
}

#[test]
fn responsive_save_stays_visible_on_every_page_and_resize_preserves_drafts() {
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let mut app = Studio::new(&ctx, dir.path().join("settings.json"));
    app.draft.active_mut().name = "A long profile name for focused creative work".into();
    let draft = app.draft.clone();
    for size in [
        vec2(480.0, 480.0),
        vec2(880.0, 680.0),
        vec2(1280.0, 600.0),
        vec2(1536.0, 1024.0),
        vec2(480.0, 480.0),
    ] {
        for page in [
            Page::Overview,
            Page::Buttons,
            Page::Pointer,
            Page::Scrolling,
            Page::Profiles,
            Page::Settings,
        ] {
            app.page = page;
            render(&mut app, &ctx, vec![], size);
            let out = render(&mut app, &ctx, vec![], size);
            assert_label_visible(&out, "Save profiles", size);
            assert_eq!(app.draft, draft);
        }
    }
}

#[test]
fn responsive_dialog_actions_fit_small_windows() {
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let mut app = Studio::new(&ctx, dir.path().join("settings.json"));
    for size in [vec2(480.0, 480.0), vec2(880.0, 680.0)] {
        app.open_advanced_settings();
        for _ in 0..3 {
            render(&mut app, &ctx, vec![], size);
        }
        let out = render(&mut app, &ctx, vec![], size);
        for label in ["Advanced button settings", "Done"] {
            assert_label_visible(&out, label, size);
        }
        app.close_editors();
        app.open_macro_editor(0, 0);
        for _ in 0..3 {
            render(&mut app, &ctx, vec![], size);
        }
        let out = render(&mut app, &ctx, vec![], size);
        for label in ["Macro editor", "Use macro", "Close editor"] {
            assert_label_visible(&out, label, size);
        }
        app.close_editors();
    }
}

#[test]
fn responsive_page_text_fits_horizontally() {
    fn check(shape: &epaint::Shape, clip: Rect, page: Page, size: Vec2) {
        match shape {
            epaint::Shape::Text(t) => {
                let rect = t.galley.rect.translate(t.pos.to_vec2());
                if rect.bottom() > clip.top() && rect.top() < clip.bottom() && rect.height() > 0.0 {
                    assert!(
                        rect.left() >= clip.left() - 1.0 && rect.right() <= clip.right() + 1.0,
                        "{page:?} at {size:?}: {:?} extends {rect:?} beyond {clip:?}",
                        t.galley.text()
                    );
                }
            }
            epaint::Shape::Vec(shapes) => {
                for shape in shapes {
                    check(shape, clip, page, size)
                }
            }
            _ => {}
        }
    }
    for size in [
        vec2(480.0, 480.0),
        vec2(640.0, 600.0),
        vec2(880.0, 680.0),
        vec2(1150.0, 900.0),
        vec2(1280.0, 800.0),
    ] {
        let ctx = Context::default();
        let dir = tempfile::tempdir().unwrap();
        let mut app = Studio::new(&ctx, dir.path().join("settings.json"));
        app.draft.active_mut().hardware.dpi = Some(1600);
        app.draft.active_mut().hardware.smartshift = Some(20);
        for page in [
            Page::Overview,
            Page::Buttons,
            Page::Pointer,
            Page::Scrolling,
            Page::Profiles,
            Page::Settings,
        ] {
            app.page = page;
            render(&mut app, &ctx, vec![], size);
            let out = render(&mut app, &ctx, vec![], size);
            for s in &out.shapes {
                check(
                    &s.shape,
                    s.clip_rect.intersect(Rect::from_min_size(Pos2::ZERO, size)),
                    page,
                    size,
                );
            }
            for _ in 0..6 {
                render(
                    &mut app,
                    &ctx,
                    vec![
                        Event::PointerMoved(pos2(size.x * 0.6, size.y * 0.5)),
                        Event::MouseWheel {
                            unit: MouseWheelUnit::Point,
                            delta: vec2(0.0, -300.0),
                            modifiers: Modifiers::NONE,
                        },
                    ],
                    size,
                );
                let out = render(&mut app, &ctx, vec![], size);
                for s in &out.shapes {
                    check(
                        &s.shape,
                        s.clip_rect.intersect(Rect::from_min_size(Pos2::ZERO, size)),
                        page,
                        size,
                    );
                }
            }
        }
    }
}

#[test]
fn responsive_save_and_discard_work_at_minimum_size() {
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let mut app = Studio::new(&ctx, path.clone());
    let size = vec2(480.0, 480.0);
    app.draft.active_mut().sensitivity = 0.42;
    click_at_size(&mut app, &ctx, "Save profiles", size);
    assert_eq!(
        Settings::load_from(&path).unwrap().active().sensitivity,
        0.42
    );
    app.draft.active_mut().sensitivity = -0.2;
    click_at_size(&mut app, &ctx, "Discard", size);
    assert_eq!(app.draft.active().sensitivity, 0.42);
}

#[test]
fn switching_mice_retains_drafts_and_saves_independent_profiles() {
    use mouse_studio::device::MouseModel;
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("settings.json");
    let mut app = Studio::new(&ctx, base.clone());
    app.draft.active_mut().sensitivity = 0.25;
    app.switch_device(MouseModel::M650L);
    assert_eq!(app.draft.device, MouseModel::M650L);
    assert_eq!(app.draft.active().sensitivity, 0.0);
    app.draft.active_mut().sensitivity = -0.3;
    app.switch_device(MouseModel::MxMaster4);
    app.draft.active_mut().sensitivity = 0.6;
    for (model, speed) in [
        (MouseModel::MxAnywhere3s, 0.25),
        (MouseModel::M650L, -0.3),
        (MouseModel::MxMaster4, 0.6),
    ] {
        app.switch_device(model);
        assert_eq!(app.draft.active().sensitivity, speed);
        click_at_size(&mut app, &ctx, "Save profiles", vec2(880.0, 680.0));
        let stored = Settings::load_from(&model.settings_path(&base)).unwrap();
        assert_eq!(stored.device, model);
        assert_eq!(stored.active().sensitivity, speed);
    }
}

#[test]
fn new_mouse_pages_and_device_picker_fit_compact_windows() {
    use mouse_studio::device::MouseModel;
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let mut app = Studio::new(&ctx, dir.path().join("settings.json"));
    for model in [MouseModel::M650L, MouseModel::MxMaster4] {
        app.switch_device(model);
        for size in [vec2(480.0, 480.0), vec2(1536.0, 1024.0)] {
            for page in [
                Page::Overview,
                Page::Buttons,
                Page::Pointer,
                Page::Scrolling,
                Page::Profiles,
                Page::Settings,
            ] {
                app.page = page;
                render(&mut app, &ctx, vec![], size);
                let out = render(&mut app, &ctx, vec![], size);
                assert_label_visible(&out, "Save profiles", size);
            }
        }
    }
}

#[test]
fn closing_warns_about_unsaved_profiles_on_another_mouse() {
    use mouse_studio::device::MouseModel;
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let mut app = Studio::new(&ctx, dir.path().join("settings.json"));
    app.draft.active_mut().sensitivity = 0.4;
    app.switch_device(MouseModel::MxMaster4);
    assert!(!app.dirty());
    let mut input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(880.0, 680.0))),
        ..Default::default()
    };
    input
        .viewports
        .get_mut(&ViewportId::ROOT)
        .unwrap()
        .events
        .push(ViewportEvent::Close);
    let output = ctx.run(input, |ctx| app.show(ctx));
    assert!(
        output.viewport_output[&ViewportId::ROOT]
            .commands
            .contains(&ViewportCommand::CancelClose)
    );
    let output = render(&mut app, &ctx, vec![], vec2(880.0, 680.0));
    text_position(&output.shapes, "Save before closing?");
    app.switch_device(MouseModel::MxAnywhere3s);
    assert_eq!(app.draft.active().sensitivity, 0.4);
}

#[test]
fn master_extra_controls_open_the_inspector_and_switching_mice_resets_selection() {
    use mouse_studio::device::MouseModel;
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let mut app = Studio::new(&ctx, dir.path().join("settings.json"));
    app.switch_device(MouseModel::MxMaster4);
    app.page = Page::Overview;
    let size = vec2(1536.0, 1024.0);
    for (index, label) in [
        (3, "Wheel-mode button"),
        (4, "Gesture button"),
        (5, "Haptic Sense panel"),
    ] {
        click_at_size(&mut app, &ctx, label, size);
        assert_eq!(app.selected_button, index);
        app.open_macro_editor(index, 0);
        for _ in 0..3 {
            render(&mut app, &ctx, vec![], size);
        }
        let output = render(&mut app, &ctx, vec![], size);
        text_position(&output.shapes, "Macro editor");
        app.close_editors();
    }
    app.switch_device(MouseModel::MxAnywhere3s);
    assert!(app.selected_button < 3);
    render(&mut app, &ctx, vec![], size);
}

#[test]
fn side_wheel_direction_is_a_saved_master_profile_control() {
    use mouse_studio::device::MouseModel;
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("settings.json");
    let mut app = Studio::new(&ctx, base.clone());
    app.switch_device(MouseModel::MxMaster4);
    app.page = Page::Scrolling;
    let size = vec2(1536.0, 1024.0);
    click_at_size(&mut app, &ctx, "Reverse side wheel", size);
    assert_eq!(app.draft.active().hardware.thumbwheel_inverted, Some(true));
    click_at_size(&mut app, &ctx, "Save profiles", size);
    assert_eq!(
        Settings::load_from(&MouseModel::MxMaster4.settings_path(&base))
            .unwrap()
            .active()
            .hardware
            .thumbwheel_inverted,
        Some(true)
    );
    click_at_size(&mut app, &ctx, "Reverse side wheel", size);
    click_at_size(&mut app, &ctx, "Discard", size);
    assert_eq!(app.draft.active().hardware.thumbwheel_inverted, Some(true));
}

#[test]
fn side_wheel_actions_are_a_saved_master_choice_and_haptics_hide_without_support() {
    use mouse_studio::device::MouseModel;
    let ctx = Context::default();
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("settings.json");
    let mut app = Studio::new(&ctx, base.clone());
    app.switch_device(MouseModel::MxMaster4);
    app.page = Page::Scrolling;
    let size = vec2(1536.0, 1024.0);
    let shown = |app: &mut Studio, text: &str| {
        render(app, &ctx, vec![], size);
        let out = render(app, &ctx, vec![], size);
        out.shapes.iter().any(|s| {
            fn find(shape: &epaint::Shape, text: &str) -> bool {
                match shape {
                    epaint::Shape::Text(t) => t.galley.text().contains(text),
                    epaint::Shape::Vec(v) => v.iter().any(|s| find(s, text)),
                    _ => false,
                }
            }
            find(&s.shape, text)
        })
    };
    assert!(shown(&mut app, "Haptic feedback"));
    assert!(
        !shown(&mut app, "Test haptic"),
        "no unverified haptic controls without a confirmed 0x19B0 feature"
    );
    assert!(!shown(&mut app, "Manage haptic feedback"));
    assert!(app.draft.active().hardware.thumbwheel_remap.is_none());
    click_at_size(&mut app, &ctx, "Remap to actions", size);
    assert!(app.draft.active().hardware.thumbwheel_remap.is_some());
    assert!(!shown(&mut app, "Reverse side wheel"));
    click_at_size(&mut app, &ctx, "Zoom", size);
    assert_eq!(
        app.draft
            .active()
            .hardware
            .thumbwheel_remap
            .as_ref()
            .unwrap()
            .right
            .value,
        "CTRL+equal"
    );
    click_at_size(&mut app, &ctx, "Swap directions", size);
    assert_eq!(
        app.draft
            .active()
            .hardware
            .thumbwheel_remap
            .as_ref()
            .unwrap()
            .right
            .value,
        "CTRL+minus"
    );
    click_at_size(&mut app, &ctx, "Horizontal scroll", size);
    assert!(app.draft.active().hardware.thumbwheel_remap.is_none());
    assert!(shown(&mut app, "Reverse side wheel"));
    // Other mice show neither control.
    app.switch_device(MouseModel::MxAnywhere3s);
    app.page = Page::Scrolling;
    assert!(!shown(&mut app, "Haptic feedback"));
    assert!(!shown(&mut app, "Remap to actions"));
}
