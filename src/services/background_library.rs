use super::{BackgroundTile, PersonalizationPreview};
use crate::settings_config::BACKGROUND_IMAGE;
#[cfg(test)]
use crate::settings_config::store_at;
use std::collections::BTreeMap;
use telorgon::ImageId;
use telorgon::services::desktop_settings::{
    self as shared, BackgroundEntry, PersonalizationSettings, PreparedBackgroundThumbnail,
    SettingsStore, ValidatedBackground,
};

const FIRST_IMAGE: u32 = 0x7000_0000;
const IMAGE_LIMIT: u32 = 0x7fff_fffe;

#[derive(Clone)]
struct CachedBackground {
    descriptor: BackgroundEntry,
    tile: BackgroundTile,
    validated: Option<ValidatedBackground>,
}

// This cache belongs to the backend worker. Views only receive small decoded resources.
pub(super) struct BackgroundLibrary {
    store: SettingsStore,
    cached: BTreeMap<String, CachedBackground>,
    next_image: u32,
}

impl BackgroundLibrary {
    pub(super) fn new(store: SettingsStore) -> Self {
        Self {
            store,
            cached: BTreeMap::new(),
            next_image: FIRST_IMAGE,
        }
    }

    pub(super) fn refresh(&mut self, preview: &mut PersonalizationPreview) -> shared::Result<()> {
        self.refresh_with_import(preview, None)
    }

    pub(super) fn refresh_with_import(
        &mut self,
        preview: &mut PersonalizationPreview,
        mut imported: Option<PreparedBackgroundThumbnail>,
    ) -> shared::Result<()> {
        let directory = self.store.background_directory()?;
        let entries = self.store.list_backgrounds()?;
        let saved = self.store.load_personalization()?;
        let mut cached = BTreeMap::new();
        let mut backgrounds = Vec::with_capacity(entries.len());
        for entry in entries {
            let Some(filename) = entry.settings.background.clone() else {
                continue;
            };
            let has_import = imported
                .as_ref()
                .is_some_and(|imported| imported.validated.settings() == &entry.settings);
            let background = if let Some(existing) = self.cached.get(&filename).filter(|cached| {
                cached.descriptor == entry && !(has_import && cached.tile.error.is_some())
            }) {
                existing.clone()
            } else {
                let prepared = if has_import {
                    Ok(imported.take().expect("matched import"))
                } else {
                    match &entry.error {
                        Some(error) => Err(error.clone()),
                        None => self
                            .store
                            .load_background_thumbnail_validated(&entry.settings, BACKGROUND_IMAGE),
                    }
                };
                self.cache_tile(entry.clone(), prepared)
            };
            backgrounds.push(background.tile.clone());
            cached.insert(filename, background);
        }
        self.cached = cached;
        preview.directory = directory.to_string_lossy().into_owned();
        preview.backgrounds = backgrounds;
        preview.saved = saved;
        preview.library_error = None;
        Ok(())
    }

    pub(super) fn selection(
        &mut self,
        settings: &PersonalizationSettings,
    ) -> shared::Result<Option<PreparedBackgroundThumbnail>> {
        settings.validate()?;
        let Some(filename) = settings.background.as_ref() else {
            return Ok(None);
        };
        if let Some(cached) = self.cached.get(filename) {
            if let Some(error) = &cached.tile.error {
                return Err(error.clone());
            }
            if let (Some(image), Some(validated)) = (&cached.tile.image, &cached.validated) {
                return Ok(Some(PreparedBackgroundThumbnail {
                    image: image.clone(),
                    validated: validated.clone(),
                }));
            }
        }
        let mut prepared = self
            .store
            .load_background_thumbnail_validated(settings, BACKGROUND_IMAGE)?;
        self.assign_image_id(&mut prepared)?;
        Ok(Some(prepared))
    }

    fn assign_image_id(
        &mut self,
        prepared: &mut PreparedBackgroundThumbnail,
    ) -> shared::Result<()> {
        if self.next_image >= IMAGE_LIMIT {
            return Err("Restart Settings to refresh the wallpaper library images.".into());
        }
        // Immutable pixels keep one ID/version for both the tile and selected preview.
        // Never reuse an ID, including after deletion and later reimport.
        prepared.image.image = ImageId(self.next_image);
        prepared.image.content_version = 1;
        self.next_image += 1;
        Ok(())
    }

    fn cache_tile(
        &mut self,
        descriptor: BackgroundEntry,
        prepared: shared::Result<PreparedBackgroundThumbnail>,
    ) -> CachedBackground {
        let prepared = prepared.and_then(|mut prepared| {
            self.assign_image_id(&mut prepared)?;
            Ok(prepared)
        });
        let (image, validated, error) = match prepared {
            Ok(prepared) => (Some(prepared.image), Some(prepared.validated), None),
            Err(error) => (None, None, Some(error)),
        };
        CachedBackground {
            tile: BackgroundTile {
                settings: descriptor.settings.clone(),
                label: descriptor.label.clone(),
                image,
                error,
            },
            descriptor,
            validated,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
    };

    static NEXT_TEST: AtomicU64 = AtomicU64::new(0);

    struct TemporarySettings(PathBuf);
    impl TemporarySettings {
        fn new() -> Self {
            Self(
                std::env::temp_dir()
                    .join(format!(
                        "telorgon-background-library-{}-{}",
                        std::process::id(),
                        NEXT_TEST.fetch_add(1, Ordering::Relaxed)
                    ))
                    .join("settings.toml"),
            )
        }

        fn import(&self) -> shared::PersonalizationSettings {
            store_at(&self.0)
                .import_background(
                    &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("assets/images/default-background.webp"),
                )
                .unwrap()
        }
    }
    impl Drop for TemporarySettings {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(self.0.parent().unwrap());
        }
    }

    #[test]
    fn unchanged_thumbnails_are_reused_and_reimport_gets_a_fresh_id() {
        let path = TemporarySettings::new();
        let settings = path.import();
        let mut library = BackgroundLibrary::new(store_at(&path.0));
        let mut preview = PersonalizationPreview::default();
        library.refresh(&mut preview).unwrap();
        let first = preview.backgrounds[0].image.clone().unwrap();
        assert_eq!(first.image, ImageId(FIRST_IMAGE));
        assert_eq!(first.content_version, 1);
        assert!(first.extent.width <= 320 && first.extent.height <= 180);

        library.refresh(&mut preview).unwrap();
        let unchanged = preview.backgrounds[0].image.clone().unwrap();
        assert_eq!(unchanged.image, first.image);
        assert!(Arc::ptr_eq(&unchanged.pixels, &first.pixels));

        store_at(&path.0).delete_background(&settings).unwrap();
        library.refresh(&mut preview).unwrap();
        assert!(preview.backgrounds.is_empty());
        path.import();
        library.refresh(&mut preview).unwrap();
        let reimported = preview.backgrounds[0].image.as_ref().unwrap();
        assert!(reimported.image.0 > first.image.0);
    }

    #[test]
    fn corrupt_entry_does_not_hide_other_images_or_retry_unchanged_failures() {
        let path = TemporarySettings::new();
        let valid = path.import();
        let invalid = shared::PersonalizationSettings {
            background: Some(format!("background-{}.png", "0".repeat(64))),
        };
        fs::write(
            store_at(&path.0)
                .background_path(&invalid)
                .unwrap()
                .unwrap(),
            b"not a managed image",
        )
        .unwrap();
        let mut library = BackgroundLibrary::new(store_at(&path.0));
        let mut preview = PersonalizationPreview::default();
        library.refresh(&mut preview).unwrap();
        assert_eq!(preview.backgrounds.len(), 2);
        assert!(
            preview
                .backgrounds
                .iter()
                .find(|tile| tile.settings == valid)
                .unwrap()
                .image
                .is_some()
        );
        let failed = preview
            .backgrounds
            .iter()
            .find(|tile| tile.settings == invalid)
            .unwrap();
        assert!(failed.image.is_none());
        assert!(failed.error.is_some());
        let failure = failed.error.clone();
        // A cached failure is deliberately reused rather than attempting another decode.
        library
            .cached
            .get_mut(invalid.background.as_ref().unwrap())
            .unwrap()
            .tile
            .error = Some("cached decode failure".into());
        library.refresh(&mut preview).unwrap();
        assert_eq!(
            preview
                .backgrounds
                .iter()
                .find(|tile| tile.settings == invalid)
                .unwrap()
                .error
                .as_deref(),
            Some("cached decode failure")
        );
        assert_ne!(failure.as_deref(), Some("cached decode failure"));
    }
    #[test]
    fn import_and_selection_share_the_prepared_preview_without_another_decode() {
        let path = TemporarySettings::new();
        let imported = store_at(&path.0)
            .import_background_with_thumbnail(
                &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("assets/images/default-background.webp"),
                BACKGROUND_IMAGE,
            )
            .unwrap();
        let settings = imported.validated.settings().clone();
        let pixels = imported.image.pixels.clone();
        let mut library = BackgroundLibrary::new(store_at(&path.0));
        let mut preview = PersonalizationPreview::default();
        library
            .refresh_with_import(&mut preview, Some(imported))
            .unwrap();
        let tile = preview.backgrounds[0].image.as_ref().unwrap();
        assert!(Arc::ptr_eq(&pixels, &tile.pixels));
        let selected = library.selection(&settings).unwrap().unwrap();
        assert!(Arc::ptr_eq(&pixels, &selected.image.pixels));
        assert_eq!(tile.image, selected.image.image);
        assert_eq!(tile.content_version, selected.image.content_version);
    }
    #[test]
    fn readding_valid_content_recovers_an_unchanged_cached_failure() {
        let path = TemporarySettings::new();
        let settings = path.import();
        let mut library = BackgroundLibrary::new(store_at(&path.0));
        let mut preview = PersonalizationPreview::default();
        library.refresh(&mut preview).unwrap();
        let cached = library
            .cached
            .get_mut(settings.background.as_ref().unwrap())
            .unwrap();
        cached.tile.image = None;
        cached.tile.error = Some("Temporary read failure".into());
        cached.validated = None;
        let imported = store_at(&path.0)
            .import_background_with_thumbnail(
                &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("assets/images/default-background.webp"),
                BACKGROUND_IMAGE,
            )
            .unwrap();
        let pixels = imported.image.pixels.clone();
        library
            .refresh_with_import(&mut preview, Some(imported))
            .unwrap();
        let tile = &preview.backgrounds[0];
        assert!(tile.error.is_none());
        assert!(Arc::ptr_eq(&pixels, &tile.image.as_ref().unwrap().pixels));
    }
}
