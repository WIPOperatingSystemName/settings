use super::Sources;
use crate::services::Status;
use telorgon::app::Component;
use telorgon::services::desktop_settings::DisplayMode;

const SCALES: [Option<f32>; 9] = [
    None,
    Some(1.0),
    Some(1.25),
    Some(1.5),
    Some(1.75),
    Some(2.0),
    Some(2.5),
    Some(3.0),
    Some(4.0),
];
pub struct ScaleChoice {
    pub value: Option<f32>,
    pub label: String,
    pub selected: bool,
}
pub struct DisplayState {
    pub enabled: bool,
    pub monitor: String,
    pub monitor_details: String,
    pub resolution: String,
    pub refresh_rate: String,
    pub modes: Vec<DisplayMode>,
    pub recommended: bool,
    pub selected_mode: Option<DisplayMode>,
    pub scales: Vec<ScaleChoice>,
    pub error: Option<String>,
}
fn enabled(status: &Status) -> bool {
    status.shell.as_ref().is_some_and(|s| s.display.ready)
        && !status.busy
        && status.preview.is_none()
}
#[derive(Clone, PartialEq)]
pub struct DisplayController {
    sources: Sources,
}
impl DisplayController {
    pub(super) fn new(sources: Sources) -> Self {
        Self { sources }
    }
    pub fn state(&self, observer: &impl Component) -> DisplayState {
        let (status, draft) = self.sources.observe(observer);
        let current = status.shell.as_ref().map(|s| &s.display);
        let mode = draft.display.mode.or_else(|| current?.current.mode);
        DisplayState {
            enabled: enabled(&status),
            monitor: current.map_or("Monitor unavailable".into(), monitor_name),
            monitor_details: current.map_or(
                "Open this app in your Telorgon shell session to configure a monitor.".into(),
                monitor_details,
            ),
            resolution: mode.map_or("Unavailable".into(), |m| {
                format!("{} × {}", m.width, m.height)
            }),
            refresh_rate: mode.map_or("Unavailable".into(), |m| {
                format!("{:.2} Hz", m.refresh_millihertz as f32 / 1000.0)
            }),
            modes: current.map_or_else(Vec::new, |s| s.modes.clone()),
            recommended: draft.display.mode.is_none(),
            selected_mode: draft.display.mode,
            scales: SCALES
                .into_iter()
                .map(|value| ScaleChoice {
                    value,
                    selected: draft.display.scale == value,
                    label: value.map_or("Auto".into(), |v| format!("{:.0}%", v * 100.0)),
                })
                .collect(),
            error: current.and_then(|s| s.error.clone()),
        }
    }
    pub fn select_mode(&self, mode: DisplayMode) {
        let status = self.sources.model.signal.snapshot();
        if !enabled(&status) {
            return;
        }
        let Some(shell) = &status.shell else {
            return;
        };
        if !shell.display.modes.contains(&mode) {
            return;
        }
        self.sources.editor.edit_display(|display| {
            display.mode = Some(mode);
            display.connector = shell.display.connector.clone();
        });
    }
    pub fn use_recommended_mode(&self) {
        if !enabled(&self.sources.model.signal.snapshot()) {
            return;
        }
        self.sources.editor.edit_display(|display| {
            display.mode = None;
            display.connector.clear();
        });
    }
    pub fn select_scale(&self, value: Option<f32>) {
        if !enabled(&self.sources.model.signal.snapshot()) || !SCALES.contains(&value) {
            return;
        }
        self.sources
            .editor
            .edit_display(|display| display.scale = value);
    }
    pub fn save(&self) {
        let status = self.sources.model.signal.snapshot();
        if enabled(&status) && !status.load_error {
            self.sources
                .model
                .save_display(self.sources.editor.draft.snapshot().display.clone());
        }
    }
}

fn monitor_name(display: &telorgon::services::desktop_settings::DisplaySnapshot) -> String {
    display
        .monitor_name
        .as_ref()
        .or(display.model.as_ref())
        .or(display.connector_name.as_ref())
        .unwrap_or(&display.connector)
        .clone()
}

fn monitor_details(display: &telorgon::services::desktop_settings::DisplaySnapshot) -> String {
    let mut details = vec![
        display
            .connector_name
            .as_ref()
            .unwrap_or(&display.connector)
            .clone(),
        format!("Active scale: {:.0}%", display.resolved_scale * 100.0),
    ];
    if let Some(size) = display.physical_millimeters {
        details.push(format!("{} × {} mm", size.width, size.height));
    }
    let mut identity = Vec::new();
    if let Some(manufacturer) = &display.manufacturer {
        identity.push(format!("Manufacturer: {manufacturer}"));
    }
    if let Some(product) = display.product_code {
        identity.push(format!("Product: {product:04X}"));
    }
    if let Some(serial) = &display.serial_number {
        identity.push(format!("Serial: {serial}"));
    }
    let mut result = details.join(" · ");
    if !identity.is_empty() {
        result.push('\n');
        result.push_str(&identity.join(" · "));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn monitor_identity_and_legacy_fallback_are_displayed() {
        let mut display = telorgon::services::desktop_settings::DisplaySnapshot {
            connector: "DRM-10-1".into(),
            resolved_scale: 1.5,
            ..Default::default()
        };
        assert_eq!(monitor_name(&display), "DRM-10-1");
        assert_eq!(monitor_details(&display), "DRM-10-1 · Active scale: 150%");
        display.connector_name = Some("DP-1".into());
        assert_eq!(monitor_name(&display), "DP-1");
        display.monitor_name = Some("Test Monitor".into());
        display.manufacturer = Some("DEL".into());
        display.serial_number = Some("123".into());
        assert_eq!(monitor_name(&display), "Test Monitor");
        assert!(monitor_details(&display).contains("Manufacturer: DEL · Serial: 123"));
    }
}
