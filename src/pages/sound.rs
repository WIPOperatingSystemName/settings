use crate::{
    components::controls::{choice, divider, group, page_content},
    controllers::{ChannelState, SoundController},
    theme::{MUTED, TEXT},
};
use telorgon::app::*;

#[component(no_default)]
pub struct SoundPage {
    #[input]
    controller: SoundController,
}
impl SoundPage {
    pub fn new(controller: SoundController) -> Self {
        Self { controller }
    }
    fn channel(&self, state: &ChannelState) -> Container {
        let channel = state.channel;
        let mut devices = group(42.0 * (state.devices.len() + 1) as f32).child(
            choice("Follow system default", state.follows_default)
                .enabled(state.enabled)
                .on_press(move |this: &mut Self| this.controller.follow_default(channel)),
        );
        for device in &state.devices {
            let name = device.name.clone();
            devices = devices.child(
                choice(&device.label, device.selected)
                    .enabled(state.enabled)
                    .on_press(move |this: &mut Self| this.controller.select_device(channel, &name)),
            );
        }
        let mut section = column()
            .height(channel_height(state))
            .gap(10.0)
            .child(text(state.title).size(15.0).weight(600).color(TEXT))
            .child(devices)
            .child(
                group(107.0)
                    .padding(12.0)
                    .gap(8.0)
                    .child(
                        slider(&state.volume_label, state.volume)
                            .height(38.0)
                            .width(Dimension::FILL)
                            .enabled(state.volume_enabled)
                            .on_change(move |this: &mut Self, value| {
                                this.controller.set_volume(channel, value)
                            }),
                    )
                    .child(divider())
                    .child(
                        switch("Mute", state.muted == Some(true))
                            .height(28.0)
                            .width(Dimension::FILL)
                            .enabled(state.mute_enabled)
                            .on_change(move |this: &mut Self, _| {
                                this.controller.toggle_mute(channel)
                            }),
                    ),
            );
        for warning in &state.warnings {
            section = section.child(text(warning).size(13.0).color(MUTED));
        }
        section
    }
}
impl Component for SoundPage {
    fn view(&self) -> impl View {
        let state = self.controller.state(self);
        let mut body = page_content(
            110.0
                + channel_height(&state.output)
                + channel_height(&state.input)
                + if state.error.is_some() { 60.0 } else { 0.0 },
        )
        .gap(24.0)
        .child(text("Sound").size(24.0).weight(600).color(TEXT))
        .child(self.channel(&state.output))
        .child(self.channel(&state.input));
        if let Some(error) = &state.error {
            body = body.child(text(error).size(13.0).color(TEXT));
        }
        body
    }
}
fn channel_height(state: &ChannelState) -> f32 {
    195.0 + 42.0 * state.devices.len() as f32 + 52.0 * state.warnings.len() as f32
}
