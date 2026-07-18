use egui::Ui;

const DEFAULT_GAP: f32 = 12.0;
const DEFAULT_MIN_FIELD_WIDTH: f32 = 120.0;

#[derive(Clone, Copy, Debug, Default)]
pub struct FieldPair;

impl FieldPair {
    pub const fn new() -> Self {
        Self
    }

    pub fn show<L, R>(
        self,
        ui: &mut Ui,
        add_left: impl FnOnce(&mut Ui) -> L,
        add_right: impl FnOnce(&mut Ui) -> R,
    ) -> (L, R) {
        if ui.available_width() < DEFAULT_MIN_FIELD_WIDTH * 2.0 + DEFAULT_GAP {
            return ui.vertical(|ui| (add_left(ui), add_right(ui))).inner;
        }

        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.x = DEFAULT_GAP;
            ui.columns_const(|[left_ui, right_ui]| (add_left(left_ui), add_right(right_ui)))
        })
        .inner
    }
}
