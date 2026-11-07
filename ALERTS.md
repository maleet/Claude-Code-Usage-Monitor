# Usage threshold notifications

Based on upstream main commit 8fe1659 (package version 2.3.2).

## Behaviour

- Settings > Usage alerts enables native Windows notifications (enabled by default).
- Default thresholds: 80%, 90%, 95%. Each checkbox can be changed separately.
- Session and weekly/long-term alerts have independent switches.
- Alerts apply to enabled providers and visible Codex accounts independently.
- Only fresh successful usage polls are evaluated; failed Codex accounts are skipped.
- Starting above a threshold sends a warning on the first successful refresh.
- A jump across several thresholds sends one warning for the highest crossed threshold.
- Simultaneous warnings are combined to avoid replacing each other's Windows balloons.
- Sent thresholds are saved separately from settings and remain consumed after restart.
- A new reset timestamp rearms that window. Expired and older windows are ignored.
- Providers without reset timestamps rearm when usage falls below all selected thresholds.
- If a provider stops returning a previously known timestamp, the previous window identity is retained.
- The Send test notification button does not consume real thresholds.
- The existing authentication-error notifications remain unchanged.

Notifications use the app's existing Windows balloon mechanism. Windows controls their
visibility and sound; acceptance by Explorer does not prove that a banner was seen.
Do not disturb may suppress banners. The app must be running. Set General > Update
frequency to Every minute for timely warnings; the existing 15-minute default is unchanged.
New alert labels/messages use English and fall back to English in other UI languages.

## Configuration

The existing settings file gains a backwards-compatible `usage_alerts` object:

```json
{
  "usage_alerts": {
    "enabled": true,
    "session": true,
    "weekly": true,
    "thresholds": [80, 90, 95]
  }
}
```

Advanced thresholds from 1 to 100 can be entered in JSON while the monitor and dashboard
are closed. Values are sorted, deduplicated and invalid values removed on load/save.
An empty list disables threshold alerts. Alert history is in `usage-alert-state.json`
beside the existing settings file. It stores window identifiers and sent percentages,
never credentials. Persistence/notification failures are recorded when diagnostic logging
is enabled. If Windows rejects a balloon, the next successful poll retries it. If saving
history fails, in-memory deduplication still works but a restart can repeat a warning.

## Build and validate on Windows

Use the upstream Rust/MSVC setup (Rust stable, Visual Studio C++ Build Tools and Windows SDK).
Run `cargo test alerts::tests` to execute the focused alert tests; this compiles the
Windows app and its existing build-time font and resource tooling. Then use
`cargo build --release` to produce the executable in the existing target/release directory.

Do not replace the installed version until the build and notification smoke test pass.
Close the existing monitor before launching a locally built executable. Open Settings >
Usage alerts > Send test notification, check both switches and the selected thresholds,
and use Refresh now to check any already-high live usage. Restart to check deduplication.
The portable app shares existing settings with the installed app. Accepting an upstream
update from the custom build can replace it and remove this feature until upstream
includes the change. Automatic update checks themselves do not install an update.
If Start with Windows still points at the installed upstream executable, switch it off
and back on from the custom build to register the custom executable's location. Keep
the custom executable in its intended permanent location before doing this.

## Validation status

All 171 Rust tests passed, including eight focused alert tests, two compiled-resource
regression tests and a settings migration
regression test. Tests cover thresholds, startup above limits, persistence round trips,
resets, stale windows, missing reset timestamps, invalid inputs, disabled alerts,
independent usage windows and separate Codex accounts. Changed Rust files pass rustfmt.

Local validation uses isolated Rust 1.99.1 (`x86_64-pc-windows-gnu`) with LLVM-MinGW
20261104 MSVCRT. GCC runtime archives come from the same Rust toolchain, in a separate
library search directory so the MinGW CRT versions are not mixed. Cargo uses the unchanged
lock file. No new project dependencies were added. The existing rendering test expects a
manifest-local target directory for preview PNGs, even when CARGO_TARGET_DIR is redirected;
creating that directory resolves its initial missing-directory failure.

The initial GNU/LLD release omitted the compiled Windows resources because the linker
discarded winres's unreferenced static archive. Without the compatibility manifest,
UpdateLayeredWindow failed with error 0x80070057 for the taskbar child window. GNU builds
now link resource.o directly. Regression tests load the actual embedded manifest and
application icons from the compiled test executable; both tests fail before the fix
and pass afterwards. The release executable must also contain the seven icon, version,
and manifest resources.

The corrected release contains all seven resources. It was launched with the existing
user configuration: polling succeeded and the former UpdateLayeredWindow error no longer
appeared. The settings UI was visually inspected and Windows accepted a test notification.
The test button resynchronises tray icons before sending, including in --no-poll mode.
Explorer accepting a notification does not prove the banner was displayed; Windows
notification settings still control banner visibility and sound.
