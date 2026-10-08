use telorgon::graphics::render::ImageResource;
use telorgon::services::desktop_settings::PersonalizationSettings;

#[derive(Clone, Debug, PartialEq)]
pub struct BackgroundTile {
    pub settings: PersonalizationSettings,
    pub label: String,
    pub image: Option<ImageResource>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PersonalizationPreview {
    pub desired: PersonalizationSettings,
    pub image: Option<ImageResource>,
    pub source_name: String,
    pub error: Option<String>,
    pub loading: bool,
    pub saved: PersonalizationSettings,
    pub backgrounds: Vec<BackgroundTile>,
    pub directory: String,
    pub library_error: Option<String>,
}

impl Default for PersonalizationPreview {
    fn default() -> Self {
        Self {
            desired: Default::default(),
            image: None,
            source_name: "Default wallpaper".into(),
            error: None,
            loading: false,
            saved: Default::default(),
            backgrounds: Vec::new(),
            directory: String::new(),
            library_error: None,
        }
    }
}
