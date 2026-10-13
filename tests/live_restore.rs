//! Opt-in integration test of actual user-service installation and headless recovery.
use mouse_studio::{
    backend::{CommandRunner, DeviceBackend, Hyprctl, Hyprland},
    model::{Settings, settings_path},
    startup::{Startup, StartupControl, Systemctl},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[test]
#[ignore = "temporarily installs and enables the user restore service, then removes it"]
fn user_service_restores_after_marker_loss_and_leaves_no_enabled_unit() {
    assert_eq!(
        std::env::var("MOUSE_TEST_RESTORE_SERVICE").as_deref(),
        Ok("1")
    );
    let session = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").expect("explicit session required");
    let config = PathBuf::from(std::env::var_os("HOME").unwrap()).join(".config");
    let unit = config.join("systemd/user/mouse-studio-restore.service");
    assert!(
        !unit.exists(),
        "Do not run this check over an existing user installation"
    );
    let temp = tempfile::Builder::new()
        .prefix("mouse restore ")
        .tempdir()
        .unwrap();
    let path = temp.path().join("settings with spaces.json");
    let original = Settings::load_from(&settings_path().unwrap()).unwrap();
    let mut settings = original.clone();
    settings.restore_on_login = true;
    settings.active_mut().bindings = Default::default();
    settings.active_mut().scroll_speed = 1.3;
    settings.save_to(&path).unwrap();
    let startup = Startup {
        config_root: config,
        data_root: temp.path().join("data with spaces"),
        source_binary: PathBuf::from(
            std::env::var_os("MOUSE_TEST_BINARY").expect("release binary required"),
        ),
        runner: Systemctl,
    };
    let runner = Hyprctl;
    let eval = |lua: &str| {
        assert_eq!(
            runner.run(&["-i", &session, "eval", lua]).unwrap().trim(),
            "ok"
        );
    };
    struct Cleanup<'a> {
        startup: &'a Startup,
        path: &'a std::path::Path,
        original: Settings,
    }
    impl Drop for Cleanup<'_> {
        fn drop(&mut self) {
            self.startup
                .sync(false, self.path)
                .expect("remove temporary user service");
            Hyprland::default()
                .apply_settings(&self.original)
                .expect("restore pointer settings");
        }
    }
    let cleanup = Cleanup {
        startup: &startup,
        path: &path,
        original: original.clone(),
    };
    startup.sync(true, &path).unwrap();
    let wait_for = |expected: f32| {
        let deadline = Instant::now() + Duration::from_secs(12);
        loop {
            let devices: serde_json::Value =
                serde_json::from_str(&runner.run(&["-i", &session, "devices", "-j"]).unwrap())
                    .unwrap();
            let mouse = devices["mice"]
                .as_array()
                .unwrap()
                .iter()
                .find(|m| m["name"] == "logitech-mx-anywhere-3s")
                .unwrap();
            if (mouse["scrollFactor"].as_f64().unwrap() - f64::from(expected)).abs() < 0.001 {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "service did not restore requested scroll speed"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
    };
    wait_for(1.3);
    // Simulate lost configuration without reloading the user's whole desktop.
    // The same expired-handle/marker path is used after an actual config reload.
    eval(
        "__mouse_studio_v1.token=nil; hl.device({name=\"logitech-mx-anywhere-3s\",scroll_factor=0.7})",
    );
    wait_for(1.3);
    settings.active_mut().scroll_speed = 1.1;
    settings.save_to(&path).unwrap();
    wait_for(1.1);
    startup.sync(false, &path).unwrap();
    assert!(!unit.exists());
    assert!(
        !startup
            .config_root
            .join("systemd/user/default.target.wants/mouse-studio-restore.service")
            .exists()
    );
    drop(cleanup);
}
