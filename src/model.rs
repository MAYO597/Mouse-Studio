use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub id: u64,
    pub name: String,
    pub sensitivity: f32,
    pub acceleration: bool,
    pub scroll_speed: f32,
    pub natural_scroll: bool,
    #[serde(deserialize_with = "deserialize_bindings")]
    pub bindings: [Binding; 6],
    #[serde(default)]
    pub advanced: crate::software::Advanced,
    #[serde(default)]
    pub app_classes: Vec<String>,
    #[serde(default)]
    pub hardware: crate::hardware::HardwareSettings,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionKind {
    #[default]
    Default,
    Application,
    Shortcut,
    Command,
    Workspace,
    Macro,
    CycleDpi,
    ActionsRing,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub kind: ActionKind,
    pub value: String,
}

/// Version-one files had three controls. Append native defaults on load.
pub(crate) fn deserialize_bindings<'de, D>(deserializer: D) -> Result<[Binding; 6], D::Error>
where
    D: serde::Deserializer<'de>,
{
    let values = Vec::<Binding>::deserialize(deserializer)?;
    if !matches!(values.len(), 3 | 6) {
        return Err(serde::de::Error::custom(
            "Expected three or six button assignments.",
        ));
    }
    let mut bindings = std::array::from_fn(|_| Binding::default());
    for (target, value) in bindings.iter_mut().zip(values) {
        *target = value;
    }
    Ok(bindings)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub device: crate::device::MouseModel,
    pub version: u32,
    pub active_id: u64,
    #[serde(default)]
    pub restore_on_login: bool,
    #[serde(default)]
    pub auto_switch: bool,
    pub profiles: Vec<Profile>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            device: Default::default(),
            version: 1,
            active_id: 1,
            restore_on_login: false,
            auto_switch: false,
            profiles: vec![Profile {
                id: 1,
                name: "Everyday".into(),
                sensitivity: 0.0,
                acceleration: true,
                scroll_speed: 1.0,
                natural_scroll: false,
                bindings: Default::default(),
                advanced: Default::default(),
                app_classes: Vec::new(),
                hardware: Default::default(),
            }],
        }
    }
}

impl Settings {
    pub fn for_device(device: crate::device::MouseModel) -> Self {
        Self {
            device,
            ..Default::default()
        }
    }
    pub fn hardware_required(&self) -> bool {
        self.profiles.iter().any(|p| {
            p.hardware.managed()
                || p.bindings
                    .iter()
                    .chain(p.advanced.hold.iter())
                    .chain(p.advanced.double.iter())
                    .chain(p.advanced.gestures.iter())
                    .any(|b| b.kind == ActionKind::CycleDpi)
        })
    }
    pub fn horizontal_required(&self) -> bool {
        self.profiles
            .iter()
            .any(|p| p.advanced.horizontal_button.is_some())
    }
    pub fn buttons_required(&self) -> bool {
        self.device == crate::device::MouseModel::MxMaster4
            && self.profiles.iter().any(|p| {
                (3..6).any(|i| p.button_managed(i)) || p.hardware.thumbwheel_remap.is_some()
            })
    }
    pub fn service_required(&self) -> bool {
        self.restore_on_login
            || self.horizontal_required()
            || self.hardware_required()
            || self.buttons_required()
            || self.thumbwheel_required()
            || self.ring_required()
    }
    pub fn thumbwheel_required(&self) -> bool {
        self.device == crate::device::MouseModel::MxMaster4
    }
    pub fn effective_hardware(&self, profile: &Profile) -> crate::hardware::HardwareSettings {
        let mut h = profile.hardware.clone();
        if self.thumbwheel_required() && h.thumbwheel_inverted.is_none() {
            h.thumbwheel_inverted = Some(false);
        }
        h
    }
    pub fn active(&self) -> &Profile {
        self.profiles
            .iter()
            .find(|p| p.id == self.active_id)
            .expect("validated active profile")
    }

    pub fn active_mut(&mut self) -> &mut Profile {
        self.profiles
            .iter_mut()
            .find(|p| p.id == self.active_id)
            .expect("validated active profile")
    }

    pub fn add_profile(&mut self, duplicate: bool) {
        let mut profile = if duplicate {
            self.active().clone()
        } else {
            Self::default().profiles.remove(0)
        };
        // A duplicated automatic match would make focus switching ambiguous.
        profile.app_classes.clear();
        let base = if duplicate {
            // Leave room for " copy" and a numeric suffix inside the 80-byte limit.
            let mut stem = profile.name.trim().to_string();
            while stem.len() > 60 {
                stem.pop();
            }
            format!("{stem} copy")
        } else {
            "New profile".into()
        };
        let taken = |candidate: &str, profiles: &[Profile]| {
            let key = candidate.trim().to_lowercase();
            profiles.iter().any(|p| p.name.trim().to_lowercase() == key)
        };
        let mut name = base.clone();
        let mut suffix = 2;
        while taken(&name, &self.profiles) {
            name = format!("{base} {suffix}");
            suffix += 1;
        }
        let mut id = 1;
        while self.profiles.iter().any(|p| p.id == id) {
            id += 1;
        }
        profile.id = id;
        profile.name = name;
        self.active_id = profile.id;
        self.profiles.push(profile);
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("Unsupported settings file version.".into());
        }
        if self.profiles.is_empty() || self.profiles.len() > 100 {
            return Err("Keep between 1 and 100 profiles.".into());
        }
        let mut ids = HashSet::new();
        let mut names = HashSet::new();
        let mut classes = HashSet::new();
        for p in &self.profiles {
            p.hardware.validate()?;
            if !self.thumbwheel_required() && p.hardware.thumbwheel_inverted.is_some() {
                return Err("Side-wheel settings are only available on MX Master 4.".into());
            }
            if !self.thumbwheel_required()
                && (p.hardware.thumbwheel_remap.is_some() || p.hardware.haptics.is_some())
            {
                return Err(
                    "Side-wheel actions and haptic feedback are only available on MX Master 4."
                        .into(),
                );
            }
            if !self.device.has_mx_wheel()
                && (p.hardware.wheel_mode.is_some() || p.hardware.smartshift.is_some())
            {
                return Err(
                    "M650 L uses SmartWheel; MX wheel-mode and SmartShift settings do not apply."
                        .into(),
                );
            }
            p.advanced.validate()?;
            crate::ring::validate_profile(p)?;
            let count = self.device.button_count();
            if (count..6).any(|i| p.button_managed(i)) {
                return Err(format!(
                    "These button assignments are not available on {}.",
                    self.device.name()
                ));
            }
            for class in &p.app_classes {
                if class.trim().is_empty()
                    || class.len() > 200
                    || class.chars().any(char::is_control)
                    || !classes.insert(class.to_ascii_lowercase())
                {
                    return Err(
                        "Application classes must be unique, nonempty and at most 200 bytes."
                            .into(),
                    );
                }
            }
            if !ids.insert(p.id) {
                return Err("Profile IDs must be unique.".into());
            }
            if p.name.trim().is_empty()
                || p.name.len() > 80
                || !names.insert(p.name.trim().to_lowercase())
            {
                return Err("Give each profile a unique name (1–80 bytes).".into());
            }
            if !p.sensitivity.is_finite()
                || !(-1.0..=1.0).contains(&p.sensitivity)
                || !p.scroll_speed.is_finite()
                || !(0.1..=5.0).contains(&p.scroll_speed)
            {
                return Err("Pointer speed must be −1 to 1; scroll speed must be 0.1 to 5.".into());
            }
            for binding in &p.bindings {
                crate::actions::dispatcher(binding)?;
                if !matches!(
                    binding.kind,
                    ActionKind::Default | ActionKind::CycleDpi | ActionKind::ActionsRing
                ) && binding.value.trim().is_empty()
                {
                    return Err(format!(
                        "Complete the button action in ‘{}’ before saving.",
                        p.name
                    ));
                }
                if binding.value.len() > 4096 || binding.value.contains('\0') {
                    return Err("Button action is too long or contains a null character.".into());
                }
                if binding.kind == ActionKind::Workspace
                    && !binding
                        .value
                        .parse::<u32>()
                        .is_ok_and(|n| (1..=99).contains(&n))
                {
                    return Err("Workspace must be a number from 1 to 99.".into());
                }
            }
        }
        if !ids.contains(&self.active_id) {
            return Err("The selected profile does not exist.".into());
        }
        Ok(())
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let data = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        write_settings_file(path, &data)
    }

    pub fn load_from(path: &Path) -> Result<Self, String> {
        let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
        if metadata.len() > 2_000_000 {
            return Err("Settings file is too large (maximum 2 MB).".into());
        }
        let data = fs::read(path).map_err(|e| e.to_string())?;
        let settings: Self =
            serde_json::from_slice(&data).map_err(|e| format!("Cannot read settings: {e}"))?;
        settings.validate()?;
        Ok(settings)
    }
}

impl Profile {
    pub fn button_managed(&self, i: usize) -> bool {
        crate::software::custom(&self.bindings[i])
            || crate::software::custom(&self.advanced.hold[i])
            || crate::software::custom(&self.advanced.double[i])
            || self.advanced.precision_button == Some(i)
            || self.advanced.horizontal_button == Some(i)
            || self.advanced.gesture_button == Some(i)
    }
}

pub(crate) fn write_settings_file(path: &Path, data: &[u8]) -> Result<(), String> {
    if data.len() > 2_000_000 {
        return Err("Settings and previous setup exceed the 2 MB limit. Export and reduce profiles before saving.".into());
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|e| format!("Cannot create settings directory: {e}"))?;
    let result = (|| -> std::io::Result<()> {
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.write_all(data)?;
        file.as_file().sync_all()?;
        file.persist(path).map_err(|e| e.error)?;
        Ok(())
    })();
    result.map_err(|e| format!("Could not save {}: {e}", path.display()))
}

pub fn settings_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("XDG_CONFIG_HOME").filter(|p| Path::new(p).is_absolute()) {
        return Ok(PathBuf::from(path).join("mouse-studio/settings.json"));
    }
    std::env::var_os("HOME")
        .map(|p| PathBuf::from(p).join(".config/mouse-studio/settings.json"))
        .ok_or_else(|| "Neither HOME nor an absolute XDG_CONFIG_HOME is available.".into())
}

impl ActionKind {
    pub const ALL: [Self; 8] = [
        Self::Default,
        Self::Application,
        Self::Shortcut,
        Self::Command,
        Self::Workspace,
        Self::Macro,
        Self::CycleDpi,
        Self::ActionsRing,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Default => "Default behavior",
            Self::Application => "Launch application",
            Self::Shortcut => "Keyboard shortcut",
            Self::Command => "Run command",
            Self::Workspace => "Switch workspace",
            Self::Macro => "Macro sequence",
            Self::CycleDpi => "Cycle DPI",
            Self::ActionsRing => "Actions Ring",
        }
    }
}
