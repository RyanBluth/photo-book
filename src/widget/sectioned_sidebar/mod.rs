use egui::{CursorIcon, Frame, Id, Sense, Ui, UiBuilder, Vec2};

use crate::{
    cursor_manager::CursorManager,
    dep_mut,
    theme::color,
    widget::sectioned_sidebar::section::{
        section_header_height, CollapsableSection, CollapsableSectionResponse,
        CollapsableSectionState,
    },
};

pub mod section;

const RESIZE_HANDLE_HEIGHT: f32 = 8.0;
const RESIZE_LINE_HEIGHT: f32 = 1.0;
pub const MIN_EXPANDED_SECTION_HEIGHT: f32 = 100.0;
const AVAILABLE_HEIGHT_CHANGE_EPSILON: f32 = 0.5;

pub struct SectionedSidebarBuilder<'a> {
    id: Id,
    ui: Box<dyn FnMut(&mut Ui) + 'a>,
    sections: Vec<SectionMemoryData>,
    section_index: usize,
}

struct SectionHeaderAction<'a> {
    icon: String,
    tooltip: String,
    on_click: Box<dyn FnMut() + 'a>,
}

#[derive(Clone, PartialEq)]
struct SectionMemoryData {
    height: f32,
    expanded: bool,
    default_expanded_height: Option<f32>,
    desired_height_adjustment: f32,
    dragging: bool,
}

#[derive(Clone, Default)]
struct SectionedSidebarMemoryData {
    sections: Vec<SectionMemoryData>,
    last_available_height: Option<f32>,
}

impl<'a> SectionedSidebarBuilder<'a> {
    pub fn new(id: impl Into<Id>) -> Self {
        Self {
            id: id.into(),
            ui: Box::new(|_| {}),
            sections: Vec::new(),
            section_index: 0,
        }
    }

    pub fn section(
        self,
        ui: &mut Ui,
        state: &'a mut CollapsableSectionState,
        content: impl FnMut(&mut Ui) + 'a,
    ) -> Self {
        self.section_inner(ui, state, None, content)
    }

    pub fn section_with_action(
        self,
        ui: &mut Ui,
        state: &'a mut CollapsableSectionState,
        action_icon: impl Into<String>,
        action_tooltip: impl Into<String>,
        on_action: impl FnMut() + 'a,
        content: impl FnMut(&mut Ui) + 'a,
    ) -> Self {
        let header_action = SectionHeaderAction {
            icon: action_icon.into(),
            tooltip: action_tooltip.into(),
            on_click: Box::new(on_action),
        };
        self.section_inner(ui, state, Some(header_action), content)
    }

    fn section_inner(
        mut self,
        ui: &mut Ui,
        state: &'a mut CollapsableSectionState,
        header_action: Option<SectionHeaderAction<'a>>,
        content: impl FnMut(&mut Ui) + 'a,
    ) -> Self {
        self.sections = Self::read_sections(self.id, ui);
        let mut current = self.ui;
        let mut header_action = header_action;
        let mut content = content;

        let default_expanded_height = state.default_expanded_height();
        let section_data =
            self.sections
                .get(self.section_index)
                .cloned()
                .unwrap_or(SectionMemoryData {
                    height: default_expanded_height.unwrap_or(0.0),
                    expanded: false,
                    default_expanded_height,
                    desired_height_adjustment: 0.0,
                    dragging: false,
                });

        if self.section_index >= self.sections.len() {
            self.sections.push(SectionMemoryData {
                height: section_data.height,
                expanded: state.expanded,
                default_expanded_height,
                desired_height_adjustment: 0.0,
                dragging: false,
            });
        } else {
            self.sections[self.section_index] = SectionMemoryData {
                height: section_data.height,
                expanded: state.expanded,
                default_expanded_height,
                desired_height_adjustment: section_data.desired_height_adjustment,
                dragging: section_data.dragging,
            };
        }
        let is_any_previous_section_expanded = self
            .sections
            .iter()
            .take(self.section_index)
            .any(|s| s.expanded);
        let can_try_resize =
            self.section_index > 0 && state.expanded && is_any_previous_section_expanded;

        let id = self.id.clone();
        let section_index = self.section_index;
        let section_id = id.with(section_index);
        let section_header_height = Self::section_header_height(ui);
        Self::write_sections(self.id, self.sections.clone(), ui);

        let total_available_height = ui.available_height();

        self.ui = Box::new(move |ui: &mut Ui| {
            current(ui);

            ui.style_mut().spacing.item_spacing = egui::vec2(0.0, 0.0);

            let (section_rect, _section_response) = ui.allocate_exact_size(
                Vec2::new(
                    ui.available_width(),
                    section_data.height + section_header_height,
                ),
                Sense::click_and_drag(),
            );

            let mut section_ui = ui.new_child(
                UiBuilder::new()
                    .id(section_id)
                    .max_rect(section_rect)
                    .layout(*ui.layout()),
            );
            section_ui.set_clip_rect(section_rect);

            section_ui.vertical(|ui| {
                let mut sections: Vec<SectionMemoryData> = Self::read_sections(id, ui);

                if sections.len() <= section_index {
                    return;
                }

                if can_try_resize {
                    let (rect, response) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), RESIZE_HANDLE_HEIGHT),
                        Sense::click_and_drag(),
                    );
                    let line_rect = egui::Rect::from_center_size(
                        rect.center(),
                        Vec2::new(rect.width(), RESIZE_LINE_HEIGHT),
                    );
                    ui.painter()
                        .rect_filled(line_rect, 0.0, color::SURFACE_EMPHASIS);

                    if response.hovered() {
                        dep_mut!(CursorManager, |cursor_manager| {
                            cursor_manager.set_cursor(CursorIcon::ResizeRow);
                        });
                    }
                    let drag_delta: f32 = response.drag_delta().y;
                    if response.drag_stopped() {
                        sections[section_index].dragging = false;
                    }
                    if response.drag_started() {
                        sections[section_index].dragging = true;
                    }

                    Self::apply_resize(&mut sections, section_index, drag_delta);
                } else {
                    sections[section_index].dragging = false;
                }

                let header_action = header_action.as_mut().map(|action| {
                    (
                        action.icon.as_str(),
                        action.tooltip.as_str(),
                        action.on_click.as_mut() as &mut dyn FnMut(),
                    )
                });

                match CollapsableSection::new(state).show_with_action(ui, header_action, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt(section_id.with(("body-scroll", section_index)))
                        .max_height(section_data.height.max(0.0))
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            content(ui);
                        });
                }) {
                    CollapsableSectionResponse::Expanded => {
                        Self::compute_heights_on_expand(
                            &mut sections,
                            section_index,
                            total_available_height,
                            section_header_height,
                        );
                    }
                    CollapsableSectionResponse::Collapsed => {
                        Self::compute_heights_on_collapse(
                            &mut sections,
                            section_index,
                            total_available_height,
                            section_header_height,
                        );
                    }
                    CollapsableSectionResponse::None => {
                        // Do nothing
                    }
                }
                Self::write_sections(id, sections, ui);
            });
        });

        self.section_index += 1;

        self
    }

    pub fn show(mut self, ui: &mut Ui) {
        let available_height = ui.available_height();
        let section_header_height = Self::section_header_height(ui);
        let sections_changed = ui.memory_mut(|mem| {
            let mem_data: Option<SectionedSidebarMemoryData> = mem.data.get_persisted(self.id);
            let last_available_height = mem_data
                .as_ref()
                .and_then(|data| data.last_available_height);
            let prev_sections = mem_data
                .as_ref()
                .map(|data| data.sections.clone())
                .unwrap_or_default();
            let mut sections = self.sections.clone();

            Self::compute_heights_on_available_height_resize(
                &mut sections,
                last_available_height,
                available_height,
                section_header_height,
            );

            let sections_changed = prev_sections != sections;

            mem.data.insert_persisted(
                self.id,
                SectionedSidebarMemoryData {
                    sections,
                    last_available_height: Some(available_height),
                },
            );

            sections_changed
        });
        Frame::new().show(ui, |ui| {
            (self.ui)(ui);
        });
        if sections_changed {
            ui.request_repaint();
        }
    }

    fn apply_resize(sections: &mut [SectionMemoryData], section_index: usize, drag_delta: f32) {
        if drag_delta == 0.0 || section_index >= sections.len() || !sections[section_index].expanded
        {
            return;
        }

        let Some(previous_expanded_section_index) =
            Self::previous_expanded_section_index(sections, section_index)
        else {
            return;
        };

        if drag_delta > 0.0 {
            let sections_to_shrink = sections
                .iter()
                .enumerate()
                .skip(section_index)
                .filter(|(_, section)| section.expanded)
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let resized_by =
                Self::shrink_sections_in_order(sections, &sections_to_shrink, drag_delta.abs());
            sections[previous_expanded_section_index].height += resized_by;
        } else {
            let sections_to_shrink = sections
                .iter()
                .enumerate()
                .take(previous_expanded_section_index + 1)
                .rev()
                .filter(|(_, section)| section.expanded)
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let resized_by =
                Self::shrink_sections_in_order(sections, &sections_to_shrink, drag_delta.abs());
            sections[section_index].height += resized_by;
        }
    }

    fn shrink_sections_in_order(
        sections: &mut [SectionMemoryData],
        section_indexes: &[usize],
        mut shrink_amount: f32,
    ) -> f32 {
        let requested_shrink_amount = shrink_amount;

        for &section_index in section_indexes {
            if shrink_amount <= 0.0 {
                break;
            }

            let shrinkable_height =
                (sections[section_index].height - MIN_EXPANDED_SECTION_HEIGHT).max(0.0);
            let section_shrink_amount = shrink_amount.min(shrinkable_height);
            sections[section_index].height -= section_shrink_amount;
            shrink_amount -= section_shrink_amount;
        }

        requested_shrink_amount - shrink_amount
    }

    fn compute_heights_on_collapse(
        sections: &mut [SectionMemoryData],
        collapsing_section_index: usize,
        available_height: f32,
        section_header_height: f32,
    ) {
        if collapsing_section_index >= sections.len() {
            return;
        }

        sections[collapsing_section_index].expanded = false;
        sections[collapsing_section_index].height = 0.0;

        Self::reset_collapsed_section_heights(sections);

        let is_collapsed_tail = sections
            .iter()
            .skip(collapsing_section_index)
            .all(|section| !section.expanded);

        if !is_collapsed_tail {
            return;
        }

        let Some(section_to_expand_index) =
            Self::previous_expanded_section_index(sections, collapsing_section_index)
        else {
            return;
        };

        let available_body_height =
            Self::available_body_height(sections, available_height, section_header_height);
        let used_by_other_expanded_sections =
            Self::sum_expanded_heights_except(sections, section_to_expand_index);

        sections[section_to_expand_index].height =
            (available_body_height - used_by_other_expanded_sections).max(0.0);
    }

    fn compute_heights_on_expand(
        sections: &mut [SectionMemoryData],
        expanding_section_index: usize,
        available_height: f32,
        section_header_height: f32,
    ) {
        if expanding_section_index >= sections.len() {
            return;
        }

        sections[expanding_section_index].expanded = true;

        let available_body_height =
            Self::available_body_height(sections, available_height, section_header_height);
        let target_height = MIN_EXPANDED_SECTION_HEIGHT.min(available_body_height);

        Self::reset_collapsed_section_heights(sections);

        let used_by_other_expanded_sections =
            Self::sum_expanded_heights_except(sections, expanding_section_index);

        let unused_height = available_body_height - used_by_other_expanded_sections;
        if unused_height >= target_height {
            sections[expanding_section_index].height = unused_height;
            return;
        }

        sections[expanding_section_index].height = target_height;

        let mut shrink_needed = target_height - unused_height;
        let expanded_after = sections
            .iter()
            .enumerate()
            .skip(expanding_section_index + 1)
            .filter(|(_, section)| section.expanded)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let expanded_before = sections
            .iter()
            .enumerate()
            .take(expanding_section_index)
            .filter(|(_, section)| section.expanded)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();

        Self::scale_sections_down_proportionally(sections, &expanded_after, &mut shrink_needed);
        Self::scale_sections_down_proportionally(sections, &expanded_before, &mut shrink_needed);

        if shrink_needed > 0.0 {
            sections[expanding_section_index].height =
                (sections[expanding_section_index].height - shrink_needed).max(0.0);
        }
    }

    fn compute_heights_on_available_height_resize(
        sections: &mut [SectionMemoryData],
        previous_available_height: Option<f32>,
        available_height: f32,
        section_header_height: f32,
    ) {
        let expanded_sections = sections
            .iter()
            .enumerate()
            .filter(|(_, section)| section.expanded)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();

        Self::reset_collapsed_section_heights(sections);

        if expanded_sections.is_empty() {
            return;
        }

        let available_body_height =
            Self::available_body_height(sections, available_height, section_header_height);

        let Some(previous_available_height) = previous_available_height else {
            Self::compute_initial_expanded_heights(
                sections,
                &expanded_sections,
                available_body_height,
            );
            return;
        };

        if (available_height - previous_available_height).abs() < AVAILABLE_HEIGHT_CHANGE_EPSILON {
            return;
        }

        if available_body_height <= 0.0 {
            for section_index in expanded_sections {
                sections[section_index].height = 0.0;
            }
            return;
        }

        let min_total_height = MIN_EXPANDED_SECTION_HEIGHT * expanded_sections.len() as f32;

        if available_body_height < min_total_height {
            let current_total_height = expanded_sections
                .iter()
                .map(|&section_index| sections[section_index].height.max(0.0))
                .sum::<f32>();

            if current_total_height > 0.0 {
                let scale = available_body_height / current_total_height;
                for &section_index in &expanded_sections {
                    sections[section_index].height =
                        sections[section_index].height.max(0.0) * scale;
                }
            } else {
                let height_per_section = available_body_height / expanded_sections.len() as f32;
                for &section_index in &expanded_sections {
                    sections[section_index].height = height_per_section;
                }
            }
            return;
        }

        let current_extra_height = expanded_sections
            .iter()
            .map(|&section_index| {
                (sections[section_index].height - MIN_EXPANDED_SECTION_HEIGHT).max(0.0)
            })
            .sum::<f32>();
        let available_extra_height = available_body_height - min_total_height;

        if current_extra_height > 0.0 {
            for &section_index in &expanded_sections {
                let current_extra =
                    (sections[section_index].height - MIN_EXPANDED_SECTION_HEIGHT).max(0.0);
                sections[section_index].height = MIN_EXPANDED_SECTION_HEIGHT
                    + current_extra / current_extra_height * available_extra_height;
            }
        } else {
            let height_per_section = available_body_height / expanded_sections.len() as f32;
            for &section_index in &expanded_sections {
                sections[section_index].height = height_per_section;
            }
        }
    }

    fn compute_initial_expanded_heights(
        sections: &mut [SectionMemoryData],
        expanded_sections: &[usize],
        available_body_height: f32,
    ) {
        if available_body_height <= 0.0 {
            for &section_index in expanded_sections {
                sections[section_index].height = 0.0;
            }
            return;
        }

        let default_total_height = expanded_sections
            .iter()
            .filter_map(|&section_index| sections[section_index].default_expanded_height)
            .sum::<f32>();

        if default_total_height <= 0.0 {
            let height_per_section = available_body_height / expanded_sections.len() as f32;
            for &section_index in expanded_sections {
                sections[section_index].height = height_per_section;
            }
            return;
        }

        let flexible_sections = expanded_sections
            .iter()
            .copied()
            .filter(|&section_index| sections[section_index].default_expanded_height.is_none())
            .collect::<Vec<_>>();
        let desired_total_height =
            default_total_height + MIN_EXPANDED_SECTION_HEIGHT * flexible_sections.len() as f32;

        if desired_total_height > available_body_height {
            let scale = available_body_height / desired_total_height;
            for &section_index in expanded_sections {
                let desired_height = sections[section_index]
                    .default_expanded_height
                    .unwrap_or(MIN_EXPANDED_SECTION_HEIGHT);
                sections[section_index].height = desired_height * scale;
            }
            return;
        }

        for &section_index in expanded_sections {
            if let Some(default_height) = sections[section_index].default_expanded_height {
                sections[section_index].height = default_height;
            }
        }

        if flexible_sections.is_empty() {
            return;
        }

        let flexible_height =
            (available_body_height - default_total_height) / flexible_sections.len() as f32;
        for section_index in flexible_sections {
            sections[section_index].height = flexible_height;
        }
    }

    fn scale_sections_down_proportionally(
        sections: &mut [SectionMemoryData],
        section_indexes: &[usize],
        shrink_needed: &mut f32,
    ) {
        if *shrink_needed <= 0.0 || section_indexes.is_empty() {
            return;
        }

        let current_total = section_indexes
            .iter()
            .map(|&i| sections[i].height.max(0.0))
            .sum::<f32>();

        if current_total <= 0.0 {
            return;
        }

        let shrink_amount = (*shrink_needed).min(current_total);
        let new_total = current_total - shrink_amount;
        let scale = new_total / current_total;

        for &section_index in section_indexes {
            sections[section_index].height = sections[section_index].height.max(0.0) * scale;
        }

        *shrink_needed -= shrink_amount;
    }

    fn section_header_height(ui: &Ui) -> f32 {
        section_header_height(ui)
    }

    fn available_body_height(
        sections: &[SectionMemoryData],
        available_height: f32,
        section_header_height: f32,
    ) -> f32 {
        (available_height - section_header_height * sections.len() as f32).max(0.0)
    }

    fn reset_collapsed_section_heights(sections: &mut [SectionMemoryData]) {
        for section in sections.iter_mut().filter(|section| !section.expanded) {
            section.height = 0.0;
        }
    }

    fn previous_expanded_section_index(
        sections: &[SectionMemoryData],
        section_index: usize,
    ) -> Option<usize> {
        (0..section_index)
            .rev()
            .find(|&index| sections[index].expanded)
    }

    fn sum_expanded_heights_except(
        sections: &[SectionMemoryData],
        excluded_section_index: usize,
    ) -> f32 {
        sections
            .iter()
            .enumerate()
            .filter(|(index, section)| *index != excluded_section_index && section.expanded)
            .map(|(_, section)| section.height.max(0.0))
            .sum()
    }

    fn read_sections(id: Id, ui: &mut Ui) -> Vec<SectionMemoryData> {
        Self::read_memory(id, ui).sections
    }

    fn write_sections(id: Id, sections: Vec<SectionMemoryData>, ui: &mut Ui) {
        Self::update_memory(id, ui, |memory_data| {
            memory_data.sections = sections;
        });
    }

    fn read_memory(id: Id, ui: &mut Ui) -> SectionedSidebarMemoryData {
        ui.memory_mut(|mem| {
            mem.data
                .get_persisted::<SectionedSidebarMemoryData>(id)
                .unwrap_or_default()
        })
    }

    fn update_memory(id: Id, ui: &mut Ui, update: impl FnOnce(&mut SectionedSidebarMemoryData)) {
        ui.memory_mut(|mem| {
            let mut memory_data = mem
                .data
                .get_persisted::<SectionedSidebarMemoryData>(id)
                .unwrap_or_default();
            update(&mut memory_data);
            mem.data.insert_persisted(id, memory_data);
        });
    }
}
