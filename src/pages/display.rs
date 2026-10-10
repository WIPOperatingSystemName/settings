use crate::{
    components::controls::{choice, disclosure, divider, group, page_content},
    controllers::DisplayController,
    theme::{MUTED, TEXT},
};
use telorgon::app::*;

#[component(no_default)]
pub struct DisplayPage {
    #[input]
    controller: DisplayController,
    #[state]
    expanded: Option<u8>,
}
impl DisplayPage {
    pub fn new(controller: DisplayController) -> Self {
        Self {
            controller,
            expanded: None,
        }
    }
    fn expand(&mut self, section: u8) {
        self.expanded = if self.expanded == Some(section) {
            None
        } else {
            Some(section)
        };
    }
}
impl Component for DisplayPage {
    fn view(&self) -> impl View {
        let state = self.controller.state(self);
        let mut modes = state.modes.clone();
        modes.sort_by_key(|m| (m.width, m.height, m.refresh_millihertz));
        modes.dedup();
        let extra = match self.expanded {
            Some(0) => (modes.len() + 1) as f32 * 42.0,
            Some(1) => state.scales.len() as f32 * 42.0,
            _ => 0.0,
        };
        let mode_label = if state.recommended {
            "Automatic".into()
        } else {
            format!("{} · {}", state.resolution, state.refresh_rate)
        };
        let scale_label = state
            .scales
            .iter()
            .find(|s| s.selected)
            .map_or("Auto", |s| s.label.as_str());
        let mut settings = group(97.0 + extra).child(
            disclosure("Display mode", &mode_label, self.expanded == Some(0))
                .enabled(state.enabled)
                .on_press(|this: &mut Self| this.expand(0)),
        );
        if self.expanded == Some(0) {
            settings = settings.child(
                choice("Automatic (recommended)", state.recommended)
                    .enabled(state.enabled)
                    .on_press(|this: &mut Self| {
                        this.controller.use_recommended_mode();
                        this.expanded = None;
                    }),
            );
            for mode in modes {
                let label = format!(
                    "{} × {} · {:.2} Hz",
                    mode.width,
                    mode.height,
                    mode.refresh_millihertz as f32 / 1000.0
                );
                let selected = state.selected_mode == Some(mode);
                settings =
                    settings.child(choice(&label, selected).enabled(state.enabled).on_press(
                        move |this: &mut Self| {
                            this.controller.select_mode(mode);
                            this.expanded = None;
                        },
                    ));
            }
        }
        settings = settings.child(divider()).child(
            disclosure(
                "Text and interface size",
                scale_label,
                self.expanded == Some(1),
            )
            .enabled(state.enabled)
            .on_press(|this: &mut Self| this.expand(1)),
        );
        if self.expanded == Some(1) {
            for scale in &state.scales {
                let value = scale.value;
                settings = settings.child(
                    choice(&scale.label, scale.selected)
                        .enabled(state.enabled)
                        .on_press(move |this: &mut Self| {
                            this.controller.select_scale(value);
                            this.expanded = None;
                        }),
                );
            }
        }
        let mut body = page_content(340.0 + extra + if state.error.is_some() { 60.0 } else { 0.0 })
            .gap(16.0)
            .child(text("Display").size(24.0).weight(600).color(TEXT))
            .child(column().height(100.0).gap(8.0).padding(12.0)
                .child(text(&state.monitor).size(18.0).weight(600).color(TEXT))
                .child(text(&state.monitor_details).size(13.0).color(MUTED)))
            .child(settings)
            .child(text("Display changes preview for 20 seconds. Keep them to confirm, or they revert automatically.")
                .size(13.0).color(MUTED).width(Dimension::FILL).height(40.0));
        if let Some(error) = &state.error {
            body = body.child(text(error).size(13.0).color(TEXT));
        }
        body
    }
}
