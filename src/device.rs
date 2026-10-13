use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouseModel {
    #[default]
    MxAnywhere3s,
    M650L,
    MxMaster4,
}
impl MouseModel {
    pub const ALL: [Self; 3] = [Self::MxAnywhere3s, Self::M650L, Self::MxMaster4];
    pub fn name(self) -> &'static str {
        match self {
            Self::MxAnywhere3s => "MX Anywhere 3S",
            Self::M650L => "Signature M650 L",
            Self::MxMaster4 => "MX Master 4",
        }
    }
    pub fn slug(self) -> &'static str {
        match self {
            Self::MxAnywhere3s => "mx-anywhere-3s",
            Self::M650L => "m650-l",
            Self::MxMaster4 => "mx-master-4",
        }
    }
    pub fn parse(value: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|m| m.slug() == value)
            .ok_or_else(|| "Unknown mouse. Use mx-anywhere-3s, m650-l or mx-master-4.".into())
    }
    pub fn input_names(self) -> &'static [&'static str] {
        match self {
            Self::MxAnywhere3s => &["logitech-mx-anywhere-3s", "mx-anywhere-3s"],
            Self::M650L => &[
                "logitech-signature-m650-l",
                "logitech-m650-l",
                "logi-m650-l",
                "signature-m650-l",
                "m650-l",
                "logitech-m650",
                "logi-m650",
                "m650",
                "logitech-signature-m650",
                "signature-m650",
            ],
            Self::MxMaster4 => &["logitech-mx-master-4", "logi-mx-master-4", "mx-master-4"],
        }
    }
    pub fn matches_name(self, name: &str) -> bool {
        let normalized = name.trim().to_ascii_lowercase().replace([' ', '_'], "-");
        self.input_names().contains(&normalized.as_str())
    }
    pub fn runtime_state(self) -> &'static str {
        match self {
            Self::MxAnywhere3s => "__mouse_studio_v1",
            Self::M650L => "__mouse_studio_m650_l_v1",
            Self::MxMaster4 => "__mouse_studio_mx_master_4_v1",
        }
    }
    pub fn macro_state(self) -> &'static str {
        match self {
            Self::MxAnywhere3s => "__mouse_studio_macro_v1",
            Self::M650L => "__mouse_studio_m650_l_macro_v1",
            Self::MxMaster4 => "__mouse_studio_mx_master_4_macro_v1",
        }
    }
    pub fn settings_file(self) -> &'static str {
        match self {
            Self::MxAnywhere3s => "settings.json",
            Self::M650L => "m650-l.json",
            Self::MxMaster4 => "mx-master-4.json",
        }
    }
    pub fn settings_path(self, legacy: &Path) -> PathBuf {
        if self == Self::MxAnywhere3s {
            legacy.to_path_buf()
        } else {
            legacy.with_file_name(self.settings_file())
        }
    }
    pub fn has_mx_wheel(self) -> bool {
        self != Self::M650L
    }
    pub fn button_count(self) -> usize {
        if self == Self::MxMaster4 { 6 } else { 3 }
    }
}

pub const BUTTON_LABELS: [&str; 6] = [
    "Back button",
    "Forward button",
    "Middle click",
    "Wheel-mode button",
    "Gesture button",
    "Haptic Sense panel",
];
pub const BUTTON_DEFAULTS: [&str; 6] = [
    "Navigate back",
    "Navigate forward",
    "Middle click",
    "Switch wheel mode",
    "Native gesture button",
    "Native panel behavior",
];
pub const BUTTON_MARKERS: [&str; 6] = ["4", "5", "3", "6", "7", "8"];
#[derive(Clone, Debug, PartialEq)]
pub enum DeviceStatus {
    Connected,
    Disconnected,
    Unavailable,
}
pub fn discover_for(model: MouseModel) -> DeviceStatus {
    match std::fs::read_to_string("/proc/bus/input/devices") {
        Ok(data) => {
            if matching_block(&data, model).is_some()
                || crate::hardware::receiver::known_live(model)
            {
                DeviceStatus::Connected
            } else {
                DeviceStatus::Disconnected
            }
        }
        Err(_) => DeviceStatus::Unavailable,
    }
}
fn matching_block(data: &str, model: MouseModel) -> Option<&str> {
    data.split("\n\n").find(|block| {
        block.lines().any(|l| {
            l.strip_prefix("N: Name=\"")
                .and_then(|s| s.strip_suffix('"'))
                .is_some_and(|s| model.matches_name(s))
        })
    })
}
pub fn connection_from_for(devices: &str, model: MouseModel) -> Option<&'static str> {
    let block = matching_block(devices, model)?;
    let bus = block
        .lines()
        .find_map(|line| line.strip_prefix("I: Bus="))?
        .split_whitespace()
        .next()?;
    Some(match bus {
        "0005" => "Bluetooth",
        "0003" => "USB receiver",
        _ => "Other input connection",
    })
}
pub fn connection_for(model: MouseModel) -> Option<&'static str> {
    let data = std::fs::read_to_string("/proc/bus/input/devices").ok()?;
    let Some(base) = connection_from_for(&data, model) else {
        return crate::hardware::receiver::known_live(model).then_some("Bolt receiver");
    };
    if base == "USB receiver"
        && let Some(block) = matching_block(&data, model)
        && let Some(event) = block
            .lines()
            .find_map(|l| l.strip_prefix("H: Handlers="))
            .and_then(|l| l.split_whitespace().find(|s| s.starts_with("event")))
        && let Ok(path) = std::fs::canonicalize(format!("/sys/class/input/{event}/device"))
    {
        for parent in path.ancestors() {
            if std::fs::read_to_string(parent.join("product"))
                .is_ok_and(|s| s.to_lowercase().contains("bolt"))
            {
                return Some("Bolt receiver");
            }
        }
    }
    Some(base)
}
pub fn discover() -> DeviceStatus {
    discover_for(MouseModel::default())
}

pub fn startup_model_from(
    requested: Option<MouseModel>,
    devices: &str,
    receiver: impl FnOnce() -> Option<MouseModel>,
) -> MouseModel {
    requested
        .or_else(|| {
            MouseModel::ALL
                .into_iter()
                .find(|model| matching_block(devices, *model).is_some())
        })
        .or_else(receiver)
        .unwrap_or_default()
}

/// Choose only the GUI's initial editor; this never applies a profile.
pub fn startup_model(requested: Option<MouseModel>) -> MouseModel {
    if let Some(model) = requested {
        return model;
    }
    let devices = std::fs::read_to_string("/proc/bus/input/devices").unwrap_or_default();
    startup_model_from(None, &devices, crate::hardware::receiver::connected_model)
}
pub fn connection_from(data: &str) -> Option<&'static str> {
    connection_from_for(data, MouseModel::default())
}
pub fn connection() -> Option<&'static str> {
    connection_for(MouseModel::default())
}
