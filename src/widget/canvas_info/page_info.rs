use eframe::egui::{self};
use egui::{ComboBox, RichText};
use strum::IntoEnumIterator;

use crate::{
    model::{
        edit_state::EditablePage,
        unit::{PageSizePreset, Unit},
    },
    utils::EditableValueTextEdit,
};

#[derive(Debug, PartialEq)]
pub struct PageInfoState<'a> {
    page: &'a mut EditablePage,
}

impl PageInfoState<'_> {
    pub fn new(page: &mut EditablePage) -> PageInfoState<'_> {
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

                ComboBox::from_label("Preset Size")
                    .selected_text("Select size preset...")
                    .show_ui(ui, |ui| {
                        for preset in PageSizePreset::iter() {
                            if ui.selectable_label(false, preset.to_string()).clicked() {
                                if let Some((width, height)) = preset.dimensions() {
                                    page.set_size(width, height);
                                    page.edit_state.width.begin_editing();
                                    page.edit_state.height.begin_editing();

                                    *page.edit_state.width.editable_value() = width.to_string();
                                    *page.edit_state.height.editable_value() = height.to_string();

                                    page.edit_state.width.end_editing();
                                    page.edit_state.height.end_editing();
                                }
                            }
                        }
                    });
            });

            ui.separator();

            ui.horizontal(|ui| {
                let page = &mut self.state.page;

                ui.label("Width:");

                let new_width = ui.text_edit_editable_value_singleline(&mut page.edit_state.width);
                let height = page.size().y;
                page.set_size(new_width, height);

                ui.label("Height:");

                let new_height =
                    ui.text_edit_editable_value_singleline(&mut page.edit_state.height);
                let width = page.size().x;
                page.set_size(width, new_height);
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

            ui.horizontal(|ui| {
                let page = &mut self.state.page;
                ui.label("PPI:");
                let new_ppi = ui.text_edit_editable_value_singleline(&mut page.edit_state.ppi);
                page.set_ppi(new_ppi);
            });
            ui.separator();
        });
    }
}
