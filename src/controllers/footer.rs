use super::{DisplayController, Page, SoundController, Sources};
use telorgon::app::Component;

pub enum FooterActions {
    None,
    Confirm {
        label: String,
        enabled: bool,
    },
    Save {
        label: &'static str,
        enabled: bool,
        reset_enabled: bool,
    },
}
pub struct FooterState {
    pub message: String,
    pub notice: Option<&'static str>,
    pub actions: FooterActions,
}
#[derive(Clone, PartialEq)]
pub struct FooterController {
    sources: Sources,
    display: DisplayController,
    sound: SoundController,
}
impl FooterController {
    pub(super) fn new(
        sources: Sources,
        display: DisplayController,
        sound: SoundController,
    ) -> Self {
        Self {
            sources,
            display,
            sound,
        }
    }
    pub fn state(&self, observer: &impl Component) -> FooterState {
        let status = observer.watch(&self.sources.model.signal);
        let busy = status.busy || self.sources.model.choosing_background();
        let page = *observer.watch(&self.sources.editor.page);
        if matches!(
            page,
            Page::Network | Page::Power | Page::Battery | Page::Personalization
        ) && status.preview.is_none()
        {
            return FooterState {
                message: String::new(),
                notice: None,
                actions: FooterActions::None,
            };
        }
        let actions = if status.preview.is_some() {
            FooterActions::Confirm {
                label: format!(
                    "Keep changes ({}s)",
                    status
                        .shell
                        .as_ref()
                        .map_or(20, |s| s.display.seconds_remaining)
                ),
                enabled: !busy,
            }
        } else {
            FooterActions::Save {
                label: if busy {
                    "Saving…"
                } else if page == Page::Display {
                    "Save display"
                } else {
                    "Save sound"
                },
                enabled: !busy
                    && !status.load_error
                    && (page == Page::Sound
                        || (page == Page::Display
                            && status.shell.as_ref().is_some_and(|s| s.display.ready))),
                reset_enabled: !busy && !status.load_error,
            }
        };
        FooterState {
            message: status.message.clone(),
            notice: status.shell.is_none().then_some(
                "Shell unavailable. Sound preferences can still be saved for the next session.",
            ),
            actions,
        }
    }
    pub fn save(&self) {
        match *self.sources.editor.page.snapshot() {
            Page::Display => self.display.save(),
            Page::Sound => self.sound.save(),
            Page::Network | Page::Power | Page::Battery | Page::Personalization => {}
        }
    }
    pub fn reset(&self) {
        let status = self.sources.model.signal.snapshot();
        if !status.busy && !self.sources.model.choosing_background() && status.preview.is_none() {
            if *self.sources.editor.page.snapshot() != Page::Personalization {
                self.sources.editor.reset_page();
            }
        }
    }
    pub fn keep(&self) {
        let status = self.sources.model.signal.snapshot();
        if !status.busy && status.preview.is_some() {
            self.sources.model.keep();
        }
    }
    pub fn revert(&self) {
        let status = self.sources.model.signal.snapshot();
        if !status.busy && status.preview.is_some() {
            self.sources.model.revert();
        }
    }
}
