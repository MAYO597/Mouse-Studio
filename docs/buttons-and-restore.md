# Button actions and automatic restore

The approved UI remains the baseline. Existing action fields and Test action now work; Settings gains one Automatic restore card.

## Button actions

Select Back, Forward or middle-click, choose an action, enter its value, then **Save profiles**. The active profile supplies all pointer, scroll and button settings. Click, Hold and Double-click are supported; see [software features](software-features.md) for timing, macros and special modes.

- **Application / Command:** a command handled by Hyprland's exec dispatcher. The application must be installed. These fields intentionally support shell command syntax. Strings are escaped as Lua data; saving never executes them.
- **Shortcut:** modifiers CTRL/CONTROL, SHIFT, ALT, SUPER/META, followed by a key, separated by `+`. Supports letters/digits, F1–F35, navigation keys, Tab/Enter/Escape/Space, BackSpace/Delete/Insert, plus/minus/equal, Print, and common XF86 audio keys. Unsupported keys/modifiers are rejected. Examples: `CTRL+C`, `CTRL+SHIFT+C`, `ALT+Tab`, `SUPER+1`.
- **Workspace:** 1–99.
- **Default:** keeps normal Click behavior and leaves Hold/Double-click unassigned. With no advanced trigger, the consuming action binding is disabled.

**Test action** executes the draft action after three seconds without saving it. For shortcuts, switch to the receiving window during the delay. A “sent” result means Hyprland accepted dispatch; the app cannot guarantee that an external process launched successfully or that the target application handles a shortcut.

Bindings use the exact device filter `logitech-mx-anywhere-3s`: Back 275, Forward 276, middle 274. They do not override primary left/right clicks, bindings for other mice, or logid. Existing user bindings on the same device/button may also fire; Mouse Studio does not delete them.

Hyprland state lives under `__mouse_studio_v1`. Owned press/release and non-consuming monitor handles are reused, with disabled handles retained for unused actions. On the installed Hyprland version, even a handle's `remove()` removes all matching key/modifier bindings; this app never calls it or broad `hl.unbind`. Repeated saves do not accumulate bindings. Config reloads may expire the handles, which are recreated when restoring.

## Automatic restore

In Settings, enable **Automatic restore**, then save. Old profiles default to off; simply opening/importing a profile does not install anything. Disabling and saving stops/disables/removes only the owned service; currently applied settings stay active.

Installed locations (respecting XDG_CONFIG_HOME / XDG_DATA_HOME):

- `~/.config/systemd/user/mouse-studio-restore.service`
- `~/.local/share/mouse-studio/bin/mouse-studio`

A stable binary copy lets restoration continue if the project moves. It updates on saving from a newer build. The service reads the same settings file as the GUI; no duplicate profile store is created. Its command is `mouse-studio --restore-service --settings /absolute/path/settings.json`.

The service starts with the user manager and waits for a connected mouse in a single Hyprland session. Every five seconds it reads validated settings and checks device identity plus a compositor profile marker and binding handles. It reapplies only when needed. Commands are registered, never executed as part of restoration. Invalid files, disconnected mice and unavailable sessions are retried; repeated identical failures are logged once. Multiple Hyprland sessions are treated as ambiguous rather than guessing.

Inspect with `systemctl --user status mouse-studio-restore.service` and `journalctl --user -u mouse-studio-restore.service`. No root, linger setting, or Hyprland configuration edit is required. Paths with spaces are supported. A pre-existing unit without Mouse Studio's ownership header is preserved and reported as a conflict.

## Verification and limits

- Unit/UI checks cover action parsing/escaping, selected-profile application, explicit testing without saving, owned binding generation, restore decisions/retries, old file compatibility, unit quoting, executable permissions, service upgrades/removal, foreign-file preservation and error reporting.
- The opt-in live action test registers all three action types, verifies the exact device filter, checks repeated-save idempotence, dispatches a harmless temporary marker command, and verifies Default disables owned bindings. It restores the original saved setup afterward.
- The opt-in live restore test installs/enables the real user service with temporary profiles and binary paths containing spaces. It verifies initial restoration, recovery after simulated marker loss, a later saved-profile change, and removal of the unit/autostart symlink. Pointer settings and previous action state are restored afterward. It does not reboot or reload the user's whole compositor.
- Physical button events and shortcut handling by each target application still require user observation. Hardware DPI, SmartShift, wheel modes, battery reporting and Cycle DPI have a separate implementation; see [hardware controls](hardware-controls.md) for setup and verification status. Software application profiles are implemented.

Live checks are ignored in ordinary `cargo test`. `tests/live_actions.rs` requires MOUSE_TEST_ACTIONS=1 and an explicit HYPRLAND_INSTANCE_SIGNATURE. `tests/live_restore.rs` requires MOUSE_TEST_RESTORE_SERVICE=1, MOUSE_TEST_BINARY pointing to the release binary, the session signature and the user runtime directory; it refuses to replace an existing service installation.

Primary references: [Hyprland bind device filters](https://wiki.hypr.land/configuring/core/binds/flags/), [mouse button bindings](https://wiki.hypr.land/configuring/core/binds/devices/mouse/), [dispatchers](https://wiki.hypr.land/configuring/core/dispatchers/), [installed-version binding implementation](https://github.com/hyprwm/Hyprland/blob/efb50993780079460b0cbed1363e2166a2de1d9f/src/config/lua/objects/LuaKeybind.cpp).
