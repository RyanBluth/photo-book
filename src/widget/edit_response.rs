use std::ops::{BitOr, BitOrAssign};

use egui::{Id, Popup, Response};

/// Describes a document edit produced by an immediate-mode control.
///
/// `active` is true only while the control that produced the edit still owns
/// the interaction. History can therefore coalesce repeated changes without
/// inferring ownership from unrelated global input.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[must_use = "edit responses must be recorded in history or intentionally discarded"]
pub struct EditResponse {
    pub changed: bool,
    pub active: bool,
    pub owner: Option<Id>,
}

impl EditResponse {
    pub const fn none() -> Self {
        Self {
            changed: false,
            active: false,
            owner: None,
        }
    }

    pub const fn discrete(changed: bool) -> Self {
        Self {
            changed,
            active: false,
            owner: None,
        }
    }

    pub fn drag(response: &Response) -> Self {
        let active = response.is_pointer_button_down_on() || response.dragged();
        Self {
            changed: response.changed(),
            active,
            owner: active.then_some(response.id),
        }
    }

    pub fn text(response: &Response, changed: bool) -> Self {
        let active = response.has_focus();
        Self {
            changed,
            active,
            owner: active.then_some(response.id),
        }
    }

    pub fn color_popup(response: &Response) -> Self {
        let active = Popup::is_id_open(&response.ctx, response.id.with("popup"));
        Self {
            changed: response.changed(),
            active,
            owner: active.then_some(response.id),
        }
    }
}

impl BitOr for EditResponse {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self {
            changed: self.changed || rhs.changed,
            active: self.active || rhs.active,
            owner: if rhs.active { rhs.owner } else { self.owner },
        }
    }
}

impl BitOrAssign for EditResponse {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}
