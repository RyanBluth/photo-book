use chrono::{DateTime, Datelike, NaiveDate, Timelike, Utc};

use crate::theme::color;

use super::{
    FileImportModal,
    graph::GraphAddStep,
    workflow::{
        ComparisonOperator, Condition, ConditionKind, FileWorkflow, MetadataField, MetadataValue,
        WorkflowStep,
    },
};

impl FileImportModal {
    pub(super) fn workflow_step_node_ui(ui: &mut egui::Ui, step: &mut WorkflowStep) {
        match step {
            WorkflowStep::Filter(condition) => {
                Self::node_section_label(ui, "Keep file when");
                Self::condition_ui(ui, condition);
            }
            WorkflowStep::AppendSubdirectory(template) => {
                Self::node_section_label(ui, "Directory");
                let mut output = egui::Grid::new("subdirectory_properties")
                    .num_columns(2)
                    .spacing(egui::vec2(12.0, 7.0))
                    .show(ui, |ui| {
                        Self::condition_property_label(ui, "Template");
                        let output = egui::TextEdit::singleline(&mut template.template)
                            .desired_width(ui.available_width())
                            .show(ui);
                        ui.end_row();
                        output
                    })
                    .inner;
                let cursor = output.cursor_range.map_or_else(
                    || template.template.chars().count(),
                    |range| range.primary.index.into(),
                );
                let completion = Self::template_completion(&template.template, cursor);
                let mut selected = None;

                ui.menu_button("+ Insert field", |ui| {
                    for field in Self::template_fields() {
                        if ui.button(*field).clicked() {
                            selected = Some(*field);
                            ui.close();
                        }
                    }
                });

                if let Some(field) = selected {
                    let (start, end) = completion
                        .as_ref()
                        .map_or((cursor, cursor), |(start, end, _)| (*start, *end));
                    let replacement = format!("{{{{{field}}}}}");
                    let mut characters = template.template.chars().collect::<Vec<_>>();
                    characters.splice(start..end, replacement.chars());
                    template.template = characters.into_iter().collect();
                    let new_cursor = start + replacement.chars().count();
                    output
                        .state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::one(
                            egui::text::CCursor::new(new_cursor),
                        )));
                    output.state.store(ui.ctx(), output.response.id);
                    output.response.request_focus();
                }
                ui.small("Example: {{capture_date:%Y-%m-%d}}")
                    .on_hover_text(
                        "Variables: {{extension}}, {{file_name}}, {{file_size}}, {{iso}}, \
                         {{capture_date}}, {{capture_date:%Y-%m-%d}}, {{modified_date}}",
                    );
            }
            WorkflowStep::Conditional {
                condition,
                then_workflow,
                else_workflow,
            } => {
                Self::node_section_label(ui, "When");
                Self::condition_ui(ui, condition);
                ui.add_space(3.0);
                ui.separator();
                ui.add_space(3.0);
                Self::graph_branch_ui(ui, "Then", then_workflow);

                let mut has_else = else_workflow.is_some();
                ui.add_space(2.0);
                if ui.checkbox(&mut has_else, "Add else branch").changed() {
                    *else_workflow = has_else.then(FileWorkflow::default);
                }
                if let Some(else_workflow) = else_workflow {
                    Self::graph_branch_ui(ui, "Else", else_workflow);
                }
            }
        }
    }

    fn node_section_label(ui: &mut egui::Ui, text: &str) {
        ui.label(
            egui::RichText::new(text.to_uppercase())
                .size(10.0)
                .strong()
                .color(color::SURFACE_EMPHASIS),
        );
    }

    fn template_fields() -> &'static [&'static str] {
        &[
            "extension",
            "file_name",
            "file_size",
            "iso",
            "capture_date",
            "capture_date:%Y-%m-%d",
            "modified_date",
            "modified_date:%Y-%m-%d",
        ]
    }

    pub(super) fn template_completion(
        template: &str,
        cursor: usize,
    ) -> Option<(usize, usize, String)> {
        let characters = template.chars().collect::<Vec<_>>();
        let cursor = cursor.min(characters.len());
        let start = (0..cursor.saturating_sub(1))
            .rev()
            .find(|index| characters[*index] == '{' && characters[*index + 1] == '{')?;
        let prefix_start = start + 2;
        if characters[prefix_start..cursor]
            .iter()
            .any(|character| matches!(character, '{' | '}'))
        {
            return None;
        }
        // Completing in the middle of an existing token replaces its remaining name too.
        let end = (cursor..characters.len())
            .find(|index| matches!(characters[*index], '{' | '}'))
            .filter(|index| characters[*index] == '}' && characters.get(*index + 1) == Some(&'}'))
            .map_or(cursor, |index| index + 2);
        Some((
            start,
            end,
            characters[prefix_start..cursor].iter().collect(),
        ))
    }

    fn graph_branch_ui(ui: &mut egui::Ui, label: &str, workflow: &mut FileWorkflow) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(label).strong());
            ui.weak(format!("{} steps", workflow.steps.len()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button("+ Add", |ui| {
                    for (label, step) in [
                        ("Filter", GraphAddStep::Filter),
                        ("Subdirectory", GraphAddStep::Subdirectory),
                        ("Conditional", GraphAddStep::Conditional),
                    ] {
                        if ui.button(label).clicked() {
                            workflow.steps.push(step.into_step());
                            ui.close();
                        }
                    }
                });
            });
        });
    }

    fn condition_ui(ui: &mut egui::Ui, condition: &mut Condition) {
        // Normalize drafts from older UI versions before showing the selected operator.
        if let Condition::Compare {
            field, operator, ..
        } = condition
            && !field.supported_operators().contains(operator)
        {
            *operator = ComparisonOperator::Equal;
        }
        let selected_field = match condition {
            Condition::Compare { field, .. } | Condition::Exists(field) => *field,
            _ => MetadataField::Extension,
        };
        let previous_kind = condition.kind();
        let mut kind = previous_kind;
        egui::Grid::new("condition_properties")
            .num_columns(2)
            .spacing(egui::vec2(12.0, 7.0))
            .show(ui, |ui| {
                Self::condition_property_label(ui, "Match");
                egui::ComboBox::from_id_salt("condition_kind")
                    .width(ui.available_width())
                    .selected_text(kind.label())
                    .show_ui(ui, |ui| {
                        for candidate in ConditionKind::ALL {
                            if let ConditionKind::Comparison(operator) = candidate
                                && !selected_field.supported_operators().contains(&operator)
                            {
                                continue;
                            }
                            ui.selectable_value(&mut kind, candidate, candidate.label());
                        }
                    });
                ui.end_row();

                if kind != previous_kind {
                    if let (Condition::Compare { operator, .. }, ConditionKind::Comparison(next)) =
                        (&mut *condition, kind)
                    {
                        *operator = next;
                    } else {
                        *condition = kind.default_condition();
                        if let Condition::Compare { field, value, .. } = condition {
                            *field = selected_field;
                            *value = field.default_value();
                        }
                    }
                }

                match condition {
                    Condition::Compare {
                        field,
                        operator,
                        value,
                    } => {
                        Self::condition_property_label(ui, "Field");
                        let previous_field = *field;
                        Self::metadata_field_ui(ui, field, "equals_field");
                        if *field != previous_field || !field.accepts(value) {
                            *value = field.default_value();
                        }
                        if !field.supported_operators().contains(operator) {
                            *operator = ComparisonOperator::Equal;
                        }
                        ui.end_row();

                        Self::condition_property_label(ui, "Value");
                        Self::metadata_value_ui(ui, value);
                        ui.end_row();
                    }
                    Condition::Exists(field) => {
                        Self::condition_property_label(ui, "Field");
                        Self::metadata_field_ui(ui, field, "exists_field");
                        ui.end_row();
                    }
                    Condition::And(_) | Condition::Or(_) | Condition::Not(_) => {}
                }
            });

        match condition {
            Condition::And(conditions) | Condition::Or(conditions) => {
                Self::condition_list_ui(ui, conditions);
            }
            Condition::Not(condition) => {
                ui.indent("not", |ui| Self::condition_ui(ui, condition));
            }
            Condition::Compare { .. } | Condition::Exists(_) => {}
        }
    }

    fn condition_property_label(ui: &mut egui::Ui, label: &str) {
        ui.allocate_ui_with_layout(
            egui::vec2(48.0, ui.spacing().interact_size.y),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.label(
                    egui::RichText::new(label)
                        .size(12.0)
                        .color(color::CONTROL_TEXT),
                );
            },
        );
    }

    fn condition_list_ui(ui: &mut egui::Ui, conditions: &mut Vec<Condition>) {
        let mut remove = None;
        for (index, condition) in conditions.iter_mut().enumerate() {
            ui.push_id(index, |ui| {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!("Condition {}", index + 1));
                        if ui.small_button("Remove").clicked() {
                            remove = Some(index);
                        }
                    });
                    Self::condition_ui(ui, condition);
                });
            });
        }
        if let Some(index) = remove {
            conditions.remove(index);
        }
        if ui.button("+ Condition").clicked() {
            conditions.push(Condition::default_equals());
        }
    }

    fn metadata_field_ui(ui: &mut egui::Ui, field: &mut MetadataField, id: &'static str) {
        egui::ComboBox::from_id_salt(id)
            .width(ui.available_width())
            .selected_text(field.label())
            .show_ui(ui, |ui| {
                for candidate in MetadataField::ALL {
                    ui.selectable_value(field, candidate, candidate.label());
                }
            });
    }

    fn metadata_value_ui(ui: &mut egui::Ui, value: &mut MetadataValue) {
        match value {
            MetadataValue::Text(value) => {
                ui.add(egui::TextEdit::singleline(value).desired_width(ui.available_width()));
            }
            MetadataValue::Integer(value) => {
                ui.add(egui::DragValue::new(value).range(0..=u64::MAX));
            }
            MetadataValue::DateTime(value) => Self::date_time_ui(ui, value),
        }
    }

    fn date_time_ui(ui: &mut egui::Ui, value: &mut DateTime<Utc>) {
        ui.vertical(|ui| {
            let Ok(mut date) = jiff::civil::Date::new(
                value.year().try_into().unwrap_or(i16::MAX),
                value.month() as i8,
                value.day() as i8,
            ) else {
                ui.colored_label(color::ERROR, "Date is outside the calendar range");
                return;
            };
            let picker_id = ui.make_persistent_id(Some("date"));
            let picker_layer = egui::LayerId::new(egui::Order::Foreground, picker_id);
            let was_open = ui
                .ctx()
                .memory(|memory| memory.areas().is_visible(&picker_layer));
            let date_response = ui.add(
                egui_extras::DatePickerButton::new(&mut date)
                    .id_salt("date")
                    .arrows(false)
                    .highlight_weekends(false)
                    .calendar_week(false),
            );
            let date_changed = date_response.changed();
            // Keep the calendar at normal screen size and anchored to the field even
            // when the graph is zoomed or panned. egui_extras uses local coordinates.
            if let Some(area) = egui::AreaState::load(ui.ctx(), picker_id) {
                let button_rect = ui
                    .ctx()
                    .layer_transform_to_global(ui.layer_id())
                    .map_or(date_response.rect, |transform| {
                        transform * date_response.rect
                    });
                let bounds = ui.ctx().content_rect().shrink(8.0);
                let calendar = area.rect();
                let mut position = button_rect.left_bottom() + egui::vec2(0.0, 4.0);
                if position.y + calendar.height() > bounds.bottom() {
                    position.y = button_rect.top() - calendar.height() - 4.0;
                }
                position.x = position.x.clamp(
                    bounds.left(),
                    (bounds.right() - calendar.width()).max(bounds.left()),
                );
                position.y = position.y.clamp(
                    bounds.top(),
                    (bounds.bottom() - calendar.height()).max(bounds.top()),
                );
                ui.ctx().set_transform_layer(
                    picker_layer,
                    egui::emath::TSTransform::from_translation(position - calendar.min),
                );
            }
            if was_open {
                // Escape dismisses the calendar before it can reach the surrounding modal.
                ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            }

            let (mut hour, mut minute, mut second) = (value.hour(), value.minute(), value.second());
            let mut time_changed = false;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                time_changed |= Self::time_component_ui(ui, &mut hour, 23, "Hours");
                ui.label(":");
                time_changed |= Self::time_component_ui(ui, &mut minute, 59, "Minutes");
                ui.label(":");
                time_changed |= Self::time_component_ui(ui, &mut second, 59, "Seconds");
                ui.weak("UTC");
            });
            if (date_changed || time_changed)
                && let Some(updated) = NaiveDate::from_ymd_opt(
                    date.year() as i32,
                    date.month() as u32,
                    date.day() as u32,
                )
                .and_then(|date| date.and_hms_opt(hour, minute, second))
            {
                *value = updated.and_utc();
            }
        });
    }

    fn time_component_ui(ui: &mut egui::Ui, value: &mut u32, max: u32, label: &str) -> bool {
        ui.add(
            egui::DragValue::new(value)
                .range(0..=max)
                .custom_formatter(|value, _| format!("{:02}", value as u32)),
        )
        .on_hover_text(label)
        .changed()
    }
}
