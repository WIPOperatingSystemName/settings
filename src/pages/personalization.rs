use crate::{
    assets,
    components::controls::{control, group, page_content},
    controllers::{BackgroundTileState, PersonalizationController},
    theme::{ACCENT, LINE, MUTED, PANEL, TEXT},
};
use telorgon::{app::*, graphics::render::ImageResource};

const TILE_GAP: f32 = 14.0;

#[component(no_default)]
pub struct PersonalizationPage {
    #[input]
    controller: PersonalizationController,
}
impl PersonalizationPage {
    pub fn new(controller: PersonalizationController) -> Self {
        Self { controller }
    }

    fn tile(&self, tile: &BackgroundTileState, width: f32) -> Container {
        let photo_height = (width * 9.0 / 16.0).ceil();
        let settings = tile.settings.clone();
        let selection = button()
            .accessible_label(match &tile.error {
                Some(error) => format!("{}: {error}", tile.label),
                None => format!("Use {} as wallpaper", tile.label),
            })
            .width(width)
            .height(photo_height)
            .enabled(tile.can_select)
            .background(PANEL)
            .uniform_border(2.0, if tile.selected { ACCENT } else { LINE })
            .corner_radius(8.0)
            .padding(2.0)
            .child(preview(
                tile.image.as_ref(),
                tile.is_default,
                &tile.label,
                width - 8.0,
                photo_height - 8.0,
            ))
            .on_press(move |this: &mut Self| this.controller.select(settings.clone()));
        let mut photo = stack().width(width).height(photo_height).child(selection);
        if !tile.is_default && !tile.in_use {
            let settings = tile.settings.clone();
            // Removal is a separate target above the photo, including for keyboard input.
            photo = photo.child(
                column()
                    .width(Dimension::FILL)
                    .height(Dimension::FILL)
                    .padding(7.0)
                    .child(
                        row().height(24.0).child(spacer()).child(
                            button()
                                .accessible_label(format!("Remove {}", tile.label))
                                .width(24.0)
                                .height(24.0)
                                .enabled(tile.can_delete)
                                .background(ColorRgba8::rgba(24, 24, 26, 220))
                                .corner_radius(12.0)
                                .child(text("×").size(19.0).color(TEXT))
                                .on_press(move |this: &mut Self| {
                                    this.controller.delete(settings.clone())
                                }),
                        ),
                    ),
            );
        }
        column()
            .width(width)
            .height(photo_height + 28.0)
            .gap(6.0)
            .child(photo)
            .child(
                text(if tile.error.is_some() {
                    "Photo unavailable"
                } else {
                    &tile.label
                })
                .size(12.0)
                .color(if tile.selected { TEXT } else { MUTED })
                .width(Dimension::FILL)
                .height(22.0)
                .overflow(Overflow::Clip),
            )
    }
}
impl Component for PersonalizationPage {
    fn view(&self) -> impl View {
        let state = self.controller.state(self);
        let width = crate::settings_app::content_width(self.viewport_size().width);
        let narrow = width < 410.0;
        let preview_width = (width - 32.0).clamp(68.0, 220.0);
        let preview_height = (preview_width * 9.0 / 16.0).ceil();
        let current_height = if narrow {
            preview_height + 140.0
        } else {
            preview_height + 32.0
        };
        let description = column()
            .width(Dimension::FILL)
            .height(92.0)
            .gap(8.0)
            .child(text("Wallpaper").size(13.0).color(MUTED).height(18.0))
            .child(
                text(&state.source_name)
                    .size(16.0)
                    .weight(600)
                    .color(TEXT)
                    .width(Dimension::FILL)
                    .height(40.0)
                    .overflow(Overflow::Clip),
            )
            .child(text("Fill Screen").size(12.0).color(MUTED).height(18.0));
        let current = group(current_height).padding(16.0).child(
            (if narrow { column() } else { row() })
                .width(Dimension::FILL)
                .height(Dimension::FILL)
                .align_items(Alignment::Center)
                .gap(16.0)
                .child(preview(
                    state.image.as_ref(),
                    state.using_default,
                    &state.source_name,
                    preview_width,
                    preview_height,
                ))
                .child(description),
        );

        let target_columns = ((width + TILE_GAP) / (154.0 + TILE_GAP)).floor().max(1.0);
        let tile_width = ((width - TILE_GAP * (target_columns - 1.0)) / target_columns)
            .floor()
            .clamp(80.0, 190.0) as u16;
        let tile_height = (f32::from(tile_width) * 9.0 / 16.0).ceil() as u16 + 28;
        let columns = ((width + TILE_GAP) / (f32::from(tile_width) + TILE_GAP))
            .floor()
            .max(1.0) as usize;
        let photos: Vec<_> = state
            .backgrounds
            .iter()
            .filter(|tile| !tile.is_default)
            .collect();
        let photo_header_height = if width < 280.0 { 64.0 } else { 36.0 };
        let rows = photos.len().div_ceil(columns);
        let photos_height = if photos.is_empty() {
            54.0
        } else {
            rows as f32 * f32::from(tile_height) + rows.saturating_sub(1) as f32 * TILE_GAP
        };
        let mut gallery = column()
            .width(Dimension::FILL)
            .grid(tile_width, tile_height)
            .gap(TILE_GAP);
        for tile in &photos {
            gallery = gallery.child(self.tile(tile, f32::from(tile_width)));
        }
        let feedback = state.error.as_deref().or(state.library_error.as_deref());
        let feedback_height = feedback.map_or(0.0, |message| wrapped_height(message, width));
        let mut content = page_content(
            30.0 + current_height
                + 24.0
                + f32::from(tile_height)
                + photo_header_height
                + photos_height
                + 5.0 * 20.0
                + if feedback.is_some() {
                    feedback_height + 20.0
                } else {
                    0.0
                },
        )
        .gap(20.0)
        .child(
            text("Personalization")
                .size(24.0)
                .weight(600)
                .color(TEXT)
                .height(30.0),
        )
        .child(current);
        if let Some(message) = feedback {
            content = content.child(
                text(message)
                    .size(13.0)
                    .color(TEXT)
                    .width(Dimension::FILL)
                    .height(feedback_height),
            );
        }
        content = content.child(
            text("Default Wallpaper")
                .size(14.0)
                .weight(600)
                .color(TEXT)
                .height(24.0),
        );
        if let Some(default) = state.backgrounds.iter().find(|tile| tile.is_default) {
            content = content.child(self.tile(default, f32::from(tile_width)));
        }
        content = content.child(
            (if width < 280.0 { column() } else { row() })
                .width(Dimension::FILL)
                .height(photo_header_height)
                .gap(8.0)
                .align_items(Alignment::Center)
                .child(
                    text("Your Photos")
                        .size(14.0)
                        .weight(600)
                        .color(TEXT)
                        .width(Dimension::FILL)
                        .height(24.0),
                )
                .child(
                    control(if state.choosing {
                        "Opening…"
                    } else if state.loading {
                        "Working…"
                    } else {
                        "+ Add Photo…"
                    })
                    .accessible_label("Add Photo")
                    .width(132.0)
                    .height(32.0)
                    .enabled(state.enabled)
                    .on_press(|this: &mut Self| this.controller.browse()),
                ),
        );
        if photos.is_empty() {
            content.child(
                text("Add a photo to use as your wallpaper.")
                    .size(13.0)
                    .color(MUTED)
                    .width(Dimension::FILL)
                    .height(54.0),
            )
        } else {
            content.child(gallery)
        }
    }
}

fn wrapped_height(message: &str, width: f32) -> f32 {
    let lines = (message.chars().count() as f32 * 6.5 / width.max(80.0)).ceil();
    (lines.max(message.lines().count() as f32).max(1.0) + 1.0) * 18.0
}

fn preview(
    resource: Option<&ImageResource>,
    using_default: bool,
    name: &str,
    width: f32,
    height: f32,
) -> Container {
    let source = if let Some(resource) = resource {
        let ratio = resource.extent.width.max(1) as f32 / resource.extent.height.max(1) as f32;
        Some((Image::resource(resource.clone()), ratio))
    } else if using_default {
        Some((image(assets::images::DEFAULT_BACKGROUND), 985.0 / 554.0))
    } else {
        None
    };
    let mut frame = stack()
        .width(width)
        .height(height)
        .background(PANEL)
        .corner_radius(5.0)
        .overflow(Overflow::Clip)
        .center_content();
    if let Some((image, ratio)) = source {
        let draw_width = width.max(height * ratio);
        let draw_height = draw_width / ratio;
        frame = frame.child(
            image
                .accessible_label(format!("{name} wallpaper preview"))
                .box_style(BoxStyle {
                    width: SizeRule::Logical(draw_width),
                    height: SizeRule::Logical(draw_height),
                    max_size: SizeRule2D {
                        width: SizeRule::Logical(f32::MAX),
                        height: SizeRule::Logical(f32::MAX),
                    },
                    transform: Transform2D {
                        translation: PointF {
                            x: (width - draw_width) * 0.5,
                            y: (height - draw_height) * 0.5,
                        },
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .without_tint(),
        );
    } else {
        frame = frame.child(text("Photo unavailable").size(12.0).color(MUTED));
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::{BackgroundTile, Model, PersonalizationPreview, Status};
    use telorgon::services::desktop_settings::PersonalizationSettings;
    use telorgon::{application_host::ComposedAppRuntime, graphics::render::ImageResourceDelta};

    #[component(no_default)]
    struct Host {
        #[input]
        controller: PersonalizationController,
    }
    impl Component for Host {
        fn view(&self) -> impl View {
            row()
                .height(Dimension::FILL)
                .width(Dimension::FILL)
                .gap(1.0)
                .child(column().width(crate::settings_app::sidebar_width(self.viewport_size().width)))
                .child(
                    column()
                        .padding(crate::settings_app::content_padding(self.viewport_size().width))
                        .width(Dimension::FILL)
                        .height(Dimension::FILL)
                        .child(
                            column()
                                .height(Dimension::FILL)
                                .width(Dimension::FILL)
                                .scrollable()
                                .child(PersonalizationPage::new(self.controller.clone())),
                        ),
                )
        }
    }

    fn at(ms: u64) -> telorgon::MonotonicInstant {
        telorgon::MonotonicInstant::from_nanos(ms * 1_000_000)
    }

    fn fixture(height: i32) -> (ComposedAppRuntime, SignalWriter<Status>, Status) {
        let status = Status {
            shell: None,
            message: String::new(),
            busy: false,
            preview: None,
            load_error: false,
            personalization: PersonalizationPreview::default(),
        };
        let (signal, writer) = Signal::new(status.clone());
        let controller = PersonalizationController::from_model(Model::from_signal(signal));
        let mut runtime = ComposedAppRuntime::from_composed_with_extent(
            Host { controller },
            SizeI {
                width: 1100,
                height,
            },
        )
        .unwrap();
        runtime.prepare_frame(at(0), false).unwrap();
        (runtime, writer, status)
    }

    fn photo() -> BackgroundTile {
        BackgroundTile {
            settings: PersonalizationSettings {
                background: Some(format!("background-{}.png", "a".repeat(64))),
            },
            label: "Added photo.webp".into(),
            image: Some(ImageResource {
                image: telorgon::ImageId(0x7000_0000),
                content_version: 1,
                extent: SizeI {
                    width: 16,
                    height: 9,
                },
                color_encoding: Default::default(),
                alpha_mode: Default::default(),
                pixel_format: Default::default(),
                pixels: std::sync::Arc::from([34, 80, 120, 255].repeat(16 * 9)),
            }),
            error: None,
        }
    }

    fn photo_node(runtime: &ComposedAppRuntime) -> telorgon::NodeId {
        runtime
            .ui()
            .semantics
            .iter()
            .find_map(|(node, semantic)| match semantic.name {
                telorgon::SemanticName::Text(name)
                    if runtime.ui().string(name) == Some("Use Added photo.webp as wallpaper") =>
                {
                    Some(node)
                }
                _ => None,
            })
            .expect("the imported photo must become a selectable tile")
    }

    #[test]
    fn published_photo_is_mounted_visible_and_bound_without_reopening_the_page() {
        let (mut runtime, writer, mut status) = fixture(720);
        assert!(runtime.ui().texts.iter().any(|(_, visual)| {
            runtime.ui().string(visual.content) == Some("Add a photo to use as your wallpaper.")
        }));
        let photo = photo();
        status.personalization.backgrounds.push(photo.clone());
        writer.publish(status.clone());
        runtime.prepare_frame(at(1), false).unwrap();
        let tile = photo_node(&runtime);
        assert!(runtime.ui().interactions.get(tile).unwrap().enabled);
        let bounds = runtime.layout().computed(tile).unwrap();
        assert!(
            bounds.visible_rect.area() > 0.0,
            "photo tile was clipped: {bounds:?}"
        );
        assert!(
            runtime
                .scene_snapshot()
                .image_resources
                .iter()
                .any(|delta| {
                    matches!(delta, ImageResourceDelta::Write(update)
                if Some(update.image) == photo.image.as_ref().map(|image| image.image))
                })
        );

        status.personalization.desired = photo.settings.clone();
        status.personalization.saved = photo.settings;
        status.personalization.image = photo.image;
        status.personalization.source_name = photo.label;
        status.personalization.error = Some("The desktop could not apply the wallpaper.".into());
        writer.publish(status);
        runtime.prepare_frame(at(2), false).unwrap();
        let tile = photo_node(&runtime);
        assert!(runtime.ui().interactions.get(tile).unwrap().enabled);
        assert!(runtime.layout().computed(tile).unwrap().visible_rect.area() > 0.0);
    }

    #[test]
    fn imported_photo_is_reachable_by_scrolling_in_a_short_window() {
        let (mut runtime, writer, mut status) = fixture(480);
        status.personalization.backgrounds.push(photo());
        writer.publish(status);
        runtime.prepare_frame(at(1), false).unwrap();
        let tile = photo_node(&runtime);
        assert_eq!(
            runtime.layout().computed(tile).unwrap().visible_rect.area(),
            0.0
        );
        runtime.queue_input(telorgon::InputEvent::mouse_moved(PointF {
            x: 500.0,
            y: 400.0,
        }));
        runtime.flush_input(at(2));
        runtime.queue_input(telorgon::InputEvent::mouse_scroll(PointF {
            x: 0.0,
            y: -1000.0,
        }));
        runtime.flush_input(at(3));
        runtime.prepare_frame(at(4), false).unwrap();
        assert!(runtime.layout().computed(tile).unwrap().visible_rect.area() > 0.0);
    }
}
