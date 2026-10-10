use super::*;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub(super) struct FakeState {
    pub snapshot: NetworkSnapshot,
    pub commands: Vec<&'static str>,
    pub fail_update: bool,
    pub fail_reapply: bool,
    pub replace_connection: bool,
}
struct Fake(Arc<Mutex<FakeState>>);
impl NetworkProvider for Fake {
    fn snapshot(&mut self) -> std::result::Result<NetworkSnapshot, NetworkError> {
        Ok(self.0.lock().unwrap().snapshot.clone())
    }
    fn execute(
        &mut self,
        command: NetworkCommand,
    ) -> std::result::Result<NetworkDispatch, NetworkError> {
        let mut state = self.0.lock().unwrap();
        let result = match command {
            NetworkCommand::UpdateProfile { profile, ip, .. } => {
                state.commands.push("update");
                if state.fail_update {
                    return Err(NetworkError::PermissionDenied);
                }
                state
                    .snapshot
                    .profiles
                    .iter_mut()
                    .find(|candidate| candidate.id == profile)
                    .unwrap()
                    .ip = ip;
                if state.replace_connection {
                    state.snapshot.interfaces[0].active_connection = None;
                }
                NetworkResult::ProfileUpdated(profile)
            }
            NetworkCommand::Reapply(interface) => {
                state.commands.push("reapply");
                if state.fail_reapply {
                    return Err(NetworkError::IpConfigurationFailed);
                }
                NetworkResult::Reapplied(interface)
            }
            NetworkCommand::SetWifiEnabled(enabled) => {
                state.commands.push("radio");
                state.snapshot.wifi_enabled = enabled;
                NetworkResult::WifiEnabled(enabled)
            }
            _ => return Err(NetworkError::Unsupported),
        };
        Ok(NetworkDispatch::Complete(result))
    }
    fn poll(
        &mut self,
        _: u64,
        _: &NetworkSnapshot,
    ) -> std::result::Result<Option<NetworkResult>, NetworkError> {
        Ok(None)
    }
    fn abandon(&mut self, _: u64) {}
}
pub(super) fn fixture() -> (
    crate::services::NetworkService,
    NetworkController,
    Arc<Mutex<FakeState>>,
) {
    let interface = NetworkInterfaceId::new();
    let profile = NetworkProfileId::new();
    let connection = NetworkConnectionId::new();
    let ap = NetworkAccessPointId::new();
    let capabilities = NetworkCapabilities {
        set_networking_enabled: true,
        set_wifi_enabled: true,
        scan_wifi: true,
        connect_wifi: true,
        wpa3_personal: true,
        disconnect: true,
        edit_profiles: true,
        configure_ip: true,
        activate_profile: true,
        reapply: true,
        ..Default::default()
    };
    let state = Arc::new(Mutex::new(FakeState {
        commands: vec![],
        fail_update: false,
        fail_reapply: false,
        replace_connection: false,
        snapshot: NetworkSnapshot {
            state: NetworkServiceState::Ready,
            capabilities: capabilities.clone(),
            networking_enabled: true,
            wifi_enabled: true,
            wifi_hardware_enabled: true,
            connectivity: NetworkConnectivity::Internet,
            interfaces: vec![NetworkInterface {
                device: Default::default(),
                id: interface,
                name: "wlan0".into(),
                kind: NetworkInterfaceKind::Wifi,
                managed: true,
                state: NetworkConnectionState::Connected,
                failure: None,
                capabilities,
                active_connection: Some(connection),
                ipv4: NetworkIpState {
                    addresses: vec![IpAddress::new("192.0.2.10".parse().unwrap(), 24).unwrap()],
                    ..Default::default()
                },
                ipv6: Default::default(),
                active_access_point: Some(ap),
                access_points: vec![NetworkAccessPoint {
                    id: ap,
                    ssid: Some(Ssid::new(b"Home".to_vec()).unwrap()),
                    bssid: "00:11:22:33:44:55".into(),
                    strength: 72,
                    frequency_mhz: 5180,
                    security: WifiSecurity::WpaPersonal,
                }],
            }],
            profiles: vec![NetworkProfile {
                interface_name: None,
                id: profile,
                name: "Home".into(),
                kind: NetworkInterfaceKind::Wifi,
                ssid: Some(Ssid::new(b"Home".to_vec()).unwrap()),
                autoconnect: true,
                persistence: NetworkPersistence::Saved,
                ip: Default::default(),
            }],
            connections: vec![NetworkConnection {
                id: connection,
                profile: Some(profile),
                interfaces: vec![interface],
                state: NetworkConnectionState::Connected,
            }],
            ..Default::default()
        },
    }));
    let controller = telorgon::network::NetworkController::with_provider(
        NetworkConfig {
            poll_interval: Duration::from_millis(10),
            request_timeout: Duration::from_secs(1),
            ..Default::default()
        },
        Fake(state.clone()),
    )
    .unwrap();
    let (owner, model) = crate::services::NetworkService::with_controller(controller).unwrap();
    until(|| model.handle.signal().snapshot().state == NetworkServiceState::Ready);
    (owner, NetworkController { model }, state)
}
fn until(mut condition: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(3);
    while !condition() {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn network_operation_stops_after_failed_save_and_reports_partial_application() {
    for (fail_update, replace_connection) in [(true, false), (false, false), (false, true)] {
        let (_owner, controller, state) = fixture();
        let snapshot = controller.model.handle.signal().snapshot();
        let profile = snapshot.profiles[0].id;
        let interface = snapshot.interfaces[0].id;
        {
            let mut state = state.lock().unwrap();
            state.fail_update = fail_update;
            state.fail_reapply = true;
            state.replace_connection = replace_connection;
        }
        assert!(controller.apply(
            vec![
                NetworkCommand::UpdateProfile {
                    profile,
                    ip: Default::default(),
                    persistence: NetworkPersistence::Saved
                },
                NetworkCommand::Reapply(interface)
            ],
            "Saved and applied"
        ));
        until(|| !controller.model.operation.snapshot().busy);
        let status = controller.model.operation.snapshot();
        assert!(status.failed);
        assert_ne!(status.message, "Saved and applied");
        let state = state.lock().unwrap();
        assert_eq!(
            state.commands,
            if fail_update || replace_connection {
                vec!["update"]
            } else {
                vec!["update", "reapply"]
            }
        );
        if !fail_update {
            assert!(status.message.contains("saved"));
        }
    }
}

#[test]
fn settings_network_deep_link_selects_the_network_page() {
    assert!(
        crate::state::Page::from_arguments(["--page=network".into()]).unwrap()
            == crate::state::Page::Network
    );
    assert!(
        crate::state::Page::from_arguments(["--page".into(), "network".into()]).unwrap()
            == crate::state::Page::Network
    );
    assert!(crate::state::Page::from_arguments(["--page".into()]).is_err());
    assert!(crate::state::Page::from_arguments(["--page=missing".into()]).is_err());
}

use telorgon::{
    application_host::ComposedAppRuntime,
    input::{ButtonState, KeyEvent, KeyText, LogicalKey, PhysicalKey, PointerButton},
};
#[component(no_default)]
struct Host {
    #[input]
    controller: NetworkController,
}
impl Component for Host {
    fn view(&self) -> impl View {
        row()
            .height(Dimension::FILL)
            .width(Dimension::FILL)
            .gap(1.0)
            .child(column().width(crate::settings_app::sidebar_width(self.viewport_size().width)))
            .child(
                column()
                    .padding(crate::settings_app::content_padding(self.viewport_size().width))
                    .width(Dimension::FILL)
                    .height(Dimension::FILL)
                    .child(NetworkPage::new(self.controller.clone())),
            )
    }
}
fn at(ms: u64) -> telorgon::MonotonicInstant {
    telorgon::MonotonicInstant::from_nanos(ms * 1_000_000)
}
fn runtime(controller: NetworkController, width: i32, height: i32) -> ComposedAppRuntime {
    let mut runtime =
        ComposedAppRuntime::from_composed_with_extent(Host { controller }, SizeI { width, height })
            .unwrap();
    runtime.prepare_frame(at(0), false).unwrap();
    runtime
}
fn has(runtime: &ComposedAppRuntime, label: &str) -> bool {
    runtime
        .ui()
        .texts
        .iter()
        .any(|(_, text)| runtime.ui().string(text.content) == Some(label))
}
fn click(runtime: &mut ComposedAppRuntime, label: &str, ms: u64) {
    let node = runtime
        .ui()
        .semantics
        .iter()
        .find_map(|(node, semantic)| match semantic.name {
            telorgon::SemanticName::Text(name)
                if runtime.ui().string(name) == Some(label)
                    && runtime.ui().kinds.get(node) == Some(&telorgon::NodeKind::Button) =>
            {
                Some(node)
            }
            _ => None,
        })
        .or_else(|| {
            runtime.ui().texts.iter().find_map(|(node, text)| {
                if runtime.ui().string(text.content) != Some(label) {
                    return None;
                }
                let mut parent = Some(node);
                while let Some(node) = parent {
                    if runtime.ui().kinds.get(node) == Some(&telorgon::NodeKind::Button) {
                        return Some(node);
                    }
                    parent = runtime.ui().nodes.core(node)?.parent;
                }
                None
            })
        })
        .unwrap_or_else(|| panic!("missing control {label}"));
    let layout = runtime.layout().computed(node).unwrap();
    assert!(
        layout.visible_rect.area() > 0.0,
        "control {label} is clipped"
    );
    let rect = layout.border_rect;
    runtime.queue_input(telorgon::InputEvent::mouse_moved(PointF {
        x: rect.x + rect.width / 2.0,
        y: rect.y + rect.height / 2.0,
    }));
    runtime.flush_input(at(ms));
    for (offset, state) in [(1, ButtonState::Pressed), (2, ButtonState::Released)] {
        runtime.queue_input(telorgon::InputEvent::mouse_button(
            PointerButton::PRIMARY,
            state,
        ));
        runtime.flush_input(at(ms + offset));
    }
    runtime.prepare_frame(at(ms + 3), false).unwrap();
}
fn type_text(runtime: &mut ComposedAppRuntime, text: &str, ms: u64) {
    runtime.queue_input(telorgon::InputEvent::Key(
        KeyEvent::new(PhysicalKey::UNIDENTIFIED, ButtonState::Pressed)
            .with_logical_key(LogicalKey::Character(KeyText::new("a").unwrap()))
            .with_text(Some(KeyText::new(text).unwrap())),
    ));
    runtime.flush_input(at(ms));
    runtime.prepare_frame(at(ms), false).unwrap();
}
fn add_interfaces(state: &Arc<Mutex<FakeState>>) {
    let mut state = state.lock().unwrap();
    for (name, kind) in [
        ("br0", NetworkDeviceType::Bridge),
        ("lo", NetworkDeviceType::Loopback),
    ] {
        let mut device = state.snapshot.interfaces[0].clone();
        device.id = NetworkInterfaceId::new();
        device.name = name.into();
        device.kind = NetworkInterfaceKind::Other;
        device.managed = false;
        device.state = NetworkConnectionState::Disconnected;
        device.active_connection = None;
        device.access_points.clear();
        device.active_access_point = None;
        device.capabilities = Default::default();
        device.device = NetworkDeviceInfo {
            device_type: kind,
            virtual_device: true,
            mtu: Some(1500),
            operational_state: Some("up".into()),
            ..Default::default()
        };
        state.snapshot.interfaces.push(device);
    }
}
#[test]
fn network_browser_keeps_all_interfaces_visible_and_handles_removal() {
    let (_owner, controller, state) = fixture();
    add_interfaces(&state);
    until(|| controller.model.handle.signal().snapshot().interfaces.len() == 3);
    let mut runtime = runtime(controller.clone(), 1100, 720);
    preview(
        &mut runtime,
        SizeI {
            width: 1100,
            height: 720,
        },
        "overview",
    );
    assert!(has(&runtime, "Bridge · br0"));
    assert!(has(&runtime, "Loopback · lo"));
    click(&mut runtime, "Select br0", 1);
    click(&mut runtime, "Network tab: Device", 5);
    preview(
        &mut runtime,
        SizeI {
            width: 1100,
            height: 720,
        },
        "device",
    );
    assert!(has(&runtime, "MTU"));
    assert!(has(&runtime, "Parent interface"));
    state
        .lock()
        .unwrap()
        .snapshot
        .interfaces
        .retain(|device| device.name != "br0");
    until(|| controller.model.handle.signal().snapshot().interfaces.len() == 2);
    runtime.prepare_frame(at(10), false).unwrap();
    assert!(has(&runtime, "This interface is no longer available"));
    click(&mut runtime, "View interfaces", 15);
    assert!(has(&runtime, "Connection overview"));
    click(&mut runtime, "Search interfaces", 20);
    type_text(&mut runtime, "lo", 24);
    assert!(has(&runtime, "Loopback · lo"));
    assert!(!has(&runtime, "Wi-Fi · wlan0"));
}
#[test]
fn network_browser_resizes_and_unmanaged_interfaces_work_without_management() {
    let (_owner, controller, state) = fixture();
    add_interfaces(&state);
    {
        let mut state = state.lock().unwrap();
        state.snapshot.state = NetworkServiceState::Unavailable;
        state.snapshot.profiles.clear();
        for device in &mut state.snapshot.interfaces {
            device.managed = false;
            device.capabilities = Default::default();
        }
    }
    until(|| controller.model.handle.signal().snapshot().state == NetworkServiceState::Unavailable);
    let mut runtime = runtime(controller, 1100, 720);
    assert!(has(&runtime, "Network management unavailable"));
    click(&mut runtime, "Select lo", 1);
    click(&mut runtime, "Network tab: Device", 5);
    assert!(has(&runtime, "Loopback"));
    runtime
        .resize(SizeI {
            width: 700,
            height: 720,
        })
        .unwrap();
    runtime.prepare_frame(at(10), false).unwrap();
    assert!(has(&runtime, "‹ All interfaces"));
    click(&mut runtime, "‹ All interfaces", 15);
    assert!(has(&runtime, "Search interfaces"));
    assert!(!has(&runtime, "Device information"));
    for (width, height) in [(640, 480), (960, 560), (700, 720), (560, 720), (1100, 720)] {
        runtime.resize(SizeI { width, height }).unwrap();
        runtime.prepare_frame(at(width as u64), false).unwrap();
        for (node, layout) in runtime.layout().computed_nodes() {
            if !runtime.ui().nodes.contains(node) {
                continue;
            }
            assert!(
                layout.border_rect.x >= -0.1 && layout.border_rect.right() <= width as f32 + 0.1,
                "out of bounds: {:?}",
                layout.border_rect
            );
        }
    }
    assert!(state.lock().unwrap().commands.is_empty());
}
#[test]
fn network_editor_applies_ipv6_and_preserves_drafts_after_failures() {
    for fail in [false, true] {
        let (_owner, controller, state) = fixture();
        state.lock().unwrap().fail_update = fail;
        let mut runtime = runtime(controller.clone(), 1100, 900);
        click(&mut runtime, "Connection settings", 1);
        assert!(has(
            &runtime,
            "Changes are applied only when you choose Apply"
        ));
        click(&mut runtime, "Connection editor: IPv6", 5);
        click(&mut runtime, "Static", 9);
        click(&mut runtime, "Addresses with prefix", 13);
        type_text(&mut runtime, "2001:db8::20/64", 17);
        click(&mut runtime, "Apply", 21);
        until(|| !controller.model.operation.snapshot().busy);
        runtime.prepare_frame(at(30), false).unwrap();
        runtime.prepare_frame(at(31), false).unwrap();
        if fail {
            assert!(has(&runtime, "2001:db8::20/64"));
            assert!(controller.model.operation.snapshot().failed);
            click(&mut runtime, "Cancel", 35);
        } else {
            assert!(has(&runtime, "Connection overview"));
            assert!(matches!(
                state.lock().unwrap().snapshot.profiles[0].ip.ipv6.method,
                IpMethod::Static(_)
            ));
        }
    }
}
#[test]
fn network_wifi_setup_uses_dedicated_editor_and_cancel_makes_no_changes() {
    let (_owner, controller, state) = fixture();
    let mut runtime = runtime(controller, 1100, 900);
    click(&mut runtime, "Network tab: Wi-Fi", 1);
    assert!(has(&runtime, "Saved connections"));
    assert!(has(&runtime, "BSSID"));
    click(&mut runtime, "Join a hidden network", 5);
    assert!(has(&runtime, "Network name (SSID)"));
    assert!(has(&runtime, "Wi-Fi password"));
    click(&mut runtime, "Cancel", 9);
    assert!(has(&runtime, "Nearby Wi-Fi networks"));
    assert!(state.lock().unwrap().commands.is_empty());
}

#[test]
fn network_editor_wraps_in_narrow_windows_and_unchanged_apply_makes_no_writes() {
    let (_owner, controller, state) = fixture();
    let mut runtime = runtime(controller, 1100, 900);
    click(&mut runtime, "Connection settings", 1);
    runtime
        .resize(SizeI {
            width: 560,
            height: 720,
        })
        .unwrap();
    runtime.prepare_frame(at(5), false).unwrap();
    runtime.queue_input(telorgon::InputEvent::mouse_moved(PointF {
        x: 400.0,
        y: 500.0,
    }));
    runtime.flush_input(at(6));
    runtime.queue_input(telorgon::InputEvent::mouse_scroll(PointF {
        x: 0.0,
        y: -1000.0,
    }));
    runtime.flush_input(at(7));
    runtime.prepare_frame(at(8), false).unwrap();
    preview(
        &mut runtime,
        SizeI {
            width: 560,
            height: 720,
        },
        "narrow-editor",
    );
    click(&mut runtime, "Apply", 10);
    assert!(has(&runtime, "Search interfaces"));
    assert!(state.lock().unwrap().commands.is_empty());
}

fn preview(runtime: &mut ComposedAppRuntime, extent: SizeI, name: &str) {
    let Some(directory) = std::env::var_os("TELORGON_NETWORK_PREVIEW") else {
        return;
    };
    use telorgon::graphics::{
        render::{RenderBackend, RenderRequest, RenderTargetInfo, TargetLoad, TargetStore},
        renderers::software::{SoftwareRenderer, SoftwareSurface, SoftwareTarget},
    };
    runtime.register_assets(crate::assets::bundle()).unwrap();
    runtime.prepare_frame(at(100), true).unwrap();
    let renderer = SoftwareRenderer;
    let mut scene = renderer.create_scene().unwrap();
    renderer
        .apply_scene_delta(&mut scene, &runtime.scene_snapshot())
        .unwrap();
    let mut surface = SoftwareSurface::default();
    renderer
        .render(
            &mut scene,
            &mut surface.begin_frame(),
            &SoftwareTarget::new(RenderTargetInfo::full(extent)),
            &RenderRequest {
                force: true,
                load: TargetLoad::Clear(crate::theme::BG),
                store: TargetStore::Store,
                region: None,
            },
        )
        .unwrap();
    let mut data = format!("P6\n{} {}\n255\n", extent.width, extent.height).into_bytes();
    for pixel in surface.pixels_rgba8().chunks_exact(4) {
        data.extend_from_slice(&pixel[..3]);
    }
    std::fs::write(
        std::path::Path::new(&directory).join(format!("network-{name}.ppm")),
        data,
    )
    .unwrap();
}
