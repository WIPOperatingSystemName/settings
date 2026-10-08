use telorgon::{
    app::{Signal, SignalWriter},
    network::*,
};

pub(super) fn name(ssid: Option<&Ssid>) -> String {
    ssid.map(|ssid| {
        ssid.display_name()
            .chars()
            .map(|ch| if ch.is_control() { ' ' } else { ch })
            .collect()
    })
    .unwrap_or_else(|| "Hidden network".into())
}
pub(super) fn security(security: WifiSecurity) -> &'static str {
    match security {
        WifiSecurity::Open => "Open",
        WifiSecurity::WpaPersonal => "WPA personal",
        WifiSecurity::Wpa3Personal => "WPA3 personal",
        WifiSecurity::Enterprise => "Enterprise",
        WifiSecurity::Wep => "WEP",
        WifiSecurity::Unknown => "Unknown security",
    }
}
pub(super) fn status(snapshot: &NetworkSnapshot) -> &'static str {
    match snapshot.state {
        NetworkServiceState::Unstarted => "Starting network monitoring…",
        NetworkServiceState::Unavailable | NetworkServiceState::Stopped => {
            "Network management unavailable"
        }
        NetworkServiceState::Restricted => "Network access restricted",
        NetworkServiceState::Ready if !snapshot.networking_enabled => "Networking is off",
        NetworkServiceState::Ready => match snapshot.connectivity {
            NetworkConnectivity::Internet => "Internet access",
            NetworkConnectivity::CaptivePortal => "Sign-in required",
            NetworkConnectivity::Local => "Local network only",
            NetworkConnectivity::Offline => "No Internet access",
            NetworkConnectivity::Unknown => "Internet access has not been checked",
        },
    }
}
pub(super) fn connection_status(interface: &NetworkInterface) -> &'static str {
    if !interface.managed {
        return "Managed elsewhere";
    }
    match interface.state {
        NetworkConnectionState::Connected => "Connected",
        NetworkConnectionState::Preparing | NetworkConnectionState::Authenticating => "Connecting…",
        NetworkConnectionState::ConfiguringIp => "Getting an IP address…",
        NetworkConnectionState::Disconnecting => "Disconnecting…",
        NetworkConnectionState::Failed => "Connection failed",
        NetworkConnectionState::Unavailable => "Unavailable",
        _ => "Disconnected",
    }
}
pub(super) fn active_profile<'a>(
    snapshot: &'a NetworkSnapshot,
    interface: &NetworkInterface,
) -> Option<&'a NetworkProfile> {
    let profile = snapshot
        .connections
        .iter()
        .find(|connection| Some(connection.id) == interface.active_connection)?
        .profile?;
    snapshot
        .profiles
        .iter()
        .find(|candidate| candidate.id == profile)
}
pub(super) fn active_name(snapshot: &NetworkSnapshot, interface: &NetworkInterface) -> String {
    if let Some(profile) = active_profile(snapshot, interface) {
        if interface.kind == NetworkInterfaceKind::Wifi && profile.ssid.is_some() {
            return name(profile.ssid.as_ref());
        }
        return profile.name.clone();
    }
    if let Some(ap) = interface
        .access_points
        .iter()
        .find(|ap| Some(ap.id) == interface.active_access_point)
    {
        return name(ap.ssid.as_ref());
    }
    match interface.kind {
        NetworkInterfaceKind::Wifi => "Wi-Fi",
        NetworkInterfaceKind::Ethernet => "Ethernet",
        _ => "Network",
    }
    .into()
}
pub(super) fn allowed(snapshot: &NetworkSnapshot, permission: NetworkPermissionKind) -> bool {
    snapshot.state == NetworkServiceState::Ready
        && !snapshot.permissions.iter().any(|entry| {
            entry.kind == permission && entry.authorization == NetworkAuthorization::Denied
        })
}
pub(super) fn nearby(interface: &NetworkInterface) -> Vec<&NetworkAccessPoint> {
    let mut aps: Vec<&NetworkAccessPoint> = Vec::new();
    for ap in &interface.access_points {
        if let Some(existing) = aps.iter_mut().find(|other| {
            ap.ssid.is_some() && ap.ssid == other.ssid && ap.security == other.security
        }) {
            let active = Some(ap.id) == interface.active_access_point;
            if active
                || (Some(existing.id) != interface.active_access_point
                    && ap.strength > existing.strength)
            {
                *existing = ap;
            }
        } else {
            aps.push(ap);
        }
    }
    aps.sort_by_key(|ap| {
        (
            Some(ap.id) != interface.active_access_point,
            std::cmp::Reverse(ap.strength),
            ap.id,
        )
    });
    aps
}
pub(super) fn can_join(interface: &NetworkInterface, security: WifiSecurity) -> bool {
    interface.managed
        && interface.capabilities.connect_wifi
        && match security {
            WifiSecurity::Open | WifiSecurity::WpaPersonal => true,
            WifiSecurity::Wpa3Personal => interface.capabilities.wpa3_personal,
            _ => false,
        }
}

#[derive(Clone, PartialEq)]
pub(super) struct WifiTarget {
    pub interface: NetworkInterfaceId,
    pub ap: Option<NetworkAccessPointId>,
    pub ssid: Option<Ssid>,
    pub security: WifiSecurity,
    pub hidden: bool,
}
#[derive(Clone, PartialEq)]
pub(super) struct IpTarget {
    pub interface: NetworkInterfaceId,
    pub profile: NetworkProfile,
}
#[derive(Clone, PartialEq)]
pub(super) enum Editor {
    Wifi(WifiTarget),
    Ip(IpTarget),
}
#[derive(Clone)]
pub(super) struct EditorState {
    pub signal: Signal<Option<Editor>>,
    writer: SignalWriter<Option<Editor>>,
}
impl Default for EditorState {
    fn default() -> Self {
        let (signal, writer) = Signal::new(None);
        Self { signal, writer }
    }
}
impl PartialEq for EditorState {
    fn eq(&self, other: &Self) -> bool {
        self.signal == other.signal
    }
}
impl EditorState {
    pub fn set(&self, editor: Option<Editor>) {
        self.writer.publish(editor);
    }
}

pub(super) fn interface_group(device: &NetworkInterface) -> u8 {
    if device.device.device_type == NetworkDeviceType::Loopback {
        2
    } else if device.device.virtual_device || device.kind == NetworkInterfaceKind::Other {
        1
    } else {
        0
    }
}
pub(super) fn device_type(device: &NetworkInterface) -> &'static str {
    match device.device.device_type {
        NetworkDeviceType::Wifi => "Wi-Fi",
        NetworkDeviceType::Ethernet => "Ethernet",
        NetworkDeviceType::Loopback => "Loopback",
        NetworkDeviceType::Bridge => "Bridge",
        NetworkDeviceType::Bond => "Bond",
        NetworkDeviceType::Vlan => "VLAN",
        NetworkDeviceType::Tunnel => "Tunnel",
        NetworkDeviceType::Virtual => "Virtual",
        NetworkDeviceType::Unknown => match device.kind {
            NetworkInterfaceKind::Wifi => "Wi-Fi",
            NetworkInterfaceKind::Ethernet => "Ethernet",
            _ => "Other",
        },
    }
}
pub(super) fn interface_status(device: &NetworkInterface) -> String {
    if device.managed {
        connection_status(device).into()
    } else {
        format!(
            "{} · Unmanaged",
            device
                .device
                .operational_state
                .as_deref()
                .unwrap_or("State unknown")
        )
    }
}
pub(super) fn ip_method(method: &IpMethod) -> &'static str {
    match method {
        IpMethod::Automatic => "Automatic",
        IpMethod::Static(_) => "Static",
        IpMethod::Disabled => "Disabled",
        IpMethod::Unmanaged => "Managed elsewhere",
        IpMethod::Unknown => "Unknown / unsupported",
    }
}
