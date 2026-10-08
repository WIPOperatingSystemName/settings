use crate::{
    components::controls::{control, divider},
    controllers::{FooterActions, FooterController},
    theme::{ACCENT, MUTED},
};
use telorgon::app::*;

#[component(no_default)]
pub struct SettingsFooter {
    #[input]
    controller: FooterController,
}
impl SettingsFooter {
    pub fn new(controller: FooterController) -> Self {
        Self { controller }
    }
}
impl Component for SettingsFooter {
    fn view(&self) -> impl View {
        let state = self.controller.state(self);
        if matches!(state.actions, FooterActions::None) {
            return column().height(0.0);
        }
        let mut footer = column()
            .height(if state.notice.is_some() { 112.0 } else { 78.0 })
            .gap(10.0)
            .child(divider());
        if let Some(notice) = state.notice {
            footer = footer.child(text(notice).size(12.0).color(MUTED));
        }
        let actions = match state.actions {
            FooterActions::None => row().height(0.0),
            FooterActions::Confirm { label, enabled } => row()
                .width(278.0)
                .height(32.0)
                .gap(8.0)
                .child(
                    control("Revert")
                        .width(90.0)
                        .height(32.0)
                        .enabled(enabled)
                        .on_press(|this: &mut Self| this.controller.revert()),
                )
                .child(
                    control(label)
                        .width(180.0)
                        .height(32.0)
                        .background(ACCENT)
                        .enabled(enabled)
                        .on_press(|this: &mut Self| this.controller.keep()),
                ),
            FooterActions::Save {
                label,
                enabled,
                reset_enabled,
            } => row()
                .width(278.0)
                .height(32.0)
                .gap(8.0)
                .child(
                    control("Restore defaults")
                        .width(140.0)
                        .height(32.0)
                        .enabled(reset_enabled)
                        .on_press(|this: &mut Self| this.controller.reset()),
                )
                .child(
                    control(label)
                        .width(120.0)
                        .height(32.0)
                        .background(ACCENT)
                        .enabled(enabled)
                        .on_press(|this: &mut Self| this.controller.save()),
                ),
        };
        footer
            .child(text(&state.message).size(12.0).color(MUTED))
            .child(
                row()
                    .height(32.0)
                    .child(row().width(Dimension::FILL))
                    .child(actions),
            )
    }
}
