use super::*;
impl NetworkPage {
    pub(super) fn saved_connections(
        &self,
        snapshot: &NetworkSnapshot,
        interface: &NetworkInterface,
    ) -> (f32, Container) {
        let busy = self.controller.model.operation.snapshot().busy;
        let enabled = snapshot.state == NetworkServiceState::Ready
            && !busy
            && interface.managed
            && allowed(snapshot, NetworkPermissionKind::ControlConnections);
        let mut rows = vec![
            ui_label("Saved connections", 16.0, TEXT)
                .height(28.0)
                .into_element(),
        ];
        let mut height = 36.0;
        for profile in snapshot.profiles.iter().filter(|profile| {
            profile.kind == interface.kind
                && profile
                    .interface_name
                    .as_ref()
                    .is_none_or(|name| name == &interface.name)
        }) {
            let target = IpTarget {
                interface: interface.id,
                profile: profile.clone(),
            };
            let id = profile.id;
            let device = interface.id;
            let active = active_profile(snapshot, interface).is_some_and(|active| active.id == id);
            rows.push(
                column()
                    .height(100.0)
                    .padding(12.0)
                    .gap(8.0)
                    .corner_radius(8.0)
                    .background(CARD)
                    .child(
                        ui_label(
                            format!(
                                "{}{}",
                                profile.name,
                                if active { " · Connected" } else { "" }
                            ),
                            14.0,
                            TEXT,
                        )
                        .height(28.0),
                    )
                    .child(
                        row()
                            .height(36.0)
                            .gap(8.0)
                            .child(
                                control(if active { "Connected" } else { "Connect" })
                                    .enabled(
                                        enabled
                                            && !active
                                            && interface.capabilities.activate_profile
                                            && snapshot.networking_enabled,
                                    )
                                    .on_press(move |this: &mut Self| {
                                        this.controller.apply(
                                            vec![NetworkCommand::ActivateProfile {
                                                interface: device,
                                                profile: id,
                                            }],
                                            "Connection activated",
                                        );
                                    }),
                            )
                            .child(
                                control("Settings")
                                    .enabled(
                                        enabled
                                            && snapshot.capabilities.edit_profiles
                                            && interface.capabilities.configure_ip,
                                    )
                                    .on_press(move |this: &mut Self| {
                                        this.editor.set(Some(Editor::Ip(target.clone())))
                                    }),
                            ),
                    )
                    .into_element(),
            );
            height += 108.0;
        }
        if height == 36.0 {
            rows.push(
                ui_label("No saved connections for this interface", 13.0, MUTED)
                    .height(40.0)
                    .into_element(),
            );
            height += 48.0;
        }
        (height, page_content(height).gap(8.0).children(rows))
    }
    pub(super) fn wifi_connections(
        &self,
        snapshot: &NetworkSnapshot,
        interface: &NetworkInterface,
        enabled: bool,
    ) -> Container {
        let id = interface.id;
        let ready = enabled
            && snapshot.networking_enabled
            && snapshot.wifi_enabled
            && snapshot.wifi_hardware_enabled;
        let mut rows = vec![
            row()
                .height(36.0)
                .gap(8.0)
                .child(ui_label("Nearby Wi-Fi networks", 16.0, TEXT))
                .child(
                    control("Refresh")
                        .width(100.0)
                        .enabled(
                            ready
                                && interface.capabilities.scan_wifi
                                && allowed(snapshot, NetworkPermissionKind::ScanWifi),
                        )
                        .on_press(move |this: &mut Self| {
                            this.controller
                                .apply(vec![NetworkCommand::ScanWifi(id)], "Wi-Fi scan complete");
                        }),
                )
                .into_element(),
        ];
        let mut height = 44.0;
        let message = if !interface.managed {
            Some("This interface is managed elsewhere")
        } else if snapshot.state != NetworkServiceState::Ready {
            Some("Network management is unavailable")
        } else if !snapshot.wifi_hardware_enabled {
            Some("Wi-Fi is blocked by the hardware switch")
        } else if !snapshot.wifi_enabled {
            Some("Wi-Fi is off")
        } else if !snapshot.networking_enabled {
            Some("Networking is off")
        } else if interface.access_points.is_empty() {
            Some("No networks reported. Refresh to scan")
        } else {
            None
        };
        if let Some(message) = message {
            rows.push(ui_label(message, 13.0, MUTED).height(48.0).into_element());
            height += 56.0;
        } else {
            for ap in nearby(interface) {
                let connected = interface.state == NetworkConnectionState::Connected
                    && Some(ap.id) == interface.active_access_point;
                let target = WifiTarget {
                    interface: id,
                    ap: Some(ap.id),
                    ssid: ap.ssid.clone(),
                    security: ap.security,
                    hidden: false,
                };
                rows.push(
                    column()
                        .height(138.0)
                        .padding(12.0)
                        .gap(6.0)
                        .corner_radius(8.0)
                        .background(CARD)
                        .child(
                            row()
                                .height(36.0)
                                .gap(8.0)
                                .child(ui_label(name(ap.ssid.as_ref()), 14.0, TEXT))
                                .child(
                                    control(if connected { "Connected" } else { "Connect" })
                                        .width(100.0)
                                        .enabled(
                                            ready
                                                && !connected
                                                && ap.ssid.is_some()
                                                && can_join(interface, ap.security),
                                        )
                                        .on_press(move |this: &mut Self| {
                                            this.editor.set(Some(Editor::Wifi(target.clone())))
                                        }),
                                ),
                        )
                        .child(
                            ui_label(
                                format!(
                                    "{}% signal · {} · {} MHz",
                                    ap.strength.min(100),
                                    security(ap.security),
                                    ap.frequency_mhz
                                ),
                                12.0,
                                MUTED,
                            )
                            .height(24.0),
                        )
                        .child(self.value("BSSID", &ap.bssid))
                        .into_element(),
                );
                height += 146.0;
            }
        }
        let target = WifiTarget {
            interface: id,
            ap: None,
            ssid: None,
            security: WifiSecurity::WpaPersonal,
            hidden: true,
        };
        rows.push(
            control("Join a hidden network")
                .enabled(ready && interface.capabilities.connect_wifi)
                .on_press(move |this: &mut Self| {
                    this.editor.set(Some(Editor::Wifi(target.clone())))
                })
                .into_element(),
        );
        height += 44.0;
        let profiles = self.saved_connections(snapshot, interface);
        page_content(height + profiles.0 + 16.0)
            .gap(16.0)
            .child(page_content(height).gap(8.0).children(rows))
            .child(profiles.1)
    }
}
