use crate::theme::{ACCENT, CARD, MUTED, TEXT};
use std::sync::{Arc, Mutex};
use telorgon::{
    app::*,
    services::clipboard::{ClipboardEditAction, ClipboardText},
    ui::{UiEvent, UiEventKind},
};

#[derive(Clone)]
pub(crate) struct EntryValue(Arc<Value>);
struct Value {
    editor: Mutex<ClipboardText>,
    revision: Signal<u64>,
    writer: SignalWriter<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use telorgon::{
        InputEvent,
        input::{ButtonState, KeyEvent, KeyText, LogicalKey, NamedKey, PhysicalKey},
    };
    #[component(no_default)]
    struct Fields {
        #[input]
        password: EntryValue,
        #[input]
        ip: EntryValue,
    }
    impl Component for Fields {
        fn view(&self) -> impl View {
            column()
                .height(120.0)
                .child(Entry::new("Password", "", self.password.clone(), true))
                .child(Entry::new("Address", "", self.ip.clone(), true))
        }
    }
    #[test]
    fn network_fields_route_keyboard_focus_and_hide_passwords() {
        let password = EntryValue::new("", true);
        let ip = EntryValue::new("", false);
        let mut runtime = telorgon::application_host::AppRuntimeCore::from_composed_with_extent(
            Fields {
                password: password.clone(),
                ip: ip.clone(),
            },
            SizeI {
                width: 500,
                height: 150,
            },
        )
        .unwrap();
        let at = |ms: u64| telorgon::MonotonicInstant::from_nanos(ms * 1_000_000);
        runtime.prepare_frame(at(0), false).unwrap();
        let key = |logical, value| {
            InputEvent::Key(
                KeyEvent::new(PhysicalKey::UNIDENTIFIED, ButtonState::Pressed)
                    .with_logical_key(logical)
                    .with_text(value),
            )
        };
        runtime.queue_input(key(LogicalKey::Named(NamedKey::Tab), None));
        runtime.flush_input(at(1));
        runtime.prepare_frame(at(1), false).unwrap();
        runtime.queue_input(key(
            LogicalKey::Character(KeyText::new("é").unwrap()),
            Some(KeyText::new("sëcret123").unwrap()),
        ));
        runtime.flush_input(at(2));
        runtime.prepare_frame(at(2), false).unwrap();
        assert_eq!(password.text(), "sëcret123");
        assert!(runtime.ui().texts.iter().all(|(_, text)| {
            !runtime
                .ui()
                .string(text.content)
                .unwrap()
                .contains("sëcret123")
        }));
        runtime.queue_input(key(LogicalKey::Named(NamedKey::Backspace), None));
        runtime.flush_input(at(3));
        runtime.prepare_frame(at(3), false).unwrap();
        assert_eq!(password.text(), "sëcret12");
        runtime.queue_input(key(LogicalKey::Named(NamedKey::Tab), None));
        runtime.flush_input(at(4));
        runtime.prepare_frame(at(4), false).unwrap();
        runtime.queue_input(key(
            LogicalKey::Character(KeyText::new("1").unwrap()),
            Some(KeyText::new("192.0.2.10/24").unwrap()),
        ));
        runtime.flush_input(at(5));
        runtime.prepare_frame(at(5), false).unwrap();
        assert_eq!(ip.text(), "192.0.2.10/24");
        assert_eq!(password.take(), "sëcret12");
        assert!(password.text().is_empty());
    }
}
impl PartialEq for EntryValue {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl EntryValue {
    pub fn new_read_only(text: &str) -> Self {
        let value = Self::new(text, false);
        value.0.editor.lock().unwrap().read_only = true;
        value
    }
    pub fn new(text: impl Into<String>, secure: bool) -> Self {
        let text = text.into();
        let (revision, writer) = Signal::new(0);
        let mut editor = ClipboardText::default();
        editor.cursor = text.len();
        editor.anchor = text.len();
        editor.text = text;
        editor.secure = secure;
        Self(Arc::new(Value {
            editor: Mutex::new(editor),
            revision,
            writer,
        }))
    }
    pub fn observe(&self, observer: &impl Component) -> String {
        observer.watch(&self.0.revision);
        self.text()
    }
    pub fn text(&self) -> String {
        self.0.editor.lock().unwrap().text.clone()
    }
    pub fn take(&self) -> String {
        let mut editor = self.0.editor.lock().unwrap();
        let text = std::mem::take(&mut editor.text);
        editor.anchor = 0;
        editor.cursor = 0;
        editor.invalidate_paste();
        drop(editor);
        self.changed();
        text
    }
    fn changed(&self) {
        self.0.writer.publish(*self.0.revision.snapshot() + 1);
    }
    fn key(&self, key: &telorgon::input::KeyEvent) -> bool {
        let mut editor = self.0.editor.lock().unwrap();
        let action = editor.key(key);
        let changed = action == ClipboardEditAction::Changed;
        match action {
            ClipboardEditAction::Copy | ClipboardEditAction::Cut => {
                if let Some(text) = editor.selected_text().map(str::to_owned) {
                    let target = editor.target();
                    let value = self.clone();
                    crate::components::clipboard::copy(text, move |result| {
                        if result.is_ok() && action == ClipboardEditAction::Cut {
                            if value.0.editor.lock().unwrap().paste(target, "").is_ok() {
                                value.changed();
                            }
                        }
                    });
                }
            }
            ClipboardEditAction::Paste => {
                if !editor.read_only {
                    let target = editor.target();
                    let value = self.clone();
                    crate::components::clipboard::read(move |result| {
                        if let Ok(text) = result {
                            if value.0.editor.lock().unwrap().paste(target, &text).is_ok() {
                                value.changed();
                            }
                        }
                    });
                }
            }
            _ => {}
        }
        drop(editor);
        if changed {
            self.changed();
        }
        changed
    }
}

#[component(no_default)]
pub(crate) struct Entry {
    #[input]
    label: String,
    #[input]
    placeholder: String,
    #[input]
    value: EntryValue,
    #[input]
    enabled: bool,
    #[input]
    compact: bool,
    #[state]
    focused: bool,
}
impl Entry {
    pub fn new(label: &str, placeholder: &str, value: EntryValue, enabled: bool) -> Self {
        Self {
            label: label.into(),
            placeholder: placeholder.into(),
            value,
            enabled,
            compact: false,
            focused: false,
        }
    }
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }
    fn input(&mut self, event: &UiEvent) -> bool {
        match &event.kind {
            UiEventKind::Focus(focused) => {
                let changed = self.focused != *focused;
                self.focused = *focused;
                changed
            }
            UiEventKind::Input(telorgon::InputEvent::Key(key)) => self.value.key(key),
            _ => false,
        }
    }
}
impl Component for Entry {
    fn view(&self) -> impl View {
        self.watch(&self.value.0.revision);
        let editor = self.value.0.editor.lock().unwrap();
        let selection = editor.selection();
        let render = |value: &str| {
            if editor.secure {
                "•".repeat(value.chars().count())
            } else {
                value.to_owned()
            }
        };
        let mut content = row()
            .width(Dimension::FILL)
            .height(20.0)
            .align_items(Alignment::Center)
            .gap(0.0);
        if editor.text.is_empty() {
            content = content.child(text(&self.placeholder).size(13.0).color(MUTED));
        } else {
            content = content.child(
                text(render(&editor.text[..selection.start]))
                    .size(13.0)
                    .color(TEXT),
            );
            if self.focused && editor.cursor == selection.start {
                content = content.child(text("│").size(13.0).color(TEXT));
            }
            if !selection.is_empty() {
                content = content.child(
                    text(render(&editor.text[selection.clone()]))
                        .size(13.0)
                        .color(TEXT)
                        .background(if self.focused { ACCENT } else { CARD }),
                );
            }
            if self.focused && editor.cursor == selection.end && !selection.is_empty() {
                content = content.child(text("│").size(13.0).color(TEXT));
            }
            content = content.child(
                text(render(&editor.text[selection.end..]))
                    .size(13.0)
                    .color(TEXT),
            );
        }
        column()
            .width(Dimension::FILL)
            .height(if self.compact { 36.0 } else { 58.0 })
            .gap(if self.compact { 0.0 } else { 4.0 })
            .children(
                (!self.compact).then(|| text(&self.label).size(12.0).color(MUTED).height(18.0)),
            )
            .child(
                button()
                    .accessible_label(&self.label)
                    .width(Dimension::FILL)
                    .height(36.0)
                    .enabled(self.enabled)
                    .padding(8.0)
                    .corner_radius(5.0)
                    .background(CARD)
                    .uniform_border(
                        1.0,
                        if self.focused {
                            ACCENT
                        } else {
                            MUTED.with_alpha(60)
                        },
                    )
                    .overflow(telorgon::ui::Overflow::Clip)
                    .child(content)
                    .on_input(|this: &mut Self, event| this.input(event)),
            )
    }
}
