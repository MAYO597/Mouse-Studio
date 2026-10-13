//! Pointer controls and device-filtered button actions through Hyprland's Lua API.
//! IPC uses argument-vector subprocesses; action execution is an explicit user choice.
use crate::{
    actions,
    model::{Binding, Profile, Settings},
};
use serde::Deserialize;

use crate::device::MouseModel;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerSettings {
    pub sensitivity: f32,
    pub natural_scroll: bool,
    pub acceleration: bool,
    pub scroll_speed: f32,
}
impl From<&Profile> for PointerSettings {
    fn from(profile: &Profile) -> Self {
        Self {
            sensitivity: profile.sensitivity,
            natural_scroll: profile.natural_scroll,
            acceleration: profile.acceleration,
            scroll_speed: profile.scroll_speed,
        }
    }
}

impl PointerSettings {
    pub fn script(self) -> Result<String, String> {
        self.script_for(MouseModel::default().input_names()[0])
    }
    pub fn script_for(self, name: &str) -> Result<String, String> {
        if !MouseModel::ALL
            .iter()
            .any(|d| d.input_names().contains(&name))
            && !crate::hardware::receiver::pointer_name(name)
        {
            return Err("Unknown input device name.".into());
        }
        if !self.sensitivity.is_finite() || !(-1.0..=1.0).contains(&self.sensitivity) {
            return Err("Pointer speed must be between -1 and 1.".into());
        }
        if !self.scroll_speed.is_finite() || !(0.1..=5.0).contains(&self.scroll_speed) {
            return Err("Scroll speed must be between 0.1× and 5×.".into());
        }
        // The device name is a fixed allowlist entry; only validated numeric/boolean
        // values and fixed acceleration profile names enter Lua. Profile names and action commands are never executed.
        let accel_profile = if self.acceleration {
            "adaptive"
        } else {
            "flat"
        };
        let lua = format!(
            "hl.device({{name=\"{name}\", sensitivity={}, natural_scroll={}, accel_profile=\"{accel_profile}\", scroll_factor={}}})",
            self.sensitivity, self.natural_scroll, self.scroll_speed
        );
        Ok(lua)
    }
}

/// Upper bound of side-wheel actions sent in one compositor call.
pub const THUMB_STEPS_PER_BATCH: usize = 8;

pub trait DeviceBackend: Send + Sync {
    /// Success means the compositor acknowledged the per-device configuration.
    fn apply(&self, settings: PointerSettings) -> Result<(), String>;
    fn apply_profile(&self, profile: &Profile) -> Result<(), String> {
        self.apply(PointerSettings::from(profile))
    }
    fn test_action(&self, _binding: &Binding) -> Result<(), String> {
        Err("Action testing is unavailable in preview mode.".into())
    }
    fn test_profile_action(&self, binding: &Binding, _profile: &Profile) -> Result<(), String> {
        self.test_action(binding)
    }
    fn apply_settings(&self, settings: &Settings) -> Result<(), String> {
        self.apply_profile(settings.active())
    }
    fn runtime_status(&self) -> Result<RuntimeStatus, String> {
        Ok(RuntimeStatus::default())
    }
    fn cancel_actions(&self) -> Result<(), String> {
        Ok(())
    }
    fn hardware_status(&self) -> crate::hardware::Snapshot {
        Default::default()
    }
    /// Plays one haptic effect on the connected mouse; no setting is changed.
    fn test_haptic(&self) -> Result<(), String> {
        Err("Haptic testing is unavailable in preview mode.".into())
    }
}

#[derive(Clone, Debug, Default)]
pub struct RuntimeStatus {
    pub pressed: [bool; 8],
    pub profile_id: Option<u64>,
    pub macro_running: bool,
}

pub trait CommandRunner: Send + Sync {
    fn run(&self, args: &[&str]) -> Result<String, String>;
}

pub struct Hyprctl;
impl CommandRunner for Hyprctl {
    fn run(&self, args: &[&str]) -> Result<String, String> {
        crate::process::run(
            std::ffi::OsStr::new("hyprctl"),
            &args.iter().map(std::ffi::OsStr::new).collect::<Vec<_>>(),
            std::time::Duration::from_secs(3),
        )
    }
}

pub struct Hyprland<R = Hyprctl> {
    pub runner: R,
    pub device: MouseModel,
    instance: Option<String>,
    wayland_display: Option<String>,
}

impl Default for Hyprland {
    fn default() -> Self {
        Self::with_runner(
            Hyprctl,
            std::env::var("HYPRLAND_INSTANCE_SIGNATURE")
                .ok()
                .filter(|s| !s.is_empty()),
            std::env::var("WAYLAND_DISPLAY")
                .ok()
                .filter(|s| !s.is_empty()),
        )
    }
}

#[derive(Deserialize)]
struct Instance {
    instance: String,
    wl_socket: String,
}
#[derive(Deserialize)]
struct Devices {
    mice: Vec<Mouse>,
}
#[derive(Deserialize)]
struct Mouse {
    name: String,
    #[serde(default)]
    address: String,
}

impl<R: CommandRunner> Hyprland<R> {
    pub fn with_runner(
        runner: R,
        instance: Option<String>,
        wayland_display: Option<String>,
    ) -> Self {
        Self {
            runner,
            device: Default::default(),
            instance,
            wayland_display,
        }
    }

    pub fn for_device(mut self, device: MouseModel) -> Self {
        self.device = device;
        self
    }
    fn check_device(&self, settings: &Settings) -> Result<(), String> {
        if settings.device != self.device {
            return Err("These profiles belong to a different mouse.".into());
        }
        settings.validate()
    }
    fn connected(&self) -> Result<(String, String, Vec<String>), String> {
        let session = self.session()?;
        let json = self.runner.run(&["-i", &session, "devices", "-j"])?;
        let devices: Devices =
            serde_json::from_str(&json).map_err(|_| "Cannot read Hyprland's connected mice.")?;
        let matches: Vec<_> = devices
            .mice
            .iter()
            .filter(|m| self.device.input_names().contains(&m.name.as_str()))
            .collect();
        match matches.as_slice() {
            [mouse] => Ok((session, mouse.address.clone(), vec![mouse.name.clone()])),
            [] if devices
                .mice
                .iter()
                .any(|m| crate::hardware::receiver::pointer_name(&m.name)) =>
            {
                crate::hardware::receiver::verify_pointer(self.device)?;
                let targets: Vec<_> = devices
                    .mice
                    .iter()
                    .filter(|m| crate::hardware::receiver::pointer_name(&m.name))
                    .collect();
                Ok((
                    session,
                    targets
                        .iter()
                        .map(|m| m.address.as_str())
                        .collect::<Vec<_>>()
                        .join("|"),
                    targets.iter().map(|m| m.name.clone()).collect(),
                ))
            }
            [] => Err(format!(
                "{} is not connected. Reconnect it, then save again.",
                self.device.name()
            )),
            _ => Err(format!(
                "Multiple {} connections found. Disconnect the extra connection before applying settings.",
                self.device.name()
            )),
        }
    }

    fn eval(&self, session: &str, lua: &str) -> Result<(), String> {
        let reply = self.runner.run(&["-i", session, "eval", lua])?;
        if reply.trim() != "ok" {
            return Err(format!(
                "Hyprland did not confirm applying settings: {}",
                reply.trim()
            ));
        }
        Ok(())
    }

    pub fn ensure_profile(&self, profile: &Profile) -> Result<bool, String> {
        self.ensure_settings(&Settings {
            device: self.device,
            active_id: profile.id,
            profiles: vec![profile.clone()],
            ..Default::default()
        })
    }

    pub fn ensure_settings(&self, settings: &Settings) -> Result<bool, String> {
        let helper = crate::startup::scroll_helper_command()?;
        self.check_device(settings)?;
        // Validate before issuing any desktop writes; resolve the exact transport-specific name.
        crate::runtime::setup_script(settings, Some(&helper))?;
        let (session, address, name) = self.connected()?;
        let script = crate::runtime::setup_script_for_inputs(settings, Some(&helper), &name)?;
        let reply = self.runner.run(&[
            "-i",
            &session,
            "repl",
            &crate::runtime::query(settings, &address),
        ])?;
        if reply.trim() == "true" {
            return Ok(false);
        }
        self.eval(
            &session,
            &format!(
                "{script}\n{}.address={}",
                self.device.runtime_state(),
                actions::lua_string(&address)
            ),
        )?;
        Ok(true)
    }

    pub fn button_profile(&self, settings: &Settings) -> Result<(String, Option<u64>), String> {
        self.check_device(settings)?;
        let session = self.session()?;
        let state = self.device.runtime_state();
        let query = format!(
            "(function() local s={state}; if not s or not s.address or s.setup_token~={} then return 0 end; return s.profile_id or 0 end)()",
            actions::lua_string(&crate::runtime::setup_token(settings))
        );
        let reply = self.runner.run(&["-i", &session, "repl", &query])?;
        let id = reply
            .trim()
            .parse::<u64>()
            .map_err(|_| "Cannot read extra-button profile.".to_string())?;
        if id != 0 && !settings.profiles.iter().any(|p| p.id == id) {
            return Err("Extra-button profile is not in the saved setup.".into());
        }
        Ok((session, (id != 0).then_some(id)))
    }

    pub fn button_events(
        &self,
        session: &str,
        settings: &Settings,
        profile: &Profile,
        edges: &[(usize, bool)],
    ) -> Result<(), String> {
        if edges.is_empty() {
            return Ok(());
        }
        if edges.iter().any(|(i, _)| !(3..6).contains(i)) {
            return Err("Invalid extra-button event.".into());
        }
        let state = self.device.runtime_state();
        let calls = edges
            .iter()
            .map(|(i, down)| {
                format!(
                    "if s.consume[{}] then s.{}({}) end",
                    i + 1,
                    if *down { "press" } else { "release" },
                    i + 1
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        // Events from a stale capture cannot run a newly selected profile's action.
        let script = format!(
            "local s={state}; if s and s.address and s.setup_token=={} and s.profile_id=={} and s.token=={} then {calls} end",
            actions::lua_string(&crate::runtime::setup_token(settings)),
            profile.id,
            actions::lua_string(&actions::token(profile))
        );
        self.eval(session, &script)
    }

    /// Runs the saved side-wheel actions. `steps` holds `true` for right, `false` for left.
    pub fn thumb_events(
        &self,
        session: &str,
        settings: &Settings,
        profile: &Profile,
        steps: &[bool],
    ) -> Result<(), String> {
        let Some(remap) = &profile.hardware.thumbwheel_remap else {
            return Ok(());
        };
        if steps.is_empty() {
            return Ok(());
        }
        let calls = steps
            .iter()
            .take(THUMB_STEPS_PER_BATCH)
            .filter(|right| remap.binding(**right).kind != crate::model::ActionKind::Default)
            .map(|right| {
                crate::runtime::action_expression_for(remap.binding(*right), self.device)
                    .map(|action| format!("hl.dispatch({action})"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if calls.is_empty() {
            return Ok(());
        }
        let state = self.device.runtime_state();
        // Events from a stale capture cannot run a newly selected profile's action.
        let script = format!(
            "local s={state}; if s and s.address and s.setup_token=={} and s.profile_id=={} and s.token=={} then {} end",
            actions::lua_string(&crate::runtime::setup_token(settings)),
            profile.id,
            actions::lua_string(&actions::token(profile)),
            calls.join("; ")
        );
        self.eval(session, &script)
    }

    pub(crate) fn session(&self) -> Result<String, String> {
        if let Some(instance) = &self.instance {
            return Ok(instance.clone());
        }
        let json = self.runner.run(&["instances", "-j"])?;
        let instances: Vec<Instance> = serde_json::from_str(&json).map_err(|_| {
            "Cannot discover a Hyprland session. Open Mouse Studio inside Hyprland.".to_string()
        })?;
        if let Some(display) = &self.wayland_display {
            let mut matches = instances.iter().filter(|i| &i.wl_socket == display);
            if let Some(instance) = matches.next()
                && matches.next().is_none()
            {
                return Ok(instance.instance.clone());
            }
            return Err("Cannot identify the Hyprland session for this window.".into());
        }
        match instances.as_slice() {
            [instance] => Ok(instance.instance.clone()),
            [] => Err("No Hyprland session is running.".into()),
            _ => Err(
                "Found multiple Hyprland sessions. Open Mouse Studio from your desktop terminal."
                    .into(),
            ),
        }
    }
}

impl<R: CommandRunner> DeviceBackend for Hyprland<R> {
    fn apply_settings(&self, settings: &Settings) -> Result<(), String> {
        let helper = crate::startup::scroll_helper_command()?;
        self.check_device(settings)?;
        // Validate before issuing any desktop writes; resolve the exact transport-specific name.
        crate::runtime::setup_script(settings, Some(&helper))?;
        if settings.horizontal_required() {
            crate::horizontal::wait_ready_for(self.device)?;
        }
        let (session, address, name) = self.connected()?;
        let script = crate::runtime::setup_script_for_inputs(settings, Some(&helper), &name)?;
        self.eval(
            &session,
            &format!(
                "{script}\n{}.address={}",
                self.device.runtime_state(),
                actions::lua_string(&address)
            ),
        )
    }
    fn runtime_status(&self) -> Result<RuntimeStatus, String> {
        let session = self.session()?;
        let query = "(function() local s=__mouse_studio_v1; if not s or not s.pressed then return '0|00000000|0' end; local p=''; for i=1,8 do p=p..((s.pressed[i] or (s.flash and s.flash[i]>0)) and '1' or '0') end; local m=__mouse_studio_macro_v1; return tostring(s.profile_id or 0)..'|'..p..'|'..(m and m.steps and '1' or '0') end)()";
        let reply = self.runner.run(&[
            "-i",
            &session,
            "repl",
            &crate::runtime::scope(query, self.device),
        ])?;
        let parts: Vec<_> = reply.trim().split('|').collect();
        if parts.len() != 3 || !matches!(parts[1].len(), 5 | 8) {
            return Err("Cannot read live mouse state.".into());
        }
        let id = parts[0]
            .parse::<u64>()
            .map_err(|_| "Invalid runtime profile")?;
        Ok(RuntimeStatus {
            profile_id: if id == 0 { None } else { Some(id) },
            pressed: std::array::from_fn(|i| {
                let index = if parts[1].len() == 5 {
                    match i {
                        0..=2 => Some(i),
                        6..=7 => Some(i - 3),
                        _ => None,
                    }
                } else {
                    Some(i)
                };
                index.is_some_and(|i| parts[1].as_bytes()[i] == b'1')
            }),
            macro_running: parts[2] == "1",
        })
    }
    fn cancel_actions(&self) -> Result<(), String> {
        let session = self.session()?;
        self.eval(&session,&crate::runtime::scope("if __mouse_studio_v1 and __mouse_studio_v1.cancel then __mouse_studio_v1.cancel() end; if __mouse_studio_macro_v1 then __mouse_studio_macro_v1.cancel() end",self.device))
    }
    fn apply_profile(&self, profile: &Profile) -> Result<(), String> {
        self.apply_settings(&Settings {
            device: self.device,
            active_id: profile.id,
            profiles: vec![profile.clone()],
            ..Default::default()
        })
    }

    fn test_action(&self, binding: &Binding) -> Result<(), String> {
        if binding.kind == crate::model::ActionKind::Default {
            return Err("Default behavior is tested by pressing the physical mouse button.".into());
        }
        if binding.kind == crate::model::ActionKind::ActionsRing {
            return Err(
                "The Actions Ring opens from the saved physical button; save, then press it."
                    .into(),
            );
        }
        let dispatch = crate::runtime::action_expression_for(binding, self.device)?;
        let session = self.session()?;
        self.eval(
            &session,
            &format!(
                "{}\nhl.dispatch({dispatch})",
                crate::runtime::macro_support_for(self.device)
            ),
        )
    }

    fn apply(&self, settings: PointerSettings) -> Result<(), String> {
        settings.script()?;
        let (session, _, name) = self.connected()?;
        let lua = name
            .iter()
            .map(|n| settings.script_for(n))
            .collect::<Result<Vec<_>, _>>()?
            .join("\n");
        self.eval(&session, &lua)
    }
}
