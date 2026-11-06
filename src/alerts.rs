//! Usage alerts are evaluated only after a fresh poll, never from the disk cache.
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::HWND;

use crate::app_settings::{self, SettingsFile};
use crate::models::{AppUsageData, UsageData, UsageSection};
use crate::providers::ProviderId;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AlertSettings {
    pub enabled: bool,
    pub session: bool,
    pub weekly: bool,
    pub thresholds: Vec<u8>,
}

impl Default for AlertSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            session: true,
            weekly: true,
            thresholds: vec![80, 90, 95],
        }
    }
}

impl AlertSettings {
    pub fn normalize(&mut self) {
        self.thresholds.retain(|value| (1..=100).contains(value));
        self.thresholds.sort_unstable();
        self.thresholds.dedup();
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct WindowState {
    reset_unix: Option<u64>,
    sent: Vec<u8>,
}

impl WindowState {
    fn evaluate(&mut self, usage: &UsageSection, thresholds: &[u8], now: u64) -> Option<u8> {
        if !usage.percentage.is_finite() || usage.percentage < 0.0 {
            return None;
        }
        let reset = usage
            .resets_at
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs());
        // An expired window is stale until the provider returns its next window.
        if reset.is_some_and(|value| value <= now) {
            return None;
        }
        match (self.reset_unix, reset) {
            (Some(previous), Some(current)) if current < previous => return None,
            (Some(previous), Some(current)) if current > previous => self.sent.clear(),
            _ => {}
        }
        if reset.is_some() {
            self.reset_unix = reset;
        }
        // Providers without reset timestamps rearm only below every enabled threshold.
        if self.reset_unix.is_none()
            && thresholds
                .iter()
                .min()
                .is_some_and(|value| usage.percentage < f64::from(*value))
        {
            self.sent.clear();
        }
        let highest = thresholds
            .iter()
            .copied()
            .filter(|value| usage.percentage >= f64::from(*value) && !self.sent.contains(value))
            .max()?;
        // A jump from 79% to 96% consumes all crossed thresholds in one notification.
        for value in thresholds.iter().copied().filter(|value| *value <= highest) {
            if !self.sent.contains(&value) {
                self.sent.push(value);
            }
        }
        Some(highest)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct AlertState {
    windows: BTreeMap<String, WindowState>,
}

impl AlertState {
    fn evaluate(&mut self, settings: &SettingsFile, data: &AppUsageData, now: u64) -> Vec<String> {
        let mut messages = Vec::new();
        if !settings.usage_alerts.enabled {
            return messages;
        }
        for provider in settings.enabled_providers().iter() {
            if provider == ProviderId::Codex && !data.account_order.is_empty() {
                for account in data.account_order.iter().filter(|account| account.visible) {
                    if let Some(entry) = data
                        .accounts
                        .get(&account.id)
                        .filter(|entry| entry.error.is_none())
                    {
                        if let Some(usage) = &entry.usage {
                            self.check_usage(
                                &format!("codex:{}", account.id),
                                &account.name,
                                usage,
                                &settings.usage_alerts,
                                now,
                                &mut messages,
                            );
                        }
                    }
                }
            } else if let Some(usage) = data.get(provider) {
                self.check_usage(
                    provider.descriptor().key,
                    provider.descriptor().display_name,
                    usage,
                    &settings.usage_alerts,
                    now,
                    &mut messages,
                );
            }
        }
        messages
    }

    fn check_usage(
        &mut self,
        key: &str,
        name: &str,
        usage: &UsageData,
        settings: &AlertSettings,
        now: u64,
        messages: &mut Vec<String>,
    ) {
        for (window, label, enabled, section) in [
            ("session", "5h", settings.session, &usage.session),
            (
                "weekly",
                usage.weekly_label.as_deref().unwrap_or("7d"),
                settings.weekly,
                &usage.weekly,
            ),
        ] {
            if !enabled {
                continue;
            }
            if let Some(threshold) = self
                .windows
                .entry(format!("{key}/{window}"))
                .or_default()
                .evaluate(section, &settings.thresholds, now)
            {
                let name: String = name
                    .chars()
                    .filter(|value| !value.is_control())
                    .take(24)
                    .collect();
                messages.push(format!(
                    "{name} {label}: {:.1}% (>= {threshold}%)",
                    section.percentage
                ));
            }
        }
    }
}

fn load_state() -> AlertState {
    let path = app_settings::app_data_directory().join("usage-alert-state.json");
    match std::fs::read(&path) {
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(state) => state,
            Err(error) => {
                crate::diagnose::log_error("Unable to decode usage alert state", error);
                AlertState::default()
            }
        },
        Err(error) => {
            if error.kind() != std::io::ErrorKind::NotFound {
                crate::diagnose::log_error("Unable to read usage alert state", error);
            }
            AlertState::default()
        }
    }
}

pub fn check(hwnd: HWND, settings: &SettingsFile, data: &AppUsageData) {
    static STATE: OnceLock<Mutex<AlertState>> = OnceLock::new();
    let mut state = STATE
        .get_or_init(|| Mutex::new(load_state()))
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut next = state.clone();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let messages = next.evaluate(settings, data, now);
    if !messages.is_empty() {
        // Keep the balloon within Windows' 255 UTF-16-unit body limit. Summarise overflow.
        let mut body = String::new();
        let mut shown = 0;
        for message in &messages {
            if body.encode_utf16().count() + message.encode_utf16().count() + 1 > 210 {
                break;
            }
            if !body.is_empty() {
                body.push('\n');
            }
            body.push_str(message);
            shown += 1;
        }
        if shown < messages.len() {
            body.push_str(&format!(
                "\n+ {} other usage alerts",
                messages.len() - shown
            ));
        }
        if !crate::tray_icon::notify_balloon(hwnd, "Usage limit warning", &body) {
            crate::diagnose::log("Windows rejected the usage notification; retrying next poll");
            return;
        }
    }
    // Retry disk persistence on every poll, but retain in-memory deduplication on failure.
    if next != *state || !messages.is_empty() {
        *state = next;
    }
    if let Err(error) = app_settings::write_json_atomic(
        &app_settings::app_data_directory().join("usage-alert-state.json"),
        &*state,
    ) {
        crate::diagnose::log_error("Unable to persist usage alert state", error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(percentage: f64, reset: Option<u64>) -> UsageSection {
        UsageSection {
            percentage,
            resets_at: reset.map(|seconds| UNIX_EPOCH + std::time::Duration::from_secs(seconds)),
        }
    }

    #[test]
    fn thresholds_fire_once_and_survive_serialization() {
        let mut state = WindowState::default();
        for (percentage, expected) in [
            (79.0, None),
            (80.0, Some(80)),
            (85.0, None),
            (90.0, Some(90)),
            (95.0, Some(95)),
            (100.0, None),
        ] {
            assert_eq!(
                state.evaluate(&usage(percentage, Some(1000)), &[80, 90, 95], 100),
                expected
            );
            state = serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        }
    }

    #[test]
    fn startup_above_thresholds_sends_only_highest() {
        let mut state = WindowState::default();
        assert_eq!(
            state.evaluate(&usage(96.0, Some(1000)), &[80, 90, 95], 100),
            Some(95)
        );
        assert_eq!(
            state.evaluate(&usage(96.0, Some(1000)), &[80, 90, 95], 100),
            None
        );
        assert_eq!(
            state.evaluate(&usage(85.0, Some(1000)), &[80, 90, 95], 100),
            None
        );
    }

    #[test]
    fn next_window_rearms_but_expired_and_older_data_do_not() {
        let mut state = WindowState::default();
        assert_eq!(
            state.evaluate(&usage(95.0, Some(1000)), &[80, 90, 95], 100),
            Some(95)
        );
        assert_eq!(
            state.evaluate(&usage(95.0, Some(900)), &[80, 90, 95], 100),
            None
        );
        assert_eq!(
            state.evaluate(&usage(95.0, Some(1000)), &[80, 90, 95], 1000),
            None
        );
        assert_eq!(
            state.evaluate(&usage(95.0, Some(2000)), &[80, 90, 95], 1001),
            Some(95)
        );
    }

    #[test]
    fn missing_reset_rearms_below_lowest_threshold() {
        let mut state = WindowState::default();
        assert_eq!(
            state.evaluate(&usage(95.0, None), &[80, 90, 95], 100),
            Some(95)
        );
        assert_eq!(state.evaluate(&usage(90.0, None), &[80, 90, 95], 100), None);
        assert_eq!(state.evaluate(&usage(10.0, None), &[80, 90, 95], 100), None);
        assert_eq!(
            state.evaluate(&usage(80.0, None), &[80, 90, 95], 100),
            Some(80)
        );
    }

    #[test]
    fn invalid_values_and_empty_thresholds_do_not_notify() {
        let mut state = WindowState::default();
        for value in [f64::NAN, f64::INFINITY, -1.0] {
            assert_eq!(
                state.evaluate(&usage(value, None), &[80, 90, 95], 100),
                None
            );
        }
        assert_eq!(state.evaluate(&usage(99.0, None), &[], 100), None);
        let mut settings = AlertSettings {
            thresholds: vec![0, 95, 80, 95, 101],
            ..Default::default()
        };
        settings.normalize();
        assert_eq!(settings.thresholds, vec![80, 95]);
    }

    #[test]
    fn session_weekly_and_provider_states_are_independent() {
        let mut state = AlertState::default();
        let settings = SettingsFile::default();
        let data: AppUsageData = [(
            ProviderId::Claude,
            UsageData {
                session: usage(95.0, Some(1000)),
                weekly: usage(90.0, Some(2000)),
                weekly_label: None,
            },
        )]
        .into_iter()
        .collect();
        assert_eq!(state.evaluate(&settings, &data, 100).len(), 2);
        assert!(state.evaluate(&settings, &data, 100).is_empty());
        let mut disabled = settings.clone();
        disabled.usage_alerts.enabled = false;
        assert!(AlertState::default()
            .evaluate(&disabled, &data, 100)
            .is_empty());
        disabled.usage_alerts.enabled = true;
        disabled.usage_alerts.weekly = false;
        assert_eq!(
            AlertState::default().evaluate(&disabled, &data, 100).len(),
            1
        );
    }

    #[test]
    fn codex_accounts_do_not_share_alert_history_or_notify_on_failed_polls() {
        let mut settings = SettingsFile::default();
        settings.set_enabled_providers(crate::providers::ProviderSet::from_enabled([
            ProviderId::Codex,
        ]));
        let mut first = crate::accounts::legacy_accounts().remove(0);
        first.id = "first".into();
        let mut second = first.clone();
        second.id = "second".into();
        second.name = "Second account".into();
        let mut data = AppUsageData::default();
        data.account_order = vec![first, second];
        let entry = crate::accounts::AccountUsage {
            usage: Some(UsageData {
                session: usage(95.0, Some(1000)),
                ..Default::default()
            }),
            ..Default::default()
        };
        data.accounts.insert("first".into(), entry.clone());
        data.accounts.insert(
            "second".into(),
            crate::accounts::AccountUsage {
                error: Some("Unable to refresh usage".into()),
                ..entry
            },
        );
        let mut state = AlertState::default();
        assert_eq!(state.evaluate(&settings, &data, 100).len(), 1);
        data.accounts.get_mut("second").unwrap().error = None;
        let messages = state.evaluate(&settings, &data, 100);
        assert_eq!(messages.len(), 1);
        assert!(messages[0].starts_with("Second account"));
        assert!(state.evaluate(&settings, &data, 100).is_empty());
        data.account_order[1].visible = false;
        assert_eq!(
            AlertState::default().evaluate(&settings, &data, 100).len(),
            1
        );
    }

    #[test]
    fn partial_settings_keep_default_thresholds() {
        let settings: AlertSettings = serde_json::from_str(r#"{"enabled":false}"#).unwrap();
        assert!(!settings.enabled);
        assert!(settings.session && settings.weekly);
        assert_eq!(settings.thresholds, vec![80, 90, 95]);
    }
}
