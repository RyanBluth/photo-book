use std::fmt::Display;

pub trait HistoricallyEqual {
    fn historically_equal_to(&self, other: &Self) -> bool;
}

#[derive(Debug, Clone, PartialEq)]
pub struct UndoRedoStack<Kind, Value> {
    pub initial_value: Value,
    pub history: Vec<(Kind, Value)>,
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
        if self.history.is_empty() {
            return self.initial_value.clone();
        }

        if self.index > 0 {
            self.index -= 1;
            let history = &self.history[self.index];
            history.1.clone()
        } else {
            self.history[self.index].1.clone()
        }
    }

    pub fn redo(&mut self) -> Value {
        if self.history.is_empty() {
            return self.initial_value.clone();
        }

        if self.index < self.history.len() - 1 {
            self.index += 1;
            let history = &self.history[self.index];
            history.1.clone()
        } else {
            self.history[self.index].1.clone()
        }
    }

    pub fn save_history(&mut self, kind: Kind, value: Value) {
        if self.history.is_empty() {
            if self.initial_value.historically_equal_to(&value) {
                return;
            }
            self.history.push((kind, value));
            self.index = 0;
            return;
        }

        if self.history[self.index].1.historically_equal_to(&value) {
            return;
        }

        self.history.truncate(self.index + 1);
        self.history.push((kind, value));

        self.index = self.history.len() - 1;
    }
}
