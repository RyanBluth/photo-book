use eframe::egui::{self};
use egui::{ComboBox, RichText, Vec2};

use strum::IntoEnumIterator;

use crate::{
    model::{edit_state::EditablePage, unit::Unit},
    utils::EditableValueTextEdit,
};

#[derive(Debug, PartialEq)]
pub struct PageInfoState<'a> {
    page: &'a mut EditablePage,
}

impl PageInfoState<'_> {
    pub fn new(page: &mut EditablePage) -> PageInfoState {
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

    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.state.page.update();

        ui.vertical(|ui| {
            ui.style_mut().spacing.text_edit_width = 80.0;

            ui.label(RichText::new("Document Info").heading());

            ui.horizontal(|ui| {
                let page = &mut self.state.page;

                ui.label("Width:");

                let new_width = ui.text_edit_editable_value_singleline(&mut page.edit_state.width);
                let height = page.size().y;
                page.set_size(Vec2::new(new_width, height));

                ui.label("Height:");

                let new_height =
                    ui.text_edit_editable_value_singleline(&mut page.edit_state.height);
                let width = page.size().x;
                page.set_size(Vec2::new(width, new_height));
            });

            ui.separator();

            ui.horizontal(|ui| {
                let page = &mut self.state.page;

                ui.label("Unit:");

                let mut page_unit = page.unit();

                ComboBox::from_label("Units")
                    .selected_text(format!("{}", page.unit()))
                    .show_ui(ui, |ui| {
                        for unit in Unit::iter() {
                            ui.selectable_value(&mut page_unit, unit, unit.to_string());
                        }
                    });

                page.set_unit(page_unit);
            });
            ui.separator();
        });
    }
}
