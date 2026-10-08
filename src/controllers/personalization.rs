use super::Sources;
use crate::{components::image_picker, services::Status};
use telorgon::services::desktop_settings::PersonalizationSettings;
use telorgon::{app::Component, graphics::render::ImageResource};

pub struct BackgroundTileState {
    pub settings: PersonalizationSettings,
    pub label: String,
    pub image: Option<ImageResource>,
    pub error: Option<String>,
    pub selected: bool,
    pub is_default: bool,
    pub in_use: bool,
    pub can_select: bool,
    pub can_delete: bool,
}

pub struct PersonalizationState {
    pub enabled: bool,
    pub image: Option<ImageResource>,
    pub using_default: bool,
    pub source_name: String,
    pub error: Option<String>,
    pub loading: bool,
    pub choosing: bool,
    pub backgrounds: Vec<BackgroundTileState>,
    pub library_error: Option<String>,
}

fn enabled(status: &Status, choosing: bool) -> bool {
    !status.busy
        && !status.load_error
        && !status.personalization.loading
        && status.preview.is_none()
        && !choosing
}

fn in_use(status: &Status, settings: &PersonalizationSettings) -> bool {
    status.personalization.saved == *settings
        || status
            .shell
            .as_ref()
            .and_then(|shell| shell.personalization.as_ref())
            .is_some_and(|personalization| personalization.current == *settings)
}

fn tiles(status: &Status, enabled: bool) -> Vec<BackgroundTileState> {
    let default = PersonalizationSettings::default();
    let mut tiles = vec![BackgroundTileState {
        selected: status.personalization.desired == default,
        in_use: in_use(status, &default),
        settings: default,
        label: "Default background".into(),
        image: None,
        error: None,
        is_default: true,
        can_select: enabled,
        can_delete: false,
    }];
    tiles.extend(
        status
            .personalization
            .backgrounds
            .iter()
            .filter(|tile| tile.settings.background.is_some())
            .map(|tile| {
                let in_use = in_use(status, &tile.settings);
                BackgroundTileState {
                    settings: tile.settings.clone(),
                    label: tile.label.clone(),
                    image: tile.image.clone(),
                    error: tile.error.clone(),
                    selected: status.personalization.desired == tile.settings,
                    is_default: false,
                    in_use,
                    can_select: enabled && tile.image.is_some() && tile.error.is_none(),
                    can_delete: enabled && !in_use,
                }
            }),
    );
    tiles
}

#[derive(Clone, PartialEq)]
pub struct PersonalizationController {
    sources: Sources,
}
impl PersonalizationController {
    #[cfg(test)]
    pub(crate) fn from_model(model: crate::services::Model) -> Self {
        Self::new(Sources {
            model,
            editor: crate::state::Editor::new(Default::default()),
        })
    }

    pub(super) fn new(sources: Sources) -> Self {
        Self { sources }
    }

    pub fn state(&self, observer: &impl Component) -> PersonalizationState {
        let status = observer.watch(&self.sources.model.signal);
        let preview = &status.personalization;
        let choosing = self.sources.model.choosing_background();
        let enabled = enabled(&status, choosing);
        PersonalizationState {
            enabled,
            image: preview.image.clone(),
            using_default: preview.desired.background.is_none(),
            source_name: preview.source_name.clone(),
            error: preview.error.clone(),
            loading: preview.loading,
            choosing,
            backgrounds: tiles(&status, enabled),
            library_error: preview.library_error.clone(),
        }
    }

    pub fn select(&self, settings: PersonalizationSettings) {
        let status = self.sources.model.signal.snapshot();
        if !enabled(&status, self.sources.model.choosing_background()) {
            return;
        }
        if settings.background.is_some()
            && !status.personalization.backgrounds.iter().any(|tile| {
                tile.settings == settings && tile.image.is_some() && tile.error.is_none()
            })
        {
            return;
        }
        self.sources.model.select_background(settings);
    }

    pub fn delete(&self, settings: PersonalizationSettings) {
        let status = self.sources.model.signal.snapshot();
        if enabled(&status, self.sources.model.choosing_background())
            && settings.background.is_some()
            && !in_use(&status, &settings)
            && status
                .personalization
                .backgrounds
                .iter()
                .any(|tile| tile.settings == settings)
        {
            self.sources.model.delete_background(settings);
        }
    }

    pub fn browse(&self) {
        if !enabled(
            &self.sources.model.signal.snapshot(),
            self.sources.model.choosing_background(),
        ) || !self.sources.model.begin_background_choice()
        {
            return;
        }
        let model = self.sources.model.clone();
        if let Err(error) = image_picker::choose(move |result| {
            model.finish_background_choice(result);
        }) {
            self.sources.model.finish_background_choice(Err(error));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::{BackgroundTile, PersonalizationPreview};
    use telorgon::app::SizeI;
    use telorgon::services::desktop_settings::{PersonalizationSnapshot, ShellSnapshot};

    fn status() -> Status {
        Status {
            shell: None,
            message: String::new(),
            busy: false,
            preview: None,
            load_error: false,
            personalization: PersonalizationPreview::default(),
        }
    }

    fn background(label: &str) -> BackgroundTile {
        BackgroundTile {
            settings: PersonalizationSettings {
                background: Some(format!("background-{label}.png")),
            },
            label: label.into(),
            image: Some(ImageResource {
                image: Default::default(),
                content_version: 1,
                extent: SizeI {
                    width: 1,
                    height: 1,
                },
                color_encoding: Default::default(),
                alpha_mode: Default::default(),
                pixel_format: Default::default(),
                pixels: std::sync::Arc::from(vec![255; 4]),
            }),
            error: None,
        }
    }

    #[test]
    fn saved_and_applied_backgrounds_are_protected_independently_of_the_draft() {
        let mut status = status();
        let saved = background("saved");
        let applied = background("applied");
        let draft = background("draft");
        status.personalization.saved = saved.settings.clone();
        status.personalization.desired = draft.settings.clone();
        status.shell = Some(ShellSnapshot {
            personalization: Some(PersonalizationSnapshot {
                current: applied.settings.clone(),
                error: None,
            }),
            ..Default::default()
        });
        status.personalization.backgrounds = vec![saved, applied, draft];
        let tiles = tiles(&status, true);
        assert!(tiles[0].is_default && tiles[0].can_select && !tiles[0].can_delete);
        assert!(tiles[1].in_use && !tiles[1].can_delete && !tiles[1].selected);
        assert!(tiles[2].in_use && !tiles[2].can_delete && !tiles[2].selected);
        assert!(tiles[3].selected && tiles[3].can_select && tiles[3].can_delete);
    }

    #[test]
    fn corrupt_unused_backgrounds_can_be_deleted_but_not_selected() {
        let mut status = status();
        let mut corrupt = background("corrupt");
        corrupt.image = None;
        corrupt.error = Some("This image could not be read.".into());
        status.personalization.backgrounds.push(corrupt);
        let available = tiles(&status, true);
        assert!(!available[1].can_select && available[1].can_delete);
        let disabled = tiles(&status, false);
        assert!(
            disabled
                .iter()
                .all(|tile| !tile.can_select && !tile.can_delete)
        );
    }
}
