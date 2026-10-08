use super::{PersonalizationPreview, background_library::BackgroundLibrary};
#[cfg(test)]
use crate::settings_config::store_at;
use crate::settings_config::{self, BACKGROUND_IMAGE};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use telorgon::app::*;
use telorgon::services::desktop_settings::{
    self as shared, DisplayConfiguration, PersonalizationSettings, PersonalizationSnapshot,
    Preferences, SettingsClient, SettingsStore, ShellSnapshot, SoundSettings,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Status {
    pub shell: Option<ShellSnapshot>,
    pub message: String,
    pub busy: bool,
    pub preview: Option<u64>,
    pub load_error: bool,
    pub personalization: PersonalizationPreview,
}
#[derive(Clone, PartialEq)]
pub struct Model {
    pub signal: Signal<Status>,
    commands: Commands,
}
#[derive(Clone)]
struct Commands {
    sender: Arc<mpsc::Sender<Command>>,
    choosing_background: Arc<AtomicBool>,
}
impl PartialEq for Commands {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.sender, &other.sender)
    }
}
enum Command {
    Sound(SoundSettings),
    Display(DisplayConfiguration),
    Keep,
    Revert,
    Background(PathBuf),
    SelectBackground(PersonalizationSettings),
    DeleteBackground(PersonalizationSettings),
    PickerState,
    PickerFinished(shared::Result<Option<PathBuf>>),
    Stop,
}
impl Model {
    #[cfg(test)]
    pub(crate) fn from_signal(signal: Signal<Status>) -> Self {
        let (sender, _) = mpsc::channel();
        Self {
            signal,
            commands: Commands {
                sender: Arc::new(sender),
                choosing_background: Arc::new(AtomicBool::new(false)),
            },
        }
    }

    pub fn choosing_background(&self) -> bool {
        self.commands.choosing_background.load(Ordering::Acquire)
    }
    pub fn save_sound(&self, value: SoundSettings) {
        if !self.choosing_background() {
            let _ = self.commands.sender.send(Command::Sound(value));
        }
    }
    pub fn save_display(&self, value: DisplayConfiguration) {
        if !self.choosing_background() {
            let _ = self.commands.sender.send(Command::Display(value));
        }
    }
    pub fn keep(&self) {
        let _ = self.commands.sender.send(Command::Keep);
    }
    pub fn revert(&self) {
        let _ = self.commands.sender.send(Command::Revert);
    }
    pub fn select_background(&self, settings: PersonalizationSettings) {
        if !self.choosing_background() {
            let _ = self
                .commands
                .sender
                .send(Command::SelectBackground(settings));
        }
    }
    pub fn delete_background(&self, settings: PersonalizationSettings) {
        if !self.choosing_background() {
            let _ = self
                .commands
                .sender
                .send(Command::DeleteBackground(settings));
        }
    }
    pub fn begin_background_choice(&self) -> bool {
        let status = self.signal.snapshot();
        if status.busy
            || status.load_error
            || status.preview.is_some()
            || status.personalization.loading
            || self
                .commands
                .choosing_background
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return false;
        }
        if self.commands.sender.send(Command::PickerState).is_err() {
            self.commands
                .choosing_background
                .store(false, Ordering::Release);
            return false;
        }
        true
    }
    pub fn finish_background_choice(&self, result: shared::Result<Option<PathBuf>>) {
        // Completion is one reliably queued event. Keep the reservation until it is handled.
        if self
            .commands
            .sender
            .send(Command::PickerFinished(result))
            .is_err()
        {
            self.commands
                .choosing_background
                .store(false, Ordering::Release);
        }
    }
}
pub struct Backend {
    stopped: Arc<AtomicBool>,
    model: Model,
    thread: Option<thread::JoinHandle<()>>,
}
impl Backend {
    pub fn start() -> shared::Result<(Self, Model, Preferences)> {
        let store = settings_config::store()?;
        let loaded = store.load();
        let preferences = loaded.clone().unwrap_or_default();
        let status = Status {
            shell: None,
            busy: false,
            preview: None,
            load_error: loaded.is_err(),
            personalization: PersonalizationPreview {
                desired: preferences.personalization.clone(),
                saved: preferences.personalization.clone(),
                source_name: if preferences.personalization.background.is_some() {
                    "Custom wallpaper".into()
                } else {
                    "Default wallpaper".into()
                },
                loading: true,
                ..Default::default()
            },
            message: loaded.err().map_or("Connecting to the shell…".into(), |e| {
                format!("Cannot load settings: {e}. Fix the file before saving.")
            }),
        };
        let (signal, writer) = Signal::new(status.clone());
        let (commands, receiver) = mpsc::channel();
        let choosing_background = Arc::new(AtomicBool::new(false));
        let model = Model {
            signal,
            commands: Commands {
                sender: Arc::new(commands),
                choosing_background: choosing_background.clone(),
            },
        };
        let stopped = Arc::new(AtomicBool::new(false));
        let worker_stop = stopped.clone();
        let thread = thread::Builder::new()
            .name("settings-connection".into())
            .spawn(move || {
                run(
                    store,
                    receiver,
                    writer,
                    status,
                    worker_stop,
                    choosing_background,
                );
            })
            .map_err(|error| error.to_string())?;
        Ok((
            Self {
                stopped,
                model: model.clone(),
                thread: Some(thread),
            },
            model,
            preferences,
        ))
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        let _ = self.model.commands.sender.send(Command::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn run(
    store: SettingsStore,
    receiver: mpsc::Receiver<Command>,
    writer: SignalWriter<Status>,
    mut status: Status,
    stopped: Arc<AtomicBool>,
    choosing_background: Arc<AtomicBool>,
) {
    let mut library = BackgroundLibrary::new(store.clone());
    refresh_backgrounds(&mut library, &mut status.personalization);
    let desired = status.personalization.desired.clone();
    if let Err(error) = select_background(&mut library, &mut status.personalization, desired) {
        status.personalization.error = Some(error);
    }
    status.personalization.loading = false;
    writer.publish_if_changed(status.clone());
    let mut client = SettingsClient::connect(settings_config::endpoint()).ok();
    let mut pending: Option<DisplayConfiguration> = None;
    let mut last_applied_background = None;
    let mut last_library_refresh = Instant::now();
    while !stopped.load(Ordering::Acquire) {
        status.busy = choosing_background.load(Ordering::Acquire);
        status.personalization.loading = status.busy;
        if last_library_refresh.elapsed() >= Duration::from_secs(4) {
            refresh_backgrounds(&mut library, &mut status.personalization);
            last_library_refresh = Instant::now();
        }
        if client.is_none() {
            client = SettingsClient::connect(settings_config::endpoint()).ok();
        }
        let snapshot = client
            .as_ref()
            .ok_or_else(|| "Session bus is unavailable".to_owned())
            .and_then(SettingsClient::snapshot);
        match snapshot {
            Ok(snapshot) => {
                if let Some(personalization) = &snapshot.personalization {
                    last_applied_background = Some(personalization.current.clone());
                }
                if let Some(token) = status.preview {
                    if snapshot.display.preview != Some(token) {
                        status.preview = None;
                        pending = None;
                        status.message = snapshot
                            .display
                            .error
                            .clone()
                            .unwrap_or("Display preview ended.".into());
                    }
                } else if status.message == "Connecting to the shell…" {
                    status.message = "Ready. Changes apply when you save.".into();
                }
                status.shell = Some(snapshot);
            }
            Err(_) => {
                status.shell = None;
                client = None;
            }
        }
        writer.publish_if_changed(status.clone());
        let mut command = match receiver.recv_timeout(Duration::from_secs(1)) {
            Ok(command) => command,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        };
        if matches!(command, Command::Stop) || stopped.load(Ordering::Acquire) {
            break;
        }
        if let Command::PickerFinished(result) = command {
            choosing_background.store(false, Ordering::Release);
            status.busy = false;
            status.personalization.loading = false;
            match result {
                Ok(Some(path)) => command = Command::Background(path),
                result => {
                    if let Err(error) = result {
                        status.message =
                            "Image browser failed. See the message by Add Photo.".into();
                        status.personalization.error = Some(error);
                    } else {
                        status.message =
                            "No image added. Your background library is unchanged.".into();
                    }
                    writer.publish_if_changed(status.clone());
                    continue;
                }
            }
        }
        match &command {
            Command::PickerState => {
                status.busy = choosing_background.load(Ordering::Acquire);
                status.personalization.loading = status.busy;
                if status.busy {
                    status.message = "Opening image browser…".into();
                    status.personalization.error = None;
                }
                writer.publish_if_changed(status.clone());
                continue;
            }
            _ => {}
        }
        if status.load_error {
            continue;
        }
        if status.preview.is_some()
            && matches!(
                command,
                Command::Background(_)
                    | Command::SelectBackground(_)
                    | Command::DeleteBackground(_)
            )
        {
            status.message = "Confirm or revert your display preview first.".into();
            writer.publish_if_changed(status.clone());
            continue;
        }
        status.busy = true;
        status.personalization.loading = matches!(
            command,
            Command::Background(_) | Command::SelectBackground(_) | Command::DeleteBackground(_)
        );
        status.message = if status.personalization.loading {
            "Updating backgrounds…".into()
        } else {
            "Applying changes…".into()
        };
        writer.publish(status.clone());
        let result = (|| -> shared::Result<String> {
            match command {
                Command::Sound(sound) => {
                    store.save_sound(&sound)?;
                    match client
                        .as_ref()
                        .ok_or("Shell is unavailable".to_owned())
                        .and_then(SettingsClient::reload)
                    {
                        Ok(()) => Ok("Sound settings saved and applied.".into()),
                        Err(e) => Ok(format!("Saved. Not applied yet: {e}")),
                    }
                }
                Command::Display(display) => {
                    if status.preview.is_some() {
                        return Err("Confirm or revert your current preview first.".into());
                    }
                    let client = client
                        .as_ref()
                        .ok_or("Start the shell to preview display changes.")?;
                    let token = client.preview_display(&display)?;
                    pending = Some(display);
                    status.preview = Some(token);
                    Ok("Keep these display settings? They will revert automatically after 20 seconds.".into())
                }
                Command::Keep => {
                    let token = status.preview.ok_or("The preview has already ended.")?;
                    client
                        .as_ref()
                        .ok_or("Shell is unavailable")?
                        .confirm_display(token)?;
                    status.preview = None;
                    let configuration = pending.take().ok_or("No pending display configuration")?;
                    store.save_display(&configuration).map_err(|e| {
                        format!("Display changes applied, but could not be saved: {e}")
                    })?;
                    Ok("Display settings saved and applied.".into())
                }
                Command::Revert => {
                    if let Some(token) = status.preview {
                        client
                            .as_ref()
                            .ok_or("Shell is unavailable")?
                            .revert_display(token)?;
                    }
                    status.preview = None;
                    pending = None;
                    Ok("Display changes reverted. Saved settings are unchanged.".into())
                }
                Command::Background(source) => {
                    let imported =
                        store.import_background_with_thumbnail(&source, BACKGROUND_IMAGE)?;
                    let desired = imported.validated.settings().clone();
                    library.refresh_with_import(&mut status.personalization, Some(imported))?;
                    apply_background(
                        &store,
                        &mut status,
                        desired,
                        &mut library,
                        |status| {
                            writer.publish_if_changed(status.clone());
                        },
                        |desired| {
                            client
                                .as_ref()
                                .ok_or_else(|| "Shell is unavailable".to_owned())?
                                .apply_personalization(desired)
                        },
                    )?;
                    Ok("Photo added and set as your wallpaper.".into())
                }
                Command::SelectBackground(desired) => {
                    apply_background(
                        &store,
                        &mut status,
                        desired,
                        &mut library,
                        |status| {
                            writer.publish_if_changed(status.clone());
                        },
                        |desired| {
                            client
                                .as_ref()
                                .ok_or_else(|| "Shell is unavailable".to_owned())?
                                .apply_personalization(desired)
                        },
                    )?;
                    Ok("Wallpaper saved and applied.".into())
                }
                Command::DeleteBackground(desired) => {
                    // A running shell checks its applied image under the same lock as apply.
                    // Offline deletion still protects the persisted selection in shared storage.
                    match client.as_ref() {
                        Some(client) => client.delete_background(&desired)?,
                        None => {
                            if last_applied_background.as_ref() == Some(&desired) {
                                return Err("This background may still be in use. Reconnect to the shell and select another background first.".into());
                            }
                            store.delete_background(&desired)?;
                        }
                    }
                    if status.personalization.desired == desired {
                        select_background(
                            &mut library,
                            &mut status.personalization,
                            PersonalizationSettings::default(),
                        )?;
                    }
                    library.refresh(&mut status.personalization)?;
                    status.personalization.error = None;
                    Ok("Background removed from the library.".into())
                }
                Command::Stop | Command::PickerState | Command::PickerFinished(_) => {
                    unreachable!()
                }
            }
        })();
        if let Err(error) = &result {
            if status.personalization.loading {
                status.personalization.error = Some(error.clone());
            }
        }
        if let Some(personalization) = status
            .shell
            .as_ref()
            .and_then(|shell| shell.personalization.as_ref())
        {
            last_applied_background = Some(personalization.current.clone());
        }
        status.message = result.unwrap_or_else(|e| format!("Could not finish: {e}"));
        status.busy = choosing_background.load(Ordering::Acquire);
        status.personalization.loading = status.busy;
        writer.publish(status.clone());
    }
    if let (Some(client), Some(token)) = (&client, status.preview) {
        let _ = client.revert_display(token);
    }
}

fn apply_background(
    store: &SettingsStore,
    status: &mut Status,
    desired: PersonalizationSettings,
    library: &mut BackgroundLibrary,
    publish: impl FnOnce(&Status),
    apply: impl FnOnce(&PersonalizationSettings) -> shared::Result<()>,
) -> shared::Result<()> {
    // Persist before replacing the visible selection. Cached validation still checks
    // the file under the deletion lock, without decoding its pixels again.
    let prepared = library.selection(&desired)?;
    match &prepared {
        Some(prepared) => store.save_personalization_validated(&prepared.validated)?,
        None => store.save_personalization(&desired)?,
    }
    set_background_preview(&mut status.personalization, desired.clone(), prepared);
    status.personalization.saved = desired.clone();
    // Show the imported tile and selected preview while the shell loads the desktop image.
    publish(status);
    if let Err(error) = apply(&desired) {
        let error = format!("Wallpaper saved, but it could not be applied: {error}");
        status.personalization.error = Some(error.clone());
        return Err(error);
    }
    if let Some(shell) = &mut status.shell {
        shell.personalization = Some(PersonalizationSnapshot {
            current: desired,
            error: None,
        });
    }
    Ok(())
}

fn refresh_backgrounds(library: &mut BackgroundLibrary, preview: &mut PersonalizationPreview) {
    if let Err(error) = library.refresh(preview) {
        preview.library_error = Some(error);
    }
}

fn select_background(
    library: &mut BackgroundLibrary,
    preview: &mut PersonalizationPreview,
    desired: PersonalizationSettings,
) -> shared::Result<()> {
    let prepared = library.selection(&desired)?;
    set_background_preview(preview, desired, prepared);
    Ok(())
}

fn set_background_preview(
    preview: &mut PersonalizationPreview,
    desired: PersonalizationSettings,
    prepared: Option<shared::PreparedBackgroundThumbnail>,
) {
    preview.source_name = if desired.background.is_none() {
        "Default wallpaper".into()
    } else {
        preview
            .backgrounds
            .iter()
            .find(|tile| tile.settings == desired)
            .map(|tile| tile.label.clone())
            .unwrap_or_else(|| "Custom wallpaper".into())
    };
    preview.desired = desired;
    preview.image = prepared.map(|prepared| prepared.image);
    preview.error = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, sync::atomic::AtomicU64};

    static NEXT_TEST: AtomicU64 = AtomicU64::new(0);

    struct TemporarySettings(PathBuf);
    impl TemporarySettings {
        fn new() -> Self {
            Self(
                std::env::temp_dir()
                    .join(format!(
                        "telorgon-wallpaper-apply-{}-{}",
                        std::process::id(),
                        NEXT_TEST.fetch_add(1, Ordering::Relaxed)
                    ))
                    .join("settings.toml"),
            )
        }

        fn import(&self) -> PersonalizationSettings {
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

    fn model() -> (Model, mpsc::Receiver<Command>) {
        let (signal, _) = Signal::new(Status {
            shell: None,
            message: String::new(),
            busy: false,
            preview: None,
            load_error: false,
            personalization: PersonalizationPreview::default(),
        });
        let (sender, receiver) = mpsc::channel();
        (
            Model {
                signal,
                commands: Commands {
                    sender: Arc::new(sender),
                    choosing_background: Arc::new(AtomicBool::new(false)),
                },
            },
            receiver,
        )
    }

    #[test]
    fn picker_reservation_gates_actions_before_the_worker_publishes_busy() {
        let (model, receiver) = model();
        assert!(model.begin_background_choice());
        assert!(!model.signal.snapshot().busy);
        assert!(!model.begin_background_choice());
        model.save_sound(SoundSettings::default());
        model.save_display(DisplayConfiguration::default());
        model.select_background(PersonalizationSettings::default());
        model.delete_background(PersonalizationSettings::default());
        assert!(matches!(receiver.try_recv(), Ok(Command::PickerState)));
        assert!(matches!(
            receiver.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn picker_completion_is_one_reliable_event_even_with_queued_commands() {
        let (model, receiver) = model();
        for _ in 0..8 {
            model.keep();
        }
        assert!(model.begin_background_choice());
        let path = PathBuf::from("/tmp/selected image.png");
        model.finish_background_choice(Ok(Some(path.clone())));
        assert!(model.choosing_background());
        for _ in 0..8 {
            assert!(matches!(receiver.try_recv(), Ok(Command::Keep)));
        }
        assert!(matches!(receiver.try_recv(), Ok(Command::PickerState)));
        assert!(matches!(receiver.try_recv(),
            Ok(Command::PickerFinished(Ok(Some(selected)))) if selected == path));
        assert!(matches!(
            receiver.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn failed_apply_keeps_saved_selection_visible_and_the_same_photo_can_be_retried() {
        let path = TemporarySettings::new();
        let desired = path.import();
        let (model, _) = model();
        let mut status = (*model.signal.snapshot()).clone();
        status.shell = Some(ShellSnapshot {
            personalization: Some(PersonalizationSnapshot::default()),
            ..Default::default()
        });
        let mut library = BackgroundLibrary::new(store_at(&path.0));
        let result = apply_background(
            &store_at(&path.0),
            &mut status,
            desired.clone(),
            &mut library,
            |_| {},
            |selection| {
                assert_eq!(
                    store_at(&path.0).load_personalization().unwrap(),
                    *selection
                );
                Err("Shell is unavailable".into())
            },
        );
        assert!(result.is_err());
        assert_eq!(status.personalization.desired, desired);
        assert_eq!(status.personalization.saved, desired);
        assert!(status.personalization.image.is_some());
        assert_eq!(
            status.personalization.error.as_deref(),
            Some("Wallpaper saved, but it could not be applied: Shell is unavailable")
        );
        assert_eq!(
            status
                .shell
                .as_ref()
                .unwrap()
                .personalization
                .as_ref()
                .unwrap()
                .current,
            PersonalizationSettings::default()
        );

        apply_background(
            &store_at(&path.0),
            &mut status,
            desired.clone(),
            &mut library,
            |_| {},
            |selection| {
                assert_eq!(
                    store_at(&path.0).load_personalization().unwrap(),
                    *selection
                );
                Ok(())
            },
        )
        .unwrap();
        assert!(status.personalization.error.is_none());
        assert_eq!(
            status.shell.unwrap().personalization.unwrap().current,
            desired
        );
    }

    #[test]
    fn failed_save_preserves_visible_selection_and_never_applies() {
        let path = TemporarySettings::new();
        let desired = path.import();
        // A directory at the settings destination makes persistence fail after decode.
        fs::create_dir(&path.0).unwrap();
        let (model, _) = model();
        let mut status = (*model.signal.snapshot()).clone();
        let before = status.personalization.clone();
        let mut library = BackgroundLibrary::new(store_at(&path.0));
        let result = apply_background(
            &store_at(&path.0),
            &mut status,
            desired,
            &mut library,
            |_| panic!("A failed save must not publish the selection"),
            |_| panic!("A failed save must not apply the wallpaper"),
        );
        assert!(result.is_err());
        assert_eq!(status.personalization, before);
        assert!(status.shell.is_none());
    }
    #[test]
    fn cached_selection_is_published_before_shell_apply_and_rejects_file_changes() {
        let path = TemporarySettings::new();
        let desired = path.import();
        let (model, _) = model();
        let mut status = (*model.signal.snapshot()).clone();
        let mut library = BackgroundLibrary::new(store_at(&path.0));
        library.refresh(&mut status.personalization).unwrap();
        let tile = status.personalization.backgrounds[0].image.clone().unwrap();
        let published = std::cell::Cell::new(false);
        apply_background(
            &store_at(&path.0),
            &mut status,
            desired.clone(),
            &mut library,
            |status| {
                let selected = status.personalization.image.as_ref().unwrap();
                assert!(Arc::ptr_eq(&tile.pixels, &selected.pixels));
                assert_eq!(tile.image, selected.image);
                assert_eq!(tile.content_version, selected.content_version);
                assert_eq!(status.personalization.saved, desired);
                published.set(true);
            },
            |_| {
                assert!(published.get());
                Ok(())
            },
        )
        .unwrap();
        let before = status.personalization.clone();
        let saved = fs::read(&path.0).unwrap();
        fs::write(
            store_at(&path.0)
                .background_path(&desired)
                .unwrap()
                .unwrap(),
            b"changed",
        )
        .unwrap();
        assert!(
            apply_background(
                &store_at(&path.0),
                &mut status,
                desired,
                &mut library,
                |_| panic!("A changed image must not replace the visible preview"),
                |_| panic!("A changed image must not be applied"),
            )
            .is_err()
        );
        assert_eq!(status.personalization, before);
        assert_eq!(fs::read(&path.0).unwrap(), saved);
    }
}
