use crate::theme::{ACCENT, CARD, LINE, MUTED, TEXT};
use telorgon::app::*;

// Scroll content must be allowed to exceed the viewport's height.
pub(crate) fn page_content(height: f32) -> Container {
    column().box_style(telorgon::BoxStyle {
        width: telorgon::SizeRule::Fill(1.0),
        height: telorgon::SizeRule::Logical(height),
        max_size: telorgon::SizeRule2D {
            width: telorgon::SizeRule::Fill(1.0),
            height: telorgon::SizeRule::Logical(f32::MAX),
        },
        ..Default::default()
    })
}

pub(crate) fn control(label: impl ToString) -> Button {
    button()
        .width(Dimension::FILL)
        .height(36.0)
        .padding(8.0)
        .background(CARD)
        .corner_radius(6.0)
        .child(text(label).size(14.0).color(TEXT))
}

pub(crate) fn divider() -> Container {
    row().width(Dimension::FILL).height(1.0).background(LINE)
}

pub(crate) fn group(height: f32) -> Container {
    column()
        .width(Dimension::FILL)
        .height(height)
        .background(CARD)
        .corner_radius(10.0)
}

pub(crate) fn choice(label: &str, selected: bool) -> Button {
    button()
        .width(Dimension::FILL)
        .height(42.0)
        .padding(12.0)
        .background(CARD)
        .child(
            row()
                .align_items(Alignment::Center)
                .gap(12.0)
                .child(text(label).size(14.0).color(TEXT).width(Dimension::FILL))
                .child(
                    text(if selected { "✓" } else { "" })
                        .size(16.0)
                        .color(ACCENT)
                        .width(20.0),
                ),
        )
}

pub(crate) fn disclosure(label: &str, value: &str, expanded: bool) -> Button {
    button()
        .width(Dimension::FILL)
        .height(48.0)
        .padding(12.0)
        .background(CARD)
        .child(
            row()
                .align_items(Alignment::Center)
                .gap(12.0)
                .child(text(label).size(14.0).color(TEXT).width(Dimension::FILL))
                .child(text(value).size(14.0).color(MUTED))
                .child(
                    text(if expanded { "⌄" } else { "›" })
                        .size(16.0)
                        .color(MUTED)
                        .width(16.0),
                ),
        )
}
