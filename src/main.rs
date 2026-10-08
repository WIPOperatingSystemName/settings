mod components;
mod controllers;
mod pages;
mod services;
mod settings_app;
mod settings_config;
mod state;
mod theme;

use settings_app::SettingsApp;
use telorgon::app::*;

telorgon::asset_catalog! {
    pub mod assets = "assets";
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let page =
        state::Page::from_arguments(std::env::args().skip(1)).map_err(std::io::Error::other)?;
    let (_backend, model, preferences) = services::Backend::start()?;
    let (_network, network) = services::NetworkService::start()?;
    let (_power, power) = services::PowerService::start()?;
    let (_battery, battery) = services::BatteryService::start()?;
    Application::gui("org.telorgon.settings", "Telorgon Settings")
        .renderer(Renderer::Auto)
        .assets(assets::bundle())
        .window(
            Window::new("Telorgon Settings")
                .size(1100, 720)
                .content(SettingsApp::new(controllers::SettingsController::new(
                    model,
                    preferences,
                    network,
                    power,
                    battery,
                    page,
                ))),
        )
        .run()?;
    Ok(())
}
