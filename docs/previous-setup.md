# Restore previous setup

Open **Settings → Previous setup → Restore previous setup**, review the confirmation, then choose **Restore & apply**.

Each changed Save keeps the previous saved Mouse Studio setup: all profiles, the selected profile, pointer and scroll values, button assignments, and the Automatic restore preference. Restoring saves that setup and applies its active profile through the existing device backend. The replaced saved setup becomes the new restore point, so a restore can itself be reversed. Unsaved edits are discarded only after the restore is confirmed and successfully saved.

One restore point is kept. The first save to a new settings file has no earlier setup; a subsequent changed save creates one. Existing settings files gain a restore point on their next changed save. Opening the app, editing, Discard, Test action and JSON export do not create restore points. Saving unchanged values, including retries after an apply failure, preserves the current restore point.

If applying fails because the mouse or desktop session is unavailable, the restored setup remains saved and the error is shown. **Save profiles** retries applying. A storage error preserves both the draft and the existing saved file and does not call the device backend. Automatic restore is enabled or disabled according to the restored preference.

## Storage

`src/recovery.rs` stores an optional `previous_setup` field alongside the current settings in `settings.json`. A single atomic file replacement commits both together. History contains an ordinary Settings object, never recursive history. Existing Settings readers and the background service read the current top-level setup; JSON exports contain only the current draft. Current and previous data must validate and fit the combined 2 MB file limit before replacement.

This restores values previously saved by Mouse Studio. It cannot reconstruct desktop values from before the app first changed them; those values were not recorded, and the desktop backend does not provide effective per-device readback for all controls.

## Verification

Storage tests cover old-file compatibility, first saves, changed saves, unchanged retries, reversible restoration, malformed history, invalid settings, and combined size limits. UI tests exercise confirmation/cancellation, complete profile and startup-preference restoration, storage failure with unsaved edits, and disconnected apply retries. Native screenshots use temporary files and never apply settings to the desktop.
