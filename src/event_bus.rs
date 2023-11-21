use std::collections::HashMap;

use crate::photo::Photo;


#[derive(Hash, Eq, PartialEq, Debug)]
pub enum EventBusId {
    App,
    Gallery,
    GalleryPhoto(String)
}

#[derive(Clone)]
pub enum GalleryImageEvent {
    Selected(Photo)
}

pub struct EventBus<T> {
    listeners: HashMap<EventBusId, Box<dyn Fn(T)>>
}

impl<T: Clone> EventBus<T> {
    pub fn new() -> Self {
        Self {
            listeners: HashMap::new()
        }
    }

    pub fn listen(&mut self, id: EventBusId, listener: Box<dyn Fn(T)>) {
        self.listeners.insert(id, listener);
    }

    pub fn emit(&self, event: T) {
        for listener in self.listeners.values() {
            listener(event.clone());
        }
    }
}