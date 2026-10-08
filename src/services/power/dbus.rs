use super::{PowerAvailability, PowerError, PowerProfile, PowerSnapshot, ProfileBackend};
use std::{collections::HashMap, time::Duration};
use zbus::{
    blocking::Connection,
    zvariant::{OwnedValue, Value},
};

const CALL_TIMEOUT: Duration = Duration::from_secs(2);
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

#[derive(Clone, Copy)]
struct Endpoint {
    bus: &'static str,
    path: &'static str,
    interface: &'static str,
}
const ENDPOINTS: [Endpoint; 2] = [
    Endpoint {
        bus: "org.freedesktop.UPower.PowerProfiles",
        path: "/org/freedesktop/UPower/PowerProfiles",
        interface: "org.freedesktop.UPower.PowerProfiles",
    },
    Endpoint {
        bus: "net.hadess.PowerProfiles",
        path: "/net/hadess/PowerProfiles",
        interface: "net.hadess.PowerProfiles",
    },
];

pub(super) struct Client {
    connection: Connection,
    endpoint: Option<Endpoint>,
}
impl Client {
    pub(super) fn connect() -> Result<Self, PowerError> {
        // The async builder also bounds bus connection/authentication, before using
        // blocking calls on this worker. No work runs on the UI thread.
        let connection = futures_lite::future::block_on(futures_lite::future::race(
            async {
                zbus::connection::Builder::system()
                    .map_err(error_message)?
                    .method_timeout(CALL_TIMEOUT)
                    .build()
                    .await
                    .map(Connection::from)
                    .map_err(error_message)
            },
            async {
                async_io::Timer::after(CALL_TIMEOUT).await;
                Err(PowerError::TimedOut)
            },
        ))?;
        Ok(Self {
            connection,
            endpoint: None,
        })
    }
    fn read_at(&self, endpoint: Endpoint) -> Result<PowerSnapshot, PowerError> {
        // GetAll is deliberately uncached so an external profile change and the
        // post-write confirmation always use the daemon's current properties.
        let reply = self
            .connection
            .call_method(
                Some(endpoint.bus),
                endpoint.path,
                Some(PROPERTIES),
                "GetAll",
                &(endpoint.interface,),
            )
            .map_err(error_message)?;
        let properties: HashMap<String, OwnedValue> =
            reply.body().deserialize().map_err(error_message)?;
        parse_properties(properties)
    }
}
impl ProfileBackend for Client {
    fn read(&mut self) -> Result<PowerSnapshot, PowerError> {
        if let Some(endpoint) = self.endpoint {
            return self.read_at(endpoint);
        }
        let mut last_error = PowerError::Unavailable;
        for endpoint in ENDPOINTS {
            match self.read_at(endpoint) {
                Ok(snapshot) => {
                    self.endpoint = Some(endpoint);
                    return Ok(snapshot);
                }
                Err(error) => {
                    if error != PowerError::Unavailable {
                        last_error = error;
                    }
                }
            }
        }
        Err(last_error)
    }
    fn set(&mut self, profile: PowerProfile) -> Result<(), PowerError> {
        let endpoint = self.endpoint.ok_or(PowerError::Unavailable)?;
        self.connection
            .call_method(
                Some(endpoint.bus),
                endpoint.path,
                Some(PROPERTIES),
                "Set",
                &(
                    endpoint.interface,
                    "ActiveProfile",
                    Value::from(profile.name()),
                ),
            )
            .map(|_| ())
            .map_err(error_message)
    }
}

fn parse_properties(
    mut properties: HashMap<String, OwnedValue>,
) -> Result<PowerSnapshot, PowerError> {
    let profiles = properties
        .remove("Profiles")
        .ok_or(PowerError::Unsupported)
        .and_then(|value| {
            Vec::<HashMap<String, OwnedValue>>::try_from(value).map_err(|_| PowerError::Unsupported)
        })?;
    let mut profiles: Vec<_> = profiles
        .iter()
        .filter_map(|profile| profile.get("Profile"))
        .filter_map(|value| <&str>::try_from(value).ok())
        .filter_map(PowerProfile::from_name)
        .collect();
    profiles.sort_unstable();
    profiles.dedup();
    if profiles.is_empty() {
        return Err(PowerError::Unsupported);
    }
    let active_name = properties
        .get("ActiveProfile")
        .and_then(|value| <&str>::try_from(value).ok())
        .filter(|name| !name.is_empty())
        .ok_or(PowerError::Unsupported)?;
    Ok(PowerSnapshot {
        availability: PowerAvailability::Ready,
        active: PowerProfile::from_name(active_name),
        active_name: Some(active_name.to_owned()),
        profiles,
        performance_degraded: optional_reason(&properties, "PerformanceDegraded"),
        performance_inhibited: optional_reason(&properties, "PerformanceInhibited"),
        message: String::new(),
    })
}
fn optional_reason(properties: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    properties
        .get(key)
        .and_then(|value| <&str>::try_from(value).ok())
        .filter(|reason| !reason.is_empty())
        .map(str::to_owned)
}

fn error_message(error: zbus::Error) -> PowerError {
    if let zbus::Error::MethodError(name, detail, _) = &error {
        return classify_error(name.as_str(), detail.as_deref().unwrap_or(""));
    }
    match error {
        zbus::Error::InputOutput(_) | zbus::Error::Handshake(_) => PowerError::Unavailable,
        zbus::Error::FDO(error) => match *error {
            zbus::fdo::Error::AccessDenied(_) => PowerError::PermissionDenied,
            zbus::fdo::Error::AuthFailed(_) => PowerError::AuthenticationRequired,
            zbus::fdo::Error::NoReply(_)
            | zbus::fdo::Error::Timeout(_)
            | zbus::fdo::Error::TimedOut(_) => PowerError::TimedOut,
            _ => PowerError::Unavailable,
        },
        _ => PowerError::Rejected,
    }
}
fn classify_error(name: &str, detail: &str) -> PowerError {
    let detail = detail.to_ascii_lowercase();
    if name.ends_with("InteractiveAuthorizationRequired")
        || name.ends_with("AuthFailed")
        || detail.contains("authentication is required")
        || detail.contains("authentication required")
        || detail.contains("not authorized")
    {
        return PowerError::AuthenticationRequired;
    }
    if name.ends_with("AccessDenied")
        || name.ends_with("PermissionDenied")
        || detail.contains("permission denied")
    {
        return PowerError::PermissionDenied;
    }
    if name.ends_with("NoReply") || name.ends_with("Timeout") || name.ends_with("TimedOut") {
        return PowerError::TimedOut;
    }
    if name.ends_with("ServiceUnknown")
        || name.ends_with("NameHasNoOwner")
        || name.ends_with("UnknownObject")
        || name.ends_with("Disconnected")
    {
        return PowerError::Unavailable;
    }
    if name.ends_with("UnknownInterface")
        || name.ends_with("UnknownProperty")
        || name.ends_with("UnknownMethod")
    {
        return PowerError::Unsupported;
    }
    PowerError::Rejected
}

#[cfg(test)]
mod tests {
    use super::*;

    fn properties(names: &[&str]) -> HashMap<String, OwnedValue> {
        let profiles: Vec<HashMap<String, OwnedValue>> = names
            .iter()
            .map(|name| {
                HashMap::from([(
                    "Profile".into(),
                    OwnedValue::try_from(Value::from(*name)).unwrap(),
                )])
            })
            .collect();
        HashMap::from([
            (
                "Profiles".into(),
                OwnedValue::try_from(Value::from(profiles)).unwrap(),
            ),
            (
                "ActiveProfile".into(),
                OwnedValue::try_from(Value::from("balanced")).unwrap(),
            ),
        ])
    }
    #[test]
    fn daemon_capabilities_hide_unsupported_profiles_and_ignore_unknown_entries() {
        let parsed = parse_properties(properties(&[
            "balanced",
            "future-profile",
            "power-saver",
            "balanced",
        ]))
        .unwrap();
        assert_eq!(
            parsed.profiles,
            vec![PowerProfile::PowerSaver, PowerProfile::Balanced]
        );
        assert_eq!(parsed.active, Some(PowerProfile::Balanced));
        assert_eq!(parsed.performance_degraded, None);
    }
    #[test]
    fn empty_or_unknown_capabilities_do_not_enable_controls() {
        assert_eq!(
            parse_properties(properties(&[])).unwrap_err(),
            PowerError::Unsupported
        );
        assert_eq!(
            parse_properties(properties(&["future-profile"])).unwrap_err(),
            PowerError::Unsupported
        );
    }
    #[test]
    fn authorization_and_absent_service_errors_have_distinct_messages() {
        assert_eq!(
            classify_error("org.freedesktop.DBus.Error.ServiceUnknown", ""),
            PowerError::Unavailable
        );
        assert_eq!(
            classify_error("org.freedesktop.DBus.Error.AccessDenied", ""),
            PowerError::PermissionDenied
        );
        assert_eq!(
            classify_error(
                "org.freedesktop.DBus.Error.InteractiveAuthorizationRequired",
                ""
            ),
            PowerError::AuthenticationRequired
        );
    }
}
