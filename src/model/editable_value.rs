use std::{fmt::Display, str::FromStr};

#[derive(Debug, Clone, PartialEq)]
pub struct EditableValue<T> {
    value: T,
    editable_value: String,
    editing: bool,
}

impl<T> EditableValue<T>
where
    T: Display,
    T: FromStr,
    T: Clone,
{
    pub fn new(value: T) -> Self {
        let editable_value = value.to_string();
        Self {
            value,
            editable_value,
            editing: false,
        }
    }

    pub fn update_if_not_active(&mut self, value: T) {
        if !self.editing {
            self.value = value;
            self.editable_value = self.value.to_string();
        }
    }

    pub fn editable_value(&mut self) -> &mut String {
        &mut self.editable_value
    }

    pub fn begin_editing(&mut self) {
        self.editing = true;
    }

    pub fn end_editing(&mut self) {
        self.update_from_editable();
        self.editable_value = self.value.to_string();
        self.editing = false;
    }

    /// Applies the current edit buffer when it contains a valid value without
    /// ending the edit or rewriting the user's text.
    pub(crate) fn update_from_editable(&mut self) -> Option<T> {
        let value = self.editable_value.parse().ok()?;
        self.value = value;
        Some(self.value.clone())
    }

    pub fn is_editing(&self) -> bool {
        self.editing
    }

    pub fn value(&self) -> T {
        self.value.clone()
    }
}

impl<T> Display for EditableValue<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.editable_value)
    }
}

#[cfg(test)]
mod tests {
    use super::EditableValue;

    #[test]
    fn invalid_edit_reverts_to_the_current_value() {
        let mut editable = EditableValue::new(42.0_f32);
        editable.begin_editing();
        *editable.editable_value() = "not a number".to_string();
        editable.end_editing();

        assert_eq!(editable.value(), 42.0);
        assert_eq!(editable.to_string(), "42");
    }
}
