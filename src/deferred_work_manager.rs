use egui::Ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeferredWorkHandle(u64);

#[derive(Debug, Clone, Copy)]
struct DeferredWork {
    handle: DeferredWorkHandle,
    scheduled_frame: bool,
}

pub struct DeferredWorkManager {
    deferred_work: Vec<DeferredWork>,
    frame: bool,
    next_handle_id: u64,
}

impl DeferredWorkManager {
    pub fn new() -> Self {
        Self {
            deferred_work: Vec::new(),
            frame: false,
            next_handle_id: 0,
        }
    }

    pub fn after_repaint(&mut self, ui: &mut Ui) -> DeferredWorkHandle {
        let handle = DeferredWorkHandle(self.next_handle_id);
        self.next_handle_id = self.next_handle_id.wrapping_add(1);

        self.deferred_work.push(DeferredWork {
            handle,
            scheduled_frame: self.frame,
        });
        ui.ctx().request_repaint();

        handle
    }

    pub fn should_perform(&self, handle: DeferredWorkHandle) -> bool {
        self.deferred_work
            .iter()
            .any(|work| work.handle == handle && work.scheduled_frame != self.frame)
    }

    pub fn end_frame(&mut self) {
        self.deferred_work
            .retain(|work| work.scheduled_frame == self.frame);
        self.frame = !self.frame;
    }
}
