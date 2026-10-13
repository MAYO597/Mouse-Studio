//! Real-desktop adapter; isolated editors and compositor tests do not open HID devices.
use crate::{
    backend::{DeviceBackend, Hyprland, PointerSettings, RuntimeStatus},
    model::{Binding, Profile, Settings},
};
#[derive(Default)]
pub struct Desktop {
    pub compositor: Hyprland,
}
impl Desktop {
    pub fn for_device(device: crate::device::MouseModel) -> Self {
        Self {
            compositor: Hyprland::default().for_device(device),
        }
    }
}
impl DeviceBackend for Desktop {
    fn apply(&self, p: PointerSettings) -> Result<(), String> {
        self.compositor.apply(p)
    }
    fn apply_settings(&self, s: &Settings) -> Result<(), String> {
        self.compositor.apply_settings(s)?;
        let live = self.compositor.runtime_status()?;
        let profile = live
            .profile_id
            .and_then(|id| s.profiles.iter().find(|p| p.id == id))
            .unwrap_or_else(|| s.active());
        if s.buttons_required() {
            super::buttons::wait_ready(s, profile.id)
                .map_err(|e| format!("Desktop settings applied; extra buttons failed: {e}"))?;
        }
        super::apply_for(&s.effective_hardware(profile), self.compositor.device)
            .map_err(|e| format!("Desktop settings applied; hardware settings failed: {e}"))
    }
    fn apply_profile(&self, p: &Profile) -> Result<(), String> {
        self.apply_settings(&Settings {
            device: self.compositor.device,
            active_id: p.id,
            profiles: vec![p.clone()],
            ..Default::default()
        })
    }
    fn runtime_status(&self) -> Result<RuntimeStatus, String> {
        self.compositor.runtime_status()
    }
    fn hardware_status(&self) -> super::Snapshot {
        let mut s = super::snapshot_for(self.compositor.device);
        if s.error.is_none() {
            s.error = action_error_for(self.compositor.device);
        }
        s
    }
    fn test_action(&self, b: &Binding) -> Result<(), String> {
        if b.kind == crate::model::ActionKind::CycleDpi {
            Err("Save a Cycle DPI assignment and test with the mouse button. It uses that saved profile's presets.".into())
        } else {
            self.compositor.test_action(b)
        }
    }
    fn test_profile_action(&self, b: &Binding, p: &Profile) -> Result<(), String> {
        if b.kind == crate::model::ActionKind::CycleDpi {
            p.hardware.validate()?;
            let result = super::cycle_for(&p.hardware.dpi_values(), self.compositor.device);
            report_action_for(&result, self.compositor.device);
            result
        } else {
            self.test_action(b)
        }
    }
    fn test_haptic(&self) -> Result<(), String> {
        let result = super::play_haptic_for(self.compositor.device);
        report_action_for(&result, self.compositor.device);
        result
    }
    fn cancel_actions(&self) -> Result<(), String> {
        self.compositor.cancel_actions()
    }
}

fn report_path(model: crate::device::MouseModel) -> Option<std::path::PathBuf> {
    let root = std::env::var_os("XDG_RUNTIME_DIR")?;
    let root = std::path::PathBuf::from(root);
    root.is_absolute().then(|| {
        root.join(format!(
            "mouse-studio-{}-hardware-result.json",
            model.slug()
        ))
    })
}
pub fn report_action(result: &Result<(), String>) {
    report_action_for(result, Default::default());
}
pub fn report_action_for(result: &Result<(), String>, model: crate::device::MouseModel) {
    if let Some(path) = report_path(model) {
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if let Ok(bytes) = serde_json::to_vec(&(time, result.as_ref().err())) {
            let _ = crate::model::write_settings_file(&path, &bytes);
        }
    }
}
fn action_error_for(model: crate::device::MouseModel) -> Option<String> {
    let path = report_path(model)?;
    let bytes = std::fs::read(path).ok()?;
    let (time, error): (u64, Option<String>) = serde_json::from_slice(&bytes).ok()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    if now.saturating_sub(time) <= 15 {
        error.map(|e| format!("Hardware action failed: {e}"))
    } else {
        None
    }
}

#[derive(Default)]
pub struct Follower {
    last: Option<(u64, String, String)>,
    last_thumb_check: Option<std::time::Instant>,
}
impl Follower {
    pub fn step(
        &mut self,
        s: &Settings,
        profile_id: Option<u64>,
        device: Option<&str>,
        apply: impl FnOnce(&super::HardwareSettings) -> Result<(), String>,
    ) -> Result<bool, String> {
        let id = profile_id.unwrap_or(s.active_id);
        let p = s
            .profiles
            .iter()
            .find(|p| p.id == id)
            .ok_or("Active hardware profile is not in the saved setup.")?;
        let hardware = s.effective_hardware(p);
        if !hardware.managed() {
            self.last = None;
            self.last_thumb_check = None;
            return Ok(false);
        }
        let Some(device) = device else {
            self.last = None;
            self.last_thumb_check = None;
            return Err("Mouse disconnected; hardware restore will retry.".into());
        };
        let key = (id, crate::actions::token(p), device.to_string());
        if self.last.as_ref() == Some(&key) {
            // A remapped (diverted) side wheel is owned and repaired by the button helper.
            if hardware.thumbwheel_inverted.is_some()
                && hardware.thumbwheel_remap.is_none()
                && self
                    .last_thumb_check
                    .is_none_or(|last| last.elapsed() >= std::time::Duration::from_secs(5))
            {
                // A paired Bolt mouse can reset reporting without the receiver
                // disappearing. Verify only side-wheel state; do not undo DPI
                // cycling or physical mode-button changes while checking it.
                apply(&super::HardwareSettings {
                    thumbwheel_inverted: hardware.thumbwheel_inverted,
                    ..Default::default()
                })?;
                self.last_thumb_check = Some(std::time::Instant::now());
                return Ok(true);
            }
            return Ok(false);
        }
        apply(&hardware)?;
        self.last = Some(key);
        self.last_thumb_check = Some(std::time::Instant::now());
        Ok(true)
    }
}

pub fn follow_in_background(path: std::path::PathBuf, model: crate::device::MouseModel) {
    // Device requests never block the service's horizontal-wheel event pump.
    std::thread::spawn(move || {
        let compositor =
            Hyprland::with_runner(crate::backend::Hyprctl, None, None).for_device(model);
        let mut follower = Follower::default();
        let mut last_error = None;
        loop {
            let result = (|| {
                let settings = Settings::load_from(&path)?;
                if settings.device != model {
                    return Err("Hardware profiles belong to another mouse.".into());
                }
                if !settings.hardware_required() && !settings.thumbwheel_required() {
                    follower = Follower::default();
                    return Ok(false);
                }
                let status = compositor.runtime_status()?;
                // Wait for the restore loop to install the complete runtime.
                if status.profile_id.is_none() {
                    return Ok(false);
                }
                follower.step(
                    &settings,
                    status.profile_id,
                    super::identity_for(model).as_deref(),
                    |h| super::apply_for(h, model),
                )
            })();
            if let Err(e) = &result
                && last_error.as_ref() != Some(e)
            {
                eprintln!("Mouse Studio hardware: {e}");
                report_action_for(&Err(e.clone()), model);
            }
            let failed = result.is_err();
            last_error = result.err();
            std::thread::sleep(std::time::Duration::from_millis(if failed {
                5000
            } else {
                500
            }));
        }
    });
}
