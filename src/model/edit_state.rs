use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};

use super::{
    editable_value::EditableValue,
    page::{Page, PageEditState},
};

#[derive(Debug, PartialEq, Clone)]
pub struct Editable<T, EditState>
where
    T: Clone + PartialEq + Debug,
    EditState: Clone + PartialEq + Debug,
{
    pub value: T,
    pub edit_state: EditState,
}

pub type EditablePage = Editable<Page, PageEditState>;

impl Editable<Page, PageEditState> {
    pub fn new(value: Page) -> Self {
        let edit_state = PageEditState {
            width: EditableValue::new(value.size().x),
            height: EditableValue::new(value.size().y),
            ppi: EditableValue::new(value.ppi()),
            unit: EditableValue::new(value.unit()),
        };
        Self { value, edit_state }
    }

    pub fn update(&mut self) {
        self.edit_state
            .width
            .update_if_not_active(self.value.size().x);
        self.edit_state
            .height
            .update_if_not_active(self.value.size().y);
        self.edit_state.ppi.update_if_not_active(self.value.ppi());
        self.edit_state.unit.update_if_not_active(self.value.unit());
    }
}

impl Deref for Editable<Page, PageEditState> {
    type Target = Page;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl DerefMut for Editable<Page, PageEditState> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}
