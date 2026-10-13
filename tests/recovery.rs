use mouse_studio::{model::Settings, recovery};

#[test]
fn changed_saves_keep_one_previous_setup_and_retries_preserve_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let original = Settings::default();
    recovery::save_with_history(&original, &path).unwrap();
    assert_eq!(recovery::previous_setup(&path).unwrap(), None);
    let mut changed = original.clone();
    changed.add_profile(false);
    changed.active_mut().sensitivity = 0.7;
    changed.restore_on_login = true;
    recovery::save_with_history(&changed, &path).unwrap();
    recovery::save_with_history(&changed, &path).unwrap();
    assert_eq!(Settings::load_from(&path).unwrap(), changed);
    assert_eq!(
        recovery::previous_setup(&path).unwrap(),
        Some(original.clone())
    );
    recovery::save_with_history(&original, &path).unwrap();
    assert_eq!(Settings::load_from(&path).unwrap(), original);
    assert_eq!(recovery::previous_setup(&path).unwrap(), Some(changed));
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert!(json["previous_setup"].get("previous_setup").is_none());
}

#[test]
fn legacy_files_gain_history_without_inventing_an_earlier_setup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    assert_eq!(recovery::previous_setup(&path).unwrap(), None);
    let original = Settings::default();
    original.save_to(&path).unwrap();
    assert_eq!(recovery::previous_setup(&path).unwrap(), None);
    let mut changed = original.clone();
    changed.active_mut().natural_scroll = true;
    recovery::save_with_history(&changed, &path).unwrap();
    assert_eq!(recovery::previous_setup(&path).unwrap(), Some(original));
}

#[test]
fn invalid_changes_and_unreadable_history_preserve_the_saved_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    Settings::default().save_to(&path).unwrap();
    let before = std::fs::read(&path).unwrap();
    let mut invalid = Settings::default();
    invalid.active_mut().scroll_speed = -1.0;
    assert!(recovery::save_with_history(&invalid, &path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let mut json: serde_json::Value = serde_json::from_slice(&before).unwrap();
    json["previous_setup"] = serde_json::json!({"broken": true});
    let broken = serde_json::to_vec(&json).unwrap();
    std::fs::write(&path, &broken).unwrap();
    assert!(recovery::previous_setup(&path).is_err());
    assert!(recovery::save_with_history(&Settings::default(), &path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), broken);
}

#[test]
fn combined_history_size_is_checked_before_replacing_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let mut large = Settings::default();
    for _ in 1..100 {
        large.add_profile(false);
    }
    for profile in &mut large.profiles {
        for binding in &mut profile.bindings[..3] {
            binding.kind = mouse_studio::model::ActionKind::Command;
            binding.value = "a".repeat(4096);
        }
    }
    recovery::save_with_history(&large, &path).unwrap();
    let before = std::fs::read(&path).unwrap();
    large.active_mut().sensitivity = 0.2;
    assert!(recovery::save_with_history(&large, &path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn an_invalid_previous_setup_does_not_block_saving() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let mut json = serde_json::to_value(Settings::default()).unwrap();
    let mut bad = json.clone();
    bad["version"] = 99.into();
    json["previous_setup"] = bad;
    std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    assert_eq!(recovery::previous_setup(&path).unwrap(), None);
    let mut changed = Settings::default();
    changed.active_mut().natural_scroll = true;
    recovery::save_with_history(&changed, &path).unwrap();
    assert_eq!(Settings::load_from(&path).unwrap(), changed);
}
