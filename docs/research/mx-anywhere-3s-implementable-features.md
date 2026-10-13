# Mouse Studio — practical first version

For your current Arch Linux + Hyprland setup. The Rust app is built and tested. Checked, crossed-out items are implemented; unchecked items remain planned.

- [x] ~~Show whether the MX Anywhere 3S is connected.~~
- [x] ~~Assign Back, Forward, and middle-click to open an application or run a command.~~
- [x] ~~Assign those buttons to keyboard shortcuts or workspace switching using [Hyprland bindings](https://wiki.hypr.land/0.55.0/Configuring/Basics/Binds/).~~
- [x] ~~Adjust pointer speed and acceleration using [Hyprland input settings](https://wiki.hypr.land/Configuring/Basics/Variables/).~~
- [x] ~~Adjust scrolling speed and switch normal/reversed scrolling.~~
- [x] ~~Create, rename, duplicate, and manually switch settings profiles.~~
- [x] ~~Save settings locally and apply the selected profile with Save profiles.~~
- [x] ~~Restore the saved profile at login, mouse reconnect and Hyprland reload (optional Settings toggle).~~
- [x] ~~Import and export settings as a JSON file.~~
- [x] ~~Restore the previous saved Mouse Studio setup (profiles, controls, button assignments and automatic-restore preference).~~
- [x] ~~Show a static mouse image with clickable button labels.~~
- [x] ~~Reset an entire profile to defaults while preserving its name.~~
- [x] ~~Show Bluetooth/USB receiver connection type (Bolt when identified by USB metadata).~~
- [x] ~~Automatically switch saved profiles for the focused application's class.~~
- [x] ~~Highlight live mouse button presses in the preview.~~
- [x] ~~Hold a button and turn the wheel for horizontal scrolling.~~
- [x] ~~Assign Hold and Double-click actions with adjustable timing.~~
- [x] ~~Use temporary pointer precision mode while holding a button.~~
- [x] ~~Rotate and zoom a photo-referenced 3D mouse preview with selectable hotspots.~~
- [x] ~~Compose, edit, test, cancel and assign macros with shortcuts and delays.~~
- [x] ~~Assign directional gestures with adjustable thresholds and a test pad.~~

- [x] ~~Show live mouse battery percentage and charging status from Linux telemetry.~~
- [x] ~~Add sensor DPI controls, named editable presets and a Cycle DPI button action.~~ (Bluetooth; verified by device write/readback.)
- [x] ~~Switch the wheel between ratchet and free-spin.~~ (Bluetooth; verified by device write/readback.)
- [x] ~~Adjust the SmartShift disengagement threshold or disable automatic switching.~~ (Bluetooth; verified by device write/readback.)

Button bindings are restricted to this MX Anywhere 3S using Hyprland’s per-device filter. Battery reporting was read from the connected mouse. Sensor DPI/cycling, both wheel modes and SmartShift passed actual device write/readback checks, with the original settings restored. The Bluetooth access rule is installed on this machine. See [hardware controls and setup](docs/hardware-controls.md).

Previous-setup restoration becomes available after saving a change to an existing setup. It restores values saved by Mouse Studio; desktop settings from before the app first changed them were not recorded.

See [software features](docs/software-features.md) for configuration, trigger behavior and required helper details.
