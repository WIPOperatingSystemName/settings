mod dbus;

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use telorgon::app::{Signal, SignalWriter};

const REFRESH_INTERVAL: Duration = Duration::from_secs(4);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PowerProfile {
    PowerSaver,
    Balanced,
    Performance,
}
impl PowerProfile {
    pub fn label(self) -> &'static str {
        match self {
            Self::PowerSaver => "Power Saver",
            Self::Balanced => "Balanced",
            Self::Performance => "Performance",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::PowerSaver => "Use less energy and reduce heat and fan noise.",
            Self::Balanced => "Balance performance with energy use.",
            Self::Performance => "Favor performance, with higher energy use and heat.",
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::PowerSaver => "power-saver",
            Self::Balanced => "balanced",
            Self::Performance => "performance",
        }
    }
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "power-saver" => Some(Self::PowerSaver),
            "balanced" => Some(Self::Balanced),
            "performance" => Some(Self::Performance),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PowerAvailability {
    #[default]
    Checking,
    Ready,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PowerSnapshot {
    pub availability: PowerAvailability,
    pub profiles: Vec<PowerProfile>,
    pub active: Option<PowerProfile>,
    pub active_name: Option<String>,
    pub performance_degraded: Option<String>,
    pub performance_inhibited: Option<String>,
    pub message: String,
}
impl Default for PowerSnapshot {
    fn default() -> Self {
        Self {
            availability: PowerAvailability::Checking,
            profiles: Vec::new(),
            active: None,
            active_name: None,
            performance_degraded: None,
            performance_inhibited: None,
            message: "Checking available power modes…".into(),
        }
    }
}
impl PowerSnapshot {
    pub fn can_select(&self, profile: PowerProfile) -> bool {
        self.availability == PowerAvailability::Ready
            && self.profiles.contains(&profile)
            && !(profile == PowerProfile::Performance && self.performance_inhibited.is_some())
    }
    fn unavailable(error: &PowerError) -> Self {
        Self {
            availability: PowerAvailability::Unavailable,
            message: error.message().into(),
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PowerOperation {
    pub busy: bool,
    pub failed: bool,
    pub message: String,
}

#[derive(Clone)]
pub struct PowerModel {
    pub signal: Signal<PowerSnapshot>,
    pub operation: Signal<PowerOperation>,
    writer: SignalWriter<PowerOperation>,
    sender: Arc<mpsc::SyncSender<Command>>,
    busy: Arc<AtomicBool>,
}
impl PartialEq for PowerModel {
    fn eq(&self, other: &Self) -> bool {
        self.signal == other.signal && self.operation == other.operation
    }
}
impl PowerModel {
    pub fn set_profile(&self, profile: PowerProfile) -> bool {
        let snapshot = self.signal.snapshot();
        if !snapshot.can_select(profile)
            || snapshot.active == Some(profile)
            || self
                .busy
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return false;
        }
        self.writer.publish(PowerOperation {
            busy: true,
            failed: false,
            message: format!("Applying {}…", profile.label()),
        });
        if self.sender.try_send(Command::Set(profile)).is_err() {
            self.writer.publish(PowerOperation {
                busy: false,
                failed: true,
                message: "Power mode controls are unavailable".into(),
            });
            self.busy.store(false, Ordering::Release);
            return false;
        }
        true
    }
}
enum Command {
    Set(PowerProfile),
    Stop,
}

pub struct PowerService {
    stopped: Arc<AtomicBool>,
    sender: Arc<mpsc::SyncSender<Command>>,
    worker: Option<thread::JoinHandle<()>>,
}
impl PowerService {
    pub fn start() -> Result<(Self, PowerModel), Box<dyn std::error::Error>> {
        let (signal, snapshot_writer) = Signal::new(PowerSnapshot::default());
        let (operation, writer) = Signal::new(PowerOperation::default());
        let (sender, receiver) = mpsc::sync_channel(1);
        let sender = Arc::new(sender);
        let busy = Arc::new(AtomicBool::new(false));
        let stopped = Arc::new(AtomicBool::new(false));
        let model = PowerModel {
            signal,
            operation,
            writer: writer.clone(),
            sender: sender.clone(),
            busy: busy.clone(),
        };
        let worker_stop = stopped.clone();
        let worker = thread::Builder::new()
            .name("settings-power-profiles".into())
            .spawn(move || run(receiver, snapshot_writer, writer, busy, worker_stop))?;
        Ok((
            Self {
                stopped,
                sender,
                worker: Some(worker),
            },
            model,
        ))
    }
}
impl Drop for PowerService {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        let _ = self.sender.try_send(Command::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PowerError {
    Unavailable,
    PermissionDenied,
    AuthenticationRequired,
    TimedOut,
    Unsupported,
    Stale,
    Rejected,
    Unconfirmed,
    Cancelled,
}
impl PowerError {
    fn message(self) -> &'static str {
        match self {
            Self::Unavailable => {
                "Power modes are unavailable. A compatible power profile service is required."
            }
            Self::PermissionDenied => "Permission to change the power mode was denied.",
            Self::AuthenticationRequired => "Authentication is required to change the power mode.",
            Self::TimedOut => "The power profile service did not respond. Try again.",
            Self::Unsupported => {
                "This power profile service does not expose supported power modes."
            }
            Self::Stale => "The available power modes changed. Choose an available mode again.",
            Self::Rejected => {
                "The system could not apply that power mode. Check the current mode and try again."
            }
            Self::Unconfirmed => {
                "The power mode change could not be confirmed. The current mode is unavailable until the service responds."
            }
            Self::Cancelled => "The power mode change was cancelled.",
        }
    }
}

trait ProfileBackend {
    fn read(&mut self) -> Result<PowerSnapshot, PowerError>;
    fn set(&mut self, profile: PowerProfile) -> Result<(), PowerError>;
}

struct ChangeResult {
    snapshot: PowerSnapshot,
    outcome: Result<(), PowerError>,
}
// Read capabilities immediately before writing, and show only the confirmed readback.
fn change_profile(
    backend: &mut impl ProfileBackend,
    desired: PowerProfile,
    stopped: &AtomicBool,
) -> ChangeResult {
    let before = match backend.read() {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return ChangeResult {
                snapshot: PowerSnapshot::unavailable(&error),
                outcome: Err(error),
            };
        }
    };
    if !before.can_select(desired) {
        return ChangeResult {
            snapshot: before,
            outcome: Err(PowerError::Stale),
        };
    }
    if stopped.load(Ordering::Acquire) {
        return ChangeResult {
            snapshot: before,
            outcome: Err(PowerError::Cancelled),
        };
    }
    if before.active == Some(desired) {
        return ChangeResult {
            snapshot: before,
            outcome: Ok(()),
        };
    }
    let result = backend.set(desired);
    if stopped.load(Ordering::Acquire) {
        return ChangeResult {
            snapshot: PowerSnapshot::unavailable(&PowerError::Unconfirmed),
            outcome: Err(PowerError::Cancelled),
        };
    }
    match backend.read() {
        Ok(snapshot) => {
            let outcome = result.and_then(|()| {
                if snapshot.active == Some(desired) {
                    Ok(())
                } else {
                    Err(PowerError::Rejected)
                }
            });
            ChangeResult { snapshot, outcome }
        }
        Err(_) => ChangeResult {
            snapshot: PowerSnapshot::unavailable(&PowerError::Unconfirmed),
            outcome: Err(result.err().unwrap_or(PowerError::Unconfirmed)),
        },
    }
}

fn run(
    receiver: mpsc::Receiver<Command>,
    snapshot_writer: SignalWriter<PowerSnapshot>,
    operation_writer: SignalWriter<PowerOperation>,
    busy: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
) {
    let mut client = None;
    while !stopped.load(Ordering::Acquire) {
        let connection_result = if client.is_none() {
            dbus::Client::connect().map(|connected| client = Some(connected))
        } else {
            Ok(())
        };
        if stopped.load(Ordering::Acquire) {
            break;
        }
        let result = connection_result.and_then(|()| {
            client
                .as_mut()
                .ok_or(PowerError::Unavailable)
                .and_then(ProfileBackend::read)
        });
        let snapshot = match result {
            Ok(snapshot) => snapshot,
            Err(error) => {
                client = None;
                PowerSnapshot::unavailable(&error)
            }
        };
        snapshot_writer.publish_if_changed(snapshot);
        let command = match receiver.recv_timeout(REFRESH_INTERVAL) {
            Ok(command) => command,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        };
        if stopped.load(Ordering::Acquire) || matches!(command, Command::Stop) {
            break;
        }
        let Command::Set(profile) = command else {
            unreachable!()
        };
        let changed = match client.as_mut() {
            Some(client) => change_profile(client, profile, &stopped),
            None => ChangeResult {
                snapshot: PowerSnapshot::unavailable(&PowerError::Unavailable),
                outcome: Err(PowerError::Unavailable),
            },
        };
        if changed.snapshot.availability == PowerAvailability::Unavailable {
            client = None;
        }
        snapshot_writer.publish_if_changed(changed.snapshot);
        operation_writer.publish(PowerOperation {
            busy: false,
            failed: changed.outcome.is_err(),
            message: changed.outcome.map_or_else(
                |error| error.message().into(),
                |()| format!("Power mode changed to {}.", profile.label()),
            ),
        });
        busy.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    struct MockBackend {
        reads: VecDeque<Result<PowerSnapshot, PowerError>>,
        writes: Vec<PowerProfile>,
    }
    impl ProfileBackend for MockBackend {
        fn read(&mut self) -> Result<PowerSnapshot, PowerError> {
            self.reads.pop_front().unwrap()
        }
        fn set(&mut self, profile: PowerProfile) -> Result<(), PowerError> {
            self.writes.push(profile);
            Ok(())
        }
    }
    fn snapshot(profiles: Vec<PowerProfile>, active: PowerProfile) -> PowerSnapshot {
        PowerSnapshot {
            availability: PowerAvailability::Ready,
            profiles,
            active: Some(active),
            active_name: Some(active.name().into()),
            ..Default::default()
        }
    }
    #[test]
    fn changed_capabilities_prevent_an_unsupported_write() {
        let mut backend = MockBackend {
            reads: VecDeque::from([Ok(snapshot(
                vec![PowerProfile::Balanced],
                PowerProfile::Balanced,
            ))]),
            writes: vec![],
        };
        let result = change_profile(
            &mut backend,
            PowerProfile::Performance,
            &AtomicBool::new(false),
        );
        assert_eq!(result.outcome, Err(PowerError::Stale));
        assert!(backend.writes.is_empty());
        assert_eq!(result.snapshot.profiles, vec![PowerProfile::Balanced]);
    }
    #[test]
    fn legacy_performance_inhibition_prevents_a_write() {
        let mut before = snapshot(
            vec![PowerProfile::Balanced, PowerProfile::Performance],
            PowerProfile::Balanced,
        );
        before.performance_inhibited = Some("lap-detected".into());
        let mut backend = MockBackend {
            reads: VecDeque::from([Ok(before)]),
            writes: vec![],
        };
        let result = change_profile(
            &mut backend,
            PowerProfile::Performance,
            &AtomicBool::new(false),
        );
        assert_eq!(result.outcome, Err(PowerError::Stale));
        assert!(backend.writes.is_empty());
    }
    #[test]
    fn accepted_write_does_not_override_the_confirmed_active_profile() {
        let available = vec![PowerProfile::Balanced, PowerProfile::Performance];
        let mut backend = MockBackend {
            reads: VecDeque::from([
                Ok(snapshot(available.clone(), PowerProfile::Balanced)),
                Ok(snapshot(available, PowerProfile::Balanced)),
            ]),
            writes: vec![],
        };
        let result = change_profile(
            &mut backend,
            PowerProfile::Performance,
            &AtomicBool::new(false),
        );
        assert_eq!(result.outcome, Err(PowerError::Rejected));
        assert_eq!(result.snapshot.active, Some(PowerProfile::Balanced));
        assert_eq!(backend.writes, vec![PowerProfile::Performance]);
    }
    #[test]
    fn failed_readback_clears_the_previous_selection() {
        let mut backend = MockBackend {
            reads: VecDeque::from([
                Ok(snapshot(
                    vec![PowerProfile::Balanced, PowerProfile::PowerSaver],
                    PowerProfile::Balanced,
                )),
                Err(PowerError::TimedOut),
            ]),
            writes: vec![],
        };
        let result = change_profile(
            &mut backend,
            PowerProfile::PowerSaver,
            &AtomicBool::new(false),
        );
        assert_eq!(result.outcome, Err(PowerError::Unconfirmed));
        assert_eq!(result.snapshot.active, None);
        assert_eq!(result.snapshot.availability, PowerAvailability::Unavailable);
    }
    #[test]
    fn successful_change_uses_service_readback() {
        let available = vec![PowerProfile::Balanced, PowerProfile::PowerSaver];
        let mut backend = MockBackend {
            reads: VecDeque::from([
                Ok(snapshot(available.clone(), PowerProfile::Balanced)),
                Ok(snapshot(available, PowerProfile::PowerSaver)),
            ]),
            writes: vec![],
        };
        let result = change_profile(
            &mut backend,
            PowerProfile::PowerSaver,
            &AtomicBool::new(false),
        );
        assert_eq!(result.outcome, Ok(()));
        assert_eq!(result.snapshot.active, Some(PowerProfile::PowerSaver));
    }
}
