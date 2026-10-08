mod battery;
mod display;
mod footer;
mod navigation;
mod network;
mod personalization;
mod power;
mod sound;
pub use network::NetworkController;

pub use crate::state::Page;
pub use battery::{BatteryController, InstalledBattery};
pub use display::DisplayController;
pub use footer::{FooterActions, FooterController};
pub use navigation::NavigationController;
pub use personalization::{BackgroundTileState, PersonalizationController};
pub use power::PowerController;
pub use sound::{ChannelState, SoundController};

use crate::{
    services::{Model, Status},
    state::Editor,
};
use telorgon::app::{Component, SignalSnapshot};
use telorgon::services::desktop_settings::Preferences;

#[derive(Clone, PartialEq)]
struct Sources {
    model: Model,
    editor: Editor,
}
impl Sources {
    // Register the owning component with the existing signal system. Derived state needs
    // no worker or duplicate store, and consumers never receive the mutable draft handles.
    fn observe(
        &self,
        observer: &impl Component,
    ) -> (SignalSnapshot<Status>, SignalSnapshot<Preferences>) {
        (
            observer.watch(&self.model.signal),
            observer.watch(&self.editor.draft),
        )
    }
}

#[derive(Clone, PartialEq)]
pub struct SettingsController {
    pub navigation: NavigationController,
    pub display: DisplayController,
    pub sound: SoundController,
    pub network: NetworkController,
    pub power: PowerController,
    pub battery: BatteryController,
    pub personalization: PersonalizationController,
    pub footer: FooterController,
}
impl SettingsController {
    pub fn new(
        model: Model,
        preferences: Preferences,
        network: crate::services::NetworkModel,
        power: crate::services::PowerModel,
        battery: crate::services::BatteryModel,
        page: Page,
    ) -> Self {
        let sources = Sources {
            model,
            editor: Editor::new(preferences),
        };
        let battery = BatteryController::new(battery);
        let navigation = NavigationController::new(sources.editor.clone(), battery.clone());
        navigation.select(page);
        let display = DisplayController::new(sources.clone());
        let sound = SoundController::new(sources.clone());
        let network = NetworkController::new(network);
        let power = PowerController::new(power);
        let personalization = PersonalizationController::new(sources.clone());
        let footer = FooterController::new(sources, display.clone(), sound.clone());
        Self {
            navigation,
            display,
            sound,
            network,
            power,
            battery,
            personalization,
            footer,
        }
    }
}
