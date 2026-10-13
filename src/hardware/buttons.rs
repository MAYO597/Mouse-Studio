//! Temporary diversion of the Master 4's three additional HID++ controls.
use super::Transport;
use crate::{
    backend::{Hyprctl, Hyprland},
    device::MouseModel,
    model::Settings,
};
use std::{
    fs::File,
    io::Read,
    os::fd::AsRawFd,
    path::PathBuf,
    time::{Duration, Instant},
};

pub const CONTROL_IDS: [u16; 3] = [0x00c4, 0x00c3, 0x01a0];

pub struct Controls {
    pub feature: u8,
    active: [bool; 3],
}
impl Controls {
    pub fn discover(wire: &mut impl Transport) -> Result<Self, String> {
        let root = wire.request(0, 0, &[0x1b, 4])?;
        let feature = *root
            .first()
            .filter(|v| **v != 0)
            .ok_or("Mouse does not support programmable HID++ controls.")?;
        let count = *wire
            .request(feature, 0, &[])?
            .first()
            .filter(|v| **v <= 64)
            .ok_or("Invalid programmable control count.")?;
        let mut found = [false; 3];
        for index in 0..count {
            let r = wire.request(feature, 0x10, &[index])?;
            if r.len() < 9 {
                return Err("Incomplete programmable control information.".into());
            }
            let cid = u16::from_be_bytes([r[0], r[1]]);
            if let Some(i) = CONTROL_IDS.iter().position(|v| *v == cid) {
                if found[i] || r[4] & 0x20 == 0 {
                    return Err("Extra mouse control is not uniquely divertable.".into());
                }
                found[i] = true;
            }
        }
        if found != [true; 3] {
            return Err("Mouse does not advertise all three MX Master 4 controls.".into());
        }
        Ok(Self {
            feature,
            active: [false; 3],
        })
    }
    fn flags(&self, wire: &mut impl Transport, cid: u16) -> Result<u8, String> {
        let r = wire.request(self.feature, 0x20, &cid.to_be_bytes())?;
        if r.len() < 5 || u16::from_be_bytes([r[0], r[1]]) != cid {
            return Err("Incomplete or mismatched button reporting response.".into());
        }
        Ok(r[2])
    }
    fn set(&self, wire: &mut impl Transport, cid: u16, enabled: bool) -> Result<(), String> {
        let [hi, lo] = cid.to_be_bytes();
        // Only change the transient diversion bit. Zero remap preserves its target;
        // all persistent, raw-motion and haptic flags remain untouched.
        wire.request(self.feature, 0x30, &[hi, lo, 2 | u8::from(enabled), 0, 0])?;
        if self.flags(wire, cid)? & 1 != u8::from(enabled) {
            return Err("Extra button diversion readback did not match.".into());
        }
        Ok(())
    }
    pub fn sync(&mut self, wire: &mut impl Transport, wanted: [bool; 3]) -> Result<(), String> {
        let previous = self.active;
        // Check every new capture before writing anything. Never steal an existing
        // Solaar/logiops diversion or turn off another program's captured button.
        for (i, &cid) in CONTROL_IDS.iter().enumerate() {
            if wanted[i] && !previous[i] && self.flags(wire, cid)? & 5 != 0 {
                return Err("This extra button is already diverted by another mouse utility. Release that assignment, then Save again.".into());
            }
        }
        let mut attempted = Vec::new();
        for (i, &cid) in CONTROL_IDS.iter().enumerate() {
            if wanted[i] == previous[i] {
                continue;
            }
            attempted.push(i);
            // A timed-out write might still have changed the device. Track it for
            // rollback and cleanup before waiting for the reply/readback.
            self.active[i] = wanted[i];
            if let Err(error) = self.set(wire, cid, wanted[i]) {
                let mut rollback_error = None;
                for &j in attempted.iter().rev() {
                    match self.set(wire, CONTROL_IDS[j], previous[j]) {
                        Ok(()) => self.active[j] = previous[j],
                        Err(e) => {
                            rollback_error = Some(e);
                            self.active[j] = true;
                        }
                    }
                }
                return Err(if let Some(e) = rollback_error {
                    format!("{error} Button cleanup failed: {e}")
                } else {
                    error
                });
            }
        }
        Ok(())
    }
    fn verify(&mut self, wire: &mut impl Transport, wanted: [bool; 3]) -> Result<bool, String> {
        let mut reset = false;
        for (i, &cid) in CONTROL_IDS.iter().enumerate() {
            if self.active[i] {
                let flags = self.flags(wire, cid)?;
                if flags & 4 != 0 {
                    return Err("Extra button ownership changed in another mouse utility.".into());
                }
                // Transient reporting can reset when a paired Bolt mouse wakes
                // or reconnects even though the receiver descriptor stays open.
                if flags & 1 == 0 {
                    self.active[i] = false;
                    reset = true;
                }
            }
        }
        self.sync(wire, wanted)?;
        Ok(reset)
    }
}

/// HID++ feature 0x2150 (THUMB_WHEEL). Function 0x10 reads `[diverted, flags]`, 0x20 writes
/// them, and function-0 notifications carry `i16 rotation, u16 timestamp, status, flags`
/// (Solaar `settings_templates.py` ThumbMode; logiops `ThumbWheel.cpp`).
pub struct ThumbWheel {
    pub feature: u8,
    active: bool,
}
impl ThumbWheel {
    pub fn discover(wire: &mut impl Transport) -> Result<Self, String> {
        let root = wire.request(0, 0, &[0x21, 0x50])?;
        let feature = *root
            .first()
            .filter(|v| **v != 0)
            .ok_or("Mouse does not expose the side wheel (HID++ 0x2150).")?;
        Ok(Self {
            feature,
            active: false,
        })
    }
    pub fn active(&self) -> bool {
        self.active
    }
    fn read(&self, wire: &mut impl Transport) -> Result<[u8; 2], String> {
        let r = wire.request(self.feature, 0x10, &[])?;
        if r.len() < 2 {
            return Err("Incomplete side-wheel settings response.".into());
        }
        Ok([r[0], r[1]])
    }
    /// Changes only the diversion bit. Direction and unrelated bits keep their values.
    fn set(&self, wire: &mut impl Transport, enabled: bool) -> Result<(), String> {
        let old = self.read(wire)?;
        let new = [(old[0] & !1) | u8::from(enabled), old[1]];
        if new != old {
            wire.request(self.feature, 0x20, &new)?;
            if self.read(wire)? != new {
                return Err("Side-wheel diversion readback did not match.".into());
            }
        }
        Ok(())
    }
    pub fn sync(&mut self, wire: &mut impl Transport, wanted: bool) -> Result<(), String> {
        if wanted == self.active {
            return Ok(());
        }
        // A timed-out write might still have changed the device: track it for cleanup.
        self.active = wanted;
        if let Err(error) = self.set(wire, wanted) {
            // Never leave the wheel diverted without a consumer: undo a failed capture,
            // and keep ownership of a failed release so cleanup is retried.
            self.active = if wanted {
                self.set(wire, false).is_err()
            } else {
                true
            };
            return Err(error);
        }
        Ok(())
    }
    /// Returns true when the device lost diversion (reconnect or wake) and had to be redone.
    pub fn verify(&mut self, wire: &mut impl Transport, wanted: bool) -> Result<bool, String> {
        let mut reset = false;
        if self.active && self.read(wire)?[0] & 1 == 0 {
            self.active = false;
            reset = true;
        }
        self.sync(wire, wanted)?;
        Ok(reset)
    }
}

/// Turns rotation notifications into discrete left/right steps.
#[derive(Default)]
pub struct ThumbSteps {
    accumulated: i32,
}
impl ThumbSteps {
    pub fn reset(&mut self) {
        self.accumulated = 0;
    }
    /// `true` is right (positive firmware rotation). At most 16 steps per report.
    pub fn update(
        &mut self,
        report: &[u8],
        address: u8,
        feature: u8,
        active: bool,
        step: u16,
    ) -> Vec<bool> {
        if report.len() < 10
            || report[0] != 0x11
            || report[1] != address
            || report[2] != feature
            || report[3] != 0
        {
            return Vec::new();
        }
        let rotation = i32::from(i16::from_be_bytes([report[4], report[5]]));
        let status = report[8];
        if !active || step == 0 {
            self.reset();
            return Vec::new();
        }
        if status == 1
            || (self.accumulated != 0 && (self.accumulated > 0) != (rotation > 0) && rotation != 0)
        {
            // A new gesture or a direction change never inherits leftover travel.
            self.reset();
        }
        self.accumulated += rotation;
        let step = i32::from(step);
        let mut out = Vec::new();
        while self.accumulated.abs() >= step && out.len() < 16 {
            out.push(self.accumulated > 0);
            self.accumulated -= step * self.accumulated.signum();
        }
        if status == 3 || status == 0 || out.len() == 16 {
            self.reset();
        }
        out
    }
}

#[derive(Default)]
pub struct Edges {
    held: [bool; 3],
    blocked: [bool; 3],
}
impl Edges {
    fn block_held(&mut self) {
        for i in 0..3 {
            self.blocked[i] |= self.held[i];
            self.held[i] = false;
        }
    }
    pub fn update(
        &mut self,
        report: &[u8],
        address: u8,
        feature: u8,
        active: [bool; 3],
    ) -> Vec<(usize, bool)> {
        if report.len() < 12
            || report[0] != 0x11
            || report[1] != address
            || report[2] != feature
            || report[3] != 0
        {
            return Vec::new();
        }
        let mut edges = Vec::new();
        for (i, &cid) in CONTROL_IDS.iter().enumerate() {
            let physical_down = report[4..12]
                .as_chunks::<2>()
                .0
                .iter()
                .any(|b| u16::from_be_bytes([b[0], b[1]]) == cid);
            if !physical_down {
                self.blocked[i] = false;
            }
            let down = active[i] && physical_down && !self.blocked[i];
            if self.held[i] != down {
                self.held[i] = down;
                edges.push((i + 3, down));
            }
        }
        edges
    }
}

/// Button edges and side-wheel steps (`true` = right) read from the HID++ stream.
type Pumped = (Vec<(usize, bool)>, Vec<bool>);

struct Capture {
    reader: File,
    address: u8,
    identity: String,
    controls: Controls,
    edges: Edges,
    thumb: Option<ThumbWheel>,
    thumb_steps: ThumbSteps,
    verified: Instant,
    obsolete: bool,
}
impl Capture {
    fn open() -> Result<Self, String> {
        let mut wire = super::linux::Hidraw::open(MouseModel::MxMaster4)?;
        let reader = wire.event_reader()?;
        let controls = Controls::discover(&mut wire)?;
        Ok(Self {
            reader,
            address: wire.address,
            identity: wire.identity.clone(),
            controls,
            edges: Edges::default(),
            thumb: None,
            thumb_steps: ThumbSteps::default(),
            verified: Instant::now(),
            obsolete: false,
        })
    }
    fn thumb_active(&self) -> bool {
        self.thumb.as_ref().is_some_and(ThumbWheel::active)
    }
    fn sync(&mut self, wanted: [bool; 3], thumb: bool) -> Result<bool, String> {
        if wanted == self.controls.active
            && thumb == self.thumb_active()
            && self.verified.elapsed() < Duration::from_secs(5)
        {
            return Ok(false);
        }
        let mut wire = super::linux::Hidraw::open(MouseModel::MxMaster4)?;
        if wire.identity != self.identity || wire.address != self.address {
            self.obsolete = true;
            return Err("Mouse connection changed while configuring extra buttons.".into());
        }
        if thumb && self.thumb.is_none() {
            self.thumb = Some(ThumbWheel::discover(&mut wire)?);
        }
        let mut reset = self.controls.verify(&mut wire, wanted)?;
        if let Some(t) = &mut self.thumb {
            reset |= t.verify(&mut wire, thumb)?;
        }
        if reset {
            self.thumb_steps.reset();
        }
        self.verified = Instant::now();
        Ok(reset)
    }
    fn poll(&self) -> Result<(), String> {
        let mut fd = libc::pollfd {
            fd: self.reader.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        if unsafe { libc::poll(&mut fd, 1, 0) } < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        if fd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
            return Err("Extra-button HID interface disconnected.".into());
        }
        Ok(())
    }
    fn pump(&mut self, step: u16) -> Result<Pumped, String> {
        self.poll()?;
        let mut result = Vec::new();
        let mut steps = Vec::new();
        let mut report = [0u8; 64];
        for _ in 0..256 {
            match self.reader.read(&mut report) {
                Ok(0) => return Err("Extra-button HID interface disconnected.".into()),
                Ok(n) => {
                    result.extend(self.edges.update(
                        &report[..n],
                        self.address,
                        self.controls.feature,
                        self.controls.active,
                    ));
                    if let Some(t) = &self.thumb {
                        steps.extend(self.thumb_steps.update(
                            &report[..n],
                            self.address,
                            t.feature,
                            t.active(),
                            step,
                        ));
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(format!("Extra-button input failed: {e}")),
            }
        }
        Ok((result, steps))
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        if !self.obsolete
            && (self.controls.active != [false; 3] || self.thumb_active())
            && self.poll().is_ok()
            && let Err(e) = self.sync([false; 3], false)
        {
            eprintln!("Mouse Studio button cleanup: {e}");
            super::desktop::report_action_for(&Err(e), MouseModel::MxMaster4);
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Ready {
    token: String,
    profile_id: u64,
    error: Option<String>,
    pid: u32,
    updated_ms: u64,
}
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
impl Ready {
    fn matches(&self, token: &str, profile_id: u64, now: u64) -> bool {
        self.token == token
            && self.profile_id == profile_id
            && now
                .checked_sub(self.updated_ms)
                .is_some_and(|age| age <= 2000)
            && self.pid > 0
            && self.pid <= i32::MAX as u32
            && unsafe { libc::kill(self.pid as libc::pid_t, 0) } == 0
    }
}
fn ready_path() -> Result<PathBuf, String> {
    let root = std::env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR is unavailable.")?;
    let root = PathBuf::from(root);
    if !root.is_absolute() {
        return Err("XDG_RUNTIME_DIR must be absolute.".into());
    }
    Ok(root.join("mouse-studio-mx-master-4-buttons.json"))
}
pub fn wait_ready(settings: &Settings, profile_id: u64) -> Result<(), String> {
    let path = ready_path()?;
    let token = crate::runtime::setup_token(settings);
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        if let Ok(bytes) = std::fs::read(&path)
            && let Ok(ready) = serde_json::from_slice::<Ready>(&bytes)
            && ready.matches(&token, profile_id, now_ms())
        {
            return ready.error.map_or(Ok(()), Err);
        }
        if Instant::now() >= deadline {
            return Err("Extra-button helper did not become ready. Check the MX Master 4 restore service and mouse hardware access, then Save again.".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[derive(Default)]
pub struct Service {
    capture: Option<Capture>,
    current: Option<(Settings, u64, String)>,
    failed: bool,
}
impl Service {
    fn release(&mut self) -> Result<(), String> {
        if let Some(capture) = &mut self.capture {
            capture.sync([false; 3], false)?;
        }
        self.capture = None;
        Ok(())
    }
    fn backend() -> Hyprland {
        Hyprland::with_runner(Hyprctl, None, None).for_device(MouseModel::MxMaster4)
    }
    fn report(&self, error: Option<String>) -> Result<(), String> {
        if let Some((settings, id, _)) = &self.current {
            let ready = Ready {
                token: crate::runtime::setup_token(settings),
                profile_id: *id,
                error,
                pid: std::process::id(),
                updated_ms: now_ms(),
            };
            crate::model::write_settings_file(
                &ready_path()?,
                &serde_json::to_vec(&ready).map_err(|e| e.to_string())?,
            )?;
        }
        Ok(())
    }
    pub fn refresh(&mut self, settings: Settings) -> Result<(), String> {
        if settings.device != MouseModel::MxMaster4 {
            return Err("Extra-button settings belong to another mouse.".into());
        }
        let (session, id) = Self::backend().button_profile(&settings)?;
        let Some(id) = id else {
            self.release()?;
            self.current = None;
            let _ = std::fs::remove_file(ready_path()?);
            return Ok(());
        };
        let profile = settings
            .profiles
            .iter()
            .find(|p| p.id == id)
            .ok_or("Extra-button profile is missing.")?;
        let wanted = std::array::from_fn(|i| profile.button_managed(i + 3));
        let thumb = profile.hardware.thumbwheel_remap.is_some();
        let changed = self
            .current
            .as_ref()
            .is_none_or(|(old, old_id, old_session)| {
                old != &settings || *old_id != id || old_session != &session
            })
            || self.failed;
        self.current = Some((settings, id, session));
        if wanted == [false; 3] && !thumb {
            self.release()?;
        } else {
            if self.capture.is_none() {
                self.capture = Some(Capture::open()?);
            }
            let capture = self.capture.as_mut().expect("opened capture");
            if changed {
                // Drain old-profile events before changing ownership. They must
                // never execute the new profile's bindings.
                capture.pump(1)?;
                capture.edges.block_held();
                capture.thumb_steps.reset();
            }
            if capture.sync(wanted, thumb)? {
                // A receiver stays open across mouse sleep/reconnect. Firmware
                // reset also invalidates held state in the compositor.
                capture.edges = Edges::default();
                crate::backend::DeviceBackend::cancel_actions(&Self::backend())?;
            }
        }
        self.failed = false;
        self.report(None)
    }
    pub fn pump(&mut self) -> Result<(), String> {
        if self.failed {
            return Ok(());
        }
        if let Some(capture) = &mut self.capture {
            let step = self
                .current
                .as_ref()
                .and_then(|(settings, id, _)| settings.profiles.iter().find(|p| p.id == *id))
                .and_then(|p| p.hardware.thumbwheel_remap.as_ref())
                .map_or(1, |r| r.step);
            let (edges, steps) = capture.pump(step)?;
            if let Some((settings, id, session)) = &self.current {
                let profile = settings
                    .profiles
                    .iter()
                    .find(|p| p.id == *id)
                    .ok_or("Extra-button profile is missing.")?;
                let backend = Self::backend();
                backend.button_events(session, settings, profile, &edges)?;
                backend.thumb_events(session, settings, profile, &steps)?;
            }
        }
        Ok(())
    }
    pub fn fail(&mut self, error: &str) {
        self.failed = true;
        let release = self.release();
        // Preserve ownership after a temporary timeout so retries can clear our
        // own diversion. A dead descriptor belongs to a disconnected interface;
        // abandon it and verify a fresh connection on the next attempt.
        if let Some(capture) = &mut self.capture {
            capture.edges.block_held();
            capture.thumb_steps.reset();
            if capture.obsolete || capture.poll().is_err() {
                self.capture = None;
            }
        }
        // Reset pending holds, gestures and precision state on a failed stream.
        let _ = crate::backend::DeviceBackend::cancel_actions(&Self::backend());
        let error = release.err().map_or_else(
            || error.to_string(),
            |e| format!("{error} Button cleanup will retry: {e}"),
        );
        let _ = self.report(Some(error.clone()));
        super::desktop::report_action_for(&Err(error), MouseModel::MxMaster4);
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.capture = None;
        if let Ok(path) = ready_path() {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Wire {
        flags: [u8; 3],
        changes: Vec<(u16, u8)>,
        fail: Option<u16>,
        readback_wrong: bool,
    }
    impl Transport for Wire {
        fn request(&mut self, f: u8, n: u8, p: &[u8]) -> Result<Vec<u8>, String> {
            if f == 0 {
                assert_eq!(p, [0x1b, 4]);
                return Ok(vec![13, 0, 6]);
            }
            assert_eq!(f, 13);
            match n {
                0 => Ok(vec![6]),
                0x10 => {
                    let cid: u16 = [0x0052, 0x0053, 0x0056, 0x00c3, 0x00c4, 0x01a0][p[0] as usize];
                    let mut r = cid.to_be_bytes().to_vec();
                    r.extend_from_slice(&[0, 0, 0x20, 0, 0, 0, 1]);
                    Ok(r)
                }
                0x20 => {
                    let cid = u16::from_be_bytes([p[0], p[1]]);
                    let i = CONTROL_IDS.iter().position(|v| *v == cid).unwrap();
                    Ok(vec![
                        p[0],
                        p[1],
                        if self.readback_wrong && !self.changes.is_empty() {
                            0
                        } else {
                            self.flags[i]
                        },
                        0,
                        0,
                        0,
                    ])
                }
                0x30 => {
                    assert_eq!(p.len(), 5);
                    assert_eq!(&p[3..], [0, 0], "remapping must be preserved");
                    let cid = u16::from_be_bytes([p[0], p[1]]);
                    self.changes.push((cid, p[2]));
                    if self.fail == Some(cid) {
                        self.fail = None;
                        return Err("write failure".into());
                    }
                    let i = CONTROL_IDS.iter().position(|v| *v == cid).unwrap();
                    self.flags[i] = (self.flags[i] & !1) | (p[2] & 1);
                    Ok(p.to_vec())
                }
                _ => panic!("unexpected request"),
            }
        }
    }
    fn wire() -> Wire {
        Wire {
            flags: [0; 3],
            changes: Vec::new(),
            fail: None,
            readback_wrong: false,
        }
    }

    #[test]
    fn only_assigned_controls_are_diverted_and_native_defaults_are_restored() {
        let mut w = wire();
        let mut c = Controls::discover(&mut w).unwrap();
        c.sync(&mut w, [true, false, true]).unwrap();
        assert_eq!(w.flags, [1, 0, 1]);
        assert_eq!(w.changes, [(0xc4, 3), (0x1a0, 3)]);
        c.sync(&mut w, [false, true, false]).unwrap();
        assert_eq!(w.flags, [0, 1, 0]);
        c.sync(&mut w, [false; 3]).unwrap();
        assert_eq!(w.flags, [0; 3]);
    }
    #[test]
    fn existing_diversion_is_preserved_and_partial_failures_are_rolled_back() {
        let mut w = wire();
        let mut c = Controls::discover(&mut w).unwrap();
        w.flags[2] = 5;
        assert!(c.sync(&mut w, [false, false, true]).is_err());
        assert!(w.changes.is_empty());
        w.flags[2] = 0;
        w.fail = Some(0x1a0);
        assert!(c.sync(&mut w, [true, false, true]).is_err());
        assert_eq!(w.flags, [0; 3]);
        assert_eq!(c.active, [false; 3]);
    }
    #[test]
    fn failed_release_keeps_ownership_for_a_later_cleanup_attempt() {
        let mut w = wire();
        let mut c = Controls::discover(&mut w).unwrap();
        c.sync(&mut w, [false, false, true]).unwrap();
        w.fail = Some(0x1a0);
        assert!(c.sync(&mut w, [false; 3]).is_err());
        assert_eq!(c.active, [false, false, true]);
        assert_eq!(w.flags, [0, 0, 1]);
        c.sync(&mut w, [false; 3]).unwrap();
        assert_eq!(w.flags, [0; 3]);
    }
    #[test]
    fn diversion_readback_is_verified() {
        let mut w = wire();
        let mut c = Controls::discover(&mut w).unwrap();
        w.readback_wrong = true;
        assert!(c.sync(&mut w, [true, false, false]).is_err());
        assert_eq!(w.flags, [0; 3]);
    }
    #[test]
    fn receiver_reconnect_restores_reporting_even_when_its_descriptor_stays_open() {
        let mut w = wire();
        let mut c = Controls::discover(&mut w).unwrap();
        c.sync(&mut w, [false, true, true]).unwrap();
        w.flags = [0; 3];
        assert!(
            c.verify(&mut w, [false, true, true]).unwrap(),
            "runtime held state must be reset on reconnect"
        );
        assert_eq!(w.flags, [0, 1, 1]);
        c.sync(&mut w, [false; 3]).unwrap();
        assert_eq!(w.flags, [0; 3]);
    }
    #[test]
    fn readiness_requires_fresh_matching_settings_and_a_live_helper() {
        let mut ready = Ready {
            token: "saved-setup".into(),
            profile_id: 2,
            error: None,
            pid: std::process::id(),
            updated_ms: 10_000,
        };
        assert!(ready.matches("saved-setup", 2, 10_500));
        assert!(!ready.matches("old-setup", 2, 10_500));
        assert!(!ready.matches("saved-setup", 1, 10_500));
        assert!(!ready.matches("saved-setup", 2, 13_000));
        assert!(!ready.matches("saved-setup", 2, 9_000));
        ready.pid = 0;
        assert!(!ready.matches("saved-setup", 2, 10_500));
    }
    #[test]
    fn notifications_filter_receiver_slot_and_report_simultaneous_press_release_edges() {
        let mut e = Edges::default();
        let down = [0x11, 2, 13, 0, 0, 0xc3, 1, 0xa0, 0, 0, 0, 0];
        assert!(e.update(&down, 1, 13, [true; 3]).is_empty());
        assert!(e.update(&down, 2, 14, [true; 3]).is_empty());
        assert_eq!(e.update(&down, 2, 13, [true; 3]), [(4, true), (5, true)]);
        assert!(e.update(&down, 2, 13, [true; 3]).is_empty());
        let reply = [0x11, 2, 13, 0x38, 0, 0, 0, 0, 0, 0, 0, 0];
        assert!(e.update(&reply, 2, 13, [true; 3]).is_empty());
        assert!(e.update(&[0x11, 2, 13, 0, 0], 2, 13, [true; 3]).is_empty());
        let up = [0x11, 2, 13, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(e.update(&up, 2, 13, [true; 3]), [(4, false), (5, false)]);
        assert!(e.update(&down, 2, 13, [false; 3]).is_empty());
    }
    #[test]
    fn switching_profiles_requires_release_before_a_held_button_can_fire_again() {
        let mut e = Edges::default();
        let panel_down = [0x11, 2, 13, 0, 1, 0xa0, 0, 0, 0, 0, 0, 0];
        assert_eq!(e.update(&panel_down, 2, 13, [true; 3]), [(5, true)]);
        e.block_held();
        let both_down = [0x11, 2, 13, 0, 1, 0xa0, 0, 0xc3, 0, 0, 0, 0];
        assert_eq!(e.update(&both_down, 2, 13, [true; 3]), [(4, true)]);
        let released = [0x11, 2, 13, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(e.update(&released, 2, 13, [true; 3]), [(4, false)]);
        assert_eq!(e.update(&panel_down, 2, 13, [true; 3]), [(5, true)]);
    }
}
