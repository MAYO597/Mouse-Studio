use super::{HapticState, HardwareSettings, Snapshot, WheelMode};
pub trait Transport {
    fn request(&mut self, feature: u8, function: u8, params: &[u8]) -> Result<Vec<u8>, String>;
}
/// Solaar `HapticWaveForms`: SUBTLE_COLLISION, SHARP_STATE_CHANGE, SHARP_COLLISION, HAPPY_ALERT.
const HAPTIC_TEST_WAVEFORMS: [u8; 4] = [0x04, 0x00, 0x02, 0x05];
pub struct Client<T> {
    wire: T,
}
#[derive(Clone, Copy)]
struct Wheel {
    index: u8,
    enhanced: bool,
}
impl<T: Transport> Client<T> {
    pub fn new(wire: T) -> Self {
        Self { wire }
    }
    fn feature(&mut self, id: u16) -> Result<Option<u8>, String> {
        let r = self.wire.request(0, 0, &id.to_be_bytes())?;
        let index = *r.first().ok_or("Incomplete HID++ feature response.")?;
        Ok((index != 0).then_some(index))
    }
    fn dpi_feature(&mut self) -> Result<Option<u8>, String> {
        self.feature(0x2201)
    }
    fn wheel_feature(&mut self) -> Result<Option<Wheel>, String> {
        if let Some(index) = self.feature(0x2110)? {
            return Ok(Some(Wheel {
                index,
                enhanced: false,
            }));
        }
        Ok(self.feature(0x2111)?.map(|index| Wheel {
            index,
            enhanced: true,
        }))
    }
    fn dpi_values(&mut self, index: u8) -> Result<Vec<u16>, String> {
        let mut bytes = Vec::new();
        for page in 0..16 {
            let r = self.wire.request(index, 0x10, &[0, 0, page])?;
            if r.len() < 3 || r[0] != 0 {
                return Err("Incomplete DPI choices response.".into());
            }
            bytes.extend_from_slice(&r[1..]);
            if bytes.ends_with(&[0, 0]) {
                return decode_dpi_list(&bytes);
            }
        }
        Err("Mouse DPI choices did not terminate.".into())
    }
    fn dpi_read(&mut self, index: u8) -> Result<u16, String> {
        let r = self.wire.request(index, 0x20, &[0])?;
        if r.len() < 5 || r[0] != 0 {
            return Err("Incomplete sensor DPI response.".into());
        }
        let d = u16::from_be_bytes([r[1], r[2]]);
        let d = if d == 0 {
            u16::from_be_bytes([r[3], r[4]])
        } else {
            d
        };
        if d == 0 {
            return Err("Mouse reported zero sensor DPI.".into());
        }
        Ok(d)
    }
    fn dpi_write(&mut self, index: u8, dpi: u16) -> Result<(), String> {
        self.wire
            .request(index, 0x30, &[0, (dpi >> 8) as u8, dpi as u8])?;
        if self.dpi_read(index)? != dpi {
            return Err("Sensor DPI readback did not match.".into());
        }
        Ok(())
    }
    fn wheel_read(&mut self, w: Wheel) -> Result<(WheelMode, u8), String> {
        let r = self
            .wire
            .request(w.index, if w.enhanced { 0x10 } else { 0 }, &[])?;
        if r.len() < 2 {
            return Err("Incomplete SmartShift response.".into());
        }
        Ok((WheelMode::from_byte(r[0])?, r[1]))
    }
    fn wheel_write(&mut self, w: Wheel, mode: WheelMode, threshold: u8) -> Result<(), String> {
        let data = [mode.byte(), threshold, 0];
        self.wire.request(
            w.index,
            if w.enhanced { 0x20 } else { 0x10 },
            &data[..if w.enhanced { 3 } else { 2 }],
        )?;
        if self.wheel_read(w)? != (mode, threshold) {
            return Err("Wheel/SmartShift readback did not match.".into());
        }
        Ok(())
    }
    fn thumbwheel_read(&mut self, index: u8) -> Result<[u8; 2], String> {
        let r = self.wire.request(index, 0x10, &[])?;
        if r.len() < 2 {
            return Err("Incomplete side-wheel settings response.".into());
        }
        Ok([r[0], r[1]])
    }
    fn thumbwheel_write(&mut self, index: u8, values: [u8; 2]) -> Result<(), String> {
        self.wire.request(index, 0x20, &values)?;
        if self.thumbwheel_read(index)? != values {
            return Err("Side-wheel settings readback did not match.".into());
        }
        Ok(())
    }
    fn haptic_read(&mut self, index: u8) -> Result<HapticState, String> {
        let r = self.wire.request(index, 0x10, &[])?;
        if r.len() < 3 {
            return Err("Incomplete haptic feedback response.".into());
        }
        Ok(HapticState {
            enabled: r[0] & 1 != 0,
            level: r[1],
            discrete_levels: r[2] & 1 != 0,
            waveforms: 0,
        })
    }
    /// Solaar writes `00 32` to disable (keeping 50 %) and `01 <level>` to enable.
    fn haptic_write(&mut self, index: u8, enabled: bool, level: u8) -> Result<(), String> {
        self.wire
            .request(index, 0x20, &[u8::from(enabled), level])?;
        let now = self.haptic_read(index)?;
        // A disabled device may report any retained strength; only enablement matters then.
        if now.enabled != enabled || (enabled && now.level != level) {
            return Err("Haptic feedback readback did not match.".into());
        }
        Ok(())
    }
    /// Read-only. `None` means the mouse does not advertise HID++ feature 0x19B0.
    pub fn read_haptics(&mut self) -> Result<Option<HapticState>, String> {
        let Some(index) = self.feature(0x19b0)? else {
            return Ok(None);
        };
        let mut state = self.haptic_read(index)?;
        let caps = self.wire.request(index, 0, &[])?;
        if caps.len() < 8 {
            return Err("Incomplete haptic capabilities response.".into());
        }
        state.waveforms = u32::from_be_bytes([caps[4], caps[5], caps[6], caps[7]]);
        Ok(Some(state))
    }
    /// Plays one effect that the mouse itself advertises. Never writes settings.
    pub fn play_haptic(&mut self) -> Result<(), String> {
        let index = self
            .feature(0x19b0)?
            .ok_or("Mouse does not expose haptic feedback.")?;
        let state = self
            .read_haptics()?
            .ok_or("Mouse does not expose haptic feedback.")?;
        if !state.enabled {
            return Err(
                "Haptic feedback is turned off on the mouse. Save with it enabled, then test."
                    .into(),
            );
        }
        let wave = HAPTIC_TEST_WAVEFORMS
            .into_iter()
            .find(|w| state.waveforms & (1 << w) != 0)
            .ok_or("The mouse reports no playable haptic effects.")?;
        self.wire.request(index, 0x40, &[wave])?;
        Ok(())
    }
    pub fn battery(&mut self) -> Result<Option<super::Battery>, String> {
        let Some(index) = self.feature(0x1004)? else {
            return Ok(None);
        };
        let data = self.wire.request(index, 0x10, &[])?;
        if data.len() < 3 || data[0] > 100 {
            return Err("Invalid battery response.".into());
        }
        Ok(Some(super::Battery {
            percent: Some(data[0]),
            status: match data[2] {
                0 => "Discharging",
                1 => "Charging",
                2 => "Almost full",
                3 => "Full",
                4 => "Slow charging",
                _ => "Unknown",
            }
            .into(),
        }))
    }
    pub fn read(&mut self) -> Result<Snapshot, String> {
        let mut s = Snapshot {
            available: true,
            ..Default::default()
        };
        if let Some(i) = self.dpi_feature()? {
            s.dpi_values = self.dpi_values(i)?;
            s.dpi = Some(self.dpi_read(i)?);
        }
        if let Some(w) = self.wheel_feature()? {
            let (mode, t) = self.wheel_read(w)?;
            s.wheel_mode = Some(mode);
            s.smartshift = Some(t.min(50));
        }
        if let Some(i) = self.feature(0x2150)? {
            let [mode, invert] = self.thumbwheel_read(i)?;
            s.thumbwheel = Some(super::ThumbWheelState {
                diverted: mode & 1 != 0,
                inverted: invert & 1 != 0,
            });
        }
        Ok(s)
    }
    pub fn cycle(&mut self, presets: &[u16]) -> Result<(), String> {
        let i = self
            .dpi_feature()?
            .ok_or("Mouse does not expose adjustable sensor DPI.")?;
        let choices = self.dpi_values(i)?;
        if presets.iter().any(|v| !choices.contains(v)) {
            return Err("A DPI preset is unsupported by this mouse.".into());
        }
        let previous = self.dpi_read(i)?;
        let next = super::next_dpi(presets, previous)?;
        if let Err(e) = self.dpi_write(i, next) {
            let rollback = self.dpi_write(i, previous);
            return Err(rollback_message(e, rollback));
        }
        Ok(())
    }
    pub fn apply(&mut self, desired: &HardwareSettings) -> Result<(), String> {
        desired.validate()?;
        // Capture all requested state and validate capabilities before any write.
        let dpi = if let Some(value) = desired.dpi {
            let i = self
                .dpi_feature()?
                .ok_or("Mouse does not expose adjustable sensor DPI.")?;
            let values = self.dpi_values(i)?;
            let previous = self.dpi_read(i)?;
            if !values.contains(&value) {
                return Err(format!("{value} DPI is unsupported by this mouse."));
            }
            Some((i, previous, value))
        } else {
            None
        };
        let wheel = if desired.wheel_mode.is_some() || desired.smartshift.is_some() {
            let w = self
                .wheel_feature()?
                .ok_or("Mouse does not expose SmartShift/wheel mode.")?;
            let (mode, threshold) = self.wheel_read(w)?;
            let newmode = desired.wheel_mode.unwrap_or(mode);
            let newthreshold = desired
                .smartshift
                .map_or(threshold, |t| if t == 50 { 255 } else { t });
            Some((w, mode, threshold, newmode, newthreshold))
        } else {
            None
        };
        let mut dpi_touched = false;
        let mut wheel_touched = false;
        // A remapped side wheel is diverted and owned by the button helper, which also
        // restores it; native restoration here would break or race with it.
        let thumbwheel = if desired.thumbwheel_remap.is_some() {
            None
        } else if let Some(inverted) = desired.thumbwheel_inverted {
            let i = self
                .feature(0x2150)?
                .ok_or("Mouse does not expose native side-wheel controls.")?;
            let old = self.thumbwheel_read(i)?;
            // Restore standard HID horizontal-wheel output. Preserve unrelated
            // bits and change only diversion and direction in the two bytes.
            let new = [old[0] & !1, (old[1] & !1) | u8::from(inverted)];
            Some((i, old, new))
        } else {
            None
        };
        let haptics = if let Some(h) = desired.haptics {
            let i = self
                .feature(0x19b0)?
                .ok_or("Mouse does not expose haptic feedback.")?;
            let old = self.haptic_read(i)?;
            if h.enabled
                && old.discrete_levels
                && !super::HAPTIC_LEVELS.iter().any(|(l, _)| *l == h.level)
            {
                return Err(
                    "This mouse supports only Low, Medium, High and Maximum haptic strength."
                        .into(),
                );
            }
            let keep = if (1..=100).contains(&old.level) {
                old.level
            } else {
                50
            };
            let new = if h.enabled {
                (true, h.level)
            } else {
                (false, keep)
            };
            Some((i, (old.enabled, old.level), new))
        } else {
            None
        };
        let mut haptics_touched = false;
        let mut thumbwheel_touched = false;
        let result = (|| {
            if let Some((i, old, new)) = dpi
                && old != new
            {
                dpi_touched = true;
                self.dpi_write(i, new)?;
            }
            if let Some((w, oldmode, oldt, newmode, newt)) = wheel
                && (oldmode, oldt) != (newmode, newt)
            {
                wheel_touched = true;
                self.wheel_write(w, newmode, newt)?;
            }
            if let Some((i, old, new)) = thumbwheel
                && old != new
            {
                thumbwheel_touched = true;
                self.thumbwheel_write(i, new)?;
            }
            if let Some((i, old, new)) = haptics
                && old != new
            {
                haptics_touched = true;
                self.haptic_write(i, new.0, new.1)?;
            }
            Ok(())
        })();
        if let Err(e) = result {
            let rollback = {
                // Try both restorations even if one fails.
                let thumb = if thumbwheel_touched {
                    let (i, old, _) = thumbwheel.unwrap();
                    self.thumbwheel_write(i, old)
                } else {
                    Ok(())
                };
                let w = if wheel_touched {
                    let (w, m, t, _, _) = wheel.unwrap();
                    self.wheel_write(w, m, t)
                } else {
                    Ok(())
                };
                let d = if dpi_touched {
                    let (i, old, _) = dpi.unwrap();
                    self.dpi_write(i, old)
                } else {
                    Ok(())
                };
                let h = if haptics_touched {
                    let (i, old, _) = haptics.unwrap();
                    self.haptic_write(i, old.0, old.1)
                } else {
                    Ok(())
                };
                h.and(thumb).and(w).and(d)
            };
            return Err(rollback_message(e, rollback));
        }
        Ok(())
    }
}
fn rollback_message(error: String, rollback: Result<(), String>) -> String {
    match rollback {
        Ok(()) => format!("{error} Previous hardware values restored."),
        Err(e) => format!("{error} Hardware restoration also failed: {e}"),
    }
}
pub fn decode_dpi_list(bytes: &[u8]) -> Result<Vec<u16>, String> {
    // The sensor byte leaves an odd number of parameter bytes in long reports.
    let bytes = if !bytes.len().is_multiple_of(2) && bytes.last() == Some(&0) {
        &bytes[..bytes.len() - 1]
    } else {
        bytes
    };
    if !bytes.len().is_multiple_of(2) {
        return Err("Incomplete DPI choices.".into());
    }
    let words: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let w = words[i];
        if w == 0 {
            break;
        }
        if w & 0xe000 == 0xe000 {
            let step = w & 0x1fff;
            let start = *out.last().ok_or("DPI range has no start.")?;
            let end = *words.get(i + 1).ok_or("DPI range has no end.")?;
            if step == 0 || end <= start || end > 32000 {
                return Err("Invalid DPI range.".into());
            }
            for d in ((start as u32 + step as u32)..=end as u32).step_by(step as usize) {
                out.push(d as u16)
            }
            i += 2;
        } else {
            if !(100..=32000).contains(&w) {
                return Err("Invalid DPI choice.".into());
            }
            out.push(w);
            i += 1;
        }
        if out.len() > 2000 {
            return Err("Too many DPI choices.".into());
        }
    }
    out.sort_unstable();
    out.dedup();
    if out.is_empty() {
        return Err("Mouse returned no DPI choices.".into());
    }
    Ok(out)
}
pub fn match_reply(
    report: &[u8],
    device: u8,
    feature: u8,
    function: u8,
) -> Result<Option<Vec<u8>>, String> {
    if report.len() < 4
        || !matches!(report[0], 0x10 | 0x11)
        || !(report[1] == device || (device == 0xff && report[1] == 0))
    {
        return Ok(None);
    }
    if matches!(report[2], 0xff | 0x8f)
        && report.len() >= 6
        && report[3] == feature
        && report[4] == function
    {
        return Err(format!(
            "Mouse rejected HID++ request ({:02x}:{:02x}, error {}).",
            feature, function, report[5]
        ));
    }
    if report[2] == feature && report[3] == function {
        return Ok(Some(report[4..].to_vec()));
    }
    Ok(None)
}

#[cfg(test)]
mod live_tests {
    use super::*;

    #[test]
    #[ignore = "temporarily changes real mouse DPI, wheel mode and SmartShift, then restores readback values"]
    fn live_hardware_roundtrip_restores_the_original_device_state() {
        let mut c = Client::new(
            crate::hardware::linux::Hidraw::open(
                std::env::var("MOUSE_TEST_DEVICE")
                    .ok()
                    .map(|s| crate::device::MouseModel::parse(&s).unwrap())
                    .unwrap_or_default(),
            )
            .expect("Mouse HID access"),
        );
        let dpi_index = c.dpi_feature().unwrap().expect("Adjustable DPI feature");
        let choices = c.dpi_values(dpi_index).unwrap();
        let original_dpi = c.dpi_read(dpi_index).unwrap();
        let wheel = c.wheel_feature().unwrap().expect("SmartShift feature");
        let (original_mode, original_threshold) = c.wheel_read(wheel).unwrap();
        let original_raw = c
            .wire
            .request(wheel.index, if wheel.enhanced { 0x10 } else { 0 }, &[])
            .unwrap();
        assert_ne!(
            original_threshold, 0,
            "Zero means 'keep current' on writes, so this state cannot be restored exactly; no writes attempted."
        );
        let next = *choices
            .iter()
            .filter(|d| **d != original_dpi)
            .min_by_key(|d| d.abs_diff(original_dpi))
            .expect("Alternative DPI");
        let other_mode = if original_mode == WheelMode::Ratchet {
            WheelMode::FreeSpin
        } else {
            WheelMode::Ratchet
        };
        println!(
            "Captured {original_dpi} DPI, {original_mode:?}, raw SmartShift threshold {original_threshold}; {} advertised DPI choices.",
            choices.len()
        );
        let trial =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), String> {
                c.apply(&HardwareSettings {
                    dpi: Some(next),
                    ..Default::default()
                })?;
                c.cycle(&[original_dpi, next])?;
                if c.dpi_read(dpi_index)? != original_dpi {
                    return Err("DPI cycling did not wrap to the original value.".into());
                }
                c.apply(&HardwareSettings {
                    wheel_mode: Some(other_mode),
                    smartshift: Some(if original_threshold == 20 { 25 } else { 20 }),
                    ..Default::default()
                })?;
                c.apply(&HardwareSettings {
                    wheel_mode: Some(WheelMode::Ratchet),
                    smartshift: Some(50),
                    ..Default::default()
                })?;
                Ok(())
            }));
        // Both cleanup operations run even when a test/write/readback fails.
        let restore_wheel = c.wheel_write(wheel, original_mode, original_threshold);
        let restore_dpi = c.dpi_write(dpi_index, original_dpi);
        assert!(
            restore_wheel.is_ok() && restore_dpi.is_ok(),
            "Hardware restoration failed: wheel={restore_wheel:?}, DPI={restore_dpi:?}"
        );
        assert_eq!(
            c.wire
                .request(wheel.index, if wheel.enhanced { 0x10 } else { 0 }, &[])
                .unwrap(),
            original_raw,
            "Full wheel state, including torque, preserved"
        );
        println!(
            "Original DPI, wheel mode, torque and raw SmartShift threshold restored and read back."
        );
        assert!(
            matches!(&trial, Ok(Ok(()))),
            "Live hardware verification failed: {trial:?}"
        );
    }
}
