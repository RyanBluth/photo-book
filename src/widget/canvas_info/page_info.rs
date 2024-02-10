use eframe::egui::{self, Response};
use egui::Vec2;
use indexmap::IndexMap;

use crate::{utils::EditableValueTextEdit, widget::page_canvas::Page};

use super::{
    layers::{Layer, LayerId, Layers},
    transform_control::{TransformControl, TransformControlState},
};

#[derive(Debug, PartialEq)]
pub struct PageInfoState<'a> {
    page: &'a mut Page,
}

impl PageInfoState<'_> {
    pub fn new(page: &mut Page) -> PageInfoState {
        PageInfoState { page }
    }
}

#[derive(Debug, PartialEq)]
pub struct PageInfo<'a> {
    pub state: &'a mut PageInfoState<'a>,
}

impl<'a> PageInfo<'a> {
    pub fn new(state: &'a mut PageInfoState<'a>) -> PageInfo<'a> {
        PageInfo { state }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> () {
        ui.vertical(|ui| {
            ui.label("Document Info");

            ui.horizontal(|ui| {
                ui.label("Width:");

                let page = &mut self.state.page;
                
                let new_width = ui.text_edit_editable_value_singleline(&mut page.edit_state.width);
                page.set_size(Vec2::new(new_width, page.size().y));

                ui.label("height:");

                let new_height = ui.text_edit_editable_value_singleline(&mut page.edit_state.height);
                page.set_size(Vec2::new(page.size().x, new_height));
            });
        });
    }
}
