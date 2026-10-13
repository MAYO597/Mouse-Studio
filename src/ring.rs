//! Actions Ring: per-profile configuration, validation, pure geometry/selection logic and the
//! command/placement strings used by the separate overlay process (`mouse-studio --ring`).
//! Nothing here touches the compositor except the small `Hyprland` helpers at the bottom.
use crate::{
    device::MouseModel,
    model::{ActionKind, Binding, Profile, Settings},
};
use serde::{Deserialize, Serialize};
use std::f32::consts::TAU;

pub const MAX_SLICES: usize = 8;
pub const MIN_SLICES: usize = 2;
pub const MAX_LABEL_CHARS: usize = 24;
/// Wayland app id / X11 class of the overlay window; used by the placement rules.
pub const WINDOW_CLASS: &str = "mouse-studio-ring";
/// Transparent margin around the disc so anti-aliased edges are not clipped.
pub const WINDOW_MARGIN: f32 = 16.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Slice {
    pub label: String,
    pub action: Binding,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ring {
    /// Slice 0 is at twelve o'clock; the rest follow clockwise.
    pub slices: Vec<Slice>,
    /// Outer radius of the disc in logical pixels.
    pub radius: f32,
    /// Radius of the neutral centre in which nothing is selected.
    pub dead_zone: f32,
    /// When the opening button is still held, releasing it over a slice chooses that slice.
    pub release_to_select: bool,
}

impl Default for Ring {
    fn default() -> Self {
        Self {
            slices: Vec::new(),
            radius: 130.0,
            dead_zone: 36.0,
            release_to_select: true,
        }
    }
}

fn slice(label: &str, kind: ActionKind, value: &str) -> Slice {
    Slice {
        label: label.into(),
        action: Binding {
            kind,
            value: value.into(),
        },
    }
}

impl Ring {
    /// A harmless editing starting point made only of keyboard shortcuts.
    pub fn starter() -> Self {
        Self {
            slices: vec![
                slice("Copy", ActionKind::Shortcut, "CTRL+C"),
                slice("Paste", ActionKind::Shortcut, "CTRL+V"),
                slice("Undo", ActionKind::Shortcut, "CTRL+Z"),
                slice("Redo", ActionKind::Shortcut, "CTRL+SHIFT+Z"),
                slice("Save", ActionKind::Shortcut, "CTRL+S"),
                slice("Workspace 1", ActionKind::Workspace, "1"),
                slice("Workspace 2", ActionKind::Workspace, "2"),
                slice("Screenshot", ActionKind::Shortcut, "Print"),
            ],
            ..Self::default()
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.slices.len() > MAX_SLICES {
            return Err(format!(
                "An Actions Ring holds at most {MAX_SLICES} slices."
            ));
        }
        if !self.radius.is_finite()
            || !(90.0..=220.0).contains(&self.radius)
            || !self.dead_zone.is_finite()
            || !(10.0..=80.0).contains(&self.dead_zone)
            || self.dead_zone > self.radius * 0.6
        {
            return Err(
                "Ring radius must be 90–220 px; centre 10–80 px and well inside it.".into(),
            );
        }
        for (i, slice) in self.slices.iter().enumerate() {
            validate_slice(slice).map_err(|e| format!("Ring slice {}: {e}", i + 1))?;
        }
        Ok(())
    }

    /// Side length of the square overlay window.
    pub fn window_size(&self) -> f32 {
        (self.radius + WINDOW_MARGIN) * 2.0
    }
}

pub fn validate_slice(slice: &Slice) -> Result<(), String> {
    let label = slice.label.trim();
    if label.is_empty()
        || label.chars().count() > MAX_LABEL_CHARS
        || label.chars().any(char::is_control)
    {
        return Err(format!(
            "give it a label of 1–{MAX_LABEL_CHARS} characters without control characters."
        ));
    }
    let action = &slice.action;
    if matches!(action.kind, ActionKind::Default | ActionKind::ActionsRing) {
        return Err("choose a real action (a ring cannot open itself).".into());
    }
    if action.kind != ActionKind::CycleDpi && action.value.trim().is_empty() {
        return Err("complete the action.".into());
    }
    if action.value.len() > 4096 || action.value.contains('\0') {
        return Err("the action is too long or contains a null character.".into());
    }
    if action.kind == ActionKind::Workspace
        && !action
            .value
            .trim()
            .parse::<u32>()
            .is_ok_and(|n| (1..=99).contains(&n))
    {
        return Err("workspace must be a number from 1 to 99.".into());
    }
    crate::actions::dispatcher(action).map(|_| ())
}

impl Profile {
    /// Whether any trigger of this profile opens the ring.
    pub fn uses_ring(&self) -> bool {
        self.bindings
            .iter()
            .chain(self.advanced.hold.iter())
            .chain(self.advanced.double.iter())
            .chain(self.advanced.gestures.iter())
            .any(|b| b.kind == ActionKind::ActionsRing)
    }
}

impl Settings {
    /// The ring runs from the installed helper binary, like horizontal scrolling.
    pub fn ring_required(&self) -> bool {
        self.profiles.iter().any(Profile::uses_ring)
    }
}

pub fn validate_profile(profile: &Profile) -> Result<(), String> {
    profile.advanced.ring.validate()?;
    if profile.uses_ring() && profile.advanced.ring.slices.len() < MIN_SLICES {
        return Err(format!(
            "‘{}’ opens the Actions Ring; add at least {MIN_SLICES} ring slices.",
            profile.name
        ));
    }
    Ok(())
}

/// Result of locating the pointer relative to the ring centre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Center,
    Slice(usize),
    Outside,
}

/// Angle of slice `i` centre measured clockwise from twelve o'clock, in radians.
pub fn slice_angle(i: usize, n: usize) -> f32 {
    i as f32 * TAU / n as f32
}

/// Offset from the ring centre of the middle of slice `i` at distance `r` (screen y points down).
pub fn slice_offset(i: usize, n: usize, r: f32) -> (f32, f32) {
    let a = slice_angle(i, n);
    (r * a.sin(), -r * a.cos())
}

/// `dx`/`dy` are pointer offsets from the centre in screen coordinates (y down).
pub fn hit(dx: f32, dy: f32, slices: usize, dead_zone: f32, radius: f32) -> Hit {
    if slices == 0 || !dx.is_finite() || !dy.is_finite() {
        return Hit::Outside;
    }
    let distance = dx.hypot(dy);
    if distance < dead_zone {
        return Hit::Center;
    }
    if distance > radius {
        return Hit::Outside;
    }
    let step = TAU / slices as f32;
    let angle = dx.atan2(-dy).rem_euclid(TAU);
    Hit::Slice(((angle + step / 2.0) / step) as usize % slices)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Pending,
    Choose(usize),
    Dismiss,
}

/// Selection state machine. An action is only ever produced by `Outcome::Choose`.
#[derive(Debug)]
pub struct Session {
    release_to_select: bool,
    seen_pressed: bool,
}

impl Session {
    pub fn new(release_to_select: bool) -> Self {
        Self {
            release_to_select,
            seen_pressed: false,
        }
    }
    /// Feed the state of the opening button(s). Only a release observed after the button was
    /// seen held, over a slice, chooses it; releasing in the centre switches to click mode.
    pub fn poll(&mut self, pressed: bool, hit: Option<Hit>) -> Outcome {
        if pressed {
            self.seen_pressed = true;
            return Outcome::Pending;
        }
        if !std::mem::take(&mut self.seen_pressed) || !self.release_to_select {
            return Outcome::Pending;
        }
        match hit {
            Some(Hit::Slice(i)) => Outcome::Choose(i),
            _ => Outcome::Pending,
        }
    }
    /// A mouse button pressed inside the overlay.
    pub fn click(&self, hit: Option<Hit>) -> Outcome {
        match hit {
            Some(Hit::Slice(i)) => Outcome::Choose(i),
            Some(Hit::Outside) => Outcome::Dismiss,
            _ => Outcome::Pending,
        }
    }
}

// ---- command line and compositor strings -------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
pub struct Args {
    pub profile_id: u64,
    pub model: MouseModel,
}

/// Parses everything after `--ring`: `<profile-id> <device-slug>`.
pub fn parse_args(args: &[String]) -> Result<Args, String> {
    match args {
        [id, device] => Ok(Args {
            profile_id: id
                .parse()
                .map_err(|_| "Usage: mouse-studio --ring <profile-id> <device>".to_string())?,
            model: MouseModel::parse(device)?,
        }),
        _ => Err("Usage: mouse-studio --ring <profile-id> <device>".into()),
    }
}

/// Shell command the compositor runs to open the ring (`helper` is already shell-quoted).
pub fn launch_command(helper: &str, profile_id: u64, model: MouseModel) -> String {
    format!("{helper} --ring {profile_id} {}", model.slug())
}

/// Parses `hyprctl cursorpos` (`123, 456`) or its JSON form.
pub fn parse_cursor(text: &str) -> Option<(f32, f32)> {
    let t = text.trim();
    if t.starts_with('{') {
        let v: serde_json::Value = serde_json::from_str(t).ok()?;
        return Some((v["x"].as_f64()? as f32, v["y"].as_f64()? as f32));
    }
    let (x, y) = t.split_once(',')?;
    let (x, y) = (x.trim().parse::<f32>().ok()?, y.trim().parse::<f32>().ok()?);
    (x.is_finite() && y.is_finite()).then_some((x, y))
}

/// Lua that floats, sizes, centres and pins the overlay. Every step is `pcall`ed and the
/// result is the list of failed steps, so a compositor API difference cannot raise an error.
pub fn placement_script(size: f32, cursor: Option<(f32, f32)>) -> String {
    let size = size.round() as i32;
    let centre = match cursor {
        Some((x, y)) => format!("{{x={x:.0},y={y:.0}}}"),
        None => "hl.get_cursor_pos()".into(),
    };
    format!(
        r#"(function()
local errs={{}}
local sel="class:^({WINDOW_CLASS})$"
local c={centre}
local function try(name,f) local ok,e=pcall(f); if not ok then errs[#errs+1]=name..": "..tostring(e) end end
try("float",function() hl.dispatch(hl.dsp.window.float({{action="enable",window=sel}})) end)
try("resize",function() hl.dispatch(hl.dsp.window.resize({{x={size},y={size},window=sel}})) end)
try("move",function() hl.dispatch(hl.dsp.window.move({{x=math.floor(c.x-{size}/2),y=math.floor(c.y-{size}/2),window=sel}})) end)
try("pin",function() hl.dispatch(hl.dsp.window.pin({{action="enable",window=sel}})) end)
return table.concat(errs,"; ")
end)()"#
    )
}

/// Lua that asks for the pointer-button state kept by the runtime (true press state, not the
/// 260 ms display flash). Replies `1` while any physical button 1–6 is held.
pub fn pressed_query(model: MouseModel) -> String {
    crate::runtime::scope(
        "(function() local s=__mouse_studio_v1; if not s or not s.pressed then return '0' end; for i=1,6 do if s.pressed[i] then return '1' end end; return '0' end)()",
        model,
    )
}

/// Loads the saved profile's ring for the overlay process. Refuses invalid or too-small rings.
pub fn load(path: &std::path::Path, args: &Args) -> Result<Ring, String> {
    let settings = Settings::load_from(path)?;
    if settings.device != args.model {
        return Err("The saved profiles belong to a different mouse.".into());
    }
    let profile = settings
        .profiles
        .iter()
        .find(|p| p.id == args.profile_id)
        .ok_or("The Actions Ring profile no longer exists.")?;
    let ring = profile.advanced.ring.clone();
    if ring.slices.len() < MIN_SLICES {
        return Err("The Actions Ring has fewer than two slices.".into());
    }
    Ok(ring)
}

/// Runs the chosen slice through the existing action dispatch (the same path as Test action).
pub fn execute(model: MouseModel, binding: &Binding) -> Result<(), String> {
    use crate::backend::DeviceBackend;
    if matches!(binding.kind, ActionKind::Default | ActionKind::ActionsRing) {
        return Err("This slice has no runnable action.".into());
    }
    crate::backend::Hyprland::default()
        .for_device(model)
        .test_action(binding)
}

/// Whether a physical button is held, from the runtime in the compositor.
pub fn buttons_held(model: MouseModel) -> Result<bool, String> {
    use crate::backend::{CommandRunner, Hyprland};
    let hypr = Hyprland::default().for_device(model);
    let session = hypr.session()?;
    let reply = hypr
        .runner
        .run(&["-i", &session, "repl", &pressed_query(model)])?;
    Ok(reply.trim() == "1")
}

/// Cursor position from the compositor, if available.
pub fn cursor(model: MouseModel) -> Option<(f32, f32)> {
    use crate::backend::{CommandRunner, Hyprland};
    let hypr = Hyprland::default().for_device(model);
    let session = hypr.session().ok()?;
    parse_cursor(&hypr.runner.run(&["-i", &session, "cursorpos"]).ok()?)
}

/// Applies the placement script; returns the diagnostics string (empty on success).
pub fn place(model: MouseModel, size: f32, cursor: Option<(f32, f32)>) -> Result<String, String> {
    use crate::backend::{CommandRunner, Hyprland};
    let hypr = Hyprland::default().for_device(model);
    let session = hypr.session()?;
    hypr.runner
        .run(&["-i", &session, "repl", &placement_script(size, cursor)])
}

/// True once the overlay window is mapped (listed by `hyprctl clients`).
pub fn window_mapped(model: MouseModel) -> bool {
    use crate::backend::{CommandRunner, Hyprland};
    let hypr = Hyprland::default().for_device(model);
    let Ok(session) = hypr.session() else {
        return false;
    };
    hypr.runner
        .run(&["-i", &session, "clients", "-j"])
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| {
            v.as_array()
                .map(|a| a.iter().any(|c| c["class"] == WINDOW_CLASS))
        })
        .unwrap_or(false)
}

/// Single-instance guard for the overlay process; `None` means another ring is already open.
pub fn acquire_lock() -> Option<std::fs::File> {
    use std::os::fd::AsRawFd;
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(std::env::temp_dir);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("mouse-studio-ring.lock"))
        .ok()?;
    // SAFETY: flock on a descriptor we own; the lock ends with the process.
    (unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0).then_some(file)
}
