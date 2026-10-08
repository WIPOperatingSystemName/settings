use crate::{
    assets,
    controllers::{NavigationController, Page},
    theme::{ACCENT, PANEL, TEXT},
};
use telorgon::app::*;

#[component(no_default)]
pub struct Sidebar {
    #[input]
    controller: NavigationController,
}
impl Sidebar {
    pub fn new(controller: NavigationController) -> Self {
        Self { controller }
    }
    fn navigation(
        &self,
        title: &str,
        icon: impl Into<ImageSource>,
        page: Page,
        selected: Page,
    ) -> Button {
        button()
            .height(38.0)
            .width(Dimension::FILL)
            .background(if selected == page { ACCENT } else { PANEL })
            .corner_radius(6.0)
            .child(
                row()
                    .padding(10.0)
                    .gap(10.0)
                    .align_items(Alignment::Center)
                    .child(image(icon).width(18.0).height(18.0).tint(TEXT))
                    .child(text(title).size(14.0).color(TEXT)),
            )
            .on_press(move |this: &mut Self| this.controller.select(page))
    }
}
impl Component for Sidebar {
    fn view(&self) -> impl View {
        let selected = self.controller.selected(self);
        let mut sidebar = column()
            .width(200.0)
            .height(Dimension::FILL)
            .padding(12.0)
            .gap(6.0)
            .background(PANEL)
            .child(
                text("Settings")
                    .size(18.0)
                    .weight(600)
                    .color(TEXT)
                    .padding(10.0),
            )
            .child(self.navigation("Display", assets::icons::MONITOR, Page::Display, selected))
            .child(self.navigation("Sound", assets::icons::AUDIO_LINES, Page::Sound, selected))
            .child(self.navigation("Network", assets::icons::NETWORK, Page::Network, selected))
            .child(self.navigation("Power", assets::icons::POWER, Page::Power, selected))
            .child(self.navigation(
                "Personalization",
                assets::icons::PERSONALIZATION,
                Page::Personalization,
                selected,
            ));
        if self.controller.has_battery(self) {
            sidebar = sidebar.child(self.navigation(
                "Battery",
                assets::icons::BATTERY,
                Page::Battery,
                selected,
            ));
        }
        sidebar
    }
}
