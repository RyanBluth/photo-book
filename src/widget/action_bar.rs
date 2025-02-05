use egui::{frame, Button, Color32, Frame, ImageSource, Stroke, Theme, Ui};
use font_kit::canvas::Canvas;

use crate::{assets::Asset, theme::color::ACTION_BAR, utils::EguiUiExt};

#[derive(Debug, Clone)]
pub enum ActionItemKind {
    Icon(ImageSource<'static>),
    Text(String),
    IconText {
        icon: ImageSource<'static>,
        text: String,
    },
}

#[derive(Debug, Clone)]
pub struct ActionItem<T> {
    pub kind: ActionItemKind,
    pub action: T,
}

pub struct ActionBar<T> {
    pub items: Vec<ActionItem<T>>,
}

pub enum ActionBarResponse<T: Clone> {
    None,
    Clicked(T),
}

impl<T: Clone> ActionBar<T> {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn with_items(items: Vec<ActionItem<T>>) -> Self {
        Self { items }
    }

    pub fn add_item(&mut self, item: ActionItem<T>) {
        self.items.push(item);
    }

    pub fn show(&mut self, ui: &mut Ui) -> ActionBarResponse<T> {
        Frame::canvas(ui.style())
            .inner_margin(10.0)
            .fill(ACTION_BAR)
            .rounding(8.0)
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    for item in &self.items {
                        ui.add_space(10.0);
                        match &item.kind {
                            ActionItemKind::Icon(icon) => {
                                if ui.add(Button::image(icon.clone())).clicked() {
                                    return ActionBarResponse::Clicked(item.action.clone());
                                }
                            }
                            ActionItemKind::Text(text) => {
                                if ui.add(Button::new(text)).clicked() {
                                    return ActionBarResponse::Clicked(item.action.clone());
                                }
                            }
                            ActionItemKind::IconText { icon, text } => {
                                if ui.add(Button::image_and_text(icon.clone(), text)).clicked() {
                                    return ActionBarResponse::Clicked(item.action.clone());
                                }
                            }
                        }
                    }
                    ui.add_space(10.0);

                    ActionBarResponse::None
                })
                .inner
            })
            .inner
    }
}
