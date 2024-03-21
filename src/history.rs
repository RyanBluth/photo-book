use std::fmt::Display;


pub trait HistoricallyEqual {
    fn historically_eqaul_to(&self, other: &Self) -> bool;
}

#[derive(Debug, Clone, PartialEq)]
pub struct UndoRedoStack<Kind, Value> {
    pub history: Vec<(Kind, Value)>,
    pub index: usize,
}

impl<Kind, Value> UndoRedoStack<Kind, Value>
where
    Kind: Display,
    Value: Clone,
    Value: HistoricallyEqual,
{
    pub fn undo(&mut self) -> Value {
        if self.index > 0 {
            self.index -= 1;
            let history = &self.history[self.index];
            history.1.clone()
        } else {
            self.history[self.index].1.clone()
        }
    }

    pub fn redo(&mut self) -> Value {
        if self.index < self.history.len() - 1 {
            self.index += 1;
            let history = &self.history[self.index];
            history.1.clone()
        } else {
            self.history[self.index].1.clone()
        }
    }

    pub fn save_history(&mut self, kind: Kind, value: Value) {
        self.history.truncate(self.index + 1);
        self.history.push((kind, value));

        self.index = self.history.len() - 1;
    }
}