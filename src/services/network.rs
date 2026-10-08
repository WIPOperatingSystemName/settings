use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};
use telorgon::{
    app::{Signal, SignalWriter},
    network::*,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetworkOperation {
    pub busy: bool,
    pub message: String,
    pub failed: bool,
}
struct Job {
    commands: Vec<NetworkCommand>,
    success: String,
}
#[derive(Clone)]
pub struct NetworkModel {
    pub handle: NetworkHandle,
    pub operation: Signal<NetworkOperation>,
    writer: SignalWriter<NetworkOperation>,
    sender: Arc<mpsc::SyncSender<Job>>,
    busy: Arc<AtomicBool>,
}
impl PartialEq for NetworkModel {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle && self.operation == other.operation
    }
}
impl NetworkModel {
    pub fn apply(&self, commands: Vec<NetworkCommand>, success: impl Into<String>) -> bool {
        if self
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        self.writer.publish(NetworkOperation {
            busy: true,
            message: "Applying network changes…".into(),
            failed: false,
        });
        if self
            .sender
            .try_send(Job {
                commands,
                success: success.into(),
            })
            .is_err()
        {
            self.busy.store(false, Ordering::Release);
            self.error("Network settings are unavailable");
            return false;
        }
        true
    }
    pub fn copy(&self, text: String) {
        let model = self.clone();
        crate::components::clipboard::copy(text, move |result| {
            if model.busy.load(Ordering::Acquire) {
                return;
            }
            match result {
                Ok(()) => {
                    model.writer.publish(NetworkOperation {
                        busy: false,
                        message: "Copied to clipboard".into(),
                        failed: false,
                    });
                }
                Err(error) => model.error(error),
            }
        });
    }
    pub fn error(&self, message: impl Into<String>) {
        if !self.busy.load(Ordering::Acquire) {
            self.writer.publish(NetworkOperation {
                busy: false,
                message: message.into(),
                failed: true,
            });
        }
    }
}

pub struct NetworkService {
    controller: NetworkController,
    stopped: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl NetworkService {
    pub fn start() -> Result<(Self, NetworkModel), Box<dyn std::error::Error>> {
        Self::with_controller(NetworkController::new(Default::default())?)
    }
    pub(crate) fn with_controller(
        mut controller: NetworkController,
    ) -> Result<(Self, NetworkModel), Box<dyn std::error::Error>> {
        controller.start()?;
        let handle = controller.handle();
        let (operation, writer) = Signal::new(NetworkOperation::default());
        let (sender, receiver) = mpsc::sync_channel::<Job>(1);
        let busy = Arc::new(AtomicBool::new(false));
        let stopped = Arc::new(AtomicBool::new(false));
        let model = NetworkModel {
            handle: handle.clone(),
            operation,
            writer: writer.clone(),
            sender: Arc::new(sender),
            busy: busy.clone(),
        };
        let worker_stop = stopped.clone();
        let worker = thread::Builder::new().name("settings-network-operations".into()).spawn(move || {
            while !worker_stop.load(Ordering::Acquire) {
                let job = match receiver.recv_timeout(std::time::Duration::from_millis(100)) {
                    Ok(job) => job,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(_) => break,
                };
                let mut result = Ok(());
                let mut updated_profile = None;
                let mut profile_saved = false;
                for command in job.commands {
                    if worker_stop.load(Ordering::Acquire) { break; }
                    if let NetworkCommand::UpdateProfile { profile, .. } = &command { updated_profile = Some(*profile); }
                    if let (NetworkCommand::Reapply(interface), Some(profile)) = (&command, updated_profile) {
                        let snapshot = handle.signal().snapshot();
                        let active = snapshot.interfaces.iter().find(|candidate| candidate.id == *interface)
                            .and_then(|candidate| candidate.active_connection)
                            .and_then(|active| snapshot.connections.iter().find(|connection| connection.id == active))
                            .and_then(|connection| connection.profile);
                        if active != Some(profile) {
                            result = Err("Connection settings saved, but the active connection changed. Reconnect to apply them".into());
                            break;
                        }
                    }
                    let saving_profile = matches!(command, NetworkCommand::UpdateProfile { .. });
                    result = handle.execute(command).map_err(|error| error_message(&error))
                        .and_then(|request| outcome(futures_lite::future::block_on(request.completion())));
                    if result.is_err() {
                        if profile_saved { result = result.map_err(|error| format!("Connection settings were saved, but a subsequent change failed. {error}")); }
                        break;
                    }
                    profile_saved |= saving_profile;
                }
                let failed = result.is_err();
                writer.publish(NetworkOperation { busy: false, message: result.err().unwrap_or(job.success), failed });
                busy.store(false, Ordering::Release);
            }
        })?;
        Ok((
            Self {
                controller,
                stopped,
                worker: Some(worker),
            },
            model,
        ))
    }
}
impl Drop for NetworkService {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        let _ = self.controller.shutdown();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn outcome(outcome: NetworkOutcome) -> Result<(), String> {
    match outcome {
        NetworkOutcome::Applied(_) => Ok(()),
        NetworkOutcome::Failed(error) => Err(error_message(&error)),
        NetworkOutcome::Unconfirmed(error) => Err(format!(
            "The change could not be confirmed. {} Check the current connection before trying again.",
            error_message(&error)
        )),
        NetworkOutcome::Cancelled => Err("The network operation was cancelled".into()),
    }
}
pub(crate) fn error_message(error: &NetworkError) -> String {
    match error {
        NetworkError::PermissionDenied => "Permission to change networking was denied",
        NetworkError::AuthorizationRequired => "Authentication is required to change networking",
        NetworkError::AuthenticationFailed => "Authentication failed. Check the Wi-Fi password",
        NetworkError::CredentialsRequired => "Enter a Wi-Fi password",
        NetworkError::IpConfigurationFailed => "An IP address could not be configured",
        NetworkError::Stale => "This adapter or connection has changed. Select it again",
        NetworkError::Unsupported => "This operation is not supported for this connection",
        NetworkError::Unavailable | NetworkError::Stopped | NetworkError::Transport => {
            "Network management is unavailable"
        }
        NetworkError::TimedOut => "The network operation timed out",
        NetworkError::Busy => "Another network operation is in progress",
        NetworkError::InvalidConfig(message) => message,
        _ => "The network change failed",
    }
    .into()
}
