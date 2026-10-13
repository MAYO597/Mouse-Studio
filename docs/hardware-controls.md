# Sensor DPI, wheel controls and battery

The MX Master 4 is also verified over the C548 Bolt receiver (slot 2, WPID B042): 200–8,000 DPI in steps of 50, preset cycling, wheel mode, SmartShift and Unified Battery. Tests restored 1000 DPI, ratchet, threshold 12 and torque 75; battery read 90%, Discharging. See [multiple mice](multiple-mice.md).

The app now includes native HID++ controls for the **MX Anywhere 3S over Bluetooth (046D:B037)**. Sensor DPI, DPI cycling, ratchet/free-spin mode and SmartShift have passed write/readback checks on the connected mouse. It advertises **200–8,000 DPI in steps of 50** (157 choices). Battery reporting reads **55%, Discharging** during development. Charging transitions have not been physically exercised.

## One-time device access

Run from your own terminal:

```bash
# installed from the Arch package:
sudo bash /usr/share/mouse-studio/scripts/install-device-access.sh
# or from a source checkout:
sudo bash scripts/install-device-access.sh
```

The script installs `packaging/70-mouse-studio.rules` into `/etc/udev/rules.d/`, reloads udev and triggers the hidraw devices. The rules grant the active desktop user access to supported detected Bluetooth product IDs and the C548 Bolt HID++ configuration interface 02. They exclude the receiver keyboard/mouse interfaces and do not make any interface world-writable. Run the application as your normal user. The app retries discovery every five seconds; if access remains unavailable, reconnect the mouse.

Once the rule is installed, the active user has read/write access to the mouse interface. Battery reporting and the existing desktop controls do not need this rule.

## Using the controls

- **Pointer → Manage sensor DPI:** choose a value, select one of the named presets, or use Supported values after discovery. The app obtains accepted values from the mouse. Edit preset names and values under Edit DPI presets; these also define the cycle order.
- **Buttons → Action type → Cycle DPI:** assign to Click, Hold or Double-click. Gesture actions can also cycle DPI. Save installs the assignment. Test action uses the draft profile's presets after the usual three-second delay. Cycling advances from the current sensor DPI, wraps at the end, and starts at the first preset if the current DPI is outside the list.
- **Scrolling → Manage wheel mode:** select Ratchet or Free-spin. Ratchet permits SmartShift to disengage it at the chosen threshold. Use threshold 50 for an always-ratcheted wheel.
- **Scrolling → Manage SmartShift:** values 1–49 set the disengagement threshold; lower values switch sooner. Value 50 disables automatic switching (device value 255). The physical mode button remains available afterward.
- **MX Master 4 → Scrolling → Side wheel:** Save restores native horizontal scrolling and verifies the device settings. Reverse side wheel changes its hardware direction. Native side-wheel restoration also applies to old profiles and keeps the Master 4 helper running; it rechecks the setting every five seconds without resetting DPI or main-wheel mode. See [side-wheel behavior](side-wheel.md), which also describes remapping the side wheel to shortcuts, volume, zoom or workspace actions.
- **Overview:** shows the mouse battery percentage and charging/discharging state from Linux power-supply telemetry. Missing percentage/status is never replaced with a sample value.

All hardware choices are per-profile drafts until **Save profiles / Ctrl+S**. Live readbacks are displayed separately. Old settings files load with DPI/main-wheel management off; Master 4 side-wheel scrolling defaults to native, normal direction. Turning DPI/main-wheel management off leaves the current device value alone; it does not reset the mouse to factory settings. Resetting or restoring a profile follows the same rule for unmanaged values.

Hardware profiles or Cycle DPI assignments require the owned user helper, so Save starts it even with Automatic restore off. Removing all hardware management/cycle actions and horizontal-scroll assignments and saving stops it unless Automatic restore is enabled. It follows the profile actually selected by Hyprland at roughly half-second intervals, applies hardware on profile changes and reconnects, and retries failures. It does not repeatedly force DPI/wheel values while staying on one profile, so cycling and physical wheel-mode changes persist. Rapid focus changes may be coalesced; the latest profile wins on the next poll.

## Master 4 haptic feedback

**MX Master 4 → Scrolling → Haptic feedback** manages the haptic motor per profile: **Manage haptic feedback**, **Haptic feedback on**, and a strength (a 1-100 slider, or Low/Medium/High/Maximum = 25/50/75/100 when the mouse reports it only supports four levels). **Test haptic** plays one effect using the mouse's current state and changes no setting. The card shows only an explanation unless the mouse reports HID++ feature `0x19B0`; Test needs at least one reported effect, and nothing is written unless the feature is confirmed. Save and background restore apply the stored value like DPI/SmartShift: capture the current value, write, require a matching readback, and roll back (reporting whether rollback succeeded) on failure. Disabling keeps the device's strength (or 50 % if it reads out of range), as Solaar does. Old profiles leave haptics unmanaged, and other mice reject the setting.

Wire format (from Solaar `HapticLevel` and `PlayHapticWaveForm` in `settings_templates.py`, and `HAPTIC = 0x19B0` / `HapticWaveForms` in `hidpp20_constants.py`): function `0x00` capabilities, bytes 4-7 a big-endian mask of playable waveforms; `0x10` read `[enabled bit0, strength %, bit0 = four-levels-only]`; `0x20` write `[enabled, strength]`; `0x40` play `[waveform]`. The test effect is the first advertised of SUBTLE_COLLISION (4), SHARP_STATE_CHANGE (0), SHARP_COLLISION (2), HAPPY_ALERT (5). **Unverified on hardware:** that the MX Master 4 firmware matches these layouts, how its strength readback behaves while disabled (only enablement is compared then), and whether the setting persists across power cycles (restore reapplies it on profile change and reconnect, not every five seconds). Live testing is read-only plus the opt-in `cargo test --test haptics -- --ignored`, which plays one effect and writes no setting.

## Failure behavior and limits

Requests are bounded, and GUI/service/cycle helpers serialize their device access. Before writing, the client discovers the requested features, validates DPI support, and captures current device values. Writes require matching readbacks. A write/readback failure attempts to restore the captured values and reports whether restoration succeeded. A physical disconnect can also prevent restoration.

Desktop and hardware application are separate operations: if the desktop apply succeeds and a hardware apply fails, the app reports that partial result. Saved profiles remain available for retry. C548 Bolt routing verifies paired-slot metadata and the live mouse name. Shared receiver software controls require one paired pointing device; multiple or ambiguous receiver routes are rejected. Desktop bindings use the exact verified pointer interfaces.

Read-only diagnostics:

```bash
./target/release/mouse-studio --hardware-status
```

Automated verification covers HID++ reply filtering/errors, padded DPI range lists, capability discovery, supported-value validation, readback/rollback, enhanced SmartShift function numbers and torque preservation, battery device selection, legacy settings, profile focus/reconnect behavior, Lua cycle dispatch, and UI draft/save/discard behavior. The native hardware UI capture uses isolated temporary settings and read-only telemetry (`MOUSE_CAPTURE_HARDWARE=1 cargo run --example capture`). The separate live test temporarily changed DPI, cycled back, switched free-spin/ratchet modes, changed SmartShift and disabled automatic switching. It then restored and read back the original **1,000 DPI, Ratchet, raw SmartShift threshold 10**. No saved profile was changed by that test. Run it only when you intend to briefly change the connected mouse:

```bash
cargo test --lib live_hardware_roundtrip_restores_the_original_device_state -- --ignored --nocapture
```

The ordinary suite has 62 passing tests; five live tests are opt-in, including this new hardware test. Physical pointer feel and charging-state transitions still require human observation.

Protocol references: [Solaar setting definitions](https://github.com/pwr-Solaar/Solaar/blob/master/lib/logitech_receiver/settings_templates.py), [HID++ request/reply handling](https://github.com/pwr-Solaar/Solaar/blob/master/lib/logitech_receiver/base.py). The Rust client implements the corresponding wire format directly; Solaar is not a runtime dependency.
