use super::*;
use crate::components::network_value::ValueRow;

fn detail_content(rows: Vec<Element>, height: f32) -> Container {
    page_content(height).gap(8.0).children(rows)
}
impl NetworkPage {
    pub(super) fn interface_details(
        &self,
        snapshot: &NetworkSnapshot,
        interface: &NetworkInterface,
        wide: bool,
    ) -> Container {
        let id = interface.id;
        let ready = snapshot.state == NetworkServiceState::Ready
            && !self.controller.model.operation.snapshot().busy;
        let enabled = ready
            && interface.managed
            && allowed(snapshot, NetworkPermissionKind::ControlConnections);
        let mut summary = column()
            .height(158.0)
            .gap(8.0)
            .padding(12.0)
            .corner_radius(10.0)
            .background(CARD)
            .child(
                ui_label(
                    interface
                        .device
                        .display_name
                        .as_deref()
                        .unwrap_or(&interface.name),
                    20.0,
                    TEXT,
                )
                .weight(600)
                .height(26.0),
            )
            .child(
                ui_label(
                    format!(
                        "{} · {} · {}",
                        device_type(interface),
                        interface.name,
                        interface_status(interface)
                    ),
                    12.0,
                    MUTED,
                )
                .height(22.0),
            )
            .child(
                ui_label(
                    if interface.active_connection.is_some() {
                        active_name(snapshot, interface)
                    } else {
                        "No active managed connection".into()
                    },
                    14.0,
                    TEXT,
                )
                .height(24.0),
            );
        let active = active_profile(snapshot, interface).cloned();
        let mut actions = row().height(36.0).gap(8.0);
        if let Some(profile) = active {
            let target = IpTarget {
                interface: id,
                profile,
            };
            actions = actions.child(
                control("Connection settings")
                    .enabled(
                        enabled
                            && snapshot.capabilities.edit_profiles
                            && interface.capabilities.configure_ip,
                    )
                    .on_press(move |this: &mut Self| {
                        this.editor.set(Some(Editor::Ip(target.clone())))
                    }),
            );
        }
        actions = actions.child(
            control(if interface.active_connection.is_some() {
                "Disconnect"
            } else {
                "Connections"
            })
            .enabled(if interface.active_connection.is_some() {
                enabled && interface.capabilities.disconnect
            } else {
                true
            })
            .on_press(move |this: &mut Self| {
                let snapshot = this.controller.model.handle.signal().snapshot();
                if snapshot
                    .interfaces
                    .iter()
                    .find(|device| device.id == id)
                    .is_some_and(|device| device.active_connection.is_some())
                {
                    this.controller.apply(
                        vec![NetworkCommand::Disconnect {
                            interface: id,
                            policy: DisconnectPolicy::BlockAutoconnect,
                        }],
                        "Disconnected",
                    );
                } else {
                    this.tab = if snapshot
                        .interfaces
                        .iter()
                        .find(|device| device.id == id)
                        .is_some_and(|device| device.kind == NetworkInterfaceKind::Wifi)
                    {
                        Tab::Wifi
                    } else {
                        Tab::Overview
                    };
                }
            }),
        );
        summary = summary.child(actions);
        let mut tabs = vec![(Tab::Overview, "Overview")];
        if interface.kind == NetworkInterfaceKind::Wifi {
            tabs.push((Tab::Wifi, "Wi-Fi"));
        }
        tabs.extend([
            (Tab::Addresses, "Addresses & DNS"),
            (Tab::Routes, "Routes"),
            (Tab::Device, "Device"),
        ]);
        let tab = if self.tab == Tab::Wifi && interface.kind != NetworkInterfaceKind::Wifi {
            Tab::Overview
        } else {
            self.tab
        };
        let available = (self.viewport_size().width - if wide { 497.0 } else { 257.0 }).max(120.0);
        let columns = ((available + 6.0) / 151.0).floor().max(1.0) as usize;
        let height = tabs.len().div_ceil(columns) as f32 * 42.0 - 6.0;
        let mut tab_buttons = column().gap(6.0).height(height);
        for group in tabs.chunks(columns) {
            let mut buttons = row().height(36.0).gap(6.0);
            for &(value, title) in group {
                buttons = buttons.child(
                    control(title)
                        .width(available.min(145.0))
                        .accessible_label(format!("Network tab: {title}"))
                        .background(if tab == value { ACCENT } else { CARD })
                        .on_press(move |this: &mut Self| this.tab = value),
                );
            }
            tab_buttons = tab_buttons.child(buttons);
        }
        let content = match tab {
            Tab::Wifi => self.wifi_connections(snapshot, interface, enabled),
            Tab::Overview => self.overview(snapshot, interface),
            Tab::Addresses => self.address_details(snapshot, interface),
            Tab::Routes => self.route_details(interface),
            Tab::Device => self.device_details(snapshot, interface),
        };
        column()
            .width(Dimension::FILL)
            .height(Dimension::FILL)
            .gap(12.0)
            .child(summary)
            .child(tab_buttons)
            .child(
                column()
                    .width(Dimension::FILL)
                    .height(Dimension::FILL)
                    .scrollable()
                    .key(format!("network-content-{:?}-{}", id, tab as u8))
                    .child(content),
            )
    }
    pub(super) fn value(&self, key: &str, value: impl ToString) -> Element {
        ValueRow::new(self.controller.clone(), key, value.to_string()).into_element()
    }
    fn overview(&self, snapshot: &NetworkSnapshot, interface: &NetworkInterface) -> Container {
        let mut rows = vec![
            ui_label("Connection overview", 16.0, TEXT)
                .height(28.0)
                .into_element(),
            self.value("Connection", active_name(snapshot, interface)),
            self.value("State", interface_status(interface)),
        ];
        if let Some(ap) = interface
            .access_points
            .iter()
            .find(|ap| Some(ap.id) == interface.active_access_point)
        {
            rows.push(self.value("Wi-Fi signal", format!("{}%", ap.strength)));
            rows.push(self.value("Security", security(ap.security)));
        }
        if let Some(speed) = interface.device.speed_mbps {
            rows.push(self.value("Link speed", format!("{speed} Mbps")));
        }
        for (title, ip) in [
            ("IPv4 address", &interface.ipv4),
            ("IPv6 address", &interface.ipv6),
        ] {
            for address in &ip.addresses {
                rows.push(self.value(title, format!("{}/{}", address.address, address.prefix)));
            }
        }
        if interface.ipv4.addresses.is_empty() && interface.ipv6.addresses.is_empty() {
            rows.push(self.value("Addresses", "None assigned"));
        }
        if let Some(failure) = &interface.failure {
            rows.push(self.value("Failure", crate::services::error_message(failure)));
        }
        if !interface.managed {
            rows.push(ui_label("Observed interface. Its configuration is managed elsewhere or network management is unavailable.",12.0,MUTED).height(54.0).into_element());
        }
        let height = rows.len() as f32 * 46.0 + 64.0;
        let mut content = detail_content(rows, height);
        if interface.kind != NetworkInterfaceKind::Wifi {
            let profiles = self.saved_connections(snapshot, interface);
            content = page_content(height + profiles.0 + 16.0)
                .gap(16.0)
                .child(content)
                .child(profiles.1);
        }
        content
    }
    fn address_details(
        &self,
        snapshot: &NetworkSnapshot,
        interface: &NetworkInterface,
    ) -> Container {
        let mut rows = Vec::new();
        let profile = active_profile(snapshot, interface);
        for (title, ip, configured) in [
            ("IPv4", &interface.ipv4, profile.map(|p| &p.ip.ipv4)),
            ("IPv6", &interface.ipv6, profile.map(|p| &p.ip.ipv6)),
        ] {
            rows.push(
                ui_label(format!("{title} · Live state"), 16.0, TEXT)
                    .height(28.0)
                    .into_element(),
            );
            if ip.addresses.is_empty() {
                rows.push(self.value("Addresses", "None assigned"));
            }
            for a in &ip.addresses {
                rows.push(self.value("Address / prefix", format!("{}/{}", a.address, a.prefix)));
            }
            rows.push(
                self.value(
                    "Gateway",
                    ip.gateway
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "Not reported".into()),
                ),
            );
            rows.push(self.value(
                "DNS servers",
                if ip.dns.is_empty() {
                    "Not reported".into()
                } else {
                    ip.dns
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                },
            ));
            if let Some(configured) = configured {
                rows.push(
                    ui_label(format!("{title} · Saved configuration"), 14.0, MUTED)
                        .height(28.0)
                        .into_element(),
                );
                rows.push(self.value("Method", ip_method(&configured.method)));
                if let IpMethod::Static(addresses) = &configured.method {
                    for a in addresses {
                        rows.push(
                            self.value("Configured address", format!("{}/{}", a.address, a.prefix)),
                        );
                    }
                }
                rows.push(
                    self.value(
                        "Configured gateway",
                        configured
                            .gateway
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "Automatic / none".into()),
                    ),
                );
                rows.push(self.value(
                    "Automatic DNS",
                    if configured.ignore_automatic_dns {
                        "Ignored"
                    } else {
                        "Used"
                    },
                ));
                for dns in &configured.dns {
                    rows.push(self.value("Extra DNS server", dns));
                }
            }
        }
        let height = rows.len() as f32 * 46.0;
        detail_content(rows, height)
    }
    fn route_details(&self, interface: &NetworkInterface) -> Container {
        let mut rows = vec![
            ui_label("Live routing tables", 16.0, TEXT)
                .height(28.0)
                .into_element(),
        ];
        for (family, ip) in [("IPv4", &interface.ipv4), ("IPv6", &interface.ipv6)] {
            rows.push(ui_label(family, 14.0, MUTED).height(24.0).into_element());
            if ip.routes.is_empty() {
                rows.push(self.value("Routes", "None reported"));
            }
            for route in &ip.routes {
                rows.push(self.value(
                    if route.destination.prefix == 0 {
                        "Default route"
                    } else {
                        "Destination"
                    },
                    format!("{}/{}", route.destination.address, route.destination.prefix),
                ));
                rows.push(self.value(
                    "Gateway / metric",
                    format!(
                            "{} · {}",
                            route
                                .gateway
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "On-link".into()),
                            route
                                .metric
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "Unspecified".into())
                        ),
                ));
            }
        }
        let height = rows.len() as f32 * 46.0;
        detail_content(rows, height)
    }
    fn device_details(
        &self,
        snapshot: &NetworkSnapshot,
        interface: &NetworkInterface,
    ) -> Container {
        let device = &interface.device;
        let bool_text = |value: Option<bool>| match value {
            Some(true) => "Yes",
            Some(false) => "No",
            None => "Not reported",
        };
        let name = |id: Option<NetworkInterfaceId>| {
            id.and_then(|id| snapshot.interfaces.iter().find(|d| d.id == id))
                .map(|d| d.name.as_str())
                .unwrap_or("None / not reported")
        };
        let mut rows = vec![
            ui_label("Device information", 16.0, TEXT)
                .height(28.0)
                .into_element(),
            self.value("Interface", &interface.name),
            self.value(
                "Interface index",
                device
                    .index
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "Not reported".into()),
            ),
            self.value("Type", device_type(interface)),
            self.value("Managed", if interface.managed { "Yes" } else { "No" }),
            self.value(
                "Management backend",
                snapshot.backend.as_deref().unwrap_or("Not available"),
            ),
            self.value(
                "MAC address",
                device.mac_address.as_deref().unwrap_or("Not reported"),
            ),
            self.value(
                "MTU",
                device
                    .mtu
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "Not reported".into()),
            ),
            self.value("Administrative up", bool_text(device.administrative_up)),
            self.value("Carrier", bool_text(device.carrier)),
            self.value(
                "Operational state",
                device
                    .operational_state
                    .as_deref()
                    .unwrap_or("Not reported"),
            ),
            self.value(
                "Link speed",
                device
                    .speed_mbps
                    .map(|v| format!("{v} Mbps"))
                    .unwrap_or_else(|| "Not reported".into()),
            ),
            self.value("Driver", device.driver.as_deref().unwrap_or("Not reported")),
            self.value("Parent interface", name(device.parent)),
            self.value("Master interface", name(device.master)),
        ];
        if let Some(ap) = interface
            .access_points
            .iter()
            .find(|ap| Some(ap.id) == interface.active_access_point)
        {
            rows.push(self.value("BSSID", &ap.bssid));
            rows.push(self.value("Frequency", format!("{} MHz", ap.frequency_mhz)));
        }
        rows.push(
            ui_label("Traffic counters · Since interface creation", 14.0, MUTED)
                .height(28.0)
                .into_element(),
        );
        if let Some(traffic) = &device.traffic {
            for (key, value) in [
                ("Received bytes", traffic.received_bytes),
                ("Transmitted bytes", traffic.transmitted_bytes),
                ("Received packets", traffic.received_packets),
                ("Transmitted packets", traffic.transmitted_packets),
                ("Receive errors", traffic.receive_errors),
                ("Transmit errors", traffic.transmit_errors),
                ("Receive drops", traffic.receive_drops),
                ("Transmit drops", traffic.transmit_drops),
            ] {
                rows.push(self.value(key, value));
            }
        } else {
            rows.push(self.value("Traffic counters", "Not reported"));
        }
        let height = rows.len() as f32 * 46.0;
        detail_content(rows, height)
    }
}
