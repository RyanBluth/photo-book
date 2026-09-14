use crate::{
    auto_persisting::AutoPersisting,
    config::{Config, ConfigModification},
    dep, dep_mut,
    project::{FileWorkflow, NamedWorkflow},
    session::Session,
};

use super::{FileImportModal, graph::WorkflowGraphState};

#[derive(Default, PartialEq, Clone, Copy)]
enum SaveLocation {
    #[default]
    Collection,
    AppPreferences,
}

impl SaveLocation {
    fn label(self) -> &'static str {
        match self {
            Self::Collection => "Collection",
            Self::AppPreferences => "App preferences",
        }
    }
}

struct WorkflowSelection {
    name: String,
    location: Option<SaveLocation>,
    workflow: FileWorkflow,
}

impl WorkflowSelection {
    fn label(&self) -> String {
        self.location.map_or_else(
            || "Direct import".into(),
            |location| format!("{} · {}", self.name, location.label()),
        )
    }
}

pub(super) struct SavedWorkflowState {
    name: String,
    location: SaveLocation,
    selected_label: String,
    loaded: FileWorkflow,
    pending: Option<WorkflowSelection>,
    message: Option<String>,
    pub(super) revision: u64,
}

impl Default for SavedWorkflowState {
    fn default() -> Self {
        Self {
            name: String::new(),
            location: SaveLocation::default(),
            selected_label: "Direct import".into(),
            loaded: FileWorkflow::default(),
            pending: None,
            message: None,
            revision: 0,
        }
    }
}

impl FileImportModal {
    fn save_named_workflow(&mut self, name: String) {
        let workflow = FileWorkflow::from(self.workflow.clone());
        let saved = NamedWorkflow {
            name: name.clone(),
            workflow: workflow.clone(),
        };
        let location = self.saved_workflows.location;
        let result = match location {
            SaveLocation::Collection => {
                dep_mut!(Session, |session| {
                    let workflows = &mut session.project_preferences.import_workflows;
                    if let Some(existing) = workflows.iter_mut().find(|entry| entry.name == name) {
                        *existing = saved;
                    } else {
                        workflows.push(saved);
                    }
                });
                Ok(())
            }
            SaveLocation::AppPreferences => dep_mut!(AutoPersisting<Config>, |config| {
                config.modify(ConfigModification::SaveImportWorkflow(saved))
            })
            .map_err(|error| error.to_string()),
        };
        match result {
            Ok(()) => {
                self.saved_workflows.loaded = workflow;
                self.saved_workflows.name = name.clone();
                self.saved_workflows.selected_label = format!("{name} · {}", location.label());
                self.saved_workflows.pending = None;
                self.saved_workflows.message = Some(
                    match location {
                        SaveLocation::Collection => {
                            "Workflow added to this collection. Save the collection to persist it."
                        }
                        SaveLocation::AppPreferences => "Workflow saved to app preferences.",
                    }
                    .into(),
                );
            }
            Err(error) => {
                self.saved_workflows.message = Some(format!("Could not save workflow: {error}"));
            }
        }
    }

    fn load_saved_workflow(&mut self, selection: WorkflowSelection) {
        self.workflow = selection.workflow.clone().into();
        self.graph = WorkflowGraphState::default();
        self.preview_expanded.clear();
        self.saved_workflows.selected_label = selection.label();
        self.saved_workflows.loaded = selection.workflow;
        self.saved_workflows.name = selection.name;
        if let Some(location) = selection.location {
            self.saved_workflows.location = location;
        }
        self.saved_workflows.pending = None;
        self.saved_workflows.message = None;
        self.saved_workflows.revision += 1;
    }

    pub(super) fn saved_workflows_ui(&mut self, ui: &mut egui::Ui) {
        // Release dependency locks before rendering or modifying either store.
        let collection = dep!(Session, |session| session
            .project_preferences
            .import_workflows
            .clone());
        let preferences = dep_mut!(AutoPersisting<Config>, |config| {
            config
                .read()
                .map(|config| config.import_workflows().to_vec())
                .map_err(|error| error.to_string())
        });
        let current = FileWorkflow::from(self.workflow.clone());
        let dirty = current != self.saved_workflows.loaded;
        let mut selected = None;
        ui.horizontal_wrapped(|ui| {
            ui.label("Workflow");
            let label = if dirty {
                format!("{} (modified)", self.saved_workflows.selected_label)
            } else {
                self.saved_workflows.selected_label.clone()
            };
            egui::ComboBox::from_id_salt("saved_import_workflow")
                .selected_text(label)
                .width(220.0)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(
                            !dirty && self.saved_workflows.selected_label == "Direct import",
                            "Direct import",
                        )
                        .clicked()
                    {
                        selected = Some(WorkflowSelection {
                            name: String::new(),
                            location: None,
                            workflow: FileWorkflow::default(),
                        });
                    }
                    for (scope, workflows) in [
                        (SaveLocation::Collection, collection.as_slice()),
                        (
                            SaveLocation::AppPreferences,
                            preferences.as_deref().unwrap_or(&[]),
                        ),
                    ] {
                        ui.separator();
                        ui.weak(scope.label());
                        for saved in workflows {
                            let label = format!("{} · {}", saved.name, scope.label());
                            if ui
                                .selectable_label(
                                    !dirty && self.saved_workflows.selected_label == label,
                                    &saved.name,
                                )
                                .clicked()
                            {
                                selected = Some(WorkflowSelection {
                                    name: saved.name.clone(),
                                    location: Some(scope),
                                    workflow: saved.workflow.clone(),
                                });
                            }
                        }
                    }
                });
        });
        if let Some(selection) = selected {
            if dirty {
                self.saved_workflows.pending = Some(selection);
            } else {
                self.load_saved_workflow(selection);
            }
        }
        if self.saved_workflows.pending.is_some() {
            ui.horizontal_wrapped(|ui| {
                ui.label("Replace the unsaved workflow edits?");
                if ui.button("Discard edits and load").clicked() {
                    let selection = self.saved_workflows.pending.take().unwrap();
                    self.load_saved_workflow(selection);
                }
                if ui.button("Keep editing").clicked() {
                    self.saved_workflows.pending = None;
                }
            });
        }
        ui.horizontal_wrapped(|ui| {
            ui.label("Name");
            ui.add(
                egui::TextEdit::singleline(&mut self.saved_workflows.name)
                    .hint_text("Workflow name")
                    .desired_width(200.0),
            );
            ui.label("Save to");
            egui::ComboBox::from_id_salt("workflow_save_location")
                .selected_text(self.saved_workflows.location.label())
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.saved_workflows.location,
                        SaveLocation::Collection,
                        "Collection",
                    );
                    ui.selectable_value(
                        &mut self.saved_workflows.location,
                        SaveLocation::AppPreferences,
                        "App preferences",
                    );
                });
            let name = self.saved_workflows.name.trim().to_owned();
            let workflows = match self.saved_workflows.location {
                SaveLocation::Collection => collection.as_slice(),
                SaveLocation::AppPreferences => preferences.as_deref().unwrap_or(&[]),
            };
            let replacing = workflows.iter().any(|saved| saved.name == name);
            let enabled = !name.is_empty()
                && (self.saved_workflows.location == SaveLocation::Collection
                    || preferences.is_ok());
            if ui
                .add_enabled(
                    enabled,
                    egui::Button::new(if replacing {
                        "Replace workflow"
                    } else {
                        "Save workflow"
                    }),
                )
                .clicked()
            {
                self.save_named_workflow(name);
            }
        });
        if let Err(error) = preferences {
            ui.colored_label(
                egui::Color32::LIGHT_RED,
                format!("Could not load app workflows: {error}"),
            );
        }
        if let Some(message) = &self.saved_workflows.message {
            ui.weak(message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{SubdirectoryTemplate, WorkflowStep};

    #[test]
    fn loading_workflow_preserves_folders_and_restores_save_location() {
        let mut modal = FileImportModal::new();
        modal.source_path = Some("/source".into());
        modal.destination_path = Some("/destination".into());
        let workflow = FileWorkflow {
            steps: vec![WorkflowStep::AppendSubdirectory(SubdirectoryTemplate {
                template: "raw/{{iso}}".into(),
            })],
        };
        modal.load_saved_workflow(WorkflowSelection {
            name: "RAW".into(),
            location: Some(SaveLocation::AppPreferences),
            workflow: workflow.clone(),
        });
        assert_eq!(FileWorkflow::from(modal.workflow.clone()), workflow);
        assert_eq!(modal.source_path, Some("/source".into()));
        assert_eq!(modal.destination_path, Some("/destination".into()));
        assert!(modal.saved_workflows.location == SaveLocation::AppPreferences);
        assert_eq!(modal.saved_workflows.revision, 1);
    }

    #[test]
    fn collection_save_replaces_only_the_matching_name() {
        let previous = dep_mut!(Session, |session| {
            std::mem::take(&mut session.project_preferences.import_workflows)
        });
        let mut modal = FileImportModal::new();
        modal.save_named_workflow("Direct copy".into());
        modal.save_named_workflow("RAW".into());
        modal
            .workflow
            .steps
            .push(super::super::workflow::WorkflowStep::AppendSubdirectory(
                super::super::workflow::SubdirectoryTemplate {
                    template: "raw".into(),
                },
            ));
        modal.save_named_workflow("RAW".into());
        let saved = dep_mut!(Session, |session| {
            std::mem::replace(&mut session.project_preferences.import_workflows, previous)
        });
        assert_eq!(saved.len(), 2);
        assert_eq!(saved[0].name, "Direct copy");
        assert!(saved[0].workflow.steps.is_empty());
        assert_eq!(saved[1].name, "RAW");
        assert_eq!(saved[1].workflow, FileWorkflow::from(modal.workflow));
    }

    #[test]
    fn selecting_default_requires_confirmation_before_discarding_edits() {
        use egui_kittest::{Harness, kittest::Queryable};
        let mut modal = FileImportModal::new();
        modal
            .workflow
            .steps
            .push(super::super::workflow::WorkflowStep::AppendSubdirectory(
                super::super::workflow::SubdirectoryTemplate {
                    template: "raw".into(),
                },
            ));
        let mut harness = Harness::new_ui_state(
            |ui, modal: &mut FileImportModal| modal.saved_workflows_ui(ui),
            modal,
        );
        harness.run();
        harness
            .get_all_by_role(egui::accesskit::Role::ComboBox)
            .next()
            .unwrap()
            .click();
        harness.run();
        harness.get_by_label("Direct import").click();
        harness.run();
        assert_eq!(harness.state().workflow.steps.len(), 1);
        harness.get_by_label("Discard edits and load").click();
        harness.run();
        assert!(harness.state().workflow.steps.is_empty());
        assert_eq!(
            harness.state().saved_workflows.selected_label,
            "Direct import"
        );
    }
}
