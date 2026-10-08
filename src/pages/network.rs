mod details;
mod editor;
mod ip;
mod list;
mod model;
#[cfg(test)]
mod tests;
mod wifi;

use crate::{
    components::{
        controls::{control, page_content},
        entry::{Entry, EntryValue},
    },
    controllers::NetworkController,
    theme::{ACCENT, CARD, MUTED, TEXT},
};
use model::*;
use telorgon::{app::*, network::*};

fn ui_label(value: impl ToString, size: f32, color: ColorRgba8) -> Text {
    text(value).size(size).color(color).width(Dimension::FILL)
}
#[derive(Clone, Copy, Default, PartialEq)]
enum Tab {
    #[default]
    Overview,
    Wifi,
    Addresses,
    Routes,
    Device,
}

#[component(no_default)]
pub struct NetworkPage {
    #[input]
    controller: NetworkController,
    #[state]
    selected: Option<NetworkInterfaceId>,
    #[state]
    editor: EditorState,
    #[state]
    tab: Tab,
    #[state]
    browsing: bool,
    #[state]
    search: EntryValue,
}
impl NetworkPage {
    pub fn new(controller: NetworkController) -> Self {
        Self {
            controller,
            selected: None,
            editor: Default::default(),
            tab: Tab::Overview,
            browsing: true,
            search: EntryValue::new("", false),
        }
    }
    fn select(&mut self, id: NetworkInterfaceId) {
        self.selected = Some(id);
        self.browsing = false;
        self.tab = Tab::Overview;
        self.editor.set(None);
    }
    fn selected<'a>(&self, snapshot: &'a NetworkSnapshot) -> Option<&'a NetworkInterface> {
        match self.selected {
            Some(id) => snapshot
                .interfaces
                .iter()
                .find(|interface| interface.id == id),
            None => snapshot
                .interfaces
                .iter()
                .find(|interface| interface.state == NetworkConnectionState::Connected)
                .or_else(|| snapshot.interfaces.first()),
        }
    }
}
impl Component for NetworkPage {
    fn view(&self) -> impl View {
        let snapshot = self.controller.snapshot(self);
        let operation = self.controller.operation(self);
        let editor = self.watch(&self.editor.signal);
        let query = self.search.observe(self).to_lowercase();
        let wide = self.viewport_size().width >= 1000.0;
        let compact = self.viewport_size().width < 750.0;
        let ready = snapshot.state == NetworkServiceState::Ready && !operation.busy;
        let mut header = column()
            .height(
                (if compact { 124.0 } else { 90.0 })
                    + if operation.message.is_empty() {
                        0.0
                    } else {
                        42.0
                    },
            )
            .gap(8.0)
            .child(
                row()
                    .height(34.0)
                    .align_items(Alignment::Center)
                    .child(ui_label("Network", 24.0, TEXT).weight(600))
                    .child(
                        ui_label(
                            format!(
                                "{} {}",
                                snapshot.interfaces.len(),
                                if snapshot.interfaces.len() == 1 {
                                    "interface"
                                } else {
                                    "interfaces"
                                }
                            ),
                            12.0,
                            MUTED,
                        )
                        .width(110.0),
                    ),
            )
            .maybe(
                compact,
                ui_label(status(&snapshot), 13.0, MUTED).height(26.0),
            )
            .child(
                row()
                    .height(40.0)
                    .gap(12.0)
                    .align_items(Alignment::Center)
                    .maybe(!compact, ui_label(status(&snapshot), 13.0, MUTED))
                    .child(
                        switch("Networking", snapshot.networking_enabled)
                            .width(145.0)
                            .height(36.0)
                            .enabled(
                                ready
                                    && snapshot.capabilities.set_networking_enabled
                                    && allowed(&snapshot, NetworkPermissionKind::EnableNetworking),
                            )
                            .on_change(|this: &mut Self, enabled| {
                                this.controller.apply(
                                    vec![NetworkCommand::SetNetworkingEnabled(enabled)],
                                    "Networking preference applied",
                                );
                            }),
                    )
                    .child(
                        switch("Wi-Fi", snapshot.wifi_enabled)
                            .width(95.0)
                            .height(36.0)
                            .enabled(
                                ready
                                    && snapshot.wifi_hardware_enabled
                                    && snapshot.capabilities.set_wifi_enabled
                                    && allowed(&snapshot, NetworkPermissionKind::EnableWifi),
                            )
                            .on_change(|this: &mut Self, enabled| {
                                this.controller.apply(
                                    vec![NetworkCommand::SetWifiEnabled(enabled)],
                                    "Wi-Fi preference applied",
                                );
                            }),
                    ),
            );
        if !operation.message.is_empty() {
            header = header.child(
                ui_label(
                    &operation.message,
                    12.0,
                    if operation.failed {
                        ColorRgba8::rgba(255, 140, 140, 255)
                    } else {
                        MUTED
                    },
                )
                .height(34.0),
            );
        }
        let interface = self.selected(&snapshot);
        let list = self.interface_list(&snapshot, &query, editor.is_none());
        let detail = if let Some(editor) = editor.as_ref() {
            let height = match editor {
                Editor::Ip(_) => ip::IpForm::height(self.viewport_size().width),
                Editor::Wifi(target) => wifi::WifiForm::height(target.hidden),
            };
            let content = match editor {
                Editor::Wifi(target) => wifi::WifiForm::new(
                    self.controller.clone(),
                    target.clone(),
                    self.editor.clone(),
                    (*operation).clone(),
                )
                .keyed(format!(
                    "wifi-editor-{:?}-{:?}",
                    target.interface, target.ap
                )),
                Editor::Ip(target) => ip::IpForm::new(
                    self.controller.clone(),
                    target.clone(),
                    self.editor.clone(),
                    (*operation).clone(),
                )
                .keyed(format!("profile-editor-{:?}", target.profile.id)),
            };
            column()
                .height(Dimension::FILL)
                .width(Dimension::FILL)
                .scrollable()
                .child(page_content(height).child(content))
                .into_element()
        } else if let Some(interface) = interface {
            self.interface_details(&snapshot, interface, wide)
                .into_element()
        } else {
            column()
                .height(Dimension::FILL)
                .gap(16.0)
                .child(
                    ui_label(
                        if self.selected.is_some() {
                            "This interface is no longer available"
                        } else {
                            "No network interfaces reported"
                        },
                        16.0,
                        TEXT,
                    )
                    .height(44.0),
                )
                .child(control("View interfaces").on_press(|this: &mut Self| {
                    this.selected = None;
                    this.tab = Tab::Overview;
                    this.browsing = true;
                }))
                .into_element()
        };
        let body = if wide {
            row()
                .width(Dimension::FILL)
                .height(Dimension::FILL)
                .gap(20.0)
                .child(list.width(220.0))
                .child(detail)
                .into_element()
        } else if self.browsing && editor.is_none() {
            list.into_element()
        } else {
            column()
                .height(Dimension::FILL)
                .gap(12.0)
                .child(
                    control("‹ All interfaces")
                        .enabled(editor.is_none())
                        .on_press(|this: &mut Self| this.browsing = true),
                )
                .child(detail)
                .into_element()
        };
        column()
            .width(Dimension::FILL)
            .height(Dimension::FILL)
            .gap(16.0)
            .child(header)
            .child(body)
    }
}
