use super::*;

impl StudioApp {
    pub(super) fn settings_page(&mut self, ui: &mut egui::Ui) {
        let language = self.language();
        let mut changed = false;
        let mut requested_theme = None;
        let mut open_theme_studio = false;
        let themes = available_themes(self.theme_path.as_deref(), &self.theme);
        if let Some(error) = &self.settings_error {
            ui.colored_label(egui::Color32::from_rgb(196, 64, 64), error);
            ui.add_space(8.0);
        }
        settings_scroll_area(ui, |ui| {
            section(ui, language.text("General"), |ui| {
                setting_row(
                    ui,
                    language.text("Update frequency"),
                    language.text("How often provider usage is refreshed"),
                    |ui| {
                        Dropdown::from_id_salt("poll_interval")
                            .width(220.0)
                            .selected_text(interval_name(language, self.settings.poll_interval_ms))
                            .show_ui(ui, |ui| {
                                changed |= dropdown_selectable_value(
                                    ui,
                                    &mut self.settings.poll_interval_ms,
                                    POLL_1_MIN,
                                    language.text("Every minute"),
                                )
                                .changed();
                                changed |= dropdown_selectable_value(
                                    ui,
                                    &mut self.settings.poll_interval_ms,
                                    POLL_5_MIN,
                                    language.text("Every 5 minutes"),
                                )
                                .changed();
                                changed |= dropdown_selectable_value(
                                    ui,
                                    &mut self.settings.poll_interval_ms,
                                    POLL_15_MIN,
                                    language.text("Every 15 minutes"),
                                )
                                .changed();
                                changed |= dropdown_selectable_value(
                                    ui,
                                    &mut self.settings.poll_interval_ms,
                                    POLL_1_HOUR,
                                    language.text("Every hour"),
                                )
                                .changed();
                            });
                        if ui.button(language.text("Refresh now")).clicked() {
                            self.post_owner(WM_APP_REFRESH_NOW);
                        }
                    },
                );
                setting_separator(ui);
                setting_row(
                    ui,
                    language.text("Start with Windows"),
                    language.text("Launch the monitor when you sign in"),
                    |ui| {
                        if Toggle::new(&mut self.startup_enabled)
                            .labels(language.text("Enabled"), language.text("Disabled"))
                            .show(ui)
                            .changed()
                        {
                            crate::window::set_startup_enabled(self.startup_enabled);
                        }
                    },
                );
            });
            section(ui, language.text("Usage alerts"), |ui| {
                setting_row(
                    ui,
                    language.text("Desktop notifications"),
                    language.text("Warn when usage reaches a selected threshold"),
                    |ui| {
                        changed |= Toggle::new(&mut self.settings.usage_alerts.enabled)
                            .labels(language.text("Enabled"), language.text("Disabled"))
                            .show(ui)
                            .changed();
                    },
                );
                setting_separator(ui);
                ui.add_enabled_ui(self.settings.usage_alerts.enabled, |ui| {
                    setting_row(
                        ui,
                        language.text("Usage windows"),
                        language.text("Check session and long-term limits independently"),
                        |ui| {
                            changed |= ui
                                .checkbox(&mut self.settings.usage_alerts.session, "5h")
                                .changed();
                            changed |= ui
                                .checkbox(&mut self.settings.usage_alerts.weekly, "7d / long-term")
                                .changed();
                        },
                    );
                    setting_separator(ui);
                    setting_row(
                        ui,
                        language.text("Alert thresholds"),
                        language.text("Once per threshold and window, remembered across restarts"),
                        |ui| {
                            ui.horizontal(|ui| {
                                // Setting controls flow from right to left.
                                for threshold in [95u8, 90, 80] {
                                    let mut selected =
                                        self.settings.usage_alerts.thresholds.contains(&threshold);
                                    if ui
                                        .checkbox(&mut selected, format!("{threshold}%"))
                                        .changed()
                                    {
                                        changed = true;
                                        self.settings
                                            .usage_alerts
                                            .thresholds
                                            .retain(|value| *value != threshold);
                                        if selected {
                                            self.settings.usage_alerts.thresholds.push(threshold);
                                        }
                                        self.settings.usage_alerts.normalize();
                                    }
                                }
                            });
                        },
                    );
                });
                setting_separator(ui);
                setting_row(
                    ui,
                    language.text("Test notification"),
                    language.text("Windows controls notification banners and sound"),
                    |ui| {
                        if ui.button(language.text("Send test notification")).clicked() {
                            self.post_owner(crate::native_interop::WM_APP_TEST_USAGE_ALERT);
                        }
                    },
                );
                ui.label(
                    language.text(
                        "Alerts follow Update frequency. Use Every minute for timely warnings.",
                    ),
                );
            });
            section(ui, language.text("Providers"), |ui| {
                for (index, descriptor) in PROVIDER_DESCRIPTORS.iter().enumerate() {
                    if index > 0 {
                        setting_separator(ui);
                    }
                    setting_row(
                        ui,
                        language.text(descriptor.display_name),
                        language.text(descriptor.settings_description),
                        |ui| {
                            let mut enabled = self.settings.provider_enabled(descriptor.id);
                            if Toggle::new(&mut enabled)
                                .labels(language.text("Enabled"), language.text("Disabled"))
                                .show(ui)
                                .changed()
                            {
                                changed |= self.settings.toggle_provider(descriptor.id);
                            }
                        },
                    );
                }
            });
            changed |= self.accounts_section(ui);
            section(ui, language.text("Display"), |ui| {
                setting_row(
                    ui,
                    language.text("Language"),
                    language.text("Language used by the app and widget"),
                    |ui| {
                        let language_code = self
                            .settings
                            .language
                            .get_or_insert_with(|| "system".into());
                        Dropdown::from_id_salt("language")
                            .width(220.0)
                            .selected_text(language_name(language, language_code))
                            .show_ui(ui, |ui| {
                                for (code, name) in languages(language) {
                                    changed |= dropdown_selectable_value(
                                        ui,
                                        language_code,
                                        code.into(),
                                        name,
                                    )
                                    .changed();
                                }
                            });
                    },
                );
            });
            section(ui, language.text("Appearance"), |ui| {
                setting_row(
                    ui,
                    language.text("Active theme"),
                    language.text("The widget is managed in Theme Studio"),
                    |ui| {
                        Dropdown::from_id_salt("active_theme")
                            .width(220.0)
                            .selected_text(&self.theme.name)
                            .show_ui(ui, |ui| {
                                for theme in &themes {
                                    let selected =
                                        self.theme_path.as_deref() == Some(theme.path.as_path());
                                    if dropdown_selectable_label(ui, selected, &theme.label)
                                        .clicked()
                                    {
                                        requested_theme = Some(theme.path.clone());
                                    }
                                }
                            });
                    },
                );
                setting_separator(ui);
                setting_row(
                    ui,
                    language.text("Theme editor"),
                    language.text("Create and edit themes"),
                    |ui| {
                        if ui.button(language.text("Open Theme Studio")).clicked() {
                            open_theme_studio = true;
                        }
                    },
                );
            });
        });
        if changed {
            let new_language = self.language();
            if new_language != language {
                configure_style(ui.ctx(), new_language);
            }
            self.preview_dirty = true;
            self.save_settings();
        }
        if let Some(path) = requested_theme {
            self.request_activate_theme(path);
        }
        if open_theme_studio {
            self.page = Page::Studio;
        }
    }
}
