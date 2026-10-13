use mouse_studio::backend::{CommandRunner, DeviceBackend, Hyprland, PointerSettings};
use std::{collections::VecDeque, sync::Mutex};

struct Fake {
    replies: Mutex<VecDeque<Result<String, String>>>,
    calls: Mutex<Vec<Vec<String>>>,
}
impl Fake {
    fn new(replies: Vec<Result<&str, &str>>) -> Self {
        Self {
            replies: Mutex::new(
                replies
                    .into_iter()
                    .map(|r| r.map(str::to_owned).map_err(str::to_owned))
                    .collect(),
            ),
            calls: Mutex::new(Vec::new()),
        }
    }
}
impl CommandRunner for Fake {
    fn run(&self, args: &[&str]) -> Result<String, String> {
        self.calls
            .lock()
            .unwrap()
            .push(args.iter().map(|s| s.to_string()).collect());
        self.replies
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected command")
    }
}
const INSTANCES: &str = r#"[{"instance":"session-one","wl_socket":"wayland-1"}]"#;
const MICE: &str = r#"{"mice":[{"name":"touchpad"},{"name":"logitech-mx-anywhere-3s"}]}"#;
fn values() -> PointerSettings {
    PointerSettings {
        sensitivity: 0.4,
        natural_scroll: true,
        acceleration: true,
        scroll_speed: 1.7,
    }
}

#[test]
fn applies_all_four_values_to_only_the_exact_connected_mouse() {
    let backend = Hyprland::with_runner(
        Fake::new(vec![Ok(INSTANCES), Ok(MICE), Ok("ok\n")]),
        None,
        None,
    );
    backend.apply(values()).unwrap();
    let calls = backend.runner.calls.lock().unwrap();
    assert_eq!(calls.len(), 3);
    assert_eq!(&calls[2][..3], &["-i", "session-one", "eval"]);
    assert_eq!(
        calls[2][3],
        "hl.device({name=\"logitech-mx-anywhere-3s\", sensitivity=0.4, natural_scroll=true, accel_profile=\"adaptive\", scroll_factor=1.7})"
    );
}

#[test]
fn rejects_invalid_values_before_any_command() {
    for sensitivity in [f32::NAN, f32::INFINITY, -1.1, 1.1] {
        let backend = Hyprland::with_runner(Fake::new(vec![]), None, None);
        assert!(
            backend
                .apply(PointerSettings {
                    sensitivity,
                    natural_scroll: false,
                    ..values()
                })
                .is_err()
        );
        assert!(backend.runner.calls.lock().unwrap().is_empty());
    }
}

#[test]
fn refuses_missing_mouse_without_writing_compositor_settings() {
    let backend = Hyprland::with_runner(
        Fake::new(vec![
            Ok(INSTANCES),
            Ok(r#"{"mice":[{"name":"mx-master-4"}]}"#),
        ]),
        None,
        None,
    );
    assert!(
        backend
            .apply(values())
            .unwrap_err()
            .contains("not connected")
    );
    assert_eq!(backend.runner.calls.lock().unwrap().len(), 2);
}

#[test]
fn never_guesses_between_multiple_desktop_sessions() {
    let backend = Hyprland::with_runner(
        Fake::new(vec![Ok(
            r#"[{"instance":"a","wl_socket":"wayland-1"},{"instance":"b","wl_socket":"wayland-2"}]"#,
        )]),
        None,
        None,
    );
    assert!(backend.apply(values()).unwrap_err().contains("multiple"));
    assert_eq!(backend.runner.calls.lock().unwrap().len(), 1);
}

#[test]
fn uses_explicit_session_and_rejects_compositor_errors_even_on_successful_process_exit() {
    let backend = Hyprland::with_runner(
        Fake::new(vec![Ok(MICE), Ok("Lua error: invalid option")]),
        Some("current-session".into()),
        None,
    );
    assert!(backend.apply(values()).unwrap_err().contains("Lua error"));
    assert_eq!(
        backend.runner.calls.lock().unwrap()[0][1],
        "current-session"
    );
}

#[test]
fn matches_wayland_session_and_propagates_process_failure() {
    let backend = Hyprland::with_runner(
        Fake::new(vec![
            Ok(
                r#"[{"instance":"a","wl_socket":"wayland-1"},{"instance":"b","wl_socket":"wayland-2"}]"#,
            ),
            Ok(MICE),
            Err("hyprctl timed out"),
        ]),
        None,
        Some("wayland-2".into()),
    );
    assert_eq!(backend.apply(values()).unwrap_err(), "hyprctl timed out");
    assert_eq!(backend.runner.calls.lock().unwrap()[1][1], "b");
}

#[test]
fn flat_acceleration_and_scroll_speed_limits_are_sent_without_changing_globals() {
    for scroll_speed in [0.1, 1.0, 5.0] {
        let backend = Hyprland::with_runner(
            Fake::new(vec![Ok(MICE), Ok("ok")]),
            Some("session-one".into()),
            None,
        );
        backend
            .apply(PointerSettings {
                acceleration: false,
                scroll_speed,
                ..values()
            })
            .unwrap();
        let calls = backend.runner.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert!(calls[1][3].contains("accel_profile=\"flat\""));
        assert!(calls[1][3].contains(&format!("scroll_factor={scroll_speed}")));
        assert!(calls[1][3].starts_with("hl.device("));
    }
}

#[test]
fn invalid_scroll_speed_is_rejected_before_any_compositor_command() {
    for scroll_speed in [
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        0.0,
        -1.0,
        0.09,
        5.1,
    ] {
        let backend = Hyprland::with_runner(Fake::new(vec![]), None, None);
        let error = backend
            .apply(PointerSettings {
                scroll_speed,
                ..values()
            })
            .unwrap_err();
        assert!(error.contains("Scroll speed"));
        assert!(backend.runner.calls.lock().unwrap().is_empty());
    }
}

#[test]
fn profile_apply_registers_owned_device_bindings_without_dispatching_them() {
    use mouse_studio::model::{ActionKind, Settings};
    let backend = Hyprland::with_runner(
        Fake::new(vec![Ok(MICE), Ok("ok")]),
        Some("session".into()),
        None,
    );
    let mut settings = Settings::default();
    settings.active_mut().bindings[0].kind = ActionKind::Application;
    settings.active_mut().bindings[0].value = "code".into();
    backend.apply_profile(settings.active()).unwrap();
    let calls = backend.runner.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(calls[1][3].contains("hl.dsp.exec_cmd(\"code\")"));
    assert!(calls[1][3].contains("mouse:275"));
    assert!(calls[1][3].contains("device={inclusive=true,list={\"logitech-mx-anywhere-3s\"}}"));
    assert!(!calls[1][3].contains("hl.dispatch(hl.dsp.exec_cmd(\"code\"))"));
}

#[test]
fn restore_skips_current_profile_and_reapplies_after_reload_or_reconnect() {
    let profile = mouse_studio::model::Settings::default().active().clone();
    for current in [true, false] {
        let mut replies = vec![Ok(MICE), Ok(if current { "true\n" } else { "false\n" })];
        if !current {
            replies.push(Ok("ok"));
        }
        let backend = Hyprland::with_runner(Fake::new(replies), Some("session".into()), None);
        assert_eq!(backend.ensure_profile(&profile).unwrap(), !current);
        let calls = backend.runner.calls.lock().unwrap();
        assert_eq!(calls.len(), if current { 2 } else { 3 });
        assert!(calls[1][3].contains("is_enabled()"));
        assert!(calls[1][3].contains("s.address"));
    }
}

#[test]
fn invalid_shortcut_prevents_all_profile_writes() {
    use mouse_studio::model::{ActionKind, Settings};
    let backend = Hyprland::with_runner(Fake::new(vec![]), None, None);
    let mut settings = Settings::default();
    settings.active_mut().bindings[0].kind = ActionKind::Shortcut;
    settings.active_mut().bindings[0].value = "BOGUS+C".into();
    assert!(backend.apply_profile(settings.active()).is_err());
    assert!(backend.runner.calls.lock().unwrap().is_empty());
}

#[test]
fn selected_models_apply_only_to_their_named_device_and_runtime() {
    use mouse_studio::{device::MouseModel, model::Settings};
    for model in [MouseModel::M650L, MouseModel::MxMaster4] {
        let mice = format!(
            r#"{{"mice":[{{"name":"logitech-mx-anywhere-3s"}},{{"name":"{}","address":"chosen"}}]}}"#,
            model.input_names()[0]
        );
        let backend = Hyprland::with_runner(
            Fake::new(vec![Ok(&mice), Ok("ok")]),
            Some("session".into()),
            None,
        )
        .for_device(model);
        let settings = Settings::for_device(model);
        backend.apply_settings(&settings).unwrap();
        let calls = backend.runner.calls.lock().unwrap();
        let script = &calls.last().unwrap()[3];
        assert!(script.contains(model.input_names()[0]));
        assert!(!script.contains("logitech-mx-anywhere-3s"));
        assert!(script.contains(model.runtime_state()));
        assert!(!script.contains("_G.__mouse_studio_v1="));
    }
}

#[test]
fn mismatched_profile_device_never_writes_to_the_compositor() {
    use mouse_studio::{device::MouseModel, model::Settings};
    let backend =
        Hyprland::with_runner(Fake::new(vec![]), None, None).for_device(MouseModel::MxMaster4);
    assert!(backend.apply_settings(&Settings::default()).is_err());
    assert!(backend.runner.calls.lock().unwrap().is_empty());
}

#[test]
fn missing_or_ambiguous_selected_mouse_never_falls_back_to_another_model() {
    use mouse_studio::{device::MouseModel, model::Settings};
    for mice in [
        MICE,
        r#"{"mice":[{"name":"logitech-mx-master-4"},{"name":"mx-master-4"}]}"#,
    ] {
        let backend =
            Hyprland::with_runner(Fake::new(vec![Ok(mice)]), Some("session".into()), None)
                .for_device(MouseModel::MxMaster4);
        assert!(
            backend
                .apply_settings(&Settings::for_device(MouseModel::MxMaster4))
                .is_err()
        );
        assert_eq!(backend.runner.calls.lock().unwrap().len(), 1);
    }
}
