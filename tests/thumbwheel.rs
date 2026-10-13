use mouse_studio::hardware::{Client, HardwareSettings, Transport};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

type Request = (u8, u8, Vec<u8>, Result<Vec<u8>, String>);
struct Wire(Arc<Mutex<VecDeque<Request>>>);
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
fn settings(inverted: bool) -> HardwareSettings {
    let mut value = serde_json::to_value(HardwareSettings::default()).unwrap();
    value["thumbwheel_inverted"] = serde_json::json!(inverted);
    serde_json::from_value(value).unwrap()
}
#[test]
fn restoring_horizontal_scroll_clears_diversion_and_reads_back_the_direction() {
    let steps = Arc::new(Mutex::new(VecDeque::from([
        (0, 0, vec![0x21, 0x50], Ok(vec![18, 0, 0])),
        (18, 0x10, vec![], Ok(vec![1, 1])),
        (18, 0x20, vec![0, 0], Ok(vec![0, 0])),
        (18, 0x10, vec![], Ok(vec![0, 0])),
    ])));
    Client::new(Wire(steps.clone()))
        .apply(&settings(false))
        .unwrap();
    assert!(
        steps.lock().unwrap().is_empty(),
        "Save never restored the silent side wheel to native horizontal scrolling"
    );
}

#[test]
fn master_profiles_restore_native_side_scrolling_even_for_legacy_defaults() {
    use mouse_studio::{device::MouseModel, hardware::desktop::Follower, model::Settings};
    let mut s = Settings::for_device(MouseModel::MxMaster4);
    assert!(
        s.service_required(),
        "Native side-wheel restoration needs to survive closing the app"
    );
    let mut follower = Follower::default();
    assert!(
        follower
            .step(&s, None, Some("bolt-slot2"), |h| {
                assert_eq!(h.thumbwheel_inverted, Some(false));
                assert_eq!(h.dpi, None, "Side-wheel repair must not alter sensor DPI");
                Ok(())
            })
            .unwrap()
    );
    s.add_profile(false);
    s.active_mut().hardware.thumbwheel_inverted = Some(true);
    assert!(
        follower
            .step(&s, None, Some("bolt-slot2"), |h| {
                assert_eq!(h.thumbwheel_inverted, Some(true));
                Ok(())
            })
            .unwrap()
    );
    assert!(
        follower
            .step(&s, None, None, |_| panic!(
                "Disconnected mouse must not be written"
            ))
            .is_err()
    );
    assert!(
        follower
            .step(&s, None, Some("reconnected-bolt-slot2"), |h| {
                assert_eq!(h.thumbwheel_inverted, Some(true));
                Ok(())
            })
            .unwrap()
    );
}

#[test]
fn side_wheel_settings_are_rejected_for_mice_without_a_side_wheel() {
    use mouse_studio::{device::MouseModel, model::Settings};
    for model in [MouseModel::MxAnywhere3s, MouseModel::M650L] {
        let mut s = Settings::for_device(model);
        s.active_mut().hardware.thumbwheel_inverted = Some(false);
        assert!(s.validate().is_err());
    }
}

#[test]
fn reversing_side_scroll_preserves_unrelated_hardware_flags() {
    let steps = Arc::new(Mutex::new(VecDeque::from([
        (0, 0, vec![0x21, 0x50], Ok(vec![18, 0, 0])),
        (18, 0x10, vec![], Ok(vec![0x81, 0x40])),
        (18, 0x20, vec![0x80, 0x41], Ok(vec![0x80, 0x41])),
        (18, 0x10, vec![], Ok(vec![0x80, 0x41])),
    ])));
    Client::new(Wire(steps.clone()))
        .apply(&settings(true))
        .unwrap();
    assert!(steps.lock().unwrap().is_empty());
}

#[test]
fn failed_side_wheel_readback_restores_the_exact_previous_state() {
    let steps = Arc::new(Mutex::new(VecDeque::from([
        (0, 0, vec![0x21, 0x50], Ok(vec![18, 0, 0])),
        (18, 0x10, vec![], Ok(vec![1, 1])),
        (18, 0x20, vec![0, 0], Ok(vec![0, 0])),
        (18, 0x10, vec![], Ok(vec![1, 0])),
        (18, 0x20, vec![1, 1], Ok(vec![1, 1])),
        (18, 0x10, vec![], Ok(vec![1, 1])),
    ])));
    assert!(
        Client::new(Wire(steps.clone()))
            .apply(&settings(false))
            .is_err()
    );
    assert!(steps.lock().unwrap().is_empty());
}

#[test]
fn native_scrolling_is_left_alone_when_already_correct() {
    let steps = Arc::new(Mutex::new(VecDeque::from([
        (0, 0, vec![0x21, 0x50], Ok(vec![18, 0, 0])),
        (18, 0x10, vec![], Ok(vec![0, 0])),
    ])));
    Client::new(Wire(steps.clone()))
        .apply(&settings(false))
        .unwrap();
    assert!(steps.lock().unwrap().is_empty());
}

#[test]
fn unavailable_side_wheel_rejects_the_apply_before_other_hardware_is_written() {
    let steps = Arc::new(Mutex::new(VecDeque::from([
        (0, 0, vec![0x22, 1], Ok(vec![14, 0, 0])),
        (14, 0x10, vec![0, 0, 0], Ok(vec![0, 3, 0x20, 6, 0x40, 0, 0])),
        (14, 0x20, vec![0], Ok(vec![0, 3, 0x20, 6, 0x40])),
        (0, 0, vec![0x21, 0x50], Ok(vec![0, 0, 0])),
    ])));
    let mut desired = settings(false);
    desired.dpi = Some(1600);
    assert!(Client::new(Wire(steps.clone())).apply(&desired).is_err());
    assert!(steps.lock().unwrap().is_empty());
}

#[test]
fn telemetry_reports_when_side_scroll_is_diverted() {
    let steps = Arc::new(Mutex::new(VecDeque::from([
        (0, 0, vec![0x22, 1], Ok(vec![0, 0, 0])),
        (0, 0, vec![0x21, 0x10], Ok(vec![0, 0, 0])),
        (0, 0, vec![0x21, 0x11], Ok(vec![0, 0, 0])),
        (0, 0, vec![0x21, 0x50], Ok(vec![18, 0, 0])),
        (18, 0x10, vec![], Ok(vec![1, 0])),
    ])));
    let snapshot = Client::new(Wire(steps.clone())).read().unwrap();
    let side = snapshot.thumbwheel.unwrap();
    assert!(side.diverted);
    assert!(!side.inverted);
    assert!(steps.lock().unwrap().is_empty());
}

#[test]
fn side_wheel_is_rechecked_after_bolt_wake_without_resetting_cycled_dpi() {
    use mouse_studio::{device::MouseModel, hardware::desktop::Follower, model::Settings};
    let mut s = Settings::for_device(MouseModel::MxMaster4);
    s.active_mut().hardware.dpi = Some(800);
    let mut follower = Follower::default();
    follower
        .step(&s, None, Some("same-bolt-connection"), |h| {
            assert_eq!(h.dpi, Some(800));
            Ok(())
        })
        .unwrap();
    // Bolt retains its sysfs identity when the paired mouse sleeps and wakes.
    std::thread::sleep(std::time::Duration::from_millis(5100));
    assert!(
        follower
            .step(&s, None, Some("same-bolt-connection"), |h| {
                assert_eq!(
                    h.dpi, None,
                    "Periodic side-wheel repair must preserve manually cycled DPI"
                );
                assert_eq!(h.wheel_mode, None);
                assert_eq!(h.smartshift, None);
                assert_eq!(h.thumbwheel_inverted, Some(false));
                Ok(())
            })
            .unwrap(),
        "An unchanged Bolt receiver identity prevented side-wheel restoration"
    );
}
