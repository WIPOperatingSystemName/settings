use super::{model::*, ui_label};
use crate::{
    components::{
        controls::{control, group},
        entry::{Entry, EntryValue},
    },
    controllers::NetworkController,
    theme::{ACCENT, MUTED, TEXT},
};
use telorgon::{app::*, network::*};

#[component(no_default)]
pub(super) struct WifiForm {
    #[input]
    controller: NetworkController,
    #[input]
    target: WifiTarget,
    #[input]
    operation: crate::services::NetworkOperation,
    #[state]
    awaiting: bool,
    #[input]
    editor: EditorState,
    #[state]
    ssid: EntryValue,
    #[state]
    password: EntryValue,
    #[state]
    security: WifiSecurity,
    #[state]
    remember: bool,
    #[state]
    autoconnect: bool,
}
impl WifiForm {
    pub fn new(
        controller: NetworkController,
        target: WifiTarget,
        editor: EditorState,
        operation: crate::services::NetworkOperation,
    ) -> Self {
        let security = target.security;
        Self {
            controller,
            operation,
            awaiting: false,
            target,
            editor,
            ssid: EntryValue::new("", false),
            password: EntryValue::new("", true),
            security,
            remember: true,
            autoconnect: true,
        }
    }
    pub fn height(hidden: bool) -> f32 {
        if hidden { 412.0 } else { 310.0 }
    }
    fn connect(&mut self) {
        let ssid = if self.target.hidden {
            Ssid::new(self.ssid.text().into_bytes())
        } else {
            self.target
                .ssid
                .clone()
                .ok_or(NetworkError::InvalidConfig("Select a named network"))
        };
        let ssid = match ssid {
            Ok(ssid) => ssid,
            Err(_) => {
                self.controller.error("Enter a network name of 1–32 bytes");
                return;
            }
        };
        let password = self.password.take();
        if matches!(
            self.security,
            WifiSecurity::WpaPersonal | WifiSecurity::Wpa3Personal
        ) && password.is_empty()
        {
            self.controller.error("Enter the Wi-Fi password");
            return;
        }
        let mut options = WifiConnectOptions::new(ssid, self.security);
        if self.security != WifiSecurity::Open {
            options.credentials = Some(NetworkSecret::new(password));
        }
        options.hidden = self.target.hidden;
        options.access_point = self.target.ap;
        options.persistence = if self.remember {
            NetworkPersistence::Saved
        } else {
            NetworkPersistence::Session
        };
        options.autoconnect = self.remember && self.autoconnect;
        self.awaiting = self.controller.apply(
            vec![NetworkCommand::ConnectWifi {
                interface: self.target.interface,
                options,
            }],
            "Connected to Wi-Fi",
        );
    }
}
impl Component for WifiForm {
    fn inputs_changed(&mut self, _: &mut InputsChangedContext<Self>) {
        if self.awaiting && !self.operation.busy {
            self.awaiting = false;
            if !self.operation.failed {
                self.editor.set(None);
            }
        }
    }
    fn view(&self) -> impl View {
        let snapshot = self.controller.snapshot(self);
        let busy = self.controller.operation(self).busy;
        let interface = snapshot
            .interfaces
            .iter()
            .find(|interface| interface.id == self.target.interface);
        let enabled = !busy
            && snapshot.networking_enabled
            && snapshot.wifi_enabled
            && snapshot.wifi_hardware_enabled
            && allowed(&snapshot, NetworkPermissionKind::ControlConnections)
            && interface.is_some_and(|interface| {
                can_join(interface, self.security)
                    && self.target.ap.is_none_or(|id| {
                        interface.access_points.iter().any(|ap| {
                            ap.id == id
                                && ap.ssid == self.target.ssid
                                && ap.security == self.security
                        })
                    })
            });
        let mut form = group(Self::height(self.target.hidden))
            .padding(12.0)
            .gap(8.0)
            .child(
                ui_label(
                    if self.target.hidden {
                        "Join a hidden network"
                    } else {
                        "Connect to Wi-Fi"
                    },
                    16.0,
                    TEXT,
                )
                .height(22.0),
            );
        if self.target.hidden {
            form = form
                .child(Entry::new(
                    "Network name (SSID)",
                    "Network name",
                    self.ssid.clone(),
                    !busy,
                ))
                .child(ui_label("Security", 12.0, MUTED).height(18.0));
            let mut choices = row().height(36.0).gap(8.0);
            for security in [
                WifiSecurity::Open,
                WifiSecurity::WpaPersonal,
                WifiSecurity::Wpa3Personal,
            ] {
                choices = choices.child(
                    control(super::model::security(security))
                        .background(if self.security == security {
                            ACCENT
                        } else {
                            crate::theme::CARD
                        })
                        .enabled(
                            !busy
                                && interface.is_some_and(|interface| can_join(interface, security)),
                        )
                        .on_press(move |this: &mut Self| {
                            this.security = security;
                            this.password.take();
                        }),
                );
            }
            form = form.child(choices);
        } else {
            form = form.child(ui_label(name(self.target.ssid.as_ref()), 14.0, TEXT).height(28.0));
        }
        if self.security != WifiSecurity::Open {
            form = form.child(Entry::new(
                "Wi-Fi password",
                "Password",
                self.password.clone(),
                !busy,
            ));
        } else {
            form = form.child(ui_label("No password required", 13.0, MUTED).height(30.0));
        }
        form.child(
            switch("Remember this network", self.remember)
                .width(Dimension::FILL)
                .height(30.0)
                .enabled(!busy)
                .on_change(|this: &mut Self, value| {
                    this.remember = value;
                    if !value {
                        this.autoconnect = false;
                    }
                }),
        )
        .child(
            switch("Connect automatically", self.autoconnect)
                .width(Dimension::FILL)
                .height(30.0)
                .enabled(!busy && self.remember)
                .on_change(|this: &mut Self, value| this.autoconnect = value),
        )
        .child(column().height(Dimension::FILL))
        .child(
            row()
                .height(36.0)
                .gap(8.0)
                .child(
                    control("Cancel")
                        .enabled(!busy)
                        .on_press(|this: &mut Self| {
                            this.password.take();
                            this.editor.set(None);
                        }),
                )
                .child(
                    control(if busy { "Connecting…" } else { "Connect" })
                        .background(ACCENT)
                        .enabled(enabled)
                        .on_press(|this: &mut Self| this.connect()),
                ),
        )
    }
}
