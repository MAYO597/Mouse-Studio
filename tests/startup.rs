use mouse_studio::{
    model::Settings,
    startup::{RestoreLoop, unit_text},
};
use std::path::Path;

#[test]
fn unit_quotes_spaces_and_systemd_expansions() {
    let text = unit_text(
        Path::new("/tmp/Mouse app/$x%/mouse-studio"),
        Path::new("/tmp/Mouse app/settings.json"),
    )
    .unwrap();
    assert!(text.contains("\"/tmp/Mouse app/$$x%%/mouse-studio\" --restore-service --settings \"/tmp/Mouse app/settings.json\""));
    assert!(text.contains("WantedBy=default.target"));
    assert!(unit_text(Path::new("/tmp/bad\npath"), Path::new("/tmp/settings.json")).is_err());
}

#[test]
fn restoration_loads_saved_profile_retries_and_obeys_opt_out() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let mut state = RestoreLoop::default();
    assert!(
        state
            .step(&path, |_| panic!("missing file must not apply"))
            .is_err()
    );
    let mut settings = Settings {
        restore_on_login: true,
        ..Default::default()
    };
    settings.active_mut().scroll_speed = 1.7;
    settings.save_to(&path).unwrap();
    assert!(state.step(&path, |_| Err("not connected".into())).is_err());
    assert!(
        state
            .step(&path, |p| {
                assert_eq!(p.scroll_speed, 1.7);
                Ok(true)
            })
            .unwrap()
    );
    assert!(!state.step(&path, |_| Ok(false)).unwrap());
    settings.restore_on_login = false;
    settings.save_to(&path).unwrap();
    assert!(
        !state
            .step(&path, |_| panic!("disabled restore must not apply"))
            .unwrap()
    );
    std::fs::write(&path, "broken").unwrap();
    assert!(
        state
            .step(&path, |_| panic!("invalid settings must not apply"))
            .is_err()
    );
}

#[test]
fn old_settings_files_do_not_enable_background_restore() {
    let mut value = serde_json::to_value(Settings::default()).unwrap();
    value.as_object_mut().unwrap().remove("restore_on_login");
    let settings: Settings = serde_json::from_value(value).unwrap();
    assert!(!settings.restore_on_login);
}

#[derive(Default)]
struct FakeService {
    calls: std::sync::Mutex<Vec<Vec<String>>>,
    fail: bool,
}
impl mouse_studio::startup::ServiceRunner for FakeService {
    fn run(&self, args: &[&str]) -> Result<(), String> {
        self.calls
            .lock()
            .unwrap()
            .push(args.iter().map(|s| s.to_string()).collect());
        if self.fail {
            Err("user bus unavailable".into())
        } else {
            Ok(())
        }
    }
}

#[test]
fn installing_updating_and_disabling_service_preserves_other_files() {
    use mouse_studio::startup::{Startup, StartupControl};
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source");
    std::fs::write(&source, b"first binary").unwrap();
    let startup = Startup {
        config_root: dir.path().join("config"),
        data_root: dir.path().join("data with spaces"),
        source_binary: source.clone(),
        runner: FakeService::default(),
    };
    let settings = dir.path().join("settings.json");
    let unit = startup
        .config_root
        .join("systemd/user/mouse-studio-restore.service");
    startup.sync(false, &settings).unwrap();
    assert!(startup.runner.calls.lock().unwrap().is_empty());
    startup.sync(true, &settings).unwrap();
    let binary = startup.data_root.join("mouse-studio/bin/mouse-studio");
    assert_eq!(std::fs::read(&binary).unwrap(), b"first binary");
    assert_eq!(
        std::fs::metadata(&binary).unwrap().permissions().mode() & 0o777,
        0o755
    );
    assert!(
        std::fs::read_to_string(&unit)
            .unwrap()
            .starts_with("# Managed by Mouse Studio")
    );
    let other = startup.config_root.join("systemd/user/other.service");
    std::fs::write(&other, "keep").unwrap();
    std::fs::write(&source, b"updated binary").unwrap();
    startup.sync(true, &settings).unwrap();
    assert_eq!(std::fs::read(&binary).unwrap(), b"updated binary");
    assert!(
        startup
            .runner
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|c| c[0] == "restart")
    );
    startup.sync(false, &settings).unwrap();
    assert!(!unit.exists());
    assert_eq!(std::fs::read_to_string(other).unwrap(), "keep");
}

#[test]
fn refuses_foreign_unit_and_reports_service_failures() {
    use mouse_studio::startup::{Startup, StartupControl};
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source");
    std::fs::write(&source, b"binary").unwrap();
    let startup = Startup {
        config_root: dir.path().join("config"),
        data_root: dir.path().join("data"),
        source_binary: source,
        runner: FakeService {
            fail: true,
            ..Default::default()
        },
    };
    let unit = startup
        .config_root
        .join("systemd/user/mouse-studio-restore.service");
    std::fs::create_dir_all(unit.parent().unwrap()).unwrap();
    std::fs::write(&unit, "custom service").unwrap();
    assert!(
        startup
            .sync(true, &dir.path().join("settings.json"))
            .unwrap_err()
            .contains("unrelated")
    );
    assert_eq!(std::fs::read_to_string(&unit).unwrap(), "custom service");
    assert!(startup.runner.calls.lock().unwrap().is_empty());
    std::fs::remove_file(unit).unwrap();
    assert!(
        startup
            .sync(true, &dir.path().join("settings.json"))
            .unwrap_err()
            .contains("user bus unavailable")
    );
}

#[test]
fn each_mouse_has_its_own_restore_service_and_disable_preserves_the_others() {
    use mouse_studio::{
        device::MouseModel,
        startup::{Startup, StartupControl, service_name},
    };
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source");
    std::fs::write(&source, b"binary").unwrap();
    let startup = Startup {
        config_root: dir.path().join("config"),
        data_root: dir.path().join("data"),
        source_binary: source,
        runner: FakeService::default(),
    };
    let base = dir.path().join("settings.json");
    for model in MouseModel::ALL {
        let settings = Settings::for_device(model);
        let path = model.settings_path(&base);
        settings.save_to(&path).unwrap();
        startup.sync(true, &path).unwrap();
        let unit = startup
            .config_root
            .join("systemd/user")
            .join(service_name(model));
        let text = std::fs::read_to_string(unit).unwrap();
        assert!(text.contains(path.to_str().unwrap()));
        if model != MouseModel::MxAnywhere3s {
            assert!(text.contains(&format!("--device {}", model.slug())));
        }
    }
    startup
        .sync(false, &MouseModel::M650L.settings_path(&base))
        .unwrap();
    for model in MouseModel::ALL {
        assert_eq!(
            startup
                .config_root
                .join("systemd/user")
                .join(service_name(model))
                .exists(),
            model != MouseModel::M650L
        );
    }
}
