# Multiple mouse support

Request: use Mouse Studio with MX Anywhere 3S, Signature M650 L, and MX Master 4 over a mixture of Bluetooth and Bolt.

Keep the approved visual style and existing Anywhere settings. Add a device picker, per-model settings/history/export paths and retained unsaved drafts when switching. Resolve exact supported input names; reject ambiguous multiple connections. Scope compositor bindings, macro state, status and cancellation to the selected model. The shared Back/Forward/Middle, pointer, scroll and software profile features work on either transport when the kernel exposes the mouse by name.

Discover Bluetooth HID++ interfaces by Logitech vendor plus exact model name; query existing feature IDs before allowing device settings. Preserve current Anywhere behavior. M650 L does not expose MX wheel-mode UI. Bolt hardware writes, Master haptics/thumb controls and additional 3D models are outside this change: they require new device/protocol verification. Display clear capability/connection messages; never substitute the Anywhere photograph for another model.

Implementation:
1. Model catalog, exact-name discovery, legacy-compatible settings identity/paths.
2. Per-device backend/runtime namespaces and hardware selection, with strict cross-device tests.
3. Independent restore services and horizontal-scroll sockets; helper CLI carries model identity.
4. Device picker, retained drafts, appropriate visuals/capability UI and import identity validation.
5. Full tests/build, native captures, documentation and explicit remaining hardware-verification limits.

## Receiver follow-up, 2026-10-09

User connected the Master 4 over its receiver and requested actual verification. Add C548 interface-02 HID++ transport, paired-slot and live-name verification, independent pointer-interface routing and cached connection telemetry. Query Unified Battery. Preserve enhanced-wheel torque. Reject ambiguous generic receivers and multiple paired pointing devices. Verify hardware changes with restoration, compositor pointer controls with isolation, and physical Back/Forward/Middle input. Keep logiops configuration intact.
