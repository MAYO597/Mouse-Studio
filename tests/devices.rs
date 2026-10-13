use mouse_studio::{
    device::{MouseModel, connection_from_for, startup_model_from},
    hardware::bluetooth_matches,
    model::Settings,
};

#[test]
fn startup_selects_the_connected_named_mouse() {
    for (name, expected) in [
        ("Logitech MX Master 4", MouseModel::MxMaster4),
        ("Logitech Signature M650 L", MouseModel::M650L),
        ("Logitech MX Anywhere 3S", MouseModel::MxAnywhere3s),
    ] {
        let data = format!("I: Bus=0005 Vendor=046d\nN: Name=\"{name}\"\n\n");
        assert_eq!(
            startup_model_from(None, &data, || panic!(
                "Named mouse needs no receiver query"
            )),
            expected
        );
    }
}

#[test]
fn startup_selects_a_verified_bolt_mouse_and_falls_back_without_one() {
    let data = "I: Bus=0003 Vendor=046d Product=c548\nN: Name=\"Logitech USB Receiver\"\n\n";
    assert_eq!(
        startup_model_from(None, data, || Some(MouseModel::MxMaster4)),
        MouseModel::MxMaster4
    );
    assert_eq!(
        startup_model_from(None, data, || Some(MouseModel::M650L)),
        MouseModel::M650L
    );
    assert_eq!(
        startup_model_from(None, data, || None),
        MouseModel::MxAnywhere3s
    );
    let other = "N: Name=\"Logitech MX Master 4 Keyboard\"\n\nN: Name=\"Other mouse\"\n\n";
    assert_eq!(
        startup_model_from(None, other, || None),
        MouseModel::MxAnywhere3s
    );
}

#[test]
fn explicit_startup_device_wins_over_connected_mice() {
    let data = "N: Name=\"Logitech MX Master 4\"\n\n";
    assert_eq!(
        startup_model_from(Some(MouseModel::M650L), data, || panic!(
            "Explicit selection needs no receiver query"
        )),
        MouseModel::M650L
    );
}

#[test]
fn startup_selection_is_stable_when_multiple_named_mice_are_connected() {
    let master = "N: Name=\"Logitech MX Master 4\"\n\n";
    let m650 = "N: Name=\"Logitech Signature M650 L\"\n\n";
    for data in [format!("{master}{m650}"), format!("{m650}{master}")] {
        assert_eq!(
            startup_model_from(None, &data, || panic!(
                "Named mouse needs no receiver query"
            )),
            MouseModel::M650L
        );
    }
}
#[test]
fn legacy_profiles_keep_the_original_mouse_and_new_models_have_distinct_files() {
    let mut legacy = serde_json::to_value(Settings::default()).unwrap();
    legacy.as_object_mut().unwrap().remove("device");
    let s: Settings = serde_json::from_value(legacy).unwrap();
    assert_eq!(s.device, MouseModel::MxAnywhere3s);
    let base = std::path::Path::new("/tmp/mouse-studio/settings.json");
    let mut files = std::collections::HashSet::new();
    for model in MouseModel::ALL {
        assert!(files.insert(model.settings_path(base)));
        assert_eq!(MouseModel::parse(model.slug()).unwrap(), model);
        assert_eq!(Settings::for_device(model).device, model);
    }
}
#[test]
fn identifies_supported_bluetooth_and_receiver_names_without_matching_other_mice() {
    for model in MouseModel::ALL {
        for name in model.input_names() {
            assert!(model.matches_name(name));
            for other in MouseModel::ALL {
                assert_eq!(other.matches_name(name), other == model);
            }
            let data = format!(
                "I: Bus=0003 Vendor=046d\nN: Name=\"{}\"\n\n",
                name.replace('-', " ")
            );
            assert_eq!(connection_from_for(&data, model), Some("USB receiver"));
            assert_eq!(
                connection_from_for(&data.replace("0003", "0005"), model),
                Some("Bluetooth")
            );
            let uevent = format!(
                "HID_NAME={}\nHID_ID=0005:0000046D:0000ABCD\n",
                name.replace('-', " ")
            );
            assert!(bluetooth_matches(&uevent, model));
            assert!(!bluetooth_matches(
                &uevent.replace("0000046D", "00001234"),
                model
            ));
            assert!(!bluetooth_matches(&uevent.replace("0005:", "0003:"), model));
        }
    }
    assert!(!MouseModel::MxMaster4.matches_name("Logitech MX Master 4 Keyboard"));
}
#[test]
fn m650_rejects_mx_wheel_configuration() {
    let mut s = Settings::for_device(MouseModel::M650L);
    s.active_mut().hardware.smartshift = Some(20);
    assert!(s.validate().unwrap_err().contains("SmartWheel"));
}
