use telorgon::app::{Signal, SignalWriter};
use telorgon::services::desktop_settings::{DisplayConfiguration, Preferences, SoundSettings};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Display,
    Sound,
    Network,
    Power,
    Battery,
    Personalization,
}
impl Page {
    pub fn from_arguments(arguments: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut arguments = arguments.into_iter();
        let mut page = Self::Display;
        while let Some(argument) = arguments.next() {
            let value = if argument == "--page" {
                arguments.next().ok_or(
                    "--page requires display, sound, network, power, battery, or personalization",
                )?
            } else if let Some(value) = argument.strip_prefix("--page=") {
                value.into()
            } else {
                return Err(format!("Unknown argument: {argument}"));
            };
            page = match value.as_str() {
                "display" => Self::Display,
                "sound" => Self::Sound,
                "network" => Self::Network,
                "power" => Self::Power,
                "battery" => Self::Battery,
                "personalization" => Self::Personalization,
                _ => return Err(format!("Unknown settings page: {value}")),
            };
        }
        Ok(page)
    }
}

/// UI-thread draft storage survives page unmounts. Saving remains an explicit backend action.
#[derive(Clone)]
pub struct Editor {
    pub page: Signal<Page>,
    pub draft: Signal<Preferences>,
    page_writer: SignalWriter<Page>,
    draft_writer: SignalWriter<Preferences>,
}
impl PartialEq for Editor {
    fn eq(&self, other: &Self) -> bool {
        self.page == other.page && self.draft == other.draft
    }
}
impl Editor {
    pub fn new(draft: Preferences) -> Self {
        let (page, page_writer) = Signal::new(Page::Display);
        let (draft, draft_writer) = Signal::new(draft);
        Self {
            page,
            page_writer,
            draft,
            draft_writer,
        }
    }
    pub fn select(&self, page: Page) {
        self.page_writer.publish_if_changed(page);
    }
    pub fn edit_display(&self, edit: impl FnOnce(&mut DisplayConfiguration)) {
        let mut draft = (*self.draft.snapshot()).clone();
        edit(&mut draft.display);
        self.draft_writer.publish_if_changed(draft);
    }
    pub fn edit_sound(&self, edit: impl FnOnce(&mut SoundSettings)) {
        let mut draft = (*self.draft.snapshot()).clone();
        edit(&mut draft.sound);
        self.draft_writer.publish_if_changed(draft);
    }
    pub fn reset_page(&self) {
        match *self.page.snapshot() {
            Page::Display => self.edit_display(|display| *display = Default::default()),
            Page::Sound => self.edit_sound(|sound| *sound = Default::default()),
            Page::Network | Page::Power | Page::Battery | Page::Personalization => {}
        }
    }
}
