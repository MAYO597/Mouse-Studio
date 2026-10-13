use mouse_studio::{
    device::MouseModel,
    hardware::{Client, HapticSettings, HardwareSettings, Transport},
    model::Settings,
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

type Request = (u8, u8, Vec<u8>, Result<Vec<u8>, String>);
type Steps = Arc<Mutex<VecDeque<Request>>>;
struct Wire(Steps);
impl Transport for Wire {
    fn request(&mut self, feature: u8, function: u8, params: &[u8]) -> Result<Vec<u8>, String> {
        let (f, n, p, r) = self
            .0
            .lock()
            .unwrap()
            .pop_front()
            .expect("Unexpected hardware request");
        assert_eq!((feature, function, params), (f, n, p.as_slice()));
        r
    }
}
fn steps(list: Vec<Request>) -> Steps {
    Arc::new(Mutex::new(list.into()))
}
fn feature_present() -> Request {
    (0, 0, vec![0x19, 0xb0], Ok(vec![21, 0, 0]))
}
fn feature_missing() -> Request {
    (0, 0, vec![0x19, 0xb0], Ok(vec![0, 0, 0]))
}
fn desired(enabled: bool, level: u8) -> HardwareSettings {
    HardwareSettings {
        haptics: Some(HapticSettings { enabled, level }),
        ..Default::default()
    }
}
/// `read` payload: [enabled bits, level, discrete-level flag]
fn read(enabled: u8, level: u8, discrete: u8) -> Request {
    (21, 0x10, vec![], Ok(vec![enabled, level, discrete]))
}

#[test]
fn telemetry_decodes_state_and_playable_effects_without_writing() {
    let s = steps(vec![
        feature_present(),
        read(1, 60, 0),
        (21, 0, vec![], Ok(vec![0, 0, 0, 0, 0, 0, 0, 0x15])),
    ]);
    let state = Client::new(Wire(s.clone()))
        .read_haptics()
        .unwrap()
        .unwrap();
    assert!(state.enabled);
    assert_eq!(state.level, 60);
    assert!(!state.discrete_levels);
    assert_eq!(state.waveforms, 0x15);
    assert!(state.can_play());
    assert!(s.lock().unwrap().is_empty());
}

#[test]
fn unsupported_mice_are_not_queried_further_or_written() {
    let s = steps(vec![feature_missing()]);
    assert_eq!(Client::new(Wire(s.clone())).read_haptics().unwrap(), None);
    let s = steps(vec![feature_missing()]);
    let error = Client::new(Wire(s.clone()))
        .apply(&desired(true, 50))
        .unwrap_err();
    assert!(error.contains("haptic"), "{error}");
    assert!(s.lock().unwrap().is_empty());
}

#[test]
fn enabling_writes_enable_and_strength_then_verifies_readback() {
    let s = steps(vec![
        feature_present(),
        read(0, 50, 0),
        (21, 0x20, vec![1, 75], Ok(vec![1, 75])),
        read(1, 75, 0),
    ]);
    Client::new(Wire(s.clone()))
        .apply(&desired(true, 75))
        .unwrap();
    assert!(s.lock().unwrap().is_empty());
}

#[test]
fn disabling_keeps_the_devices_strength_and_matching_state_is_not_rewritten() {
    let s = steps(vec![
        feature_present(),
        read(1, 40, 0),
        (21, 0x20, vec![0, 40], Ok(vec![0, 40])),
        read(0, 40, 0),
    ]);
    Client::new(Wire(s.clone()))
        .apply(&desired(false, 90))
        .unwrap();
    assert!(s.lock().unwrap().is_empty());
    let s = steps(vec![feature_present(), read(1, 75, 0)]);
    Client::new(Wire(s.clone()))
        .apply(&desired(true, 75))
        .unwrap();
    assert!(s.lock().unwrap().is_empty());
    // An invalid retained strength falls back to Solaar's 50 % when disabling.
    let s = steps(vec![
        feature_present(),
        read(1, 0, 0),
        (21, 0x20, vec![0, 50], Ok(vec![])),
        read(0, 50, 0),
    ]);
    Client::new(Wire(s.clone()))
        .apply(&desired(false, 10))
        .unwrap();
}

#[test]
fn four_level_devices_reject_other_strengths_before_any_write() {
    let s = steps(vec![feature_present(), read(1, 50, 1)]);
    let error = Client::new(Wire(s.clone()))
        .apply(&desired(true, 60))
        .unwrap_err();
    assert!(error.contains("Low, Medium"), "{error}");
    assert!(s.lock().unwrap().is_empty());
    let s = steps(vec![
        feature_present(),
        read(1, 50, 1),
        (21, 0x20, vec![1, 100], Ok(vec![])),
        read(1, 100, 1),
    ]);
    Client::new(Wire(s)).apply(&desired(true, 100)).unwrap();
}

#[test]
fn readback_mismatch_restores_the_previous_haptic_state() {
    let s = steps(vec![
        feature_present(),
        read(1, 30, 0),
        (21, 0x20, vec![1, 80], Ok(vec![])),
        read(1, 30, 0),
        // rollback
        (21, 0x20, vec![1, 30], Ok(vec![])),
        read(1, 30, 0),
    ]);
    let error = Client::new(Wire(s.clone()))
        .apply(&desired(true, 80))
        .unwrap_err();
    assert!(
        error.contains("readback") && error.contains("restored"),
        "{error}"
    );
    assert!(s.lock().unwrap().is_empty());
}

#[test]
fn a_failed_haptic_write_reports_when_its_rollback_also_fails() {
    // Haptics succeed, then nothing else is requested: write failure on rollback is reported.
    let s = steps(vec![
        feature_present(),
        read(0, 50, 0),
        (21, 0x20, vec![1, 50], Err("write failed".into())),
        // rollback attempt after the failed write
        (21, 0x20, vec![0, 50], Err("still failing".into())),
    ]);
    let error = Client::new(Wire(s.clone()))
        .apply(&desired(true, 50))
        .unwrap_err();
    assert!(
        error.contains("write failed") && error.contains("also failed"),
        "{error}"
    );
}

#[test]
fn test_haptic_plays_an_advertised_effect_and_changes_no_setting() {
    // Bit 4 (SUBTLE_COLLISION) available.
    let s = steps(vec![
        feature_present(),
        feature_present(),
        read(1, 50, 0),
        (21, 0, vec![], Ok(vec![0, 0, 0, 0, 0, 0, 0, 0x30])),
        (21, 0x40, vec![4], Ok(vec![])),
    ]);
    Client::new(Wire(s.clone())).play_haptic().unwrap();
    assert!(s.lock().unwrap().is_empty());
    // Falls back to the first advertised effect from the preference list.
    let s = steps(vec![
        feature_present(),
        feature_present(),
        read(1, 50, 0),
        (21, 0, vec![], Ok(vec![0, 0, 0, 0, 0, 0, 0, 0x04])),
        (21, 0x40, vec![2], Ok(vec![])),
    ]);
    Client::new(Wire(s)).play_haptic().unwrap();
}

#[test]
fn test_haptic_refuses_when_unsupported_disabled_or_without_effects() {
    let s = steps(vec![feature_missing()]);
    assert!(Client::new(Wire(s.clone())).play_haptic().is_err());
    assert!(s.lock().unwrap().is_empty());
    let s = steps(vec![
        feature_present(),
        feature_present(),
        read(0, 50, 0),
        (21, 0, vec![], Ok(vec![0, 0, 0, 0, 0, 0, 0, 0x10])),
    ]);
    assert!(Client::new(Wire(s.clone())).play_haptic().is_err());
    assert!(
        s.lock().unwrap().is_empty(),
        "no play request while disabled"
    );
    let s = steps(vec![
        feature_present(),
        feature_present(),
        read(1, 50, 0),
        (21, 0, vec![], Ok(vec![0, 0, 0, 0, 0, 0, 0, 0])),
    ]);
    assert!(Client::new(Wire(s.clone())).play_haptic().is_err());
    assert!(s.lock().unwrap().is_empty());
}

#[test]
fn haptic_profiles_validate_round_trip_and_stay_master_only() {
    assert!(desired(true, 0).validate().is_err());
    assert!(desired(true, 101).validate().is_err());
    assert!(desired(false, 1).validate().is_ok());
    let mut master = Settings::for_device(MouseModel::MxMaster4);
    assert!(
        !master.active().hardware.managed() || master.active().hardware.haptics.is_none(),
        "old profiles must not manage haptics"
    );
    master.active_mut().hardware.haptics = Some(HapticSettings {
        enabled: true,
        level: 75,
    });
    assert!(master.hardware_required());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("master.json");
    master.save_to(&path).unwrap();
    let mut loaded = Settings::load_from(&path).unwrap();
    assert_eq!(loaded.active().hardware.haptics.unwrap().level, 75);
    loaded.add_profile(true);
    assert_eq!(loaded.active().hardware.haptics.unwrap().level, 75);
    for model in [MouseModel::MxAnywhere3s, MouseModel::M650L] {
        let mut other = Settings::for_device(model);
        other.active_mut().hardware.haptics = Some(HapticSettings::default());
        assert!(other.validate().is_err(), "{model:?}");
    }
}

#[test]
fn legacy_settings_without_haptic_fields_still_load() {
    let mut value = serde_json::to_value(Settings::for_device(MouseModel::MxMaster4)).unwrap();
    let hardware = value["profiles"][0]["hardware"].as_object_mut().unwrap();
    hardware.remove("haptics");
    hardware.remove("thumbwheel_remap");
    let loaded: Settings = serde_json::from_value(value).unwrap();
    assert_eq!(loaded.active().hardware.haptics, None);
    assert_eq!(loaded.active().hardware.thumbwheel_remap, None);
    loaded.validate().unwrap();
}

#[test]
fn restore_applies_the_saved_haptic_setting_with_the_rest_of_the_profile() {
    use mouse_studio::hardware::desktop::Follower;
    let mut s = Settings::for_device(MouseModel::MxMaster4);
    s.active_mut().hardware.haptics = Some(HapticSettings {
        enabled: false,
        level: 50,
    });
    let mut follower = Follower::default();
    assert!(
        follower
            .step(&s, None, Some("bolt"), |h| {
                assert_eq!(h.haptics.map(|h| h.enabled), Some(false));
                Ok(())
            })
            .unwrap()
    );
}

#[test]
#[ignore = "plays a real haptic effect on the connected MX Master 4; changes no setting"]
fn live_test_haptic_effect_on_the_connected_master() {
    mouse_studio::hardware::play_haptic_for(MouseModel::MxMaster4).unwrap();
}
