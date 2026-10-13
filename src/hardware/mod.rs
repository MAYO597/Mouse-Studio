//! Native HID++ controls for supported Logitech Bluetooth and verified Bolt mice.
pub mod buttons;
pub mod desktop;
mod linux;
mod protocol;
pub mod receiver;
pub mod ui;
pub use linux::{
    apply, apply_for, bluetooth_matches, cycle, cycle_for, identity, identity_for, play_haptic_for,
    read_battery, read_battery_for, snapshot, snapshot_for, toggle_wheel_for,
};
pub use protocol::{Client, Transport, decode_dpi_list, match_reply};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WheelMode {
    Ratchet,
    FreeSpin,
}
impl WheelMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ratchet => "Ratchet",
            Self::FreeSpin => "Free-spin",
        }
    }
    pub(super) fn byte(self) -> u8 {
        match self {
            Self::Ratchet => 2,
            Self::FreeSpin => 1,
        }
    }
    pub(super) fn from_byte(b: u8) -> Result<Self, String> {
        match b {
            1 => Ok(Self::FreeSpin),
            2 => Ok(Self::Ratchet),
            _ => Err("Mouse reported an unknown wheel mode.".into()),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DpiPreset {
    pub name: String,
    pub dpi: u16,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HardwareSettings {
    pub dpi: Option<u16>,
    pub wheel_mode: Option<WheelMode>,
    /// 1–49 is a speed threshold; 50 disables automatic switching (wire value 255).
    pub smartshift: Option<u8>,
    /// None keeps legacy files compatible; Master 4 defaults to native, normal scrolling.
    pub thumbwheel_inverted: Option<bool>,
    /// MX Master 4 only. None keeps native horizontal scrolling; Some diverts the side wheel
    /// and runs the saved actions (see docs/side-wheel.md).
    pub thumbwheel_remap: Option<ThumbWheelRemap>,
    /// MX Master 4 only. None leaves the device's haptic feedback alone.
    pub haptics: Option<HapticSettings>,
    pub presets: Vec<DpiPreset>,
}
impl Default for HardwareSettings {
    fn default() -> Self {
        Self {
            dpi: None,
            wheel_mode: None,
            smartshift: None,
            thumbwheel_inverted: None,
            thumbwheel_remap: None,
            haptics: None,
            presets: vec![
                DpiPreset {
                    name: "Precision".into(),
                    dpi: 800,
                },
                DpiPreset {
                    name: "Balanced".into(),
                    dpi: 1600,
                },
                DpiPreset {
                    name: "Fast".into(),
                    dpi: 3200,
                },
            ],
        }
    }
}
impl HardwareSettings {
    pub fn managed(&self) -> bool {
        self.dpi.is_some()
            || self.wheel_mode.is_some()
            || self.smartshift.is_some()
            || self.thumbwheel_inverted.is_some()
            || self.thumbwheel_remap.is_some()
            || self.haptics.is_some()
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.dpi.is_some_and(|d| !(100..=32000).contains(&d)) {
            return Err(
                "Sensor DPI must be 100–32000, and supported by the connected mouse.".into(),
            );
        }
        if self.smartshift.is_some_and(|t| !(1..=50).contains(&t)) {
            return Err("SmartShift must be 1–50.".into());
        }
        if let Some(h) = &self.haptics {
            h.validate()?;
        }
        if let Some(r) = &self.thumbwheel_remap {
            r.validate()?;
        }
        if self.presets.is_empty() || self.presets.len() > 8 {
            return Err("Keep 1–8 DPI presets.".into());
        }
        let mut values = std::collections::HashSet::new();
        for p in &self.presets {
            if p.name.trim().is_empty()
                || p.name.len() > 40
                || p.name.chars().any(char::is_control)
                || !(100..=32000).contains(&p.dpi)
                || !values.insert(p.dpi)
            {
                return Err(
                    "DPI presets need a name (1–40 bytes) and distinct DPI values of 100–32000."
                        .into(),
                );
            }
        }
        Ok(())
    }
    pub fn dpi_values(&self) -> Vec<u16> {
        self.presets.iter().map(|p| p.dpi).collect()
    }
}
/// Haptic feedback level in percent. Confirmed devices expose either the four steps in
/// [`HAPTIC_LEVELS`] or any value 1–100 (Solaar `HapticLevel`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HapticSettings {
    pub enabled: bool,
    pub level: u8,
}
impl Default for HapticSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            level: 50,
        }
    }
}
impl HapticSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=100).contains(&self.level) {
            return Err("Haptic strength must be 1–100.".into());
        }
        Ok(())
    }
}
pub const HAPTIC_LEVELS: [(u8, &str); 4] =
    [(25, "Low"), (50, "Medium"), (75, "High"), (100, "Maximum")];
/// Live haptic state read from the mouse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HapticState {
    pub enabled: bool,
    pub level: u8,
    /// The device only accepts the four [`HAPTIC_LEVELS`].
    pub discrete_levels: bool,
    /// Bit n set means waveform n can be played.
    pub waveforms: u32,
}
impl HapticState {
    pub fn can_play(&self) -> bool {
        self.waveforms != 0
    }
}

/// What the thumb wheel does instead of native horizontal scrolling.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThumbWheelRemap {
    pub left: crate::model::Binding,
    pub right: crate::model::Binding,
    /// Reported wheel units that make one action.
    pub step: u16,
}
impl Default for ThumbWheelRemap {
    fn default() -> Self {
        Self {
            left: Default::default(),
            right: Default::default(),
            step: 1,
        }
    }
}
impl ThumbWheelRemap {
    pub fn validate(&self) -> Result<(), String> {
        use crate::model::ActionKind::*;
        if !(1..=200).contains(&self.step) {
            return Err("Side-wheel step must be 1–200.".into());
        }
        for b in [&self.left, &self.right] {
            if !matches!(
                b.kind,
                Default | Application | Shortcut | Command | Workspace
            ) {
                return Err(
                    "Side-wheel actions can be a shortcut, application, command or workspace."
                        .into(),
                );
            }
            if b.kind != Default && b.value.trim().is_empty() {
                return Err(
                    "Complete both side-wheel actions or leave a direction on Default.".into(),
                );
            }
            crate::actions::dispatcher(b)?;
        }
        if self.left.kind == Default && self.right.kind == Default {
            return Err("Choose an action for at least one side-wheel direction.".into());
        }
        Ok(())
    }
    /// Positive firmware rotation is treated as scrolling right.
    pub fn binding(&self, right: bool) -> &crate::model::Binding {
        if right { &self.right } else { &self.left }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Battery {
    pub percent: Option<u8>,
    pub status: String,
}
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub available: bool,
    pub dpi: Option<u16>,
    pub dpi_values: Vec<u16>,
    pub wheel_mode: Option<WheelMode>,
    pub smartshift: Option<u8>,
    pub thumbwheel: Option<ThumbWheelState>,
    pub haptics: Option<HapticState>,
    pub battery: Option<Battery>,
    pub error: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThumbWheelState {
    pub diverted: bool,
    pub inverted: bool,
}
pub fn next_dpi(presets: &[u16], current: u16) -> Result<u16, String> {
    if presets.is_empty() || presets.len() > 8 || presets.iter().any(|d| !(100..=32000).contains(d))
    {
        return Err("Invalid DPI preset list.".into());
    }
    Ok(presets
        .iter()
        .position(|d| *d == current)
        .map_or(presets[0], |i| presets[(i + 1) % presets.len()]))
}
