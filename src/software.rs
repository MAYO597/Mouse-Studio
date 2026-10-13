//! Validated host-managed features; no HID++ calls.
use crate::{
    actions,
    model::{ActionKind, Binding, Profile},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Advanced {
    #[serde(deserialize_with = "crate::model::deserialize_bindings")]
    pub hold: [Binding; 6],
    #[serde(deserialize_with = "crate::model::deserialize_bindings")]
    pub double: [Binding; 6],
    pub hold_ms: u32,
    pub double_ms: u32,
    pub precision_button: Option<usize>,
    pub precision_speed: f32,
    pub horizontal_button: Option<usize>,
    pub gesture_button: Option<usize>,
    pub gesture_threshold: f32,
    pub gestures: [Binding; 4], // Up, Down, Left, Right
    pub ring: crate::ring::Ring,
}
impl Default for Advanced {
    fn default() -> Self {
        Self {
            hold: Default::default(),
            double: Default::default(),
            hold_ms: 450,
            double_ms: 280,
            precision_button: None,
            precision_speed: -0.65,
            horizontal_button: None,
            gesture_button: None,
            gesture_threshold: 60.0,
            gestures: Default::default(),
            ring: Default::default(),
        }
    }
}
impl Advanced {
    pub fn validate(&self) -> Result<(), String> {
        if !(150..=2000).contains(&self.hold_ms) || !(100..=600).contains(&self.double_ms) {
            return Err("Hold delay must be 150–2000 ms; double-click window 100–600 ms.".into());
        }
        if !self.precision_speed.is_finite()
            || !(-1.0..=1.0).contains(&self.precision_speed)
            || !self.gesture_threshold.is_finite()
            || !(10.0..=500.0).contains(&self.gesture_threshold)
        {
            return Err("Precision speed must be −1 to 1; gesture threshold 10–500 pixels.".into());
        }
        let mut used = std::collections::HashSet::new();
        for button in [
            self.precision_button,
            self.horizontal_button,
            self.gesture_button,
        ]
        .into_iter()
        .flatten()
        {
            if button >= 6 || !used.insert(button) {
                return Err(
                    "Choose different buttons for precision, horizontal scrolling and gestures."
                        .into(),
                );
            }
            if custom(&self.hold[button]) || custom(&self.double[button]) {
                return Err(
                    "Special modes cannot share a button with Hold or Double-click actions.".into(),
                );
            }
        }
        for binding in self.hold.iter().chain(&self.double).chain(&self.gestures) {
            actions::dispatcher(binding)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MacroStep {
    Shortcut(String),
    Delay(u32),
}

pub fn validate_macro(steps: &[MacroStep]) -> Result<(), String> {
    if steps.is_empty() || steps.len() > 64 {
        return Err("A macro needs 1–64 steps.".into());
    }
    let mut duration = 0;
    for step in steps {
        match step {
            MacroStep::Shortcut(value) => {
                actions::shortcut(value)?;
            }
            MacroStep::Delay(ms) => {
                if *ms > 10_000 {
                    return Err("Each delay must be at most 10 seconds.".into());
                }
                duration += ms;
            }
        }
    }
    if duration > 60_000 {
        return Err("Macro delays must total at most 60 seconds.".into());
    }
    Ok(())
}
pub fn parse_macro(value: &str) -> Result<Vec<MacroStep>, String> {
    let steps: Vec<MacroStep> =
        serde_json::from_str(value).map_err(|_| "Invalid macro sequence.".to_string())?;
    validate_macro(&steps)?;
    Ok(steps)
}
pub fn compose_macro(text: &str) -> Result<String, String> {
    let steps = text
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|line| {
            if let Some(ms) = line.strip_prefix("wait ") {
                ms.parse()
                    .map(MacroStep::Delay)
                    .map_err(|_| "Use wait followed by milliseconds.".to_string())
            } else {
                Ok(MacroStep::Shortcut(line.into()))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_macro(&steps)?;
    serde_json::to_string(&steps).map_err(|e| e.to_string())
}
pub fn macro_text(binding: &Binding) -> String {
    parse_macro(&binding.value)
        .unwrap_or_default()
        .iter()
        .map(|step| match step {
            MacroStep::Shortcut(s) => s.clone(),
            MacroStep::Delay(ms) => format!("wait {ms}"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}
pub fn reset_profile(profile: &mut Profile) {
    let original = crate::model::Settings::default().profiles.remove(0);
    let id = profile.id;
    let name = profile.name.clone();
    *profile = Profile {
        id,
        name,
        ..original
    };
}
pub fn direction(dx: f32, dy: f32, threshold: f32) -> Option<usize> {
    if dx.abs().max(dy.abs()) < threshold {
        None
    } else if dx.abs() > dy.abs() {
        Some(if dx < 0.0 { 2 } else { 3 })
    } else {
        Some(if dy < 0.0 { 0 } else { 1 })
    }
}
pub fn custom(binding: &Binding) -> bool {
    binding.kind != ActionKind::Default
}
