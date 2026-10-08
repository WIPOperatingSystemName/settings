use crate::services::NetworkModel;
use telorgon::{app::Component, network::*};

#[derive(Clone, PartialEq)]
pub struct NetworkController {
    pub model: NetworkModel,
}
impl NetworkController {
    pub(super) fn new(model: NetworkModel) -> Self {
        Self { model }
    }
    pub fn snapshot(
        &self,
        observer: &impl Component,
    ) -> telorgon::app::SignalSnapshot<NetworkSnapshot> {
        observer.watch(&self.model.handle.signal())
    }
    pub fn operation(
        &self,
        observer: &impl Component,
    ) -> telorgon::app::SignalSnapshot<crate::services::NetworkOperation> {
        observer.watch(&self.model.operation)
    }
    pub fn apply(&self, commands: Vec<NetworkCommand>, success: &str) -> bool {
        self.model.apply(commands, success)
    }
    pub fn copy(&self, value: String) {
        self.model.copy(value);
    }
    pub fn error(&self, message: impl Into<String>) {
        self.model.error(message);
    }
}
