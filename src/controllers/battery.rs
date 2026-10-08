use crate::services::BatteryModel;
use std::time::Duration;
use telorgon::{
    app::Component,
    battery::{BatteryMetrics, BatteryState},
};

pub struct BatteryDetail {
    pub label: &'static str,
    pub value: String,
}

pub struct InstalledBattery {
    pub id: String,
    pub title: String,
    pub charge: String,
    pub state: &'static str,
    pub estimate: String,
    pub maximum_capacity: String,
    pub cycle_count: String,
    pub condition: String,
    pub details: Vec<BatteryDetail>,
}

pub struct BatteryPageState {
    pub batteries: Vec<InstalledBattery>,
    pub external_power: &'static str,
    pub warning: Option<String>,
    pub loading: bool,
}

#[derive(Clone, PartialEq)]
pub struct BatteryController {
    model: BatteryModel,
}
impl BatteryController {
    pub(super) fn new(model: BatteryModel) -> Self {
        Self { model }
    }

    pub fn has_battery(&self, observer: &impl Component) -> bool {
        observer.watch(&self.model.signal).has_battery()
    }

    pub fn state(&self, observer: &impl Component) -> BatteryPageState {
        let readings = observer.watch(&self.model.signal);
        BatteryPageState {
            batteries: readings.batteries.iter().map(installed_battery).collect(),
            external_power: match readings.external_power {
                Some(true) => "Connected",
                Some(false) => "Disconnected",
                None => "Not reported",
            },
            warning: readings.error.as_ref().map(|_| {
                if readings.has_battery() {
                    "Battery readings could not be refreshed. Showing the last available readings."
                        .into()
                } else {
                    "Battery readings are currently unavailable.".into()
                }
            }),
            loading: readings.loading,
        }
    }
}

fn installed_battery(battery: &BatteryMetrics) -> InstalledBattery {
    let mut details = vec![BatteryDetail {
        label: "Device",
        value: battery.id.clone(),
    }];
    for (label, value) in [
        ("Manufacturer", &battery.manufacturer),
        ("Model", &battery.model),
        ("Technology", &battery.technology),
    ] {
        if let Some(value) = value {
            details.push(BatteryDetail {
                label,
                value: value.clone(),
            });
        }
    }
    for (label, value, unit) in [
        (
            match battery.state {
                BatteryState::Charging => "Charging rate",
                BatteryState::Discharging => "Discharge rate",
                _ => "Battery power",
            },
            battery.power_watts,
            "W",
        ),
        ("Voltage", battery.voltage_volts, "V"),
        ("Current", battery.current_amps, "A"),
        ("Temperature", battery.temperature_celsius, "°C"),
        ("Remaining energy", battery.energy_now_wh, "Wh"),
        ("Full-charge energy", battery.energy_full_wh, "Wh"),
        ("Design energy", battery.energy_full_design_wh, "Wh"),
        ("Empty energy", battery.energy_empty_wh, "Wh"),
        ("Remaining charge", battery.charge_now_ah, "Ah"),
        ("Full-charge capacity", battery.charge_full_ah, "Ah"),
        ("Design capacity", battery.charge_full_design_ah, "Ah"),
        ("Empty capacity", battery.charge_empty_ah, "Ah"),
    ] {
        if let Some(value) = value.filter(|value| value.is_finite()) {
            details.push(BatteryDetail {
                label,
                value: format!("{value:.2} {unit}"),
            });
        }
    }
    InstalledBattery {
        id: battery.id.clone(),
        title: battery
            .model
            .as_ref()
            .filter(|model| !model.is_empty())
            .map_or_else(
                || battery.id.clone(),
                |model| format!("{model} · {}", battery.id),
            ),
        charge: finite(battery.charge_percent).map_or_else(
            || "Charge unavailable".into(),
            |percent| format!("{percent:.0}%"),
        ),
        state: match battery.state {
            BatteryState::Charging => "Charging",
            BatteryState::Discharging => "Running on battery",
            BatteryState::NotCharging => "Not charging",
            BatteryState::Full => "Fully charged",
            BatteryState::Unknown => "Charge state unavailable",
        },
        estimate: match battery.state {
            BatteryState::Charging => battery.time_to_full.map_or_else(
                || "Charging time estimate unavailable".into(),
                |estimate| format!("{} until full", approximate_duration(estimate.duration)),
            ),
            BatteryState::Discharging => battery.time_to_empty.map_or_else(
                || "Runtime estimate unavailable".into(),
                |estimate| format!("{} remaining", approximate_duration(estimate.duration)),
            ),
            BatteryState::Full => "Battery is at full charge".into(),
            BatteryState::NotCharging => "Battery is not currently charging".into(),
            BatteryState::Unknown => "Time estimate unavailable".into(),
        },
        maximum_capacity: finite(battery.capacity_health_percent).map_or_else(
            || "Not reported".into(),
            |percent| format!("{percent:.0}% of design capacity"),
        ),
        cycle_count: battery
            .cycle_count
            .map_or_else(|| "Not reported".into(), |cycles| cycles.to_string()),
        condition: battery
            .health
            .clone()
            .unwrap_or_else(|| "Not reported".into()),
        details,
    }
}

fn finite(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite())
}

fn approximate_duration(duration: Duration) -> String {
    if duration.as_secs() < 60 {
        return "Less than 1 min".into();
    }
    let minutes = duration.as_secs().div_ceil(60);
    match (minutes / 60, minutes % 60) {
        (0, minutes) => format!("About {minutes} min"),
        (hours, 0) => format!("About {hours} hr"),
        (hours, minutes) => format!("About {hours} hr {minutes} min"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_labels_do_not_claim_exact_predictions_or_zero_minutes() {
        assert_eq!(
            approximate_duration(Duration::from_secs(1)),
            "Less than 1 min"
        );
        assert_eq!(
            approximate_duration(Duration::from_secs(3_599)),
            "About 1 hr"
        );
        assert_eq!(
            approximate_duration(Duration::from_secs(12_000)),
            "About 3 hr 20 min"
        );
    }
}
