# MX Anywhere 3S hardware controls

Implement the remaining device features for the user's current Arch/Hyprland Bluetooth mouse without changing the approved layout. Keep draft editing and explicit Save semantics. Hardware choices default to unmanaged so loading old profiles never invents device settings.

Use a native Rust HID++ 2 client on the exact Logitech MX Anywhere 3S Bluetooth hidraw interface (046D:B037). Discover adjustable DPI (2201) and SmartShift (2110, falling back to 2111). Read advertised DPI choices; reject unsupported values before writes. Write/read back each requested setting, and restore a captured device snapshot if any write/readback fails. Bound requests and serialize access between GUI, restore service and cycle helper. Do not divert physical buttons or alter onboard profiles.

Read live battery percentage/charging from the exact mouse's Linux power-supply entry, without needing hidraw permissions. Clear missing data on disconnect. Display readback separately from draft values. Poll device state off the GUI thread.

Store optional sensor DPI, wheel mode (ratchet/free-spin), optional SmartShift threshold, and named editable DPI presets per profile. Add a Cycle DPI button action using those presets. The owned user service follows compositor application-profile selection for hardware profiles; restore-on-login also reapplies after reconnect. Avoid repeatedly forcing wheel mode so the physical mode button remains useful.

Install an active-seat uaccess rule limited to this Bluetooth product through a reviewable root-only script. Administrator authentication cannot be supplied by the agent. When unavailable, show the exact setup command and keep existing software controls usable. USB/Bolt transport is outside this verified implementation.

Verification: protocol fixtures for wrong replies/errors, DPI ranges, unsupported choices, SmartShift serialization, write readback/rollback and cycle order; real temporary sysfs fixtures for battery; legacy JSON/profile reset; native UI capture; formatter, Clippy, full tests and release build. Run read-only and reversible live write/readback checks if user enables hidraw access. Physical feel and charging transition require human observation.
