use super::*;
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Method {
    Automatic,
    Static,
    Disabled,
    Unmanaged,
    Unchanged,
}
impl Method {
    pub fn title(self) -> &'static str {
        match self {
            Self::Automatic => "Automatic",
            Self::Static => "Static",
            Self::Disabled => "Disabled",
            Self::Unmanaged => "Managed elsewhere (keep current)",
            Self::Unchanged => "Unsupported (keep current)",
        }
    }
}
pub(super) struct FamilyDraft {
    pub method: Method,
    pub addresses: EntryValue,
    pub gateway: EntryValue,
    pub dns: EntryValue,
    pub routes: EntryValue,
    pub automatic_dns: bool,
    pub automatic_routes: bool,
    baseline: IpSettings,
}
impl FamilyDraft {
    pub fn new(ip: &IpSettings) -> Self {
        Self {
            method: match ip.method {
                IpMethod::Automatic => Method::Automatic,
                IpMethod::Static(_) => Method::Static,
                IpMethod::Disabled => Method::Disabled,
                IpMethod::Unmanaged => Method::Unmanaged,
                IpMethod::Unknown => Method::Unchanged,
            },
            addresses: EntryValue::new(
                if let IpMethod::Static(addresses) = &ip.method {
                    addresses
                        .iter()
                        .map(|a| format!("{}/{}", a.address, a.prefix))
                        .collect::<Vec<_>>()
                        .join(", ")
                } else {
                    String::new()
                },
                false,
            ),
            gateway: EntryValue::new(ip.gateway.map(|v| v.to_string()).unwrap_or_default(), false),
            dns: EntryValue::new(
                ip.dns
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
                false,
            ),
            routes: EntryValue::new(
                ip.routes
                    .iter()
                    .map(|r| {
                        format!(
                            "{}/{}{}{}",
                            r.destination.address,
                            r.destination.prefix,
                            r.gateway.map(|v| format!(" via {v}")).unwrap_or_default(),
                            r.metric.map(|v| format!(" metric {v}")).unwrap_or_default()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; "),
                false,
            ),
            automatic_dns: !ip.ignore_automatic_dns,
            automatic_routes: !ip.ignore_automatic_routes,
            baseline: ip.clone(),
        }
    }
    pub fn editable(&self) -> bool {
        !matches!(
            self.method,
            Method::Disabled | Method::Unmanaged | Method::Unchanged
        )
    }
    pub fn build(&self, family: IpFamily) -> std::result::Result<IpSettings, String> {
        if self.method == Method::Disabled {
            if self.baseline.method == IpMethod::Disabled {
                return Ok(self.baseline.clone());
            }
            return Ok(IpSettings {
                method: IpMethod::Disabled,
                ..Default::default()
            });
        }
        if matches!(self.method, Method::Unmanaged | Method::Unchanged) {
            return Ok(self.baseline.clone());
        }
        Ok(IpSettings {
            method: if self.method == Method::Automatic {
                IpMethod::Automatic
            } else {
                IpMethod::Static(addresses(&self.addresses.text(), family)?)
            },
            gateway: if self.method == Method::Static
                || (self.method == Method::Automatic && self.baseline.method == IpMethod::Automatic)
            {
                optional_ip(&self.gateway.text(), family)?
            } else {
                None
            },
            dns: ip_list(&self.dns.text(), family)?,
            routes: routes(&self.routes.text(), family)?,
            ignore_automatic_dns: !self.automatic_dns,
            ignore_automatic_routes: !self.automatic_routes,
        })
    }
}
fn ip(value: &str, family: IpFamily) -> std::result::Result<std::net::IpAddr, String> {
    let ip = value
        .parse::<std::net::IpAddr>()
        .map_err(|_| "Enter a valid IP address")?;
    if ip.is_ipv4() != (family == IpFamily::V4) {
        return Err("Address does not match the selected IP family".into());
    }
    Ok(ip)
}
fn prefix(value: &str, family: IpFamily) -> std::result::Result<IpAddress, String> {
    let (value, prefix) = value
        .split_once('/')
        .ok_or("Include an address prefix, such as /24 or /64")?;
    IpAddress::new(
        ip(value, family)?,
        prefix.parse().map_err(|_| "Enter a valid address prefix")?,
    )
    .map_err(|_| "Prefix is out of range for this IP family".into())
}
fn split(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(|ch: char| ch == ',' || ch.is_whitespace())
        .filter(|s| !s.is_empty())
}
fn addresses(value: &str, family: IpFamily) -> std::result::Result<Vec<IpAddress>, String> {
    let values = split(value)
        .map(|v| prefix(v, family))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if values.is_empty() {
        return Err("A static configuration needs at least one address".into());
    }
    Ok(values)
}
fn optional_ip(
    value: &str,
    family: IpFamily,
) -> std::result::Result<Option<std::net::IpAddr>, String> {
    if value.trim().is_empty() {
        Ok(None)
    } else {
        ip(value.trim(), family).map(Some)
    }
}
fn ip_list(value: &str, family: IpFamily) -> std::result::Result<Vec<std::net::IpAddr>, String> {
    split(value).map(|v| ip(v, family)).collect()
}
fn routes(value: &str, family: IpFamily) -> std::result::Result<Vec<NetworkRoute>, String> {
    value
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|row| {
            let mut parts = row.split_whitespace();
            let destination = prefix(parts.next().ok_or("Route needs a destination")?, family)?;
            let (mut gateway, mut metric) = (None, None);
            while let Some(key) = parts.next() {
                let value = parts.next().ok_or("Route option needs a value")?;
                match key {
                    "via" if gateway.is_none() => gateway = Some(ip(value, family)?),
                    "metric" if metric.is_none() => {
                        metric = Some(
                            value
                                .parse::<u32>()
                                .map_err(|_| "Route metric must be a nonnegative integer")?,
                        )
                    }
                    _ => return Err("Use 'via gateway' and 'metric number' once per route".into()),
                }
            }
            Ok(NetworkRoute {
                destination,
                gateway,
                metric,
            })
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn network_drafts_roundtrip_both_families_and_validate_routes() {
        for family in [IpFamily::V4, IpFamily::V6] {
            let address = if family == IpFamily::V4 {
                "192.0.2.10/24"
            } else {
                "2001:db8::10/64"
            };
            let gateway = if family == IpFamily::V4 {
                "192.0.2.1"
            } else {
                "fe80::1"
            };
            let baseline = IpSettings {
                method: IpMethod::Static(addresses(address, family).unwrap()),
                gateway: Some(ip(gateway, family).unwrap()),
                dns: vec![ip(gateway, family).unwrap()],
                routes: routes(&format!("{address} via {gateway} metric 50"), family).unwrap(),
                ignore_automatic_dns: false,
                ignore_automatic_routes: true,
            };
            let mut draft = FamilyDraft::new(&baseline);
            assert_eq!(draft.build(family).unwrap(), baseline);
            draft.method = Method::Disabled;
            assert_eq!(
                draft.build(family).unwrap(),
                IpSettings {
                    method: IpMethod::Disabled,
                    ..Default::default()
                }
            );
        }
        for method in [
            IpMethod::Automatic,
            IpMethod::Disabled,
            IpMethod::Unmanaged,
            IpMethod::Unknown,
        ] {
            let baseline = IpSettings {
                method,
                gateway: Some("192.0.2.1".parse().unwrap()),
                dns: vec!["192.0.2.53".parse().unwrap()],
                ..Default::default()
            };
            assert_eq!(
                FamilyDraft::new(&baseline).build(IpFamily::V4).unwrap(),
                baseline
            );
        }
        for invalid in [
            "192.0.2.0/33",
            "2001:db8::/64",
            "192.0.2.0/24 via fe80::1",
            "192.0.2.0/24 metric -1",
            "192.0.2.0/24 via 192.0.2.1 via 192.0.2.2",
        ] {
            assert!(routes(invalid, IpFamily::V4).is_err());
        }
    }
}
