//! Explicit opt-in only; creates harmless temporary actions and restores owned binding state.
use mouse_studio::{
    actions,
    backend::{CommandRunner, DeviceBackend, Hyprctl, Hyprland},
    model::{ActionKind, Settings, settings_path},
};
use std::time::{Duration, Instant};

#[test]
#[ignore = "registers real mouse binds and executes a temporary marker command"]
fn live_bindings_are_device_filtered_idempotent_and_dispatch_commands() {
    assert_eq!(std::env::var("MOUSE_TEST_ACTIONS").as_deref(), Ok("1"));
    let session = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").expect("explicit test session");
    let runner = Hyprctl;
    let eval = |lua: &str| -> String { runner.run(&["-i", &session, "eval", lua]).unwrap() };
    let repl = |lua: &str| -> String { runner.run(&["-i", &session, "repl", lua]).unwrap() };
    let before_errors = runner.run(&["-i", &session, "configerrors"]).unwrap();
    let original = Settings::load_from(&settings_path().unwrap()).unwrap();
    struct Cleanup(Settings);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            Hyprland::default()
                .apply_settings(&self.0)
                .expect("restore original saved setup");
        }
    }
    let _cleanup = Cleanup(original.clone());
    let temp = tempfile::tempdir().unwrap();
    let marker = temp.path().join("button-action-worked");
    let mut profile = original.active().clone();
    profile.advanced = Default::default();
    profile.app_classes.clear();
    profile.bindings = Default::default();
    profile.bindings[0].kind = ActionKind::Command;
    profile.bindings[0].value = format!("touch '{}'", marker.display());
    profile.bindings[1].kind = ActionKind::Shortcut;
    profile.bindings[1].value = "CTRL+SHIFT+C".into();
    profile.bindings[2].kind = ActionKind::Workspace;
    profile.bindings[2].value = "2".into();
    let backend = Hyprland::default();
    backend.apply_profile(&profile).unwrap();
    assert!(!marker.exists(), "Saving must not execute an action");
    let count_binds = || -> usize {
        let value: serde_json::Value =
            serde_json::from_str(&runner.run(&["-i", &session, "binds", "-j"]).unwrap()).unwrap();
        value
            .as_array()
            .unwrap()
            .iter()
            .filter(|b| {
                b["description"]
                    .as_str()
                    .is_some_and(|s| s.starts_with("Mouse Studio: "))
            })
            .count()
    };
    let registered = count_binds();
    assert!(registered >= 18);
    backend.apply_profile(&profile).unwrap();
    assert_eq!(
        count_binds(),
        registered,
        "Repeated saves duplicated bindings"
    );
    for i in 1..=3 {
        assert_eq!(
            repl(&format!("__mouse_studio_v1.handles[{i}].device_inclusive")).trim(),
            "true"
        );
        assert_eq!(
            repl(&format!("__mouse_studio_v1.handles[{i}].devices[1]")).trim(),
            actions::MOUSE
        );
    }
    assert!(
        !backend.ensure_profile(&profile).unwrap(),
        "Unchanged profile must not be reapplied"
    );
    assert_eq!(
        eval("hl.dispatch(__mouse_studio_v1.actions[1])").trim(),
        "ok"
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    while !marker.exists() {
        assert!(Instant::now() < deadline, "action did not create marker");
        std::thread::sleep(Duration::from_millis(20));
    }
    profile.bindings = Default::default();
    backend.apply_profile(&profile).unwrap();
    assert_eq!(count_binds(), registered);
    for i in 1..=3 {
        assert_eq!(
            repl(&format!("__mouse_studio_v1.handles[{i}]:is_enabled()")).trim(),
            "false"
        );
    }
    assert_eq!(
        runner.run(&["-i", &session, "configerrors"]).unwrap(),
        before_errors
    );
}
