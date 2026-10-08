use crate::{
    components::controls::{disclosure, divider, group, page_content},
    controllers::{BatteryController, InstalledBattery},
    theme::{MUTED, TEXT},
};
use telorgon::app::*;

#[component(no_default)]
pub struct BatteryPage {
    #[input]
    controller: BatteryController,
    #[state]
    expanded: Option<String>,
}
impl BatteryPage {
    pub fn new(controller: BatteryController) -> Self {
        Self {
            controller,
            expanded: None,
        }
    }

    fn battery(&self, battery: &InstalledBattery, narrow: bool) -> Container {
        let expanded = self.expanded.as_deref() == Some(battery.id.as_str());
        let id = battery.id.clone();
        let mut details = group(details_height(battery, expanded, narrow)).child(
            disclosure("Battery details", "", expanded).on_press(move |this: &mut Self| {
                this.expanded = if this.expanded.as_ref() == Some(&id) {
                    None
                } else {
                    Some(id.clone())
                };
            }),
        );
        if expanded {
            details = details.child(divider());
            for detail in &battery.details {
                details = details.child(value_row(detail.label, &detail.value, narrow));
            }
        }
        let summary = if narrow {
            column().height(64.0).gap(2.0)
        } else {
            row().height(38.0).gap(16.0).align_items(Alignment::Center)
        }
        .child(
            text(&battery.charge)
                .size(if battery.charge.ends_with('%') {
                    30.0
                } else {
                    20.0
                })
                .weight(600)
                .color(TEXT)
                .height(38.0),
        )
        .child(text(battery.state).size(14.0).color(MUTED).height(24.0));
        column()
            .width(Dimension::FILL)
            .height(battery_height(battery, expanded, narrow))
            .gap(12.0)
            .child(
                group(if narrow { 150.0 } else { 124.0 })
                    .padding(12.0)
                    .gap(6.0)
                    .child(
                        text(&battery.title)
                            .size(18.0)
                            .weight(600)
                            .color(TEXT)
                            .height(26.0),
                    )
                    .child(summary)
                    .child(text(&battery.estimate).size(13.0).color(MUTED).height(24.0)),
            )
            .child(
                text("Battery health")
                    .size(15.0)
                    .weight(600)
                    .color(TEXT)
                    .height(22.0),
            )
            .child(
                group(3.0 * row_height(narrow) + 2.0)
                    .child(value_row(
                        "Maximum capacity",
                        &battery.maximum_capacity,
                        narrow,
                    ))
                    .child(divider())
                    .child(value_row("Cycle count", &battery.cycle_count, narrow))
                    .child(divider())
                    .child(value_row("Reported condition", &battery.condition, narrow)),
            )
            .child(details)
    }
}
impl Component for BatteryPage {
    fn view(&self) -> impl View {
        let state = self.controller.state(self);
        let narrow = self.viewport_size().width < 800.0;
        let battery_heights = state
            .batteries
            .iter()
            .map(|battery| {
                battery_height(
                    battery,
                    self.expanded.as_deref() == Some(battery.id.as_str()),
                    narrow,
                ) + 16.0
            })
            .sum::<f32>();
        let mut body = page_content(
            168.0
                + row_height(narrow)
                + battery_heights
                + if state.warning.is_some() { 90.0 } else { 0.0 }
                + if state.batteries.is_empty() {
                    50.0
                } else {
                    0.0
                },
        )
        .gap(16.0)
        .child(
            text("Battery")
                .size(24.0)
                .weight(600)
                .color(TEXT)
                .height(32.0),
        )
        .child(group(row_height(narrow)).child(value_row(
            "External power",
            state.external_power,
            narrow,
        )));
        if let Some(warning) = &state.warning {
            body = body.child(text(warning).size(13.0).color(TEXT).height(74.0));
        }
        if state.batteries.is_empty() {
            body = body.child(
                text(if state.loading {
                    "Checking for installed batteries…"
                } else {
                    "No installed system battery detected."
                })
                .size(14.0)
                .color(MUTED)
                .height(34.0),
            );
        }
        for battery in &state.batteries {
            body = body.child(self.battery(battery, narrow));
        }
        body.child(
            text("Maximum capacity compares full-charge capacity with the original design capacity. Reported condition comes from the device. Time estimates change with workload and charging speed.")
                .size(13.0)
                .color(MUTED)
                .height(88.0),
        )
    }
}

fn value_row(label: &str, value: &str, narrow: bool) -> Container {
    if narrow {
        return column()
            .width(Dimension::FILL)
            .height(row_height(true))
            .padding(10.0)
            .gap(2.0)
            .child(text(label).size(12.0).color(MUTED).height(20.0))
            .child(text(value).size(13.0).color(TEXT).height(22.0));
    }
    row()
        .width(Dimension::FILL)
        .height(40.0)
        .padding(12.0)
        .gap(12.0)
        .align_items(Alignment::Center)
        .child(text(label).size(13.0).color(TEXT).width(Dimension::FILL))
        .child(text(value).size(13.0).color(MUTED))
}

fn row_height(narrow: bool) -> f32 {
    if narrow { 64.0 } else { 40.0 }
}

fn details_height(battery: &InstalledBattery, expanded: bool, narrow: bool) -> f32 {
    48.0 + if expanded {
        1.0 + battery.details.len() as f32 * row_height(narrow)
    } else {
        0.0
    }
}

fn battery_height(battery: &InstalledBattery, expanded: bool, narrow: bool) -> f32 {
    184.0
        + if narrow { 26.0 } else { 0.0 }
        + 3.0 * row_height(narrow)
        + details_height(battery, expanded, narrow)
}
