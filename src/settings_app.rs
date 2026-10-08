use crate::{
    components::{SettingsFooter, Sidebar},
    controllers::{Page, SettingsController},
    pages::{BatteryPage, DisplayPage, NetworkPage, PersonalizationPage, PowerPage, SoundPage},
    theme::BG,
};
use telorgon::app::*;

#[component(no_default)]
pub struct SettingsApp {
    #[input]
    controller: SettingsController,
}
impl SettingsApp {
    pub fn new(controller: SettingsController) -> Self {
        Self { controller }
    }
}
impl Component for SettingsApp {
    fn view(&self) -> impl View {
        let page = self.controller.navigation.selected(self);
        row()
            .gap(1.0)
            .background(BG)
            .child(Sidebar::new(self.controller.navigation.clone()))
            .child(
                column()
                    .width(Dimension::FILL)
                    .height(Dimension::FILL)
                    .padding(28.0)
                    .gap(16.0)
                    .background(BG)
                    .child(if page == Page::Network {
                        NetworkPage::new(self.controller.network.clone()).into_element()
                    } else {
                        column()
                            .height(Dimension::FILL)
                            .width(Dimension::FILL)
                            .scrollable()
                            .child(match page {
                                Page::Display => {
                                    DisplayPage::new(self.controller.display.clone()).into_element()
                                }
                                Page::Sound => {
                                    SoundPage::new(self.controller.sound.clone()).into_element()
                                }
                                Page::Power => {
                                    PowerPage::new(self.controller.power.clone()).into_element()
                                }
                                Page::Battery => {
                                    BatteryPage::new(self.controller.battery.clone()).into_element()
                                }
                                Page::Personalization => PersonalizationPage::new(
                                    self.controller.personalization.clone(),
                                )
                                .into_element(),
                                Page::Network => unreachable!(),
                            })
                            .into_element()
                    })
                    .child(SettingsFooter::new(self.controller.footer.clone())),
            )
    }
}
