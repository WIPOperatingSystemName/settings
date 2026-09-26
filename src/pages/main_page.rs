use crate::assets;
use telorgon::app::*;


#[component]
pub struct SettingsApp {}

impl Component for SettingsApp {
    fn view(&self) -> impl View {
        row()
            .padding(16.0)
            .gap(16.0)
            .background(ColorRgba8::rgba(18, 21, 28, 255))
            .child(self.sidebar())
            .child(self.content_panel())
    }
}

impl SettingsApp {
    fn settings_row(&self, txt: impl ToString, icon: impl Into<ImageSource>) -> impl View {
        button()
            .height(32.0)
            .width(Dimension::FILL)
            .hover_effect(InteractionEffect::Background(ColorRgba8::rgba(18, 21, 28, 255)))
            .press_effect(InteractionEffect::Background(ColorRgba8::rgba(33, 37, 46, 255)))
            .child(
                row()
                    .padding(4.0)
                    .gap(8.0)
                    .align_items(Alignment::Center)
                    .child(
                        image(icon)
                            .height(Dimension::FILL)
                            .aspect_ratio(1.0)
                    )
                    .child(
                        text(txt)
                            .width(Dimension::FILL)
                            .height(Dimension::FILL)
                            .fit_height(true)
                            .color(ColorRgba8::rgba(240, 243, 250, 255))
                    )
            )
        
    }

    fn sidebar(&self) -> impl View {
        column()
            .width(240.0)
            .height(Dimension::FILL)
            .padding(20.0)
            .corner_radius(12.0)
            .background(ColorRgba8::rgba(28, 33, 43, 255))
            .child(self.settings_row("Display", assets::icons::MONITOR))
            .child(self.settings_row("Sound", assets::icons::AUDIO_LINES))
    }

    fn content_panel(&self) -> impl View {
        column()
            .width(Dimension::FILL)
            .height(Dimension::FILL)
            .padding(28.0)
            .gap(12.0)
            .corner_radius(12.0)
            .background(ColorRgba8::rgba(35, 41, 53, 255))
            .child(
                text("General")
                    .size(28.0)
                    .weight(700)
                    .color(ColorRgba8::rgba(240, 243, 250, 255)),
            )
            .child(
                text("Add your settings controls here.")
                    .size(16.0)
                    .color(ColorRgba8::rgba(166, 177, 196, 255)),
            )
    }
}