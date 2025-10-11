use std::sync::Mutex;

use once_cell::sync::Lazy;

pub type LayerId = usize;
pub type PageId = usize;
pub type ModalId = usize;
pub type QueryResultId = usize;

struct IdGenerator {
    next_id: LayerId,
}

static ID_GENERATOR: Lazy<Mutex<IdGenerator>> =
    Lazy::new(|| Mutex::new(IdGenerator { next_id: 0 }));

fn next_id() -> usize {
    let mut id_generator = ID_GENERATOR.lock().unwrap();
    let id = id_generator.next_id;
    if id == usize::MAX {
        id_generator.next_id = 0;
    } else {
        id_generator.next_id += 1;
    }
    id
}

pub fn next_layer_id() -> LayerId {
    next_id()
}

pub fn next_page_id() -> PageId {
    next_id()
}

pub fn next_modal_id() -> ModalId {
    next_id()
}

pub fn next_quick_layout_index() -> usize {
    next_id()
}

pub fn next_query_result_id() -> QueryResultId {
    next_id()
}

pub fn set_min_layer_id(id: LayerId) {
    let mut id_generator = ID_GENERATOR.lock().unwrap();
    id_generator.next_id = id_generator.next_id.max(id);
}
