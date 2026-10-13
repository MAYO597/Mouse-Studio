use mouse_studio::model::{ActionKind, Settings};

#[test]
fn edited_profiles_survive_a_disk_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested/settings.json");
    let mut settings = Settings::default();
    settings.profiles[0].name = "Design work".into();
    settings.profiles[0].sensitivity = 0.35;
    settings.profiles[0].bindings[0].kind = ActionKind::Application;
    settings.profiles[0].bindings[0].value = "code".into();
    settings.save_to(&path).unwrap();
    assert_eq!(Settings::load_from(&path).unwrap(), settings);
}

#[test]
fn invalid_edits_do_not_overwrite_saved_settings() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let mut settings = Settings::default();
    settings.save_to(&path).unwrap();
    settings.profiles[0].scroll_speed = -3.0;
    assert!(settings.save_to(&path).is_err());
    assert_eq!(Settings::load_from(&path).unwrap(), Settings::default());
}

#[test]
fn broken_files_are_reported_instead_of_silently_reset() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    std::fs::write(&path, "{broken").unwrap();
    assert!(Settings::load_from(&path).is_err());
}

#[test]
fn invalid_profile_references_and_actions_are_rejected() {
    let mut settings = Settings {
        active_id: 99,
        ..Default::default()
    };
    assert!(settings.validate().is_err());
    settings.active_id = 1;
    settings.profiles.push(settings.profiles[0].clone());
    assert!(settings.validate().is_err());
    settings.profiles.pop();
    settings.profiles[0].bindings[1].kind = ActionKind::Workspace;
    settings.profiles[0].bindings[1].value = "not-a-number".into();
    assert!(settings.validate().is_err());
}

#[test]
fn duplicated_profile_names_stay_unique_and_within_limits() {
    let mut settings = Settings::default();
    settings.profiles[0].name = "Work".into();
    settings.add_profile(true);
    settings.profiles[1].name = "work copy".into();
    settings.active_id = settings.profiles[0].id;
    settings.add_profile(true);
    settings.profiles[0].name = "x".repeat(80);
    settings.active_id = settings.profiles[0].id;
    settings.add_profile(true);
    assert!(settings.validate().is_ok());
}
