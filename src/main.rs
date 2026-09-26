mod pages;

use telorgon::app::*;
use pages::*;

telorgon::asset_catalog! {
    pub mod assets = "assets";
}


fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    Application::gui("org.telorgon.settings", "Telorgon Settings")
        .renderer(Renderer::Vulkan)
        .assets(assets::bundle())
        .window(
            Window::new("Telorgon Settings")
                .size(1100, 720)
                .content(SettingsApp::default()),
        )
        .run()?;
    Ok(())
}
