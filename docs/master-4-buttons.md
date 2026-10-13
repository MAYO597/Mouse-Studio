# MX Master 4 button assignments

Select **MX Master 4**, open **Buttons**, choose a marker or named control, assign an action and press **Save profiles**. Six controls are configurable:

- Back
- Forward
- Middle click
- Wheel-mode button, behind the main scroll wheel
- Gesture button, the separate small button in front of Back/Forward
- Haptic Sense panel, the large thumb-rest button

All six support Click, Hold and Double-click assignments, shortcuts, applications, commands, workspaces, macros and DPI cycling. **Advanced button settings** can use any of them for precision, directional gestures or holding to scroll horizontally. Assign different buttons to each special mode. Profiles, duplication, reset, import/export, discard and previous-setup restoration include the new assignments. Existing three-button settings load with native defaults for the extra controls; other mouse models continue to expose three controls.

Save installs and starts `mouse-studio-mx-master-4-restore.service` whenever any profile uses an extra button. It listens for HID++ reports independently of the open window and restores reporting after receiver reconnect/wake. Native hardware access uses the existing device-access rule and verified mouse identity. Save reports an error if the listener cannot configure and acknowledge the active profile. Assignments remain locally saved for retry. Switching application profiles cancels old actions; a control already held must be released before it can trigger the new profile.

Unassigned extra controls keep native behavior. A customized wheel-mode button with Default Click still switches ratchet/free-spin on a short click; its custom Hold/Double-click triggers take precedence. For the gesture and panel buttons, Default Click is unassigned while custom triggers or a special mode take over that control. Choose an explicit Click action if needed. Native behavior returns when every assignment/special mode for that control is cleared. The listener uses temporary diversion, verifies writes, rolls back partial failures and releases its controls when the service stops. Existing diversion by another mouse utility is reported rather than overwritten. Left/right click, Easy-Switch and native thumb-wheel behavior remain available as before.

The Haptic Sense **button** can run the app's actions. Haptic feedback strength and a test effect are configured separately under Scrolling (see [hardware controls](hardware-controls.md#master-4-haptic-feedback)) and are unverified on hardware. The same listener also diverts the side wheel when a profile remaps it ([side wheel](side-wheel.md)). Logitech's Actions Ring is available as the **Actions Ring** action type; see [Actions Ring](actions-ring.md).

## Verification

Automated tests exercise actual generated Lua for all three extra controls, timed triggers, default wheel-mode fallback, macro/gesture editors, saved-profile migration and stale-event suppression. Protocol tests cover control discovery, receiver-slot filtering, simultaneous press/release reports, transient diversion/readback, rollback, native restoration and receiver reporting reset. UI tests cover device switching and compact/desktop layouts.

The development session had no accessible `/dev/hidraw*` interface or running Hyprland session. These new physical buttons have **not been tested on the mouse**. Earlier hardware verification for Back/Forward/Middle, DPI and scrolling still applies only to those controls.

Protocol references: [Solaar's HID++ implementation](https://github.com/pwr-Solaar/Solaar/blob/master/lib/logitech_receiver/hidpp20.py) and [control identifiers](https://github.com/pwr-Solaar/Solaar/blob/master/lib/logitech_receiver/special_keys.py). The listener queries feature `0x1b04`, uses CIDs `0x00c4`, `0x00c3`, `0x01a0`, gets reporting with function `0x20`, and changes only transient diversion with `0x30`. [Logitech's product guide](https://support.logi.com/hc/en-gb/articles/28321445268247-Getting-Started-MX-Master-4) identifies the separate gesture button and Haptic Sense panel.
