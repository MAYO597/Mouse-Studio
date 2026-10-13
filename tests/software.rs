use mouse_studio::{
    model::{ActionKind, Binding, Settings},
    software,
};
#[test]
fn defaults_reset_identity_and_legacy_settings() {
    let legacy = r#"{"version":1,"active_id":1,"profiles":[{"id":1,"name":"Work","sensitivity":0.5,"acceleration":false,"scroll_speed":2.0,"natural_scroll":true,"bindings":[{"kind":"Default","value":""},{"kind":"Default","value":""},{"kind":"Default","value":""}]}]}"#;
    let mut settings: Settings = serde_json::from_str(legacy).unwrap();
    settings.validate().unwrap();
    assert!(!settings.auto_switch);
    software::reset_profile(settings.active_mut());
    assert_eq!(settings.active().name, "Work");
    assert_eq!(settings.active_id, 1);
    assert_eq!(settings.active().sensitivity, 0.0);
}
#[test]
fn macros_and_special_modes_reject_ambiguous_or_unbounded_data() {
    let value = software::compose_macro("CTRL+C\nwait 250\nALT+Tab").unwrap();
    assert_eq!(software::parse_macro(&value).unwrap().len(), 3);
    assert!(software::compose_macro("wait 10001").is_err());
    assert!(software::compose_macro("CTRL+UNKNOWN").is_err());
    let mut settings = Settings::default();
    settings.active_mut().bindings[0] = Binding {
        kind: ActionKind::Macro,
        value,
    };
    settings.validate().unwrap();
    settings.active_mut().advanced.precision_button = Some(1);
    settings.active_mut().advanced.gesture_button = Some(1);
    assert!(settings.validate().is_err());
}
#[test]
fn app_classes_are_unique_and_directions_have_a_dead_zone() {
    let mut settings = Settings::default();
    settings.active_mut().app_classes.push("Code".into());
    settings.add_profile(false);
    settings.active_mut().app_classes.push("code".into());
    assert!(settings.validate().is_err());
    assert_eq!(software::direction(10.0, 10.0, 50.0), None);
    assert_eq!(software::direction(80.0, 20.0, 50.0), Some(3));
    assert_eq!(software::direction(0.0, -80.0, 50.0), Some(0));
}

#[test]
fn connection_metadata_and_horizontal_requests_are_read_without_hid_access() {
    let bluetooth = "I: Bus=0005 Vendor=046d Product=b037\nN: Name=\"MX Anywhere 3S\"\n\n";
    assert_eq!(
        mouse_studio::device::connection_from(bluetooth),
        Some("Bluetooth")
    );
    assert_eq!(
        mouse_studio::device::connection_from(&bluetooth.replace("0005", "0003")),
        Some("USB receiver")
    );
    assert_eq!(
        mouse_studio::device::connection_from("N: Name=\"Other mouse\""),
        None
    );
    assert_eq!(
        mouse_studio::horizontal::validate_request(b"-1").unwrap(),
        -1
    );
    assert!(mouse_studio::horizontal::validate_request(b"100000").is_err());
    let mut settings = Settings::default();
    assert!(!settings.horizontal_required());
    settings.active_mut().advanced.horizontal_button = Some(0);
    assert!(settings.horizontal_required());
}
