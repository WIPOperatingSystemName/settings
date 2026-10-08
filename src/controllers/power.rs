use crate::services::{PowerAvailability, PowerModel, PowerProfile};
use telorgon::app::Component;

#[derive(Clone, PartialEq)]
pub struct PowerController {
    model: PowerModel,
}
pub struct PowerChoice {
    pub profile: PowerProfile,
    pub selected: bool,
    pub enabled: bool,
}
pub struct PowerState {
    pub enabled: bool,
    pub current: String,
    pub choices: Vec<PowerChoice>,
    pub availability_message: String,
    pub performance_warning: Option<String>,
    pub operation_message: String,
    pub failed: bool,
}
impl PowerController {
    pub fn new(model: PowerModel) -> Self {
        Self { model }
    }
    pub fn state(&self, observer: &impl Component) -> PowerState {
        let snapshot = observer.watch(&self.model.signal);
        let operation = observer.watch(&self.model.operation);
        let performance_warning = if let Some(reason) = snapshot.performance_inhibited.as_deref() {
            Some(performance_warning(reason, true))
        } else {
            snapshot
                .performance_degraded
                .as_deref()
                .map(|reason| performance_warning(reason, false))
        };
        PowerState {
            enabled: snapshot.availability == PowerAvailability::Ready && !operation.busy,
            current: if snapshot.availability == PowerAvailability::Checking {
                "Checking…".into()
            } else {
                snapshot
                    .active
                    .map(|profile| profile.label().to_owned())
                    .unwrap_or_else(|| snapshot.active_name.clone().unwrap_or("Unavailable".into()))
            },
            choices: snapshot
                .profiles
                .iter()
                .map(|&profile| PowerChoice {
                    profile,
                    selected: snapshot.active == Some(profile),
                    enabled: snapshot.can_select(profile),
                })
                .collect(),
            availability_message: snapshot.message.clone(),
            performance_warning,
            operation_message: operation.message.clone(),
            failed: operation.failed,
        }
    }
    pub fn set_profile(&self, profile: PowerProfile) {
        self.model.set_profile(profile);
    }
}
fn performance_warning(reason: &str, inhibited: bool) -> String {
    let state = if inhibited { "unavailable" } else { "limited" };
    match reason {
        "lap-detected" => {
            format!("Performance is {state} because the device detects it is on a lap.")
        }
        "high-operating-temperature" => {
            format!("Performance is {state} because the device is running hot.")
        }
        _ => format!("Performance is currently {state} because of system conditions."),
    }
}
