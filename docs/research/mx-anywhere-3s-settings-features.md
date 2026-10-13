# Mouse Studio — MX Anywhere 3S feature proposal

A personal Linux mouse settings app with a visual, interactive device preview. This is the original proposal. Implemented software features and remaining hardware work are tracked in [the practical checklist](mx-anywhere-3s-implementable-features.md); current behavior is documented in [software features](docs/software-features.md).

## Main features

- **Interactive 3D mouse:** rotate, zoom, switch between top and side views, select button hotspots, and highlight live button presses. The concept image contains a rendered preview, not a reusable 3D mesh.
- **Button assignments:** map supported buttons to keyboard shortcuts, application launchers, media controls, workspace navigation, or custom commands. Offer click, hold, and double-click triggers where event handling permits.
- **Application profiles:** global defaults plus overrides for Brave, VS Code, Blender, and other apps; automatically select the profile for the focused application.
- **DPI and pointer controls:** precise sensor DPI, named presets, a cycle-DPI action, temporary precision mode, and separate OS pointer speed/acceleration controls. Logitech documents a 400–8,000 DPI range for this mouse.
- **Scrolling controls:** ratchet/free-spin mode, SmartShift threshold, scroll direction and speed, and horizontal-scroll combinations, where exposed by the device.
- **Gestures:** hold a selected button and move up, down, left, or right to invoke actions; adjustable threshold and a gesture test area.
- **Macro editor:** record or compose shortcut sequences with delays; edit, test, cancel, and assign them to supported triggers.
- **Device status:** battery level, charging status, connection type, and reconnect/disconnected states, based on available telemetry.
- **Profile management:** create, duplicate, rename, import/export, restore defaults, and start the background service at login.
- **Safe editing:** test an action before saving, show pending changes, undo/reset mappings, and retain a recovery route for primary-click changes.

## Suggested implementation order

1. Device discovery, capability detection, DPI, scrolling, and simple button mappings.
2. Focus-aware application profiles, app launching, shortcuts, and workspace actions.
3. Interactive 3D model, gestures, macros, import/export, and polish.

## Implementation considerations

Verify HID++ controls on this specific mouse and Bluetooth/Bolt connection before enabling hardware settings. App detection and global remapping must integrate with the Linux desktop/compositor. Host-managed profiles and macros require the background service; do not present them as onboard mouse memory. Show unsupported controls as unavailable with a reason. The illustrated battery level is sample data.

Hardware reference: https://support.logi.com/hc/en-sg/articles/12895017855127-Specification-MX-Anywhere-3S
