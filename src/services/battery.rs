use std::{sync::mpsc, thread, time::Duration};
use telorgon::{
    app::{Signal, SignalWriter},
    battery::{self, BatteryError, BatteryKind, BatteryMetrics, BatteryScope, BatterySnapshot},
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BatteryReadings {
    pub batteries: Vec<BatteryMetrics>,
    pub external_power: Option<bool>,
    pub error: Option<String>,
    pub loading: bool,
}
impl BatteryReadings {
    pub fn has_battery(&self) -> bool {
        !self.batteries.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn from_snapshot(snapshot: BatterySnapshot) -> Self {
        let mut readings = Self::default();
        readings.update(Ok(snapshot));
        readings
    }

    fn update(&mut self, result: Result<BatterySnapshot, BatteryError>) {
        self.loading = false;
        match result {
            Ok(snapshot) => {
                self.batteries = snapshot
                    .batteries
                    .into_iter()
                    .filter(installed_system_battery)
                    .collect();
                self.external_power = snapshot.external_power;
                self.error = None;
            }
            // A failed read does not prove previously detected hardware disappeared.
            Err(error) => self.error = Some(error.to_string()),
        }
    }
}

fn installed_system_battery(battery: &BatteryMetrics) -> bool {
    battery.present && battery.kind == BatteryKind::Battery && battery.scope != BatteryScope::Device
}

#[derive(Clone, PartialEq)]
pub struct BatteryModel {
    pub signal: Signal<BatteryReadings>,
}
impl BatteryModel {
    #[cfg(test)]
    pub(crate) fn from_signal(signal: Signal<BatteryReadings>) -> Self {
        Self { signal }
    }
}

pub struct BatteryService {
    stop: mpsc::Sender<()>,
    worker: Option<thread::JoinHandle<()>>,
}
impl BatteryService {
    pub fn start() -> Result<(Self, BatteryModel), Box<dyn std::error::Error>> {
        let readings = BatteryReadings {
            loading: true,
            ..Default::default()
        };
        let (signal, writer) = Signal::new(readings.clone());
        let (stop, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("settings-battery-readings".into())
            .spawn(move || run(receiver, writer, readings))?;
        Ok((
            Self {
                stop,
                worker: Some(worker),
            },
            BatteryModel { signal },
        ))
    }
}
impl Drop for BatteryService {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run(
    receiver: mpsc::Receiver<()>,
    writer: SignalWriter<BatteryReadings>,
    mut readings: BatteryReadings,
) {
    loop {
        readings.update(battery::read_snapshot());
        // The observation timestamp is intentionally excluded: unchanged readings need no redraw.
        writer.publish_if_changed(readings.clone());
        match receiver.recv_timeout(Duration::from_secs(5)) {
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn battery(id: &str, kind: BatteryKind, scope: BatteryScope, present: bool) -> BatteryMetrics {
        BatteryMetrics {
            id: id.into(),
            kind,
            scope,
            present,
            manufacturer: None,
            model: None,
            technology: None,
            state: battery::BatteryState::Discharging,
            charge_percent: Some(70.0),
            health: None,
            capacity_health_percent: None,
            cycle_count: None,
            energy_now_wh: None,
            energy_full_wh: None,
            energy_full_design_wh: None,
            energy_empty_wh: None,
            charge_now_ah: None,
            charge_full_ah: None,
            charge_full_design_ah: None,
            charge_empty_ah: None,
            power_watts: None,
            voltage_volts: None,
            current_amps: None,
            temperature_celsius: None,
            time_to_empty: None,
            time_to_full: None,
        }
    }

    fn snapshot(batteries: Vec<BatteryMetrics>) -> BatterySnapshot {
        BatterySnapshot {
            observed_at: std::time::SystemTime::now(),
            batteries,
            external_supplies: Vec::new(),
            external_power: Some(false),
        }
    }

    #[test]
    fn detects_present_host_batteries_without_requiring_scope_reporting() {
        let mut readings = BatteryReadings::default();
        readings.update(Ok(snapshot(vec![
            battery("BAT0", BatteryKind::Battery, BatteryScope::System, true),
            battery("BAT1", BatteryKind::Battery, BatteryScope::Unknown, true),
            battery("mouse", BatteryKind::Battery, BatteryScope::Device, true),
            battery("ups", BatteryKind::Ups, BatteryScope::System, true),
            battery("empty", BatteryKind::Battery, BatteryScope::System, false),
        ])));
        assert_eq!(
            readings
                .batteries
                .iter()
                .map(|battery| battery.id.as_str())
                .collect::<Vec<_>>(),
            ["BAT0", "BAT1"]
        );
        assert!(readings.has_battery());
    }

    #[test]
    fn read_failure_retains_detection_until_a_successful_removal() {
        let mut readings = BatteryReadings::default();
        readings.update(Ok(snapshot(vec![battery(
            "BAT0",
            BatteryKind::Battery,
            BatteryScope::System,
            true,
        )])));
        let last_batteries = readings.batteries.clone();
        readings.update(Err(BatteryError::Unsupported));
        assert!(readings.has_battery());
        assert_eq!(readings.batteries, last_batteries);
        assert_eq!(readings.external_power, Some(false));
        assert!(readings.error.is_some());

        readings.update(Ok(snapshot(Vec::new())));
        assert!(!readings.has_battery());
        assert!(readings.error.is_none());
    }
}
