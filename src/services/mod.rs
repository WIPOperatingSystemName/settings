mod backend;
mod background_library;
mod battery;
mod network;
mod personalization;
mod power;
#[cfg(test)]
pub(crate) use battery::BatteryReadings;
pub use battery::{BatteryModel, BatteryService};
pub use network::{NetworkModel, NetworkOperation, NetworkService};
pub use personalization::{BackgroundTile, PersonalizationPreview};
pub use power::{PowerAvailability, PowerModel, PowerProfile, PowerService};

pub use backend::{Backend, Model, Status};

pub(crate) use network::error_message;
