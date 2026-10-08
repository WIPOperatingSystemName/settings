use super::Sources;
use crate::services::Status;
use telorgon::app::Component;
use telorgon::services::desktop_settings::{SoundChannel, SoundSettings};

#[derive(Clone, Copy)]
pub enum Channel {
    Output,
    Input,
}
impl Channel {
    fn input(self) -> bool {
        matches!(self, Self::Input)
    }
    fn title(self) -> &'static str {
        if self.input() { "Input" } else { "Output" }
    }
    fn get(self, sound: &SoundSettings) -> &SoundChannel {
        if self.input() {
            &sound.input
        } else {
            &sound.output
        }
    }
    fn get_mut(self, sound: &mut SoundSettings) -> &mut SoundChannel {
        if self.input() {
            &mut sound.input
        } else {
            &mut sound.output
        }
    }
}
pub struct DeviceChoice {
    pub name: String,
    pub label: String,
    pub selected: bool,
}
pub struct ChannelState {
    pub channel: Channel,
    pub title: &'static str,
    pub enabled: bool,
    pub devices: Vec<DeviceChoice>,
    pub warnings: Vec<String>,
    pub volume: f32,
    pub volume_label: String,
    pub volume_enabled: bool,
    pub mute_enabled: bool,
    pub muted: Option<bool>,
    pub follows_default: bool,
}
pub struct SoundState {
    pub output: ChannelState,
    pub input: ChannelState,
    pub error: Option<String>,
    pub audio: Option<telorgon::services::desktop_settings::AudioSnapshot>,
}
#[derive(Clone, PartialEq)]
pub struct SoundController {
    sources: Sources,
}
impl SoundController {
    pub(super) fn new(sources: Sources) -> Self {
        Self { sources }
    }
    pub fn state(&self, observer: &impl Component) -> SoundState {
        let (status, draft) = self.sources.observe(observer);
        SoundState {
            output: channel_state(Channel::Output, &status, &draft.sound),
            input: channel_state(Channel::Input, &status, &draft.sound),
            error: status.shell.as_ref().and_then(|s| s.audio_error.clone()),
            audio: status.shell.as_ref().and_then(|s| s.audio.clone()),
        }
    }
    fn channel_state(&self, channel: Channel) -> ChannelState {
        channel_state(
            channel,
            &self.sources.model.signal.snapshot(),
            &self.sources.editor.draft.snapshot().sound,
        )
    }
    pub fn follow_default(&self, channel: Channel) {
        self.select_device(channel, "");
    }
    pub fn select_device(&self, channel: Channel, name: &str) {
        let state = self.channel_state(channel);
        if !state.enabled || (!name.is_empty() && !state.devices.iter().any(|d| d.name == name)) {
            return;
        }
        self.sources
            .editor
            .edit_sound(|sound| channel.get_mut(sound).device = name.into());
    }
    pub fn set_volume(&self, channel: Channel, value: f32) {
        if !value.is_finite()
            || !(0.0..=1.0).contains(&value)
            || !self.channel_state(channel).volume_enabled
        {
            return;
        }
        self.sources
            .editor
            .edit_sound(|sound| channel.get_mut(sound).volume = Some(value));
    }
    pub fn toggle_mute(&self, channel: Channel) {
        let state = self.channel_state(channel);
        if !state.mute_enabled {
            return;
        }
        self.sources
            .editor
            .edit_sound(|sound| channel.get_mut(sound).muted = Some(state.muted != Some(true)));
    }
    pub fn save(&self) {
        let status = self.sources.model.signal.snapshot();
        if !status.busy && !status.load_error && status.preview.is_none() {
            self.sources
                .model
                .save_sound(self.sources.editor.draft.snapshot().sound.clone());
        }
    }
}
fn channel_state(kind: Channel, status: &Status, sound: &SoundSettings) -> ChannelState {
    let channel = kind.get(sound);
    let devices = status
        .shell
        .as_ref()
        .map(|s| {
            s.devices
                .iter()
                .filter(|d| d.input == kind.input())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let selected = devices.iter().find(|d| {
        if channel.device.is_empty() {
            d.is_default
        } else {
            d.name == channel.device
        }
    });
    let volume = channel.volume.or_else(|| selected.and_then(|d| d.volume));
    let muted = channel.muted.or_else(|| selected.and_then(|d| d.muted));
    let enabled = !status.busy && status.preview.is_none();
    let mut warnings = Vec::new();
    if devices.is_empty() {
        warnings.push("No devices available. Connect an audio device to configure it.".into());
    }
    if !channel.device.is_empty() && selected.is_none() {
        warnings.push(format!("Saved device is unavailable: {}", channel.device));
    }
    ChannelState {
        channel: kind,
        title: kind.title(),
        enabled,
        warnings,
        volume: volume.unwrap_or(0.0),
        volume_label: format!(
            "{} volume: {}",
            kind.title(),
            volume.map_or("Unavailable".into(), |v| format!("{:.0}%", v * 100.0))
        ),
        volume_enabled: enabled && selected.is_some_and(|d| d.can_set_volume),
        mute_enabled: enabled && muted.is_some(),
        muted,
        follows_default: channel.device.is_empty(),
        devices: devices
            .iter()
            .map(|d| DeviceChoice {
                name: d.name.clone(),
                selected: channel.device == d.name,
                label: format!(
                    "{}{}",
                    d.label,
                    if d.is_default {
                        " (current default)"
                    } else {
                        ""
                    }
                ),
            })
            .collect(),
    }
}
