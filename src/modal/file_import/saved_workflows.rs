use crate::{
    auto_persisting::AutoPersisting,
    config::{Config, ConfigModification},
    dep, dep_mut,
    project::{FileWorkflow, NamedWorkflow},
    session::Session,
};

use super::{FileImportModal, graph::WorkflowGraphState};

#[derive(Default, PartialEq, Clone, Copy)]
pub(super) enum SaveLocation {
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

pub(super) enum WorkflowSelection {
    Direct,
    New,
    Saved {
        name: String,
        location: SaveLocation,
        workflow: FileWorkflow,
    },
}

#[derive(Default, PartialEq, Clone, Copy)]
pub(super) enum WorkflowMode {
    #[default]
    Direct,
    New,
    Saved,
}

pub(super) struct SavedWorkflowState {
    name: String,
    location: SaveLocation,
    selected_label: String,
    loaded: FileWorkflow,
    pending: Option<WorkflowSelection>,
    message: Option<String>,
    pub(super) revision: u64,
    mode: WorkflowMode,
    graph_expanded: bool,
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
            mode: WorkflowMode::Direct,
            graph_expanded: false,
        }
    }
}

impl SavedWorkflowState {
    pub(super) fn graph_visible(&self) -> bool {
        self.mode != WorkflowMode::Direct && self.graph_expanded
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
                self.saved_workflows.mode = WorkflowMode::Saved;
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

    pub(super) fn load_saved_workflow(&mut self, selection: WorkflowSelection) {
        let selected_label = match &selection {
            WorkflowSelection::Direct => "Direct import".into(),
            WorkflowSelection::New => "New workflow".into(),
            WorkflowSelection::Saved { name, location, .. } => {
                format!("{name} · {}", location.label())
            }
        };
        let (mode, name, location, workflow) = match selection {
            WorkflowSelection::Direct => (
                WorkflowMode::Direct,
                String::new(),
                None,
                FileWorkflow::default(),
            ),
            WorkflowSelection::New => (
                WorkflowMode::New,
                String::new(),
                None,
                FileWorkflow::default(),
            ),
            WorkflowSelection::Saved {
                name,
                location,
                workflow,
            } => (WorkflowMode::Saved, name, Some(location), workflow),
        };
        self.workflow = workflow.clone().into();
        self.graph = WorkflowGraphState::default();
        self.preview_expanded.clear();
        self.saved_workflows.mode = mode;
        self.saved_workflows.graph_expanded = mode == WorkflowMode::New;
        self.saved_workflows.selected_label = selected_label;
        self.saved_workflows.loaded = workflow;
        self.saved_workflows.name = name;
        if let Some(location) = location {
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
        let dirty = self.saved_workflows.mode != WorkflowMode::Direct
            && current != self.saved_workflows.loaded;
        let mut selected = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().interact_size.y = 28.0;
            ui.add_sized(
                [64.0, 28.0],
                egui::Label::new(egui::RichText::new("Workflow").strong()),
            );
            let label = if dirty && self.saved_workflows.mode == WorkflowMode::Saved {
                format!("{} (modified)", self.saved_workflows.selected_label)
            } else {
                self.saved_workflows.selected_label.clone()
            };
            egui::ComboBox::from_id_salt("saved_import_workflow")
                .selected_text(label)
                .width(260.0)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(
                            self.saved_workflows.mode == WorkflowMode::Direct,
                            "Direct import",
                        )
                        .clicked()
                    {
                        selected = Some(WorkflowSelection::Direct);
                    }
                    if ui
                        .selectable_label(
                            self.saved_workflows.mode == WorkflowMode::New,
                            "New workflow…",
                        )
                        .clicked()
                    {
                        selected = Some(WorkflowSelection::New);
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
                                selected = Some(WorkflowSelection::Saved {
                                    name: saved.name.clone(),
                                    location: scope,
                                    workflow: saved.workflow.clone(),
                                });
                            }
                        }
                    }
                });
            if self.saved_workflows.mode != WorkflowMode::Direct {
                ui.horizontal(|ui| {
                    let label = if self.saved_workflows.graph_expanded {
                        "Hide workflow editor"
                    } else {
                        "Edit workflow"
                    };
                    if ui.button(label).clicked() {
                        self.saved_workflows.graph_expanded = !self.saved_workflows.graph_expanded;
                    }
                });
            }
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
                if ui.button("Discard edits and load").clicked()
                    && let Some(selection) = self.saved_workflows.pending.take()
                {
                    self.load_saved_workflow(selection);
                }
                if ui.button("Keep editing").clicked() {
                    self.saved_workflows.pending = None;
                }
            });
        }
        if self.saved_workflows.mode == WorkflowMode::Direct {
            ui.weak("Import files directly into the destination folder.");
        } else if self.saved_workflows.mode == WorkflowMode::New
            || self.saved_workflows.graph_expanded
            || dirty
        {
            self.workflow_save_controls_ui(
                ui,
                &collection,
                preferences.as_deref().unwrap_or(&[]),
                preferences.is_ok(),
            );
        }
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

    fn workflow_save_controls_ui(
        &mut self,
        ui: &mut egui::Ui,
        collection: &[NamedWorkflow],
        preferences: &[NamedWorkflow],
        preferences_available: bool,
    ) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().interact_size.y = 28.0;
            ui.horizontal(|ui| {
                ui.add_sized([64.0, 28.0], egui::Label::new("Name"));
                ui.add_sized(
                    [260.0, 28.0],
                    egui::TextEdit::singleline(&mut self.saved_workflows.name)
                        .hint_text("Workflow name"),
                );
            });
            ui.label("Save to");
            egui::ComboBox::from_id_salt("workflow_save_location")
                .width(160.0)
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
                SaveLocation::Collection => collection,
                SaveLocation::AppPreferences => preferences,
            };
            let replacing = workflows.iter().any(|saved| saved.name == name);
            let enabled = !name.is_empty()
                && (self.saved_workflows.location == SaveLocation::Collection
                    || preferences_available);
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
        modal.load_saved_workflow(WorkflowSelection::Saved {
            name: "RAW".into(),
            location: SaveLocation::AppPreferences,
            workflow: workflow.clone(),
        });
        assert_eq!(FileWorkflow::from(modal.workflow.clone()), workflow);
        assert_eq!(modal.source_path, Some("/source".into()));
        assert_eq!(modal.destination_path, Some("/destination".into()));
        assert!(modal.saved_workflows.location == SaveLocation::AppPreferences);
        assert_eq!(modal.saved_workflows.revision, 1);
        assert!(modal.saved_workflows.mode == WorkflowMode::Saved);
        assert!(!modal.saved_workflows.graph_expanded);
    }

    #[test]
    fn saved_workflow_editor_can_be_expanded_without_changing_the_workflow() {
        use egui_kittest::{Harness, kittest::Queryable};
        let mut modal = FileImportModal::new();
        let workflow = FileWorkflow {
            steps: vec![WorkflowStep::AppendSubdirectory(SubdirectoryTemplate {
                template: "photos".into(),
            })],
        };
        modal.load_saved_workflow(WorkflowSelection::Saved {
            name: "Photos".into(),
            location: SaveLocation::Collection,
            workflow: workflow.clone(),
        });
        let mut harness = Harness::new_ui_state(
            |ui, modal: &mut FileImportModal| modal.saved_workflows_ui(ui),
            modal,
        );
        harness.run();
        assert!(!harness.state().saved_workflows.graph_expanded);
        harness.get_by_label("Edit workflow").click();
        harness.run();
        assert!(harness.state().saved_workflows.graph_expanded);
        harness.get_by_label("Hide workflow editor").click();
        harness.run();
        assert!(!harness.state().saved_workflows.graph_expanded);
        assert_eq!(
            FileWorkflow::from(harness.state().workflow.clone()),
            workflow
        );
        harness
            .state_mut()
            .load_saved_workflow(WorkflowSelection::Direct);
        harness.run();
        assert!(harness.state().workflow.steps.is_empty());
        assert!(harness.query_by_label("Edit workflow").is_none());
        assert!(harness.query_by_label("Save to").is_none());
        assert!(harness.query_by_label("Direct import (modified)").is_none());
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
        modal.load_saved_workflow(WorkflowSelection::New);
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
