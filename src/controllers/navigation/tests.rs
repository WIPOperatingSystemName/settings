use super::*;
use crate::{
    components::Sidebar,
    services::{BatteryModel, BatteryReadings},
};
use telorgon::{
    app::*,
    application_host::ComposedAppRuntime,
    battery::{BatteryKind, BatteryMetrics, BatteryScope, BatterySnapshot, BatteryState},
};

#[component(no_default)]
struct NavigationHost {
    #[input]
    controller: NavigationController,
}
impl Component for NavigationHost {
    fn view(&self) -> impl View {
        row()
            .width(Dimension::FILL)
            .height(Dimension::FILL)
            .child(Sidebar::new(self.controller.clone()))
            .child(text(format!(
                "Selected: {:?}",
                self.controller.selected(self)
            )))
    }
}

fn runtime(
    initial: BatteryReadings,
    page: Page,
) -> (ComposedAppRuntime, SignalWriter<BatteryReadings>) {
    let (signal, writer) = Signal::new(initial);
    let editor = Editor::new(Default::default());
    editor.select(page);
    let controller = NavigationController::new(
        editor,
        BatteryController::new(BatteryModel::from_signal(signal)),
    );
    let mut runtime = ComposedAppRuntime::from_composed_with_extent(
        NavigationHost { controller },
        SizeI {
            width: 640,
            height: 480,
        },
    )
    .unwrap();
    frame(&mut runtime, 0);
    (runtime, writer)
}

fn frame(runtime: &mut ComposedAppRuntime, tick: u64) {
    runtime
        .prepare_frame(telorgon::MonotonicInstant::from_nanos(tick * 1_000_000), false)
        .unwrap();
}

fn has(runtime: &ComposedAppRuntime, label: &str) -> bool {
    runtime
        .ui()
        .texts
        .iter()
        .any(|(_, text)| runtime.ui().string(text.content) == Some(label))
        || runtime.ui().semantics.iter().any(|(_, semantics)| {
            matches!(semantics.name, telorgon::SemanticName::Text(name)
                if runtime.ui().string(name) == Some(label))
        })
}

#[test]
fn compact_navigation_remains_visible_across_resizes() {
    let (mut runtime, _writer) = runtime(readings(Vec::new()), Page::Display);
    for (tick, width, height) in [(1000, 640, 480), (2000, 960, 560), (3000, 640, 480)] {
        runtime.queue_input(telorgon::PlatformInput::Resize(telorgon::SizeF {
            width: width as f32,
            height: height as f32,
        }));
        runtime.flush_input(telorgon::MonotonicInstant::from_nanos(tick * 1_000_000));
        frame(&mut runtime, tick);
        for label in ["Display", "Sound", "Network", "Power", "Personalization"] {
            let node = runtime
                .ui()
                .semantics
                .iter()
                .find_map(|(node, semantics)| {
                    matches!(semantics.name, telorgon::SemanticName::Text(name)
                        if runtime.ui().string(name) == Some(label))
                        .then_some(node)
                })
                .unwrap();
            let layout = runtime.layout().computed(node).unwrap();
            assert!(layout.visible_rect.width >= layout.border_rect.width - 0.5);
            assert!(layout.visible_rect.height >= layout.border_rect.height - 0.5);
            assert!(layout.border_rect.width < if width < 800 { 64.0 } else { 200.0 });
        }
    }
}

fn readings(batteries: Vec<BatteryMetrics>) -> BatteryReadings {
    BatteryReadings::from_snapshot(BatterySnapshot {
        observed_at: std::time::SystemTime::UNIX_EPOCH,
        batteries,
        external_supplies: Vec::new(),
        external_power: Some(false),
    })
}

fn battery(kind: BatteryKind, scope: BatteryScope, present: bool) -> BatteryMetrics {
    BatteryMetrics {
        id: "fixture".into(),
        kind,
        scope,
        present,
        manufacturer: None,
        model: None,
        technology: None,
        state: BatteryState::Discharging,
        charge_percent: Some(70.0),
        health: None,
        capacity_health_percent: None,
        cycle_count: None,
        energy_now_wh: None,
        energy_full_wh: None,
        energy_full_design_wh: None,
        energy_empty_wh: None,
        charge_now_ah: None,
        charge_full_ah: None,
        charge_full_design_ah: None,
        charge_empty_ah: None,
        power_watts: None,
        voltage_volts: None,
        current_amps: None,
        temperature_celsius: None,
        time_to_empty: None,
        time_to_full: None,
    }
}

#[test]
fn power_stays_visible_and_battery_navigation_requires_installed_host_hardware() {
    let page = Page::from_arguments(["--page=power".into()]).unwrap();
    let (mut runtime, writer) = runtime(readings(Vec::new()), page);
    assert!(has(&runtime, "Power"));
    assert!(has(&runtime, "Selected: Power"));
    assert!(!has(&runtime, "Battery"));

    for (tick, kind, scope, present, visible) in [
        (1, BatteryKind::Battery, BatteryScope::Device, true, false),
        (2, BatteryKind::Ups, BatteryScope::System, true, false),
        (3, BatteryKind::Battery, BatteryScope::System, false, false),
        (4, BatteryKind::Battery, BatteryScope::System, true, true),
        (5, BatteryKind::Battery, BatteryScope::Unknown, true, true),
    ] {
        writer.publish_if_changed(readings(vec![battery(kind, scope, present)]));
        frame(&mut runtime, tick);
        assert!(has(&runtime, "Power"));
        assert!(has(&runtime, "Selected: Power"));
        assert_eq!(has(&runtime, "Battery"), visible);
    }
}

#[test]
fn battery_deep_link_waits_for_detection_and_falls_back_after_confirmed_removal() {
    let page = Page::from_arguments(["--page".into(), "battery".into()]).unwrap();
    let (mut runtime, writer) = runtime(
        BatteryReadings {
            loading: true,
            ..Default::default()
        },
        page,
    );
    assert!(has(&runtime, "Selected: Power"));
    assert!(!has(&runtime, "Battery"));

    let mut detected = readings(vec![battery(
        BatteryKind::Battery,
        BatteryScope::System,
        true,
    )]);
    writer.publish_if_changed(detected.clone());
    frame(&mut runtime, 1);
    assert!(has(&runtime, "Battery"));
    assert!(has(&runtime, "Selected: Battery"));

    detected.error = Some("Temporary read failure".into());
    writer.publish_if_changed(detected);
    frame(&mut runtime, 2);
    assert!(has(&runtime, "Battery"));
    assert!(has(&runtime, "Selected: Battery"));

    writer.publish_if_changed(readings(Vec::new()));
    frame(&mut runtime, 3);
    assert!(has(&runtime, "Power"));
    assert!(!has(&runtime, "Battery"));
    assert!(has(&runtime, "Selected: Power"));
    assert!(!has(&runtime, "Selected: Battery"));
}
