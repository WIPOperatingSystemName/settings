use super::BatteryController;
use crate::state::{Editor, Page};
use telorgon::app::Component;

#[derive(Clone, PartialEq)]
pub struct NavigationController {
    editor: Editor,
    battery: BatteryController,
}
impl NavigationController {
    pub(super) fn new(editor: Editor, battery: BatteryController) -> Self {
        Self { editor, battery }
    }
    pub fn selected(&self, observer: &impl Component) -> Page {
        let page = *observer.watch(&self.editor.page);
        if page == Page::Battery && !self.has_battery(observer) {
            Page::Power
        } else {
            page
        }
    }
    pub fn has_battery(&self, observer: &impl Component) -> bool {
        self.battery.has_battery(observer)
    }
    pub fn select(&self, page: Page) {
        self.editor.select(page);
    }
}

#[cfg(test)]
mod tests;
