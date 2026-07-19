use egui::{Color32, RichText, Ui};

use crate::{
    model::{editable_value::EditableValue, hex_color::HexColor},
    theme::color,
    utils::EditableValueTextEdit,
    widget::field_pair::FieldPair,
};

const SECTION_SPACING: f32 = 12.0;
const FIELD_SPACING: f32 = 5.0;

/// Applies consistent vertical spacing to a group of canvas properties.
pub fn property_group<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = SECTION_SPACING;
        add_contents(ui)
    })
    .inner
}

/// Shows a label above its control so controls can use the full available width.
pub fn field<R>(ui: &mut Ui, label: &str, add_control: impl FnOnce(&mut Ui) -> R) -> R {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = FIELD_SPACING;
        ui.label(
            RichText::new(label)
                .small()
                .strong()
                .color(color::CONTROL_TEXT),
        );
        add_control(ui)
    })
    .inner
}

/// Lays out related fields in two responsive columns.
pub fn field_pair<L, R>(
    ui: &mut Ui,
    add_left: impl FnOnce(&mut Ui) -> L,
    add_right: impl FnOnce(&mut Ui) -> R,
) -> (L, R) {
    FieldPair::new().show(ui, add_left, add_right)
}

/// Shows the native color picker with an editable hexadecimal value beside it.
pub fn color_field(ui: &mut Ui, value: &mut Color32, edit_state: &mut EditableValue<HexColor>) {
    edit_state.update_if_not_active(HexColor(*value));

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if ui.color_edit_button_srgba(value).changed() {
            edit_state.update_if_not_active(HexColor(*value));
        }

        ui.style_mut().spacing.text_edit_width = 116.0_f32.min(ui.available_width());
        *value = ui.text_edit_editable_value_singleline(edit_state).0;
    });
}
