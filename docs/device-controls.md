# Working pointer and scrolling controls

Four controls apply to the active profile's **Logitech MX Anywhere 3S** through Hyprland's per-device Lua API:

- Pointer speed: `sensitivity`, -1 to 1.
- Pointer acceleration: `accel_profile`, adaptive when on and flat when off.
- Scroll speed: `scroll_factor`, 0.1× to 5×; 1× is normal.
- Normal/reversed scrolling: `natural_scroll`.

Sensor DPI and wheel modes now have a separate native HID++ backend; see [hardware controls](hardware-controls.md) for access setup and verification status.

## User behavior

1. Adjust the controls in Overview, Pointer or Scrolling.
2. Press **Save profiles** or **Ctrl+S**. This saves all profiles and applies all four values plus button assignments from the selected profile.
3. The status reports applying, success, or a failure with its reason. Save again to retry. Profiles remain safely saved if applying fails.

Opening the app, editing a draft, switching profiles, importing and discarding do not apply changes. Save after selecting/importing a profile to apply it. Save & close waits for applying to finish; failure keeps the window open.

The compositor retains these values after the app closes. They are runtime overrides: enable the optional **Automatic restore** setting to restore the saved profile at login, reconnect and config reload. Otherwise reopen and save after a compositor restart/reload. See [button actions and restore](buttons-and-restore.md).

## Integration

- Uses `hyprctl` directly as an argument-vector subprocess, never a shell. Only the fixed verified device name, fixed acceleration profile names and validated float/boolean values enter Lua.
- Uses `HYPRLAND_INSTANCE_SIGNATURE` if available, otherwise matches `WAYLAND_DISPLAY` against discovered instances, or accepts a single unambiguous session. Multiple sessions are never guessed.
- Requires the exact device name in the session's connected mice before writing. Missing mouse, invalid JSON, process failures/timeouts, and non-`ok` compositor replies are errors.
- All four values are submitted in one `hl.device` call. Global input configuration, touchpad settings, Hyprland config files and logid are not changed. Enabling Automatic restore installs an owned user-service unit and a stable copy of the binary.
- A worker thread keeps compositor calls off the GUI thread. Each subprocess has a three-second deadline. Concurrent applies are blocked. The snapshot being saved is the snapshot applied.
- `Studio::new` is an isolated editor for captures/tests. Only the real executable injects `Hyprland` and the startup installer through `Studio::with_services`. Tests inject fakes; captures remain isolated.

Targets the verified Arch desktop running **Hyprland 0.56.2 with Lua configuration**. Other compositors, legacy Hyprland configuration and receiver device aliases are not implemented in this milestone.

## Verification

Automated tests cover adaptive/flat acceleration, scroll multiplier boundaries and rejection of invalid/non-finite values, exact device targeting, session selection/ambiguity, validation, disconnection, failure replies/timeouts propagated by the runner, saving the selected profile, apply failures and retry, failed disk writes, and discard isolation. Ordinary tests never write to the real compositor.

An opt-in live test applies both acceleration profiles, both scroll directions and two pointer/scroll speeds. It reads back the mouse's actual configured `scrollFactor` (tested at 0.6× and 1.8×), checks that other pointer devices and global input settings remain unchanged, checks the compositor error list, and restores explicitly supplied effective values. The live test passed on the connected mouse.

Hyprland 0.56 does not expose effective per-device sensitivity, direction or acceleration profile through `getoption` (`devices.defaultSpeed` is a default, not current speed). Those three controls are verified by compositor acknowledgment, not claimed hardware readback. Physical pointer feel and wheel movement still need human observation. The desktop's global `force_no_accel` is false; enabling it externally bypasses pointer acceleration/sensitivity. The app does not alter that global setting.

To rerun the live test, first establish the current values and set `HYPRLAND_INSTANCE_SIGNATURE`, `MOUSE_TEST_RESTORE_SENSITIVITY`, `MOUSE_TEST_RESTORE_NATURAL_SCROLL`, `MOUSE_TEST_RESTORE_ACCELERATION` and `MOUSE_TEST_RESTORE_SCROLL_SPEED`, then run `cargo test --test live_hyprland -- --ignored --nocapture`. It deliberately has no default restoration values. Restoration writes explicit per-device values; it does not remove overrides or restore inheritance from global config.

## Primary references

- [Hyprland input options](https://wiki.hypr.land/Configuring/Basics/Variables/)
- [Hyprland per-device configuration](https://wiki.hypr.land/configuring/core/devices/)
- [hyprctl eval and session selection](https://wiki.hypr.land/configuring/core/advanced-configuration/using-hyprctl/)
- [Installed-version implementation of hl.device](https://github.com/hyprwm/Hyprland/blob/efb50993780079460b0cbed1363e2166a2de1d9f/src/config/lua/bindings/LuaBindingsConfigRules.cpp)
