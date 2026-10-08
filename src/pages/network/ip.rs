use super::{model::*, ui_label};
use crate::{
    components::{
        controls::{control, group},
        entry::{Entry, EntryValue},
    },
    controllers::NetworkController,
    services::NetworkOperation,
    theme::{ACCENT, CARD, MUTED, TEXT},
};
use telorgon::{app::*, network::*};
mod draft;
use draft::*;
#[derive(Clone, Copy, Default, PartialEq)]
enum EditTab {
    #[default]
    General,
    V4,
    V6,
    Dns,
    Routes,
}
#[component(no_default)]
pub(super) struct IpForm {
    #[input]
    controller: NetworkController,
    #[input]
    target: IpTarget,
    #[input]
    editor: EditorState,
    #[input]
    operation: NetworkOperation,
    #[state]
    tab: EditTab,
    #[state]
    v4: FamilyDraft,
    #[state]
    v6: FamilyDraft,
    #[state]
    autoconnect: bool,
    #[state]
    awaiting: bool,
    #[state]
    forgetting: bool,
}
impl IpForm {
    fn columns(width: f32) -> usize {
        let available = (width - if width >= 1000.0 { 529.0 } else { 289.0 }).max(100.0);
        ((available + 6.0) / 106.0).floor().max(1.0) as usize
    }
    pub fn height(width: f32) -> f32 {
        604.0 + 5usize.div_ceil(Self::columns(width)) as f32 * 42.0 - 6.0
    }
    pub fn new(
        controller: NetworkController,
        target: IpTarget,
        editor: EditorState,
        operation: NetworkOperation,
    ) -> Self {
        Self {
            controller,
            v4: FamilyDraft::new(&target.profile.ip.ipv4),
            v6: FamilyDraft::new(&target.profile.ip.ipv6),
            autoconnect: target.profile.autoconnect,
            target,
            editor,
            operation,
            tab: Default::default(),
            awaiting: false,
            forgetting: false,
        }
    }
    fn configuration(&self) -> std::result::Result<NetworkIpConfig, String> {
        Ok(NetworkIpConfig {
            ipv4: self.v4.build(IpFamily::V4)?,
            ipv6: self.v6.build(IpFamily::V6)?,
        })
    }
    fn save(&mut self) {
        let ip = match self.configuration() {
            Ok(ip) => ip,
            Err(error) => {
                self.controller.error(error);
                return;
            }
        };
        let snapshot = self.controller.model.handle.signal().snapshot();
        let baseline = &self.target.profile;
        if snapshot
            .profiles
            .iter()
            .find(|profile| profile.id == baseline.id)
            != Some(baseline)
        {
            self.controller
                .error("This connection changed. Cancel and reopen its settings");
            return;
        }
        let mut commands = Vec::new();
        let ip_changed = ip != baseline.ip;
        if ip_changed {
            commands.push(NetworkCommand::UpdateProfile {
                profile: baseline.id,
                ip,
                persistence: baseline.persistence,
            });
        }
        if self.autoconnect != baseline.autoconnect {
            commands.push(NetworkCommand::SetAutoconnect {
                profile: baseline.id,
                enabled: self.autoconnect,
                persistence: baseline.persistence,
            });
        }
        if commands.is_empty() {
            self.editor.set(None);
            return;
        }
        let device = snapshot
            .interfaces
            .iter()
            .find(|d| d.id == self.target.interface);
        let active = device
            .and_then(|d| active_profile(&snapshot, d))
            .is_some_and(|p| p.id == baseline.id);
        let reapply = ip_changed && active && device.is_some_and(|d| d.capabilities.reapply);
        if reapply {
            commands.push(NetworkCommand::Reapply(self.target.interface));
        }
        self.awaiting = self.controller.apply(
            commands,
            if reapply {
                "Connection settings saved and applied"
            } else if active && ip_changed {
                "Connection settings saved. Reconnect to apply them"
            } else {
                "Connection settings saved"
            },
        );
    }
    fn family(&self, family: IpFamily, enabled: bool) -> Container {
        let draft = if family == IpFamily::V4 {
            &self.v4
        } else {
            &self.v6
        };
        let mut choices = row().height(36.0).gap(8.0);
        for (method, title) in [
            (Method::Automatic, "Automatic"),
            (Method::Static, "Static"),
            (Method::Disabled, "Disabled"),
        ] {
            choices = choices.child(
                control(title)
                    .background(if draft.method == method { ACCENT } else { CARD })
                    .enabled(enabled)
                    .on_press(move |this: &mut Self| {
                        if family == IpFamily::V4 {
                            this.v4.method = method;
                        } else {
                            this.v6.method = method;
                        }
                    }),
            );
        }
        column().height(330.0).gap(14.0)
            .child(ui_label(if family==IpFamily::V4{"IPv4 configuration"}else{"IPv6 configuration"},16.0,TEXT).height(28.0))
            .child(ui_label(format!("Method: {}",draft.method.title()),12.0,MUTED).height(24.0))
            .child(choices)
            .child(Entry::new("Addresses with prefix",if family==IpFamily::V4{"192.168.1.20/24, 192.168.1.21/24"}else{"2001:db8::20/64"},draft.addresses.clone(),enabled&&draft.method==Method::Static))
            .child(Entry::new("Gateway (optional)",if family==IpFamily::V4{"192.168.1.1"}else{"fe80::1"},draft.gateway.clone(),enabled&&draft.method==Method::Static))
            .child(ui_label(if family==IpFamily::V4{"Automatic uses DHCP. Static requires at least one address."}else{"Automatic uses the backend's automatic IPv6 configuration. Existing settings remain intact until you apply changes."},12.0,MUTED).height(54.0))
    }
}
impl Component for IpForm {
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
        let enabled = !busy
            && snapshot.state == NetworkServiceState::Ready
            && snapshot.capabilities.edit_profiles
            && snapshot
                .interfaces
                .iter()
                .find(|d| d.id == self.target.interface)
                .is_some_and(|d| d.managed && d.capabilities.configure_ip)
            && snapshot
                .profiles
                .iter()
                .any(|p| p.id == self.target.profile.id);
        let available = (self.viewport_size().width
            - if self.viewport_size().width >= 1000.0 {
                529.0
            } else {
                289.0
            })
        .max(100.0);
        let columns = Self::columns(self.viewport_size().width);
        let mut tabs = column()
            .height(5usize.div_ceil(columns) as f32 * 42.0 - 6.0)
            .gap(6.0);
        let choices = [
            (EditTab::General, "General"),
            (EditTab::V4, "IPv4"),
            (EditTab::V6, "IPv6"),
            (EditTab::Dns, "DNS"),
            (EditTab::Routes, "Routes"),
        ];
        for group in choices.chunks(columns) {
            let mut buttons = row().height(36.0).gap(6.0);
            for &(tab, title) in group {
                buttons = buttons.child(
                    control(title)
                        .width(available.min(100.0))
                        .accessible_label(format!("Connection editor: {title}"))
                        .background(if self.tab == tab { ACCENT } else { CARD })
                        .enabled(!busy)
                        .on_press(move |this: &mut Self| this.tab = tab),
                );
            }
            tabs = tabs.child(buttons);
        }
        let content=match self.tab {
            EditTab::V4=>self.family(IpFamily::V4,enabled),EditTab::V6=>self.family(IpFamily::V6,enabled),
            EditTab::General=> {
                let mut content=column().height(330.0).gap(14.0)
                    .child(ui_label(format!("Profile: {}",self.target.profile.name),16.0,TEXT).height(28.0))
                    .child(ui_label(format!("Interface: {}",snapshot.interfaces.iter().find(|d|d.id==self.target.interface).map(|d|d.name.as_str()).unwrap_or("Removed")),12.0,MUTED).height(24.0))
                    .child(ui_label(if self.target.profile.persistence==NetworkPersistence::Saved{"Saved on disk"}else{"Session connection"},12.0,MUTED).height(24.0))
                    .child(switch("Connect automatically",self.autoconnect).width(Dimension::FILL).height(36.0).enabled(enabled).on_change(|this:&mut Self,value|this.autoconnect=value));
                if self.forgetting {
                    let id=self.target.profile.id;
                    content=content.child(ui_label("Forget this saved connection?",14.0,TEXT).height(28.0))
                        .child(row().height(36.0).gap(8.0).child(control("Keep connection").enabled(!busy).on_press(|this:&mut Self|this.forgetting=false))
                            .child(control("Forget connection").enabled(enabled).on_press(move|this:&mut Self|{this.awaiting=this.controller.apply(vec![NetworkCommand::ForgetProfile(id)],"Connection forgotten");})));
                }else{content=content.child(control("Forget connection…").enabled(enabled).on_press(|this:&mut Self|this.forgetting=true));}
                content
            },
            EditTab::Dns=>column().height(330.0).gap(12.0)
                .child(ui_label("DNS configuration",16.0,TEXT).height(28.0))
                .child(switch("Use automatic IPv4 DNS",self.v4.automatic_dns).width(Dimension::FILL).height(30.0).enabled(enabled&&self.v4.editable()).on_change(|this:&mut Self,value|this.v4.automatic_dns=value))
                .child(Entry::new("IPv4 DNS servers (optional)","192.168.1.1, 192.168.1.2",self.v4.dns.clone(),enabled&&self.v4.editable()))
                .child(switch("Use automatic IPv6 DNS",self.v6.automatic_dns).width(Dimension::FILL).height(30.0).enabled(enabled&&self.v6.editable()).on_change(|this:&mut Self,value|this.v6.automatic_dns=value))
                .child(Entry::new("IPv6 DNS servers (optional)","2001:db8::53",self.v6.dns.clone(),enabled&&self.v6.editable()))
                .child(ui_label("Extra DNS servers can be combined with automatic DNS. Turn automatic DNS off to use only the listed servers.",12.0,MUTED).height(54.0)),
            EditTab::Routes=>column().height(330.0).gap(12.0)
                .child(ui_label("Saved static routes",16.0,TEXT).height(28.0))
                .child(Entry::new("IPv4 routes","10.0.0.0/8 via 192.168.1.1 metric 100",self.v4.routes.clone(),enabled&&self.v4.editable()))
                .child(switch("Use automatic IPv4 routes",self.v4.automatic_routes).width(Dimension::FILL).height(30.0).enabled(enabled&&self.v4.editable()).on_change(|this:&mut Self,value|this.v4.automatic_routes=value))
                .child(Entry::new("IPv6 routes","2001:db8:1::/64 via fe80::1 metric 100",self.v6.routes.clone(),enabled&&self.v6.editable()))
                .child(switch("Use automatic IPv6 routes",self.v6.automatic_routes).width(Dimension::FILL).height(30.0).enabled(enabled&&self.v6.editable()).on_change(|this:&mut Self,value|this.v6.automatic_routes=value))
                .child(ui_label("Format: destination/prefix [via gateway] [metric number]. Separate routes with semicolons. These edit saved configuration; live routes are shown in the Routes tab.",12.0,MUTED).height(54.0)),
        };
        group(Self::height(self.viewport_size().width))
            .padding(16.0)
            .gap(14.0)
            .child(
                ui_label("Connection settings", 20.0, TEXT)
                    .weight(600)
                    .height(32.0),
            )
            .child(tabs)
            .child(content)
            .child(column().height(Dimension::FILL))
            .child(
                ui_label(
                    if !enabled && !busy {
                        "This connection cannot currently be edited"
                    } else {
                        "Changes are applied only when you choose Apply"
                    },
                    12.0,
                    MUTED,
                )
                .height(30.0),
            )
            .child(
                row()
                    .height(36.0)
                    .gap(8.0)
                    .child(
                        control("Cancel")
                            .enabled(!busy)
                            .on_press(|this: &mut Self| this.editor.set(None)),
                    )
                    .child(
                        control(if busy { "Applying…" } else { "Apply" })
                            .background(ACCENT)
                            .enabled(enabled)
                            .on_press(|this: &mut Self| this.save()),
                    ),
            )
    }
}
