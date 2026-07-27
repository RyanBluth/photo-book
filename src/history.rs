use std::fmt::Display;

pub trait HistoricallyEqual {
    fn historically_equal_to(&self, other: &Self) -> bool;
}

#[derive(Debug, Clone, PartialEq)]
pub struct UndoRedoStack<Kind, Value> {
    pub initial_value: Value,
    pub history: Vec<(Kind, Value)>,
    /// The number of history entries currently applied. A value of zero points
    /// at `initial_value`; `history.len()` points at the latest entry.
    pub index: usize,
}

impl<Kind, Value> UndoRedoStack<Kind, Value>
where
    Kind: Display,
    Value: Clone,
    Value: HistoricallyEqual,
{
    pub fn new(initial_value: Value) -> Self {
        Self {
            initial_value,
            history: vec![],
            index: 0,
        }
    }

    pub fn undo(&mut self) -> Value {
        if self.index > 0 {
            self.index -= 1;
        }

        if self.index == 0 {
            self.initial_value.clone()
        } else {
            self.history[self.index - 1].1.clone()
        }
    }

    pub fn redo(&mut self) -> Value {
        if self.index < self.history.len() {
            let value = self.history[self.index].1.clone();
            self.index += 1;
            value
        } else if self.index == 0 {
            self.initial_value.clone()
        } else {
            self.history[self.index - 1].1.clone()
        }
    }

    pub fn save_history(&mut self, kind: Kind, value: Value) {
        let current_value = if self.index == 0 {
            &self.initial_value
        } else {
            &self.history[self.index - 1].1
        };

        if current_value.historically_equal_to(&value) {
            return;
        }

        self.history.truncate(self.index);
        self.history.push((kind, value));
        self.index = self.history.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    struct Value(i32);

    impl HistoricallyEqual for Value {
        fn historically_equal_to(&self, other: &Self) -> bool {
            self == other
        }
    }

    #[test]
    fn first_change_can_be_undone_and_redone() {
        let mut stack = UndoRedoStack::new(Value(0));
        stack.save_history("change", Value(1));

        assert_eq!(stack.undo(), Value(0));
        assert_eq!(stack.redo(), Value(1));
    }

    #[test]
    fn saving_after_undo_discards_redo_entries() {
        let mut stack = UndoRedoStack::new(Value(0));
        stack.save_history("one", Value(1));
        stack.save_history("two", Value(2));
        assert_eq!(stack.undo(), Value(1));

        stack.save_history("replacement", Value(3));

        assert_eq!(stack.history.len(), 2);
        assert_eq!(stack.redo(), Value(3));
        assert_eq!(stack.undo(), Value(1));
    }
}
