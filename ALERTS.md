# Usage alerts on 2.23.0

This custom build is based on CodeZeno's exact v2.23.0 release,
commit dea72c9d48cfe1ba4c5ebd0265e1fa005e7d2053. It adds usage threshold
notifications while retaining upstream's taskbar collision handling and account model.

## Automatic floating when the taskbar fills

Leave Settings > Display > Lock in taskbar off (the Floating state).
Upstream detects overlapping app buttons or an authored layout that cannot fit,
moves the widget into a floating card, and restores it when the saved dock position
has enough room. Return clearance prevents repeated docking and undocking.

Intentional floating placements stay floating. Enabling Lock in taskbar suppresses
automatic floating caused by application buttons. Existing themes and placements follow
the 2.23.0 migration and positioning rules; the alert feature does not change them.

## Usage notifications

Settings > Usage alerts controls:

- Desktop notifications, enabled by default.
- Session and weekly/long-term limits independently.
- Individual 80%, 90% and 95% threshold checkboxes.
- A Send test notification button that does not consume real thresholds.

Set General > Update frequency to 1 minute for timely warnings. Alerts are evaluated
after successful polls while the monitor is running. They use consumed quota, even if
the widget is configured to display remaining quota.

Each enabled account has independent alert history. Failed accounts, stale readings,
unavailable windows and expired windows cannot trigger alerts. A jump across several
thresholds sends one warning for the highest crossed threshold. Simultaneous warnings
are combined into one native Windows notification.

History survives restarts. A new reset timestamp rearms the window; providers without
reset timestamps rearm below all selected thresholds. Existing default Claude and local
Codex histories migrate from the previous custom build. Custom account configuration
uses the upstream 2.23.0 credential-profile model.

Windows controls banner visibility and sound, including Do not disturb. Explorer accepting
a notification is not proof that a banner was displayed. Rejected notifications retry on
the next poll. History-write failures preserve in-memory deduplication and are logged
when diagnostics are enabled, but a restart can repeat a warning after a write failure.
New specialist alert labels use English in locales without a reviewed translation.

## Saved configuration

The existing settings file keeps the backwards-compatible usage_alerts object:

    "usage_alerts": {
      "enabled": true,
      "session": true,
      "weekly": true,
      "thresholds": [80, 90, 95]
    }

Advanced JSON thresholds from 1 to 100 are supported; values are sorted and deduplicated.
An empty list disables threshold alerts. History is stored separately in
usage-alert-state.json beside settings and contains no credentials.

## Build and checks

Use the upstream Windows development setup in CONTRIBUTING.md. The Rust 1.95.0 project
toolchain and the 2.23.0 dependency lock file remain unchanged. No dependencies were added
for alerts. Run cargo fmt --all -- --check, cargo test --locked, and
cargo build --locked --release.

Local validation uses isolated Rust 1.99.1 GNU with LLVM-MinGW 20261104 MSVCRT.
The upstream winresource version links the Windows resource object directly, so the
older winres-specific workaround is unnecessary. Regression tests load the embedded
compatibility manifest and application icons from the compiled executable.

Keep the custom EXE in its intended permanent location. If Start with Windows still points
to a different executable, switch it off and on from the custom build. Accepting an
upstream update can replace the custom alert feature; automatic update checks alone do
not install an update.

## Validation for this port

- Formatting check passed.
- Full suite: 632 passed, 6 upstream integration tests ignored by default.
- The ignored live taskbar occupancy test was also run separately and passed.
- Release EXE reports 2.23.0 and retains all seven Windows resources.
- Live desktop: the existing full taskbar triggered automatic floating after startup.
- Live polling succeeded and Windows accepted the test notification from the dashboard.
- Return-to-taskbar clearance and capacity handling passed the upstream regression tests.

The other five ignored checks require external HTTPS/WinGet services or an authenticated
Claude desktop integration and were not needed to validate this port. Banner visibility
and sound remain controlled by Windows notification settings.
