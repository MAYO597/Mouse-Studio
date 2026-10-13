# Three mouse models

The app opens on a connected supported mouse. Use the mouse selector above the sidebar navigation (the dropdown arrow above Home in compact windows). Mouse Studio supports these model families:

- MX Anywhere 3S
- Signature M650 L (some Linux devices identify this family simply as M650)
- MX Master 4

Switching mice keeps unsaved drafts in memory. Save profiles applies only the selected mouse. The old Anywhere setup stays in `settings.json`; M650 L uses `m650-l.json`, and Master 4 uses `mx-master-4.json`, all under the same Mouse Studio config directory. Each file has independent profiles, previous-setup history, imports and exports. Imports from a different model are rejected. Save each mouse separately before closing to retain its edits.

## Controls and verification

| Feature | Bluetooth | Bolt |
|---|---|---|
| Pointer speed/acceleration, scroll speed/direction | Implemented for all three models | Implemented for named mice and verified C548 Bolt receivers |
| Back/Forward/Middle clicks, hold/double-click, shortcuts, commands, macros | Implemented per model | Implemented per model |
| Master 4 wheel-mode/gesture/Haptic Sense panel button assignments | Implemented with native HID++ listener; physically unverified | Implemented with native HID++ listener; physically unverified |
| Application profiles, gestures, precision and horizontal-scroll combinations | Implemented per model | Implemented per model |
| Battery status | Linux power-supply or HID++ Unified Battery | Master 4 verified: 90%, Discharging |
| Sensor DPI and preset cycling | Feature discovery implemented; M650 L/Master Bluetooth still unverified | Master 4 write/readback verified, 200–8,000 DPI in steps of 50 |
| Ratchet/free-spin and SmartShift | Queried on MX models; Master Bluetooth still unverified | Master 4 write/readback verified, preserving wheel torque |
| Master 4 native side-wheel scrolling and direction | Implemented; physically unverified | Implemented; physically unverified |
| Master 4 haptic feedback effects | Not implemented | Not implemented |
| Actions Ring overlay (button action) | Implemented; overlay window not visually verified | Not tested on hardware |

The M650 L uses [SmartWheel](https://www.logitech.com/en-us/shop/p/m650-signature-wireless-mouse), so the app excludes MX wheel-mode controls. Master 4-specific [haptic controls](https://support.logi.com/hc/en-150/articles/34335429816727-Haptic-Feedback-MX-Master-4) need a separate implementation. Existing logiops configuration is preserved. Primary clicks and native thumb-wheel behavior remain handled by the system.

The MX Master 4 was physically verified on 2026-10-09 through a 046d:c548 Bolt receiver, slot 2, WPID B042. DPI changes/cycling, wheel modes, SmartShift and restoration passed. Both compositor receiver pointers accepted pointer speed, acceleration, scroll direction and scroll multiplier; scroll readback and unrelated-device isolation passed. The user pressed Back, Forward and Middle, and the real device-filtered runtime received each once. M650 L and Master Bluetooth still require physical verification. Automated tests additionally cover profiles, transport switching, services and UI. The Master 4 has six official graphite photo views with button hotspots: Perspective, Top, Side, Front, Rear angle and Bottom. Only the Anywhere has the existing interactive 3D preview. The M650 L has official graphite Perspective and Top photos on Overview and Buttons, with Back/Forward/Middle-click markers and selectors.

For Bluetooth hardware access, connect the mouse over Bluetooth, then run `sudo bash scripts/install-device-access.sh` from this project (or `/usr/share/mouse-studio/scripts/install-device-access.sh` if you installed the package). The script adds permissions only for the detected supported Logitech Bluetooth product IDs and preserves previously installed product-specific grants. It also grants access to the C548 Bolt HID++ configuration interface (USB interface 02), excluding its keyboard and mouse interfaces. Generic receiver routing needs this access to verify the live mouse identity. Unsupported or unavailable hardware features stay disabled for the new models.

For generic C548 receivers the app queries paired slots and confirms the live mouse name before applying controls. Software routing requires exactly one paired pointing device and one unambiguous receiver. Multiple paired mice, extra generic receivers, unknown receiver products and sleeping/unresponsive devices are rejected rather than guessed. Wake the mouse and retry when needed. If two connections of the same model are present, disconnect the extra one before saving. Separate physical mice of the same model are not individually managed.

## Background services and CLI

Each model gets an independent restore service when enabled or required by a profile. Master 4 native side-wheel restoration keeps its service running after Save, including with Automatic restore off; extra-button assignments also use this service. Save manages it automatically. Disabling one preserves the other models' services. Horizontal-scroll sockets, macro state and cancellation are also per model. See [Master 4 buttons](master-4-buttons.md) and [side wheel](side-wheel.md) for native defaults and verification limits.

```bash
./target/release/mouse-studio --device m650-l
./target/release/mouse-studio --device mx-master-4
./target/release/mouse-studio --device mx-master-4 --hardware-status
```

Opening the GUI without `--device` selects a connected supported mouse. Named Linux input devices are detected first; a generic C548 Bolt receiver is queried read-only, with live identity verification rather than pairing metadata alone. If several named mice are connected, selection follows Anywhere 3S, M650 L, then Master 4; verified Bolt candidates use the same order and skip unresponsive mice. If none can be identified, the app opens on Anywhere 3S. An explicit `--device` overrides detection. Background services and command helpers without `--device` retain their original Anywhere target. Opening or switching editors never applies a profile automatically; Save does.
