use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
};

use crate::{cursor_manager::CursorManager, dep_mut, theme::color};

use super::{
    FileImportModal,
    graph_model::*,
    workflow::{Condition, FileWorkflow, SubdirectoryTemplate, WorkflowStep},
};

#[derive(Default)]
pub(super) struct WorkflowGraphState {
    pub(super) view: egui_graph::View,
    signature: u64,
    insert_menu: Option<WorkflowGraphEdgeId>,
    node_sizes: HashMap<egui_graph::NodeId, egui::Vec2>,
    needs_layout: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum GraphAddStep {
    Filter,
    Subdirectory,
    Conditional,
}

impl GraphAddStep {
    pub(super) fn into_step(self) -> WorkflowStep {
        match self {
            Self::Filter => WorkflowStep::Filter(Condition::default_equals()),
            Self::Subdirectory => WorkflowStep::AppendSubdirectory(SubdirectoryTemplate {
                template: String::new(),
            }),
            Self::Conditional => WorkflowStep::Conditional {
                condition: Condition::default_equals(),
                then_workflow: FileWorkflow::default(),
                else_workflow: None,
            },
        }
    }
}

impl FileImportModal {
    pub(super) fn workflow_graph_ui(
        ui: &mut egui::Ui,
        workflow: &mut FileWorkflow,
        state: &mut WorkflowGraphState,
    ) {
        let WorkflowGraphState {
            view,
            signature: previous_signature,
            insert_menu,
            node_sizes,
            needs_layout,
        } = state;
        let graph = WorkflowGraph::new(workflow);
        let signature = graph.signature();
        let structure_changed = signature != *previous_signature;
        if structure_changed {
            node_sizes.clear();
            if insert_menu.is_some() {
                egui::Popup::close_all(ui.ctx());
            }
            *insert_menu = None;
        }
        let mut should_layout = structure_changed || view.layout.is_empty() || *needs_layout;
        *needs_layout = false;
        let mut add_step = None;

        egui::Frame::new()
            .fill(color::SURFACE_DARK)
            .stroke(egui::Stroke::new(1.0, color::SURFACE_MUTED))
            .corner_radius(8)
            .inner_margin(8)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(egui::RichText::new("Workflow graph").color(color::WHITE));
                    ui.separator();
                    ui.label("Add node");
                    if ui.button("Filter").clicked() {
                        add_step = Some(GraphAddStep::Filter);
                    }
                    if ui.button("Subdirectory").clicked() {
                        add_step = Some(GraphAddStep::Subdirectory);
                    }
                    if ui.button("Conditional").clicked() {
                        add_step = Some(GraphAddStep::Conditional);
                    }
                    ui.separator();
                    if ui.button("Auto layout").clicked() {
                        should_layout = true;
                    }
                });
            });
        ui.add_space(4.0);

        if should_layout {
            let socket_padding = egui_graph::socket_padding(ui.style());
            view.layout = egui_graph::layout(
                graph.nodes.iter().map(|node| {
                    let size = node_sizes
                        .get(&node.id)
                        .copied()
                        .unwrap_or_else(|| node.layout_size());
                    let layout_node = egui_graph::LayoutNode::new(size)
                        .socket_padding(socket_padding)
                        .inputs(node.inputs)
                        .outputs(node.outputs);
                    (node.id, layout_node)
                }),
                graph
                    .edges
                    .iter()
                    .map(|edge| ((edge.from, edge.output), (edge.to, edge.input))),
                egui_graph::LayoutParams::new(egui::Direction::LeftToRight)
                    .layer_gap(100.0)
                    .node_gap(45.0),
            );
            *previous_signature = signature;
        }

        let mut inline_add = None;
        let mut remove_address = None;
        let mut measured_sizes = HashMap::new();
        let graph_height = ui
            .available_height()
            .min(ui.ctx().content_rect().height() * 0.58)
            .max(120.0);
        let mut plus_hovered = false;
        let graph_size = egui::vec2(ui.available_width(), graph_height);
        let response = ui.allocate_ui(graph_size, |ui| {
            egui_graph::Graph::new("file_import_workflow_graph")
                .center_view(should_layout)
                .immutable(true)
                .selection_enabled(false)
                .scroll_zoom(true)
                .drag_pan_buttons(
                    egui::containers::DragPanButtons::PRIMARY
                        | egui::containers::DragPanButtons::MIDDLE,
                )
                .show(view, ui, |ui, show| {
                    show.nodes(ui, |node_context, ui| {
                        for node in &graph.nodes {
                            let response = egui_graph::node::Node::from_id(node.id)
                                .animation_time(0.0)
                                .inputs(node.inputs)
                                .outputs(node.outputs)
                                .max_width(node.layout_size().x)
                                .show(node_context, ui, |node_context| {
                                    let interaction = node_context.interaction();
                                    let endpoint = node.address.is_none();
                                    let fill = if endpoint {
                                        color::ACCENT_MUTED
                                    } else {
                                        color::SURFACE_DARK
                                    };
                                    let stroke = if interaction.hovered {
                                        egui::Stroke::new(1.0, color::SURFACE_EMPHASIS)
                                    } else {
                                        egui::Stroke::new(1.0, color::SURFACE_MUTED)
                                    };
                                    let frame = egui::Frame::new()
                                        .fill(fill)
                                        .stroke(stroke)
                                        .corner_radius(8)
                                        .inner_margin(14);
                                    // Addresses shift after insert/remove; do not transfer text
                                    // cursors or calendar drafts to the next node at that address.
                                    let node_ui_id =
                                        node_context.egui_id().with(("controls", signature));
                                    node_context.framed_with_interactive(frame, |ui, _| {
                                        ui.push_id(node_ui_id, |ui| {
                                            ui.set_min_width(node.layout_size().x - 28.0);
                                            ui.spacing_mut().item_spacing = egui::vec2(8.0, 7.0);
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    egui::RichText::new(&node.title)
                                                        .strong()
                                                        .size(14.0)
                                                        .color(color::WHITE),
                                                );
                                                if let Some(address) = node.address.as_ref() {
                                                    ui.with_layout(
                                                        egui::Layout::right_to_left(
                                                            egui::Align::Center,
                                                        ),
                                                        |ui| {
                                                            if ui
                                                                .small_button("×")
                                                                .on_hover_text("Remove node")
                                                                .clicked()
                                                            {
                                                                remove_address =
                                                                    Some(address.clone());
                                                            }
                                                        },
                                                    );
                                                }
                                            });

                                            if let Some(address) = node.address.as_ref()
                                                && let Some(step) =
                                                    Self::workflow_step_mut(workflow, address)
                                            {
                                                ui.add_space(2.0);
                                                ui.separator();
                                                ui.add_space(2.0);
                                                Self::workflow_step_node_ui(ui, step);
                                            } else if !node.detail.is_empty() {
                                                ui.label(
                                                    egui::RichText::new(&node.detail)
                                                        .color(color::CONTROL_TEXT),
                                                );
                                            }

                                            if node.outputs == 2 {
                                                ui.add_space(2.0);
                                                ui.label(
                                                    egui::RichText::new(
                                                        "Outputs  ·  Then (top)  ·  Else (bottom)",
                                                    )
                                                    .small()
                                                    .color(color::SURFACE_EMPHASIS),
                                                );
                                            }
                                        });
                                    })
                                });
                            measured_sizes.insert(node.id, response.rect.size());
                        }
                    })
                    .edges(ui, |edge_context, ui| {
                        for edge in &graph.edges {
                            let edge_id = edge.id();
                            let mut is_selected = false;
                            let mut midpoint = None;
                            let branch_label = graph
                                .nodes
                                .iter()
                                .find(|node| node.id == edge.from && node.title == "Conditional")
                                .map(|_| if edge.output == 0 { "Then" } else { "Else" });
                            egui_graph::edge::Edge::new(
                                (edge.from, edge.output),
                                (edge.to, edge.input),
                                &mut is_selected,
                            )
                            .show_with(
                                edge_context,
                                ui,
                                |ui, context| {
                                    ui.painter().add(egui::Shape::line(
                                        context.points.to_vec(),
                                        context.stroke,
                                    ));
                                    midpoint = context
                                        .points
                                        .get(context.points.len().saturating_sub(1) / 2)
                                        .copied();
                                    if let (Some(label), Some(point)) =
                                        (branch_label, context.points.first())
                                    {
                                        ui.painter().text(
                                            *point
                                                + egui::vec2(
                                                    10.0,
                                                    if edge.output == 0 { -12.0 } else { 12.0 },
                                                ),
                                            egui::Align2::LEFT_CENTER,
                                            label,
                                            egui::FontId::proportional(12.0),
                                            color::CONTROL_TEXT,
                                        );
                                    }
                                },
                            );

                            let popup_id = ui.id().with(("insert_on_edge", edge_id));
                            if let Some(midpoint) = midpoint {
                                // Foreground areas do not inherit the graph's clipping.
                                if !ui.clip_rect().contains(midpoint) {
                                    if *insert_menu == Some(edge_id) {
                                        egui::Popup::close_id(ui.ctx(), popup_id);
                                        *insert_menu = None;
                                    }
                                    continue;
                                }
                                let scene_layer = ui.layer_id();
                                let midpoint = ui
                                    .ctx()
                                    .layer_transform_to_global(scene_layer)
                                    .map_or(midpoint, |transform| transform * midpoint);
                                let button = egui::Area::new(popup_id.with("button"))
                                    .order(egui::Order::Foreground)
                                    .pivot(egui::Align2::CENTER_CENTER)
                                    .fixed_pos(midpoint)
                                    .default_size(egui::vec2(24.0, 24.0))
                                    .constrain(false)
                                    .movable(false)
                                    .show(ui.ctx(), |ui| {
                                        ui.spacing_mut().interact_size = egui::vec2(24.0, 24.0);
                                        let (response, painter) = ui.allocate_painter(
                                            egui::vec2(24.0, 24.0),
                                            egui::Sense::click(),
                                        );
                                        response.widget_info(|| {
                                            egui::WidgetInfo::labeled(
                                                egui::WidgetType::Button,
                                                ui.is_enabled(),
                                                "+",
                                            )
                                        });
                                        let hovered =
                                            response.hovered() || *insert_menu == Some(edge_id);
                                        painter.circle(
                                            response.rect.center(),
                                            9.0,
                                            if hovered {
                                                color::SURFACE_STRONG
                                            } else {
                                                color::SURFACE_MUTED
                                            },
                                            egui::Stroke::new(
                                                1.0,
                                                if hovered {
                                                    color::WHITE
                                                } else {
                                                    color::SURFACE_EMPHASIS
                                                },
                                            ),
                                        );
                                        painter.text(
                                            response.rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            "+",
                                            egui::FontId::proportional(14.0),
                                            color::WHITE,
                                        );
                                        response
                                    })
                                    .inner
                                    .on_hover_text("Insert a workflow node");
                                plus_hovered |= button.hovered();
                                // Attach controls to the modal so raising its background
                                // cannot cover them. Popups remain above these controls.
                                if let Some(modal_layer) =
                                    ui.ctx().memory(|memory| memory.top_modal_layer())
                                {
                                    ui.ctx().set_sublayer(modal_layer, button.layer_id);
                                }
                                egui::Popup::menu(&button)
                                    .id(popup_id)
                                    .width(200.0)
                                    .show(|ui| {
                                        ui.strong("Insert node");
                                        ui.separator();
                                        for (label, step) in [
                                            ("Filter", GraphAddStep::Filter),
                                            ("Subdirectory", GraphAddStep::Subdirectory),
                                            ("Conditional", GraphAddStep::Conditional),
                                        ] {
                                            if ui.button(label).clicked() {
                                                inline_add = Some((edge.insertion.clone(), step));
                                                ui.close();
                                            }
                                        }
                                    });
                                if egui::Popup::is_id_open(ui.ctx(), popup_id) {
                                    ui.ctx().move_to_top(egui::LayerId::new(
                                        egui::Order::Foreground,
                                        popup_id,
                                    ));
                                    *insert_menu = Some(edge_id);
                                } else if *insert_menu == Some(edge_id) {
                                    *insert_menu = None;
                                }
                            }
                        }
                    });
                })
        });
        let response = response.inner;

        let cursor = if plus_hovered {
            Some(egui::CursorIcon::PointingHand)
        } else if response.response.dragged() {
            Some(egui::CursorIcon::Grabbing)
        } else if response.response.hovered() {
            Some(egui::CursorIcon::Grab)
        } else {
            None
        };
        if let Some(cursor) = cursor {
            dep_mut!(CursorManager, |manager| manager.set_cursor(cursor));
        }

        for (id, size) in measured_sizes {
            if node_sizes
                .get(&id)
                .is_none_or(|previous| (*previous - size).length() > 1.0)
            {
                node_sizes.insert(id, size);
                *needs_layout = true;
            }
        }
        if *needs_layout {
            // One follow-up pass packs the actual content, including nested conditions.
            ui.ctx().request_discard("workflow node dimensions changed");
            ui.ctx().request_repaint();
        }

        // Apply structural edits only after every node has used the graph's addresses.
        if let Some(add_step) = add_step {
            workflow.steps.push(add_step.into_step());
        } else if let Some((insertion, step)) = inline_add {
            Self::insert_graph_step_at(workflow, &insertion, step.into_step());
        } else if let Some(address) = remove_address.as_ref() {
            Self::remove_workflow_step(workflow, address);
        }
    }

    pub(super) fn insert_graph_step_at(
        workflow: &mut FileWorkflow,
        insertion: &WorkflowInsertionPoint,
        step: WorkflowStep,
    ) {
        if let Some(workflow) = Self::workflow_at_path_mut(workflow, &insertion.workflow_path) {
            workflow
                .steps
                .insert(insertion.index.min(workflow.steps.len()), step);
        }
    }

    pub(super) fn remove_workflow_step(workflow: &mut FileWorkflow, address: &WorkflowStepAddress) {
        if let Some((workflow, index)) = Self::workflow_parent_and_index_mut(workflow, &address.0) {
            workflow.steps.remove(index);
        }
    }

    fn workflow_at_path_mut<'a>(
        workflow: &'a mut FileWorkflow,
        path: &[WorkflowAddressSegment],
    ) -> Option<&'a mut FileWorkflow> {
        match path {
            [] => Some(workflow),
            [WorkflowAddressSegment::Step(index), branch, rest @ ..] => {
                match (workflow.steps.get_mut(*index)?, branch) {
                    (
                        WorkflowStep::Conditional { then_workflow, .. },
                        WorkflowAddressSegment::Then,
                    ) => Self::workflow_at_path_mut(then_workflow, rest),
                    (
                        WorkflowStep::Conditional { else_workflow, .. },
                        WorkflowAddressSegment::Else,
                    ) => Self::workflow_at_path_mut(
                        else_workflow.get_or_insert_with(FileWorkflow::default),
                        rest,
                    ),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn workflow_parent_and_index_mut<'a>(
        workflow: &'a mut FileWorkflow,
        address: &[WorkflowAddressSegment],
    ) -> Option<(&'a mut FileWorkflow, usize)> {
        match address {
            [WorkflowAddressSegment::Step(index)] if *index < workflow.steps.len() => {
                Some((workflow, *index))
            }
            [WorkflowAddressSegment::Step(index), branch, rest @ ..] => {
                let step = workflow.steps.get_mut(*index)?;
                match (step, branch) {
                    (
                        WorkflowStep::Conditional { then_workflow, .. },
                        WorkflowAddressSegment::Then,
                    ) => Self::workflow_parent_and_index_mut(then_workflow, rest),
                    (
                        WorkflowStep::Conditional {
                            else_workflow: Some(else_workflow),
                            ..
                        },
                        WorkflowAddressSegment::Else,
                    ) => Self::workflow_parent_and_index_mut(else_workflow, rest),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn workflow_step_mut<'a>(
        workflow: &'a mut FileWorkflow,
        address: &WorkflowStepAddress,
    ) -> Option<&'a mut WorkflowStep> {
        let (workflow, index) = Self::workflow_parent_and_index_mut(workflow, &address.0)?;
        workflow.steps.get_mut(index)
    }

    pub(super) fn workflow_config_signature(workflow: &FileWorkflow) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        serde_json::to_vec(workflow)
            .unwrap_or_default()
            .hash(&mut hasher);
        hasher.finish()
    }
}
