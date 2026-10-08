use super::*;
impl NetworkPage {
    pub(super) fn interface_list(
        &self,
        snapshot: &NetworkSnapshot,
        query: &str,
        enabled: bool,
    ) -> Container {
        let mut content = Vec::new();
        let mut height = 0.0;
        for (group_id, title) in [
            (0, "Physical interfaces"),
            (1, "Virtual & other interfaces"),
            (2, "Loopback"),
        ] {
            let mut devices: Vec<_> = snapshot
                .interfaces
                .iter()
                .filter(|device| interface_group(device) == group_id)
                .filter(|device| {
                    format!(
                        "{} {} {}",
                        device.name,
                        device.device.display_name.as_deref().unwrap_or(""),
                        device_type(device)
                    )
                    .to_lowercase()
                    .contains(query)
                })
                .collect();
            devices.sort_by(|a, b| a.name.cmp(&b.name));
            if devices.is_empty() {
                continue;
            }
            content.push(ui_label(title, 11.0, MUTED).height(24.0).into_element());
            height += 30.0;
            for device in devices {
                let id = device.id;
                let selected = self
                    .selected(snapshot)
                    .is_some_and(|current| current.id == id);
                let icon = match device.kind {
                    NetworkInterfaceKind::Wifi => crate::assets::icons::NETWORK_WIFI_HIGH,
                    NetworkInterfaceKind::Ethernet => crate::assets::icons::NETWORK_WIRED,
                    _ => crate::assets::icons::NETWORK,
                };
                content.push(
                    button()
                        .accessible_label(format!("Select {}", device.name))
                        .width(Dimension::FILL)
                        .height(80.0)
                        .padding(10.0)
                        .corner_radius(8.0)
                        .background(if selected {
                            ACCENT.with_alpha(110)
                        } else {
                            CARD
                        })
                        .uniform_border(1.0, if selected { ACCENT } else { crate::theme::LINE })
                        .enabled(enabled)
                        .child(
                            row()
                                .gap(10.0)
                                .align_items(Alignment::Center)
                                .child(image(icon).width(22.0).height(22.0).tint(TEXT))
                                .child(
                                    column()
                                        .width(Dimension::FILL)
                                        .height(60.0)
                                        .gap(2.0)
                                        .child(
                                            ui_label(
                                                device
                                                    .device
                                                    .display_name
                                                    .as_deref()
                                                    .unwrap_or(&device.name),
                                                14.0,
                                                TEXT,
                                            )
                                            .height(20.0),
                                        )
                                        .child(
                                            ui_label(
                                                format!(
                                                    "{} · {}",
                                                    device_type(device),
                                                    device.name
                                                ),
                                                11.0,
                                                MUTED,
                                            )
                                            .height(16.0),
                                        )
                                        .child(
                                            ui_label(
                                                interface_status(device),
                                                11.0,
                                                if device.state == NetworkConnectionState::Connected
                                                {
                                                    ColorRgba8::rgba(115, 210, 160, 255)
                                                } else {
                                                    MUTED
                                                },
                                            )
                                            .height(18.0),
                                        ),
                                ),
                        )
                        .on_press(move |this: &mut Self| this.select(id))
                        .into_element(),
                );
                height += 86.0;
            }
        }
        if content.is_empty() {
            content.push(
                ui_label(
                    if query.is_empty() {
                        "No interfaces reported"
                    } else {
                        "No matching interfaces"
                    },
                    13.0,
                    MUTED,
                )
                .height(48.0)
                .into_element(),
            );
            height = 48.0;
        }
        column()
            .height(Dimension::FILL)
            .width(Dimension::FILL)
            .gap(12.0)
            .child(Entry::new(
                "Search interfaces",
                "Name or type",
                self.search.clone(),
                enabled,
            ))
            .child(
                column()
                    .height(Dimension::FILL)
                    .width(Dimension::FILL)
                    .scrollable()
                    .child(page_content(height).gap(6.0).children(content)),
            )
    }
}
