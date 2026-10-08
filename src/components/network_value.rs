use crate::{
    components::{
        controls::control,
        entry::{Entry, EntryValue},
    },
    controllers::NetworkController,
    theme::{MUTED, TEXT},
};
use telorgon::app::*;
#[component(no_default)]
pub(crate) struct ValueRow {
    #[input]
    controller: NetworkController,
    #[input]
    label: String,
    #[input]
    value: String,
    #[state]
    entry: EntryValue,
}
impl ValueRow {
    pub fn new(controller: NetworkController, label: &str, value: String) -> Self {
        Self {
            controller,
            label: label.into(),
            entry: EntryValue::new_read_only(&value),
            value,
        }
    }
}
impl Component for ValueRow {
    fn inputs_changed(&mut self, _: &mut InputsChangedContext<Self>) {
        if self.entry.text() != self.value {
            self.entry = EntryValue::new_read_only(&self.value);
        }
    }
    fn view(&self) -> impl View {
        let narrow = self.viewport_size().width < 700.0;
        row()
            .width(Dimension::FILL)
            .height(38.0)
            .gap(8.0)
            .align_items(Alignment::Center)
            .child(
                text(&self.label)
                    .width(if narrow { 95.0 } else { 140.0 })
                    .size(12.0)
                    .color(MUTED),
            )
            .child(Entry::new(&self.label, "", self.entry.clone(), true).compact())
            .child(
                control("Copy")
                    .accessible_label(format!("Copy {}", self.label))
                    .width(64.0)
                    .height(28.0)
                    .enabled(!self.controller.operation(self).busy)
                    .on_press(|this: &mut Self| this.controller.copy(this.value.clone())),
            )
            .background(TEXT.with_alpha(0))
    }
}
