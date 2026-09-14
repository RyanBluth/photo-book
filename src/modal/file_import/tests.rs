use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use super::preview_tree::PreviewTree;
use crate::theme::style;

use super::{graph::*, graph_model::*, preview::*, workflow::*, *};

fn extension_condition(extension: &str) -> Condition {
    Condition::Compare {
        field: MetadataField::Extension,
        operator: ComparisonOperator::Equal,
        value: MetadataValue::Text(extension.to_string()),
    }
}

fn subdirectory(template: &str) -> WorkflowStep {
    WorkflowStep::AppendSubdirectory(SubdirectoryTemplate {
        template: template.to_string(),
    })
}

#[test]
fn graph_represents_linear_workflow() {
    let workflow = FileWorkflow {
        steps: vec![
            WorkflowStep::Filter(extension_condition("jpg")),
            subdirectory("photos"),
        ],
    };

    let graph = WorkflowGraph::new(&workflow);

    assert_eq!(graph.nodes.len(), 4);
    assert_eq!(graph.edges.len(), 3);
    assert_eq!(
        graph
            .nodes
            .iter()
            .map(|node| node.title.as_str())
            .collect::<Vec<_>>(),
        ["Input", "Filter", "Subdirectory", "Import"]
    );
}

#[test]
fn disabled_graph_selection_preserves_navigation_and_controls() {
    use egui_kittest::{Harness, kittest::Queryable};
    struct State {
        view: egui_graph::View,
        enabled: bool,
        seed_selection: bool,
        edge_selected: bool,
        node_selected: bool,
        control_clicks: usize,
        node_layer: Option<egui::LayerId>,
    }
    let first = egui_graph::NodeId::new("first");
    let second = egui_graph::NodeId::new("second");
    let mut harness = Harness::new_ui_state(
        move |ui, state: &mut State| {
            let mut graph = egui_graph::Graph::new("selection_test")
                .selection_enabled(state.enabled)
                .snap(None);
            if state.seed_selection {
                graph = graph.selected_nodes([first].into_iter().collect());
                state.seed_selection = false;
            }
            graph.show(&mut state.view, ui, |ui, show| {
                show.nodes(ui, |nodes, ui| {
                    egui_graph::node::Node::from_id(first)
                        .outputs(1)
                        .animation_time(0.0)
                        .show(nodes, ui, |node| {
                            state.node_selected = node.interaction().selected;
                            node.framed(|ui, _| {
                                state.node_layer = Some(ui.layer_id());
                                if ui.button("Inline control").clicked() {
                                    state.control_clicks += 1;
                                }
                            })
                        });
                    egui_graph::node::Node::from_id(second)
                        .inputs(1)
                        .animation_time(0.0)
                        .show(nodes, ui, |node| {
                            node.framed(|ui, _| {
                                ui.label("End");
                            })
                        });
                })
                .edges(ui, |edges, ui| {
                    egui_graph::edge::Edge::new((first, 0), (second, 0), &mut state.edge_selected)
                        .show(edges, ui);
                });
            });
        },
        State {
            view: egui_graph::View {
                scene_rect: egui::Rect::from_min_size(
                    egui::pos2(-100.0, -100.0),
                    egui::vec2(800.0, 600.0),
                ),
                layout: [
                    (first, egui::pos2(0.0, 0.0)),
                    (second, egui::pos2(400.0, 0.0)),
                ]
                .into_iter()
                .collect(),
            },
            enabled: true,
            seed_selection: true,
            edge_selected: true,
            node_selected: false,
            control_clicks: 0,
            node_layer: None,
        },
    );
    harness.run();
    assert!(harness.state().node_selected);
    harness.state_mut().enabled = false;
    harness.run();
    assert!(!harness.state().node_selected);
    assert!(!harness.state().edge_selected);
    let layout = harness.state().view.layout.clone();
    let node_layer = harness.state().node_layer.unwrap();
    let transform = harness.ctx.layer_transform_to_global(node_layer).unwrap();
    let control = transform * harness.get_by_label("Inline control").rect().center();
    harness.hover_at(control);
    harness.drag_at(control);
    harness.drop_at(control);
    harness.run();
    assert_eq!(harness.state().control_clicks, 1);
    assert!(!harness.state().node_selected);
    let edge =
        egui_graph::with_graph_memory(&harness.ctx, egui_graph::id("selection_test"), |memory| {
            memory.node_sockets()[&first]
                .output(0)
                .unwrap()
                .0
                .lerp(memory.node_sockets()[&second].input(0).unwrap().0, 0.5)
        });
    let edge = transform * edge;
    harness.hover_at(edge);
    harness.drag_at(edge);
    harness.drop_at(edge);
    harness.run();
    assert!(!harness.state().edge_selected);
    let blank = egui::pos2(400.0, 350.0);
    harness.hover_at(blank);
    harness.drag_at(blank);
    harness.hover_at(egui::pos2(50.0, 50.0));
    harness.run();
    assert!(!harness.state().node_selected);
    harness.drop_at(egui::pos2(50.0, 50.0));
    harness.run();
    assert_eq!(harness.state().view.layout, layout);
    let before_pan = harness.state().view.scene_rect;
    harness.hover_at(blank);
    harness.event(egui::Event::PointerButton {
        pos: blank,
        button: egui::PointerButton::Middle,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.hover_at(blank + egui::vec2(70.0, 40.0));
    harness.event(egui::Event::PointerButton {
        pos: blank + egui::vec2(70.0, 40.0),
        button: egui::PointerButton::Middle,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run();
    assert_ne!(
        harness.state().view.scene_rect.center(),
        before_pan.center()
    );
    let before_zoom = harness.state().view.scene_rect;
    harness.hover_at(blank);
    harness.event(egui::Event::Zoom(1.25));
    harness.run();
    assert!(harness.state().view.scene_rect.width() < before_zoom.width());
    assert_eq!(harness.state().view.layout, layout);
    assert!(!harness.state().node_selected && !harness.state().edge_selected);
}

#[test]
fn conditional_without_else_keeps_only_then_edge_after_background_clicks() {
    use egui_kittest::{Harness, kittest::Queryable};
    let mut modal = FileImportModal::new();
    modal.load_saved_workflow(saved_workflows::WorkflowSelection::New);
    modal
        .workflow
        .steps
        .push(GraphAddStep::Conditional.into_step());
    let graph = WorkflowGraph::new(&modal.workflow);
    let conditional = graph
        .nodes
        .iter()
        .find(|node| node.title == "Conditional")
        .unwrap();
    let branches = graph
        .edges
        .iter()
        .filter(|edge| edge.from == conditional.id)
        .collect::<Vec<_>>();
    assert_eq!(conditional.outputs, 1);
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].output, 0);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1440.0, 900.0))
        .with_max_steps(12)
        .build_ui(move |ui| {
            egui::Modal::new(egui::Id::new("edge_test_modal")).show(ui.ctx(), |ui| {
                modal.body_ui(ui);
            });
        });
    style::apply(&harness.ctx);
    harness.run();
    harness.run_steps(5);
    if std::env::var("PHOTOBOOK_IMPORT_REVIEW_SNAPSHOT").is_ok() {
        harness
            .render()
            .unwrap()
            .save("/tmp/photobook-conditional-selection.png")
            .unwrap();
    }
    for _ in 0..3 {
        let buttons = harness
            .get_all_by_label("+")
            .map(|node| node.rect().center())
            .collect::<Vec<_>>();
        assert_eq!(buttons.len(), 2);
        let start = egui_graph::NodeId::new("file_import/start");
        let start_layer = egui::LayerId::new(
            egui::Order::Foreground,
            egui_graph::node::egui_id(egui_graph::id("file_import_workflow_graph"), start),
        );
        let socket = egui_graph::with_graph_memory(
            &harness.ctx,
            egui_graph::id("file_import_workflow_graph"),
            |memory| memory.node_sockets()[&start].output(0).unwrap().0,
        );
        let socket = harness.ctx.layer_transform_to_global(start_layer).unwrap() * socket;
        assert!(
            (buttons[0].y - socket.y).abs() <= 1.0,
            "plus must be centered on the line"
        );
        for position in &buttons {
            let layer = harness.ctx.layer_id_at(*position).expect("button layer");
            let area = egui::AreaState::load(&harness.ctx, layer.id).unwrap();
            assert!(area.rect().width() < 40.0, "modal covers an edge button");
        }
        let blank = egui::pos2(buttons[0].x, buttons[0].y - 90.0);
        harness.hover_at(blank);
        harness.drag_at(blank);
        harness.drop_at(blank);
        harness.run();
    }
    if std::env::var("PHOTOBOOK_IMPORT_REVIEW_SNAPSHOT").is_ok() {
        harness
            .render()
            .unwrap()
            .save("/tmp/photobook-empty-conditional.png")
            .unwrap();
    }
    harness.get_all_by_label("+").last().unwrap().click();
    harness.run();
    assert!(harness.query_by_label("Insert node").is_some());
}

#[test]
fn graph_rejoins_conditional_branches() {
    let workflow = FileWorkflow {
        steps: vec![
            WorkflowStep::Conditional {
                condition: extension_condition("raw"),
                then_workflow: FileWorkflow {
                    steps: vec![subdirectory("raw")],
                },
                else_workflow: Some(FileWorkflow {
                    steps: vec![subdirectory("processed")],
                }),
            },
            subdirectory("{{capture_date}}"),
        ],
    };

    let graph = WorkflowGraph::new(&workflow);
    let conditional = graph
        .nodes
        .iter()
        .find(|node| node.title == "Conditional")
        .unwrap();
    let joined = graph
        .nodes
        .iter()
        .find(|node| node.detail == "{{capture_date}}")
        .unwrap();

    assert_eq!(conditional.outputs, 2);
    assert_eq!(joined.inputs, 2);
    assert_eq!(graph.nodes.len(), 6);
    assert_eq!(graph.edges.len(), 6);
}

#[test]
fn graph_signature_changes_when_structure_changes() {
    let mut workflow = FileWorkflow::default();
    let empty_signature = WorkflowGraph::new(&workflow).signature();

    workflow.steps.push(subdirectory("photos"));
    let populated_signature = WorkflowGraph::new(&workflow).signature();

    assert_ne!(empty_signature, populated_signature);
}

#[test]
fn graph_connection_inserts_into_conditional_branch() {
    let mut workflow = FileWorkflow {
        steps: vec![WorkflowStep::Conditional {
            condition: extension_condition("raw"),
            then_workflow: FileWorkflow::default(),
            else_workflow: Some(FileWorkflow::default()),
        }],
    };
    let graph = WorkflowGraph::new(&workflow);
    let insertion = graph
        .edges
        .iter()
        .find(|edge| edge.output == 1)
        .unwrap()
        .insertion
        .clone();

    FileImportModal::insert_graph_step_at(
        &mut workflow,
        &insertion,
        GraphAddStep::Subdirectory.into_step(),
    );

    let WorkflowStep::Conditional { else_workflow, .. } = &workflow.steps[0] else {
        panic!("expected conditional");
    };
    assert!(matches!(
        else_workflow.as_ref().unwrap().steps[0],
        WorkflowStep::AppendSubdirectory(_)
    ));
}

#[test]
fn graph_remove_deletes_selected_nested_node() {
    let mut workflow = FileWorkflow {
        steps: vec![WorkflowStep::Conditional {
            condition: extension_condition("raw"),
            then_workflow: FileWorkflow {
                steps: vec![subdirectory("raw")],
            },
            else_workflow: None,
        }],
    };
    let address = WorkflowStepAddress(vec![
        WorkflowAddressSegment::Step(0),
        WorkflowAddressSegment::Then,
        WorkflowAddressSegment::Step(0),
    ]);

    FileImportModal::remove_workflow_step(&mut workflow, &address);

    let WorkflowStep::Conditional { then_workflow, .. } = &workflow.steps[0] else {
        panic!("expected conditional");
    };
    assert!(then_workflow.steps.is_empty());
}

#[test]
fn graph_scroll_zoom_drag_pan_and_insert_cursor() {
    use crate::{cursor_manager::CursorManager, dep, dep_mut};
    use egui_kittest::{Harness, kittest::Queryable};

    let mut harness = Harness::builder()
        .with_size(egui::vec2(1440.0, 900.0))
        .build_ui_state(
            |ui, modal: &mut FileImportModal| {
                dep_mut!(CursorManager, |manager| manager.begin_frame(ui.ctx()));
                FileImportModal::workflow_graph_ui(ui, &mut modal.workflow, &mut modal.graph);
                dep!(CursorManager, |manager| manager.end_frame(ui.ctx()));
            },
            FileImportModal::new(),
        );
    harness.run();
    let blank = egui::pos2(700.0, 450.0);
    harness.hover_at(blank);
    harness.run();
    assert_eq!(
        harness.output().platform_output.cursor_icon,
        egui::CursorIcon::Grab
    );

    let before = harness.state().graph.view.scene_rect;
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -40.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run();
    assert!(harness.state().graph.view.scene_rect.width() > before.width());
    let before = harness.state().graph.view.scene_rect;
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, 20.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run();
    assert!(harness.state().graph.view.scene_rect.width() < before.width());

    let before = harness.state().graph.view.scene_rect;
    let layout = harness.state().graph.view.layout.clone();
    let end = blank + egui::vec2(50.0, 25.0);
    harness.drag_at(blank);
    harness.hover_at(end);
    harness.run();
    assert_eq!(
        harness.output().platform_output.cursor_icon,
        egui::CursorIcon::Grabbing
    );
    harness.drop_at(end);
    harness.run();
    assert_ne!(
        harness.state().graph.view.scene_rect.center(),
        before.center()
    );
    assert_eq!(harness.state().graph.view.layout, layout);

    let plus = harness.get_by_label("+").rect().center();
    harness.hover_at(plus);
    harness.run();
    assert_eq!(
        harness.output().platform_output.cursor_icon,
        egui::CursorIcon::PointingHand
    );
    let before = harness.state().graph.view.scene_rect;
    harness.drag_at(plus);
    harness.drop_at(plus);
    harness.run();
    assert!(harness.query_by_label("Insert node").is_some());
    assert_eq!(harness.state().graph.view.scene_rect, before);
    harness.get_all_by_label("Filter").last().unwrap().click();
    harness.run();
    assert_eq!(harness.state().workflow.steps.len(), 1);
}

#[test]
fn graph_edge_insert_menu_is_clickable() {
    use egui_kittest::{Harness, kittest::Queryable};

    let mut modal = FileImportModal::new();
    modal.workflow = FileWorkflow {
        steps: vec![
            WorkflowStep::Filter(extension_condition("jpg")),
            WorkflowStep::Conditional {
                condition: extension_condition("raw"),
                then_workflow: FileWorkflow {
                    steps: vec![subdirectory("raw")],
                },
                else_workflow: Some(FileWorkflow {
                    steps: vec![subdirectory("processed")],
                }),
            },
            subdirectory("{{capture_date:%Y-%m-%d}}"),
        ],
    };

    let mut harness = Harness::builder()
        .with_size(egui::vec2(1440.0, 900.0))
        .build_ui(move |ui| {
            FileImportModal::workflow_graph_ui(ui, &mut modal.workflow, &mut modal.graph);
        });
    style::apply(&harness.ctx);
    harness.run();
    let plus_position = harness
        .get_all_by_label("+")
        .next()
        .expect("expected an edge insertion button")
        .rect()
        .center();
    harness.drag_at(plus_position);
    harness.event(egui::Event::PointerButton {
        pos: plus_position,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run_steps(2);
    assert!(harness.query_by_label("Insert node").is_some());
}

#[test]
fn preview_tree_preserves_single_child_directories() {
    let mut tree = PreviewTree::default();
    tree.insert(Path::new("2026-08-17/raw/DSC_1003.nef"));
    let items = tree.into_items(PathBuf::from("Destination"));

    assert_eq!(
        items
            .iter()
            .map(|item| item.path.clone())
            .collect::<Vec<_>>(),
        [
            PathBuf::from("Destination"),
            PathBuf::from("Destination/2026-08-17"),
            PathBuf::from("Destination/2026-08-17/raw"),
            PathBuf::from("Destination/2026-08-17/raw/DSC_1003.nef"),
        ]
    );
}

#[test]
fn import_rejects_identical_source_and_destination_without_touching_files() {
    let directory = tempfile::tempdir().unwrap();
    let photo = directory.path().join("photo.jpg");
    std::fs::write(&photo, b"original bytes").unwrap();
    let import = FileImportWorkflow::new(
        Some(directory.path().to_path_buf()),
        Some(directory.path().to_path_buf()),
    );

    let result = import.run();

    assert!(matches!(
        result,
        Err(FileImportWorkflowError::OverlappingPaths { .. })
    ));
    assert_eq!(std::fs::read(photo).unwrap(), b"original bytes");
}

#[test]
fn import_preflights_all_destination_conflicts_before_copying() {
    let source = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();
    std::fs::write(source.path().join("new.jpg"), b"new").unwrap();
    std::fs::write(source.path().join("existing.jpg"), b"replacement").unwrap();
    std::fs::write(destination.path().join("existing.jpg"), b"keep me").unwrap();
    let import = FileImportWorkflow::new(
        Some(source.path().to_path_buf()),
        Some(destination.path().to_path_buf()),
    );

    let result = import.run();

    assert!(matches!(
        result,
        Err(FileImportWorkflowError::DestinationConflict(_))
    ));
    assert!(!destination.path().join("new.jpg").exists());
    assert_eq!(
        std::fs::read(destination.path().join("existing.jpg")).unwrap(),
        b"keep me"
    );
}

#[test]
fn copy_never_overwrites_an_existing_file() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.jpg");
    let destination = directory.path().join("destination.jpg");
    std::fs::write(&source, b"source").unwrap();
    std::fs::write(&destination, b"destination").unwrap();
    let result = FileWorkflowStepResult {
        original_path: source.clone(),
        output_path: destination.clone(),
        file_metadata: std::fs::metadata(source).unwrap(),
        exif: FileExifMetadata::default(),
    };

    let error = FileImportWorkflow::copy(&result).unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(std::fs::read(destination).unwrap(), b"destination");
}

#[cfg(unix)]
#[test]
fn dangling_destination_symlink_is_preflighted_before_any_copy() {
    use std::os::unix::fs::symlink;

    let source = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();
    std::fs::write(source.path().join("good.jpg"), b"good").unwrap();
    std::fs::write(source.path().join("linked.jpg"), b"linked").unwrap();
    symlink(
        destination.path().join("missing-target"),
        destination.path().join("linked.jpg"),
    )
    .unwrap();
    let import = FileImportWorkflow::new(
        Some(source.path().to_path_buf()),
        Some(destination.path().to_path_buf()),
    );

    let result = import.run();

    assert!(matches!(
        result,
        Err(FileImportWorkflowError::DestinationConflict(_))
    ));
    assert!(!destination.path().join("good.jpg").exists());
    assert!(
        std::fs::symlink_metadata(destination.path().join("linked.jpg"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[cfg(unix)]
#[test]
fn destination_directory_symlink_cannot_escape_import_root() {
    use std::os::unix::fs::symlink;

    let source = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir(source.path().join("album")).unwrap();
    std::fs::write(source.path().join("album/photo.jpg"), b"photo").unwrap();
    symlink(outside.path(), destination.path().join("album")).unwrap();
    let import = FileImportWorkflow::new(
        Some(source.path().to_path_buf()),
        Some(destination.path().to_path_buf()),
    );

    let result = import.run();

    assert!(matches!(
        result,
        Err(FileImportWorkflowError::DestinationConflict(_))
    ));
    assert!(!outside.path().join("photo.jpg").exists());
}

#[test]
fn virtual_preview_paths_always_create_file_leaves() {
    let mut tree = PreviewTree::default();
    tree.insert(Path::new("album/photo.jpg"));
    let items = tree.into_items(PathBuf::from("Destination"));
    assert_eq!(items.len(), 3);
    assert!(!items[2].is_directory);
}

#[test]
fn stale_preview_generation_cannot_replace_newer_cache() {
    let old_key = ImportPreviewKey {
        source_path: PathBuf::from("old"),
        destination_path: PathBuf::from("destination"),
        workflow_signature: 1,
    };
    let new_key = ImportPreviewKey {
        source_path: PathBuf::from("new"),
        destination_path: PathBuf::from("destination"),
        workflow_signature: 2,
    };
    let mut state = ImportPreviewState {
        worker_running: true,
        generation: 2,
        active_key: Some(new_key),
        cache_source: Some(PathBuf::from("new")),
        cached_files: Some(Arc::new(Vec::new())),
        status: ImportPreviewStatus::Loading,
    };

    state.publish(
        1,
        &old_key,
        PathBuf::from("old"),
        Some(Arc::new(Vec::new())),
        Err("stale".to_string()),
    );

    assert_eq!(state.cache_source, Some(PathBuf::from("new")));
    assert!(matches!(state.status, ImportPreviewStatus::Loading));
}

#[test]
fn template_completion_matches_partial_field_and_closing_braces() {
    let template = "raw/{{capt}}";
    let cursor = "raw/{{capt".chars().count();

    let (start, end, prefix) = FileImportModal::template_completion(template, cursor).unwrap();

    assert_eq!(
        &template.chars().collect::<Vec<_>>()[start..end],
        &['{', '{', 'c', 'a', 'p', 't', '}', '}']
    );
    assert_eq!(prefix, "capt");
}

#[test]
fn template_field_completion_can_be_clicked() {
    use egui_kittest::{Harness, kittest::Queryable};

    let step = Arc::new(Mutex::new(subdirectory("raw/{{capt")));
    let ui_step = step.clone();
    let mut harness = Harness::new_ui(move |ui| {
        FileImportModal::workflow_step_node_ui(ui, &mut ui_step.lock().unwrap());
    });
    harness.run();

    harness.get_by_label("+ Insert field").click();
    harness.run();
    harness.get_by_label("capture_date").click();
    harness.run();

    let step = step.lock().unwrap();
    let WorkflowStep::AppendSubdirectory(template) = &*step else {
        panic!("expected subdirectory step");
    };
    assert_eq!(template.template, "raw/{{capture_date}}");
}

#[test]
fn invalid_date_template_returns_an_error_without_copying() {
    let source = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();
    std::fs::write(source.path().join("photo.jpg"), b"photo").unwrap();
    let mut import =
        FileImportWorkflow::new(Some(source.path().into()), Some(destination.path().into()));
    import
        .workflow
        .steps
        .push(subdirectory("{{modified_date:%Q}}"));

    assert!(matches!(
        import.run(),
        Err(FileImportWorkflowError::InvalidDateFormat(_))
    ));
    assert_eq!(std::fs::read_dir(destination.path()).unwrap().count(), 0);
}

#[test]
fn preview_and_copy_agree_after_branching_and_filtering() {
    let source = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();
    for name in ["photo.RAW", "photo.jpg", "skip.txt"] {
        std::fs::write(source.path().join(name), name.as_bytes()).unwrap();
    }
    let mut import =
        FileImportWorkflow::new(Some(source.path().into()), Some(destination.path().into()));
    import.workflow.steps = vec![
        WorkflowStep::Filter(Condition::Not(Box::new(extension_condition("txt")))),
        WorkflowStep::Conditional {
            condition: extension_condition("raw"),
            then_workflow: FileWorkflow {
                steps: vec![subdirectory("raw")],
            },
            else_workflow: None,
        },
        subdirectory("day/{{file_size}}"),
    ];
    let preview = import
        .build_preview(&import.scan_source().unwrap())
        .unwrap();
    assert_eq!((preview.imported_count, preview.dropped_count), (2, 1));
    assert_eq!(std::fs::read_dir(destination.path()).unwrap().count(), 0);
    let imported_paths = import.run().unwrap();
    assert_eq!(imported_paths.len(), 2);
    for name in ["raw/day/9/photo.RAW", "day/9/photo.jpg"] {
        assert!(imported_paths.contains(&destination.path().join(name)));
        assert!(destination.path().join(name).is_file());
        assert!(preview.items.iter().any(|item| item.path.ends_with(name)));
    }
    assert!(!destination.path().join("skip.txt").exists());
    assert_eq!(
        std::fs::read(source.path().join("photo.RAW")).unwrap(),
        b"photo.RAW"
    );
}

#[test]
fn changing_comparison_operator_preserves_field_and_value() {
    use egui_kittest::{Harness, kittest::Queryable};
    let step = Arc::new(Mutex::new(WorkflowStep::Filter(Condition::Compare {
        field: MetadataField::Iso,
        operator: ComparisonOperator::Equal,
        value: MetadataValue::Integer(800),
    })));
    let ui_step = step.clone();
    let mut harness = Harness::new_ui(move |ui| {
        FileImportModal::workflow_step_node_ui(ui, &mut ui_step.lock().unwrap());
    });
    harness
        .get_all_by_role(egui::accesskit::Role::ComboBox)
        .next()
        .unwrap()
        .click();
    harness.run();
    harness.get_by_label("At least (>=)").click();
    harness.run();
    assert!(matches!(
        &*step.lock().unwrap(),
        WorkflowStep::Filter(Condition::Compare {
            field: MetadataField::Iso,
            operator: ComparisonOperator::GreaterOrEqual,
            value: MetadataValue::Integer(800),
        })
    ));
}

#[test]
fn comparisons_handle_numbers_dates_and_missing_metadata() {
    use chrono::{DateTime, Duration, Utc};
    let source = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();
    let file = source.path().join("photo.JPG");
    std::fs::write(&file, b"1234").unwrap();
    let modified: DateTime<Utc> = std::fs::metadata(&file).unwrap().modified().unwrap().into();
    let mut import =
        FileImportWorkflow::new(Some(source.path().into()), Some(destination.path().into()));
    let files = import.scan_source().unwrap();
    for (operator, expected) in [
        (ComparisonOperator::Equal, [false, true, false]),
        (ComparisonOperator::Greater, [true, false, false]),
        (ComparisonOperator::Less, [false, false, true]),
        (ComparisonOperator::GreaterOrEqual, [true, true, false]),
        (ComparisonOperator::LessOrEqual, [false, true, true]),
    ] {
        for (index, offset) in [-1, 0, 1].into_iter().enumerate() {
            for (field, value) in [
                (
                    MetadataField::FileSize,
                    MetadataValue::Integer((4 + offset) as u64),
                ),
                (
                    MetadataField::ModifiedDateTime,
                    MetadataValue::DateTime(modified + Duration::seconds(offset)),
                ),
                (MetadataField::Iso, MetadataValue::Integer(100)),
            ] {
                import.workflow.steps = vec![WorkflowStep::Filter(Condition::Compare {
                    field,
                    operator,
                    value,
                })];
                let preview = import.build_preview(&files).unwrap();
                let expected = field != MetadataField::Iso && expected[index];
                assert_eq!(
                    preview.imported_count,
                    usize::from(expected),
                    "{field:?} {operator:?} offset={offset}"
                );
            }
        }
    }
    import.workflow.steps = vec![WorkflowStep::Filter(extension_condition(".jpg"))];
    assert_eq!(import.build_preview(&files).unwrap().imported_count, 1);
}

#[test]
fn switching_to_text_resets_and_hides_ordered_comparisons() {
    use egui_kittest::{Harness, kittest::Queryable};
    let step = Arc::new(Mutex::new(WorkflowStep::Filter(Condition::Compare {
        field: MetadataField::Iso,
        operator: ComparisonOperator::Greater,
        value: MetadataValue::Integer(800),
    })));
    let ui_step = step.clone();
    let mut harness = Harness::new_ui(move |ui| {
        FileImportModal::workflow_step_node_ui(ui, &mut ui_step.lock().unwrap());
    });
    harness
        .get_all_by_role(egui::accesskit::Role::ComboBox)
        .nth(1)
        .unwrap()
        .click();
    harness.run();
    harness.get_by_label("Extension").click();
    harness.run();
    assert!(matches!(
        &*step.lock().unwrap(),
        WorkflowStep::Filter(Condition::Compare {
            field: MetadataField::Extension,
            operator: ComparisonOperator::Equal,
            ..
        })
    ));
    harness
        .get_all_by_role(egui::accesskit::Role::ComboBox)
        .next()
        .unwrap()
        .click();
    harness.run();
    assert!(harness.query_by_label("Equals (=)").is_some());
    for label in [
        "Greater than (>)",
        "Less than (<)",
        "At least (>=)",
        "At most (<=)",
    ] {
        assert!(harness.query_by_label(label).is_none());
    }
    for field in [MetadataField::Extension, MetadataField::FileName] {
        assert_eq!(field.supported_operators(), &[ComparisonOperator::Equal]);
    }
}

#[test]
fn planned_file_directory_conflicts_are_order_independent() {
    let source = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();
    std::fs::write(source.path().join("raw"), b"file").unwrap();
    std::fs::write(source.path().join("photo.jpg"), b"photo").unwrap();
    let mut import =
        FileImportWorkflow::new(Some(source.path().into()), Some(destination.path().into()));
    import.workflow.steps = vec![WorkflowStep::Conditional {
        condition: extension_condition("jpg"),
        then_workflow: FileWorkflow {
            steps: vec![subdirectory("raw")],
        },
        else_workflow: None,
    }];
    let mut files = import.scan_source().unwrap();
    for _ in 0..2 {
        let preview = import.build_preview(&files).unwrap();
        assert!(!preview.conflicts.is_empty());
        assert!(
            preview
                .items
                .iter()
                .any(|item| item.path.ends_with("raw/photo.jpg"))
        );
        files.reverse();
    }
    assert!(matches!(
        import.run(),
        Err(FileImportWorkflowError::DestinationConflict(_))
    ));
    assert_eq!(std::fs::read_dir(destination.path()).unwrap().count(), 0);
}

#[test]
fn preview_edits_coalesce_until_the_worker_finishes() {
    let first = ImportPreviewKey {
        source_path: "source".into(),
        destination_path: "destination".into(),
        workflow_signature: 1,
    };
    let latest = ImportPreviewKey {
        workflow_signature: 2,
        ..first.clone()
    };
    let mut state = ImportPreviewState::default();
    let (_, generation) = state.start(&first).unwrap();
    assert!(state.start(&latest).is_none());
    assert!(state.start(&latest).is_none());
    state.publish(
        generation,
        &first,
        first.source_path.clone(),
        None,
        Err("stale".into()),
    );
    assert!(matches!(state.status, ImportPreviewStatus::Loading));
    let (_, generation) = state.start(&latest).unwrap();
    state.publish(
        generation,
        &latest,
        latest.source_path.clone(),
        None,
        Err("latest".into()),
    );
    assert!(matches!(&state.status, ImportPreviewStatus::Error(message) if message == "latest"));
    assert!(state.start(&latest).is_none());
}

#[test]
fn completion_replaces_the_entire_token_from_its_middle() {
    let template = "写真/{{capture_date}}/raw";
    let (start, end, prefix) =
        FileImportModal::template_completion(template, "写真/{{cap".chars().count()).unwrap();
    assert_eq!(prefix, "cap");
    let mut chars = template.chars().collect::<Vec<_>>();
    chars.splice(start..end, "{{file_name}}".chars());
    assert_eq!(
        chars.into_iter().collect::<String>(),
        "写真/{{file_name}}/raw"
    );
}

#[test]
fn graph_layout_fits_expanded_nodes_and_calendar_edits() {
    use egui_kittest::{Harness, kittest::Queryable};
    let date = chrono::NaiveDate::from_ymd_opt(2026, 9, 5)
        .unwrap()
        .and_hms_opt(12, 30, 0)
        .unwrap()
        .and_utc();
    let modal = Arc::new(Mutex::new(FileImportModal::new()));
    modal
        .lock()
        .unwrap()
        .load_saved_workflow(saved_workflows::WorkflowSelection::New);
    modal.lock().unwrap().workflow.steps = vec![
        WorkflowStep::Filter(Condition::Compare {
            field: MetadataField::CaptureDateTime,
            operator: ComparisonOperator::GreaterOrEqual,
            value: MetadataValue::DateTime(date),
        }),
        WorkflowStep::Conditional {
            condition: Condition::And(vec![extension_condition("jpg"), extension_condition("raw")]),
            then_workflow: FileWorkflow {
                steps: vec![subdirectory("photos")],
            },
            else_workflow: Some(FileWorkflow {
                steps: vec![subdirectory("other")],
            }),
        },
    ];
    let ui_modal = modal.clone();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1440.0, 900.0))
        .with_max_steps(12)
        .build_ui(move |ui| {
            let mut modal = ui_modal.lock().unwrap();
            egui::Modal::new(egui::Id::new("review_import"))
                .frame(style::dialog_frame())
                .show(ui.ctx(), |ui| {
                    style::dialog_title(ui, modal.title());
                    ui.add_space(12.0);
                    modal.body_ui(ui);
                    ui.add_space(24.0);
                    egui::Sides::new().show(ui, |_| {}, |ui| modal.actions_ui(ui));
                });
        });
    style::apply(&harness.ctx);
    harness.run();
    assert!(harness.query_by_label("Import preview").is_some());
    assert!(
        harness
            .ctx
            .content_rect()
            .contains_rect(harness.get_by_label("Cancel").rect())
    );
    let bounds = {
        let modal = modal.lock().unwrap();
        egui_graph::with_graph_memory(
            &harness.ctx,
            egui_graph::id("file_import_workflow_graph"),
            |memory| {
                modal
                    .graph
                    .view
                    .layout
                    .iter()
                    .map(|(id, position)| {
                        egui::Rect::from_min_size(*position, memory.node_sizes()[id])
                    })
                    .collect::<Vec<_>>()
            },
        )
    };
    for (index, rect) in bounds.iter().enumerate() {
        for other in &bounds[index + 1..] {
            assert!(
                !rect.intersects(*other),
                "nodes overlap: {rect:?} and {other:?}"
            );
        }
    }
    if let Ok(path) = std::env::var("PHOTOBOOK_IMPORT_REVIEW_SNAPSHOT") {
        harness.render().unwrap().save(path).unwrap();
    }
    let node_layer = egui::LayerId::new(
        egui::Order::Foreground,
        egui_graph::node::egui_id(
            egui_graph::id("file_import_workflow_graph"),
            egui_graph::NodeId::new("root/step-0"),
        ),
    );
    let click = |harness: &mut Harness<'_>, label: &str| {
        // kittest reports node bounds in graph coordinates; pointer input is global.
        let transform = harness.ctx.layer_transform_to_global(node_layer).unwrap();
        let position = transform * harness.get_by_label(label).rect().center();
        harness.hover_at(position);
        harness.drag_at(position);
        harness.drop_at(position);
        harness.run();
    };
    click(&mut harness, "2026-09-05 📆");
    if std::env::var("PHOTOBOOK_IMPORT_REVIEW_SNAPSHOT").is_ok() {
        harness
            .render()
            .unwrap()
            .save("/tmp/photobook-calendar-review.png")
            .unwrap();
    }
    assert!(harness.query_by_label("Save").is_some());
    // The calendar is a separate, unscaled foreground area.
    let calendar_layer = harness
        .ctx
        .memory(|memory| memory.areas().visible_layer_ids())
        .into_iter()
        .find(|layer| {
            layer.order == egui::Order::Foreground
                && egui::AreaState::load(&harness.ctx, layer.id).is_some_and(|area| {
                    area.rect()
                        .contains(harness.get_by_label("Save").rect().center())
                        && area.rect().width() < 400.0
                })
        })
        .expect("calendar area");
    for label in ["6", "Save"] {
        let transform = harness
            .ctx
            .layer_transform_to_global(calendar_layer)
            .unwrap();
        let position = transform * harness.get_by_label(label).rect().center();
        harness.hover_at(position);
        harness.drag_at(position);
        harness.drop_at(position);
        harness.run();
    }
    let modal = modal.lock().unwrap();
    let WorkflowStep::Filter(Condition::Compare {
        value: MetadataValue::DateTime(value),
        ..
    }) = &modal.workflow.steps[0]
    else {
        panic!("expected date condition")
    };
    assert_eq!(
        value.format("%Y-%m-%d %H:%M:%S").to_string(),
        "2026-09-06 12:30:00"
    );
    drop(modal);
    harness.set_size(egui::vec2(1024.0, 768.0));
    harness.run();
    assert!(
        harness
            .ctx
            .content_rect()
            .contains_rect(harness.get_by_label("Cancel").rect())
    );
    assert!(
        harness
            .ctx
            .content_rect()
            .contains_rect(harness.get_by_label("Import preview").rect())
    );
}

#[test]
fn import_dialog_adapts_to_narrow_windows_and_can_refit_workflow() {
    use egui_kittest::{Harness, kittest::Queryable};

    let mut harness = Harness::builder()
        .with_size(egui::vec2(1440.0, 900.0))
        .build_ui_state(
            |ui, modal: &mut FileImportModal| {
                egui::Modal::new(egui::Id::new("responsive_import"))
                    .frame(style::dialog_frame())
                    .show(ui.ctx(), |ui| {
                        style::dialog_title(ui, modal.title());
                        ui.add_space(12.0);
                        modal.body_ui(ui);
                        ui.add_space(24.0);
                        egui::Sides::new().show(ui, |_| {}, |ui| modal.actions_ui(ui));
                    });
            },
            FileImportModal::new(),
        );
    style::apply(&harness.ctx);
    harness.run_steps(5);
    harness.run();

    assert!(harness.state().add_to_collection);
    harness
        .get_by_label("Add imported images to collection")
        .click();
    harness.run();
    assert!(!harness.state().add_to_collection);

    assert!(harness.query_by_label("Fit workflow").is_none());
    assert!(harness.query_by_label("Save to").is_none());
    assert!(harness.query_by_label("Direct import (modified)").is_none());
    harness
        .get_all_by_role(egui::accesskit::Role::ComboBox)
        .next()
        .unwrap()
        .click();
    harness.run();
    harness.get_by_label("New workflow…").click();
    harness.run();
    assert!(harness.query_by_label("Save to").is_some());

    let fit = harness.get_by_label("Fit workflow").rect();
    let preview = harness.get_by_label("Import preview").rect();
    assert!(preview.left() > fit.right());
    assert!((preview.center().y - fit.center().y).abs() < 5.0);

    harness.state_mut().graph.view.scene_rect = harness
        .state()
        .graph
        .view
        .scene_rect
        .translate(egui::vec2(2000.0, 1000.0));
    harness.run();
    let panned = harness.state().graph.view.scene_rect;
    harness.get_by_label("Fit workflow").click();
    harness.run();
    assert!((harness.state().graph.view.scene_rect.center() - panned.center()).length() > 500.0);
    assert!(!harness.state().graph.fit_requested);

    assert!(harness.query_by_label("Save to").is_some());

    harness.set_size(egui::vec2(800.0, 700.0));
    harness.run();
    assert!(
        harness.get_by_label("Import preview").rect().top()
            > harness.get_by_label("Fit workflow").rect().bottom()
    );

    harness.set_size(egui::vec2(640.0, 640.0));
    harness.run_steps(5);
    harness.run();
    assert!(
        harness
            .get_by_label("Choose destination folder…")
            .rect()
            .top()
            > harness
                .get_by_label("Choose source folder…")
                .rect()
                .bottom()
    );
    for label in ["Cancel", "Import"] {
        assert!(
            harness.ctx.content_rect().contains_rect(
                harness
                    .get_by_role_and_label(egui::accesskit::Role::Button, label)
                    .rect()
            )
        );
    }
    if let Ok(path) = std::env::var("PHOTOBOOK_IMPORT_NARROW_SNAPSHOT") {
        harness.render().unwrap().save(path).unwrap();
    }
}

#[test]
fn collection_import_reads_only_copied_supported_images() {
    let destination = tempfile::tempdir().unwrap();
    let imported = destination.path().join("imported.PNG");
    let existing = destination.path().join("existing.png");
    let missing = destination.path().join("missing.jpg");
    let sidecar = destination.path().join("metadata.xmp");
    let image = image::RgbImage::new(2, 2);
    image.save(&imported).unwrap();
    image.save(&existing).unwrap();
    std::fs::write(&sidecar, b"metadata").unwrap();
    let (photos, failures) =
        FileImportModal::imported_photos(vec![imported.clone(), missing, sidecar]);
    assert_eq!(failures, 1);
    assert_eq!(photos.len(), 1);
    assert_eq!(photos[0].path, imported);
}
