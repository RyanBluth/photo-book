use std::sync::Mutex;

use once_cell::sync::Lazy;

pub type LayerId = usize;
pub type PageId = usize;

struct IdGenerator {
    next_id: LayerId,
}

fn next_id() -> usize {
    static ID_GENERATOR: Lazy<Mutex<IdGenerator>> =
        Lazy::new(|| Mutex::new(IdGenerator { next_id: 0 }));
    let mut id_generator = ID_GENERATOR.lock().unwrap();
    let id = id_generator.next_id;
    id_generator.next_id += 1;
    id
}

pub fn next_layer_id() -> LayerId {
    next_id()
}

pub fn next_page_id() -> PageId {
    next_id()
}
