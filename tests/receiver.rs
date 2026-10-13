use mouse_studio::{
    device::MouseModel,
    hardware::receiver::{Pairing, pairing, pointer_name, validate_shared_pointer},
    model::Settings,
    runtime,
};
#[test]
fn paired_master_is_identified_and_shared_receivers_are_rejected() {
    let p = pairing(
        2,
        &[0x52, 2, 0x42, 0xb0, 1, 2, 3, 4],
        b"\x62\x01\x0bMX Master 4",
    )
    .unwrap();
    assert_eq!(p.model, Some(MouseModel::MxMaster4));
    assert!(validate_shared_pointer(MouseModel::MxMaster4, std::slice::from_ref(&p)).is_ok());
    assert!(validate_shared_pointer(MouseModel::M650L, std::slice::from_ref(&p)).is_err());
    assert!(
        validate_shared_pointer(
            MouseModel::MxMaster4,
            &[
                p.clone(),
                Pairing {
                    slot: 1,
                    kind: 2,
                    model: None
                }
            ]
        )
        .is_err()
    );
    assert!(
        validate_shared_pointer(
            MouseModel::MxMaster4,
            &[
                p,
                Pairing {
                    slot: 1,
                    kind: 1,
                    model: None
                }
            ]
        )
        .is_ok()
    );
    assert!(pairing(2, &[0x51, 2, 0, 0, 0, 0, 0, 0], b"\x62\x01\x0bMX Master 4").is_err());
    assert!(pairing(2, &[0x52, 2, 0, 0, 0, 0, 0, 0], b"\x62\x01\x0bMX").is_err());
}
#[test]
fn receiver_names_exclude_consumer_controls_and_arbitrary_input() {
    for n in [
        "logitech-usb-receiver",
        "logitech-usb-receiver-mouse",
        "logitech-usb-receiver-1",
    ] {
        assert!(pointer_name(n));
    }
    for n in [
        "logitech-usb-receiver-consumer-control-1",
        "logitech-usb-receiver-keyboard",
        "logitech-usb-receiver-",
        "logitech-usb-receiver-1\"",
        "other-mouse",
    ] {
        assert!(!pointer_name(n));
    }
}
#[test]
fn receiver_runtime_scopes_all_pointer_interfaces_to_master() {
    let s = Settings::for_device(MouseModel::MxMaster4);
    let names = vec![
        "logitech-usb-receiver-mouse".into(),
        "logitech-usb-receiver-1".into(),
    ];
    let lua = runtime::setup_script_for_inputs(&s, None, &names).unwrap();
    assert!(lua.contains(
        "local input_names={\"logitech-usb-receiver-mouse\",\"logitech-usb-receiver-1\"}"
    ));
    assert!(lua.contains("__mouse_studio_mx_master_4_v1"));
    assert!(!lua.contains("logitech-mx-anywhere-3s"));
    assert!(runtime::setup_script_for_inputs(&s, None, &["other-mouse".into()]).is_err());
    assert!(runtime::setup_script_for_inputs(&s, None, &[]).is_err());
}
#[test]
fn startup_receiver_selection_skips_sleeping_mice_and_non_mouse_pairs() {
    use mouse_studio::{
        device::MouseModel,
        hardware::receiver::{Pairing, connected_model_from},
    };
    let pairs = [
        Pairing {
            slot: 1,
            kind: 2,
            model: Some(MouseModel::MxAnywhere3s),
        },
        Pairing {
            slot: 2,
            kind: 2,
            model: Some(MouseModel::M650L),
        },
        Pairing {
            slot: 3,
            kind: 1,
            model: Some(MouseModel::MxMaster4),
        },
    ];
    assert_eq!(
        connected_model_from(&pairs, |model| model != MouseModel::MxAnywhere3s),
        Some(MouseModel::M650L)
    );
    assert_eq!(connected_model_from(&pairs, |_| false), None);
    let unsupported = [
        Pairing {
            slot: 1,
            kind: 2,
            model: None,
        },
        Pairing {
            slot: 2,
            kind: 1,
            model: Some(MouseModel::MxMaster4),
        },
    ];
    assert_eq!(
        connected_model_from(&unsupported, |_| panic!(
            "Only supported mice should be queried"
        )),
        None
    );
}
