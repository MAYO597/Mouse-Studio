# Software features

All features below use Linux metadata or host-side input processing. Save profiles / Ctrl+S applies the saved setup; edits, reset and imports remain drafts until then. Old version-1 JSON files load with the new features off.

## Profiles and connection

In **Profiles**, select a profile and enter comma-separated Hyprland application classes, such as `Code`, `brave-browser` or `blender`. Enable **Application profiles** and save. Exact matches ignore ASCII letter case. A class belongs to one profile; unmatched apps use the manually selected fallback. All profiles are applied as complete setups. The interface shows the currently applied profile separately from the draft selection. Duplicate clears application matches to avoid ambiguity. Reset preserves the profile ID/name and resets its controls, bindings, advanced modes and application matches; Discard can undo it before saving.

Overview shows Bluetooth or USB receiver from Linux input bus metadata. Bolt is named when the USB product metadata explicitly identifies it. Disconnected devices have no fabricated connection type.

## Triggers and special modes

Select Back, Forward or Middle in the inspector, then **Click**, **Hold** or **Double-click**. All action kinds, including macros, can be assigned to these triggers. Default Hold/Double-click means no action. A custom hold suppresses the click after its delay; shorter presses run Click on release. Double-click waits for the second release within the configured window, otherwise runs a single Click. Default Click still delivers the normal button event when an advanced trigger consumes it. Without advanced triggers, normal default clicks pass through unchanged.

**Advanced button settings** configures timing (hold 150–2000 ms, double-click 100–600 ms) and three special modes. They must use different buttons and cannot share a button with a custom Hold/Double-click action.

- **Precision:** while held, applies the chosen desktop sensitivity with flat acceleration. Release, profile switching, another Save or cancellation restores the profile's pointer settings. This mode takes over the selected button; it does not set sensor DPI.
- **Horizontal scrolling:** hold the chosen button and turn the wheel. The owned user service creates a virtual pointer and emits `REL_HWHEEL` events. Releasing after scrolling suppresses the button's Click; a tap without scrolling runs Click. Save starts the helper when any saved profile needs it, even with Automatic restore off; removing all horizontal-scroll assignments and saving stops it unless Automatic restore still needs the service. `/dev/uinput` must be writable by the user (already true on this machine). Startup failures are reported. The socket is private to the user and accepts only one left/right wheel step per request.
- **Gestures:** hold the chosen button, move, then release. The dominant direction selects Up/Down/Left/Right once movement exceeds the threshold (10–500 screen pixels). Below threshold, Click runs. Cursor movement stays visible. The test pad classifies a drag without running any action. Movement is measured from the compositor cursor while the device-filtered trigger is held.

The compositor scheduler has 20 ms resolution, with delays extended under load. Bindings target only `logitech-mx-anywhere-3s`. Primary clicks are monitored without consumption. Existing user bindings are retained; overlapping user bindings can also fire. No physical input nodes are opened or grabbed.

## Macros

Choose **Macro sequence → Edit macro**. Enter one shortcut per line; a delay is written as `wait 250`. For example:

```text
CTRL+C
wait 250
CTRL+V
```

**Use macro** puts the sequence into the chosen draft binding. Save registers it. **Test macro** starts after three seconds so you can switch to a target window. **Cancel running macro**, or the inspector's **Cancel actions**, cancels pending tests and scheduled actions. Maximum 64 steps, at most 10 seconds per delay and 60 seconds total. Macros compose shortcut taps; every key press has an immediate matching release. Starting a new macro replaces the previous one. Changing the applied profile cancels pending actions/macros; switching to a different app profile during a macro also cancels its remainder. This editor composes sequences; it does not record global keyboard input.

## Preview

Product photographs remain the initial view. Select **3D** for the photo-referenced mouse model, drag to rotate, scroll to zoom and select numbered hotspots. Reset returns to the original photo and resets the orbit. The model follows all four supplied product images, with a measured top silhouette, contoured shell, sculpted grip channels, recessed steel wheel, traced logo and separate buttons. Hotspot positions are exported with the mesh, and the camera preserves the reference orientation. It is a visual approximation from the reference photographs, not manufacturer CAD. Editable Blender and GLB files are in `assets/models/`; see its README for regeneration instructions. After Save installs monitoring, live Back/Forward/Middle/Left/Right presses highlight the preview; short clicks remain visible briefly to support asynchronous display updates.

## Runtime and verification

The saved setup installs one Lua runtime for focus events, device-filtered consuming action binds, non-consuming monitors and timers. It remains active after closing the GUI. Automatic restore reinstalls it after login/reconnect/config reload. Horizontal-scroll and hardware profiles require the existing service and therefore also restore their full saved setup. Sensor DPI, SmartShift, wheel modes, battery reporting and Cycle DPI are documented in [hardware controls](hardware-controls.md).

Rust tests cover schema compatibility, reset, macro bounds, class matching, gesture directions, connection parsing, wheel requests and UI controls. A deterministic Lua mock tests actual generated callbacks, hold/double timing, focus fallback, precision restoration, macro cancellation and handle reuse. The opt-in live software test uses a temporary virtual pointer and an isolated Mouse Studio preview: it checks real compositor APIs/timers, focus matching, balanced key taps, horizontal-axis emission and private socket/client/pump behavior. It does not change the physical mouse. Physical-button behavior in target applications remains a user-observation check.

Primary API references: [Hyprland binding flags](https://wiki.hypr.land/configuring/core/binds/flags/), [Lua events](https://wiki.hypr.land/configuring/core/advanced-configuration/events/), [installed-version dispatch implementation](https://github.com/hyprwm/Hyprland/blob/efb50993780079460b0cbed1363e2166a2de1d9f/src/config/shared/actions/ConfigActions.cpp), [evdev virtual devices](https://docs.rs/evdev/0.13.2/evdev/uinput/struct.VirtualDevice.html).
