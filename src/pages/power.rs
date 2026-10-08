use crate::{
    components::controls::{choice, divider, group, page_content},
    controllers::PowerController,
    theme::{MUTED, TEXT},
};
use telorgon::app::*;

#[component(no_default)]
pub struct PowerPage {
    #[input]
    controller: PowerController,
}
impl PowerPage {
    pub fn new(controller: PowerController) -> Self {
        Self { controller }
    }
}
impl Component for PowerPage {
    fn view(&self) -> impl View {
        let state = self.controller.state(self);
        let compact = self.viewport_size().width < 800.0;
        let current_height = if compact { 82.0 } else { 56.0 };
        let description_height = if compact { 68.0 } else { 48.0 };
        let warning_height = if state.performance_warning.is_some() {
            64.0
        } else {
            0.0
        };
        let operation_height = if state.operation_message.is_empty() {
            0.0
        } else {
            54.0
        };
        let availability_height = if state.availability_message.is_empty() {
            0.0
        } else {
            70.0
        };
        let modes_height = state.choices.len() as f32 * (42.0 + description_height)
            + state.choices.len().saturating_sub(1) as f32;
        let mut body = page_content(
            215.0 + current_height - 56.0
                + modes_height
                + warning_height
                + operation_height
                + availability_height,
        )
        .gap(16.0)
        .child(text("Power").size(24.0).weight(600).color(TEXT))
        .child(
            text("Manage energy use and performance on this device.")
                .size(13.0)
                .color(MUTED)
                .width(Dimension::FILL),
        )
        .child(
            group(current_height).padding(12.0).child(
                (if compact { column().gap(8.0) } else { row() })
                    .align_items(Alignment::Center)
                    .child(
                        text("Current power mode")
                            .size(14.0)
                            .color(TEXT)
                            .width(Dimension::FILL),
                    )
                    .child(text(&state.current).size(14.0).color(MUTED)),
            ),
        );
        if !state.choices.is_empty() {
            let mut modes = group(modes_height);
            for (index, mode) in state.choices.iter().enumerate() {
                let profile = mode.profile;
                if index > 0 {
                    modes = modes.child(divider());
                }
                modes = modes
                    .child(
                        choice(profile.label(), mode.selected)
                            .enabled(state.enabled && mode.enabled)
                            .on_press(move |this: &mut Self| this.controller.set_profile(profile)),
                    )
                    .child(
                        text(profile.description())
                            .size(12.0)
                            .color(MUTED)
                            .width(Dimension::FILL)
                            .height(description_height)
                            .padding(12.0),
                    );
            }
            body = body.child(modes).child(
                text("Power mode changes apply immediately.")
                    .size(13.0)
                    .color(MUTED)
                    .width(Dimension::FILL),
            );
        }
        if !state.availability_message.is_empty() {
            body = body.child(
                text(&state.availability_message)
                    .size(13.0)
                    .color(MUTED)
                    .width(Dimension::FILL),
            );
        }
        if let Some(warning) = &state.performance_warning {
            body = body.child(text(warning).size(13.0).color(MUTED).width(Dimension::FILL));
        }
        if !state.operation_message.is_empty() {
            body = body.child(
                text(&state.operation_message)
                    .size(13.0)
                    .color(if state.failed { TEXT } else { MUTED })
                    .width(Dimension::FILL),
            );
        }
        body
    }
}
