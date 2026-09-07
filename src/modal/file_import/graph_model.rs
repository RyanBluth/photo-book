use std::hash::{Hash, Hasher};

use super::workflow::{FileWorkflow, WorkflowStep};

pub(super) struct WorkflowGraph {
    pub(super) nodes: Vec<WorkflowGraphNode>,
    pub(super) edges: Vec<WorkflowGraphEdge>,
}

pub(super) struct WorkflowGraphNode {
    pub(super) id: egui_graph::NodeId,
    pub(super) title: String,
    pub(super) detail: String,
    pub(super) inputs: usize,
    pub(super) outputs: usize,
    pub(super) address: Option<WorkflowStepAddress>,
}

impl WorkflowGraphNode {
    pub(super) fn layout_size(&self) -> egui::Vec2 {
        if self.address.is_none() {
            egui::vec2(220.0, 82.0)
        } else {
            match self.title.as_str() {
                "Conditional" => egui::vec2(360.0, 330.0),
                "Filter" => egui::vec2(330.0, 220.0),
                "Subdirectory" => egui::vec2(330.0, 230.0),
                _ => egui::vec2(280.0, 120.0),
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct WorkflowStepAddress(pub(super) Vec<WorkflowAddressSegment>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum WorkflowAddressSegment {
    Step(usize),
    Then,
    Else,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct WorkflowInsertionPoint {
    pub(super) workflow_path: Vec<WorkflowAddressSegment>,
    pub(super) index: usize,
}

#[derive(Debug, Clone)]
struct WorkflowGraphTail {
    from: egui_graph::NodeId,
    output: usize,
    insertion: WorkflowInsertionPoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct WorkflowGraphEdgeId {
    from: egui_graph::NodeId,
    output: usize,
    to: egui_graph::NodeId,
    input: usize,
}

#[derive(Clone, Hash)]
pub(super) struct WorkflowGraphEdge {
    pub(super) from: egui_graph::NodeId,
    pub(super) output: usize,
    pub(super) to: egui_graph::NodeId,
    pub(super) input: usize,
    pub(super) insertion: WorkflowInsertionPoint,
}

impl WorkflowGraphEdge {
    pub(super) fn id(&self) -> WorkflowGraphEdgeId {
        WorkflowGraphEdgeId {
            from: self.from,
            output: self.output,
            to: self.to,
            input: self.input,
        }
    }
}

struct WorkflowGraphBuilder {
    nodes: Vec<WorkflowGraphNode>,
    edges: Vec<WorkflowGraphEdge>,
}

impl WorkflowGraph {
    pub(super) fn new(workflow: &FileWorkflow) -> Self {
        let mut builder = WorkflowGraphBuilder {
            nodes: Vec::new(),
            edges: Vec::new(),
        };
        let start = egui_graph::NodeId::new("file_import/start");
        builder.nodes.push(WorkflowGraphNode {
            id: start,
            title: "Input".to_string(),
            detail: "Source files".to_string(),
            inputs: 0,
            outputs: 1,
            address: None,
        });

        let tails = builder.build_workflow(
            workflow,
            vec![WorkflowGraphTail {
                from: start,
                output: 0,
                insertion: WorkflowInsertionPoint {
                    workflow_path: Vec::new(),
                    index: 0,
                },
            }],
            "root",
            &[],
        );
        builder.add_node(
            "file_import/end",
            "Import",
            "Copy to destination",
            0,
            &tails,
            None,
        );

        Self {
            nodes: builder.nodes,
            edges: builder.edges,
        }
    }

    pub(super) fn signature(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for node in &self.nodes {
            node.id.hash(&mut hasher);
            node.inputs.hash(&mut hasher);
            node.outputs.hash(&mut hasher);
        }
        self.edges.hash(&mut hasher);
        hasher.finish()
    }
}

impl WorkflowGraphBuilder {
    fn build_workflow(
        &mut self,
        workflow: &FileWorkflow,
        mut incoming: Vec<WorkflowGraphTail>,
        path: &str,
        address_prefix: &[WorkflowAddressSegment],
    ) -> Vec<WorkflowGraphTail> {
        for (index, step) in workflow.steps.iter().enumerate() {
            let step_path = format!("{path}/step-{index}");
            let mut step_address = address_prefix.to_vec();
            step_address.push(WorkflowAddressSegment::Step(index));
            match step {
                WorkflowStep::Filter(condition) => {
                    let id = self.add_node(
                        &step_path,
                        "Filter",
                        &condition.summary(),
                        1,
                        &incoming,
                        Some(WorkflowStepAddress(step_address)),
                    );
                    incoming = vec![WorkflowGraphTail {
                        from: id,
                        output: 0,
                        insertion: WorkflowInsertionPoint {
                            workflow_path: address_prefix.to_vec(),
                            index: index + 1,
                        },
                    }];
                }
                WorkflowStep::AppendSubdirectory(template) => {
                    let id = self.add_node(
                        &step_path,
                        "Subdirectory",
                        &template.template,
                        1,
                        &incoming,
                        Some(WorkflowStepAddress(step_address)),
                    );
                    incoming = vec![WorkflowGraphTail {
                        from: id,
                        output: 0,
                        insertion: WorkflowInsertionPoint {
                            workflow_path: address_prefix.to_vec(),
                            index: index + 1,
                        },
                    }];
                }
                WorkflowStep::Conditional {
                    condition,
                    then_workflow,
                    else_workflow,
                } => {
                    let id = self.add_node(
                        &step_path,
                        "Conditional",
                        &condition.summary(),
                        1 + usize::from(else_workflow.is_some()),
                        &incoming,
                        Some(WorkflowStepAddress(step_address.clone())),
                    );
                    let mut then_address = step_address.clone();
                    then_address.push(WorkflowAddressSegment::Then);
                    let mut branches = self.build_workflow(
                        then_workflow,
                        vec![WorkflowGraphTail {
                            from: id,
                            output: 0,
                            insertion: WorkflowInsertionPoint {
                                workflow_path: then_address.clone(),
                                index: 0,
                            },
                        }],
                        &format!("{step_path}/then"),
                        &then_address,
                    );
                    let mut else_address = step_address;
                    else_address.push(WorkflowAddressSegment::Else);
                    if let Some(else_workflow) = else_workflow {
                        branches.extend(self.build_workflow(
                            else_workflow,
                            vec![WorkflowGraphTail {
                                from: id,
                                output: 1,
                                insertion: WorkflowInsertionPoint {
                                    workflow_path: else_address.clone(),
                                    index: 0,
                                },
                            }],
                            &format!("{step_path}/else"),
                            &else_address,
                        ));
                    }
                    incoming = branches;
                }
            }
        }

        incoming
    }

    fn add_node(
        &mut self,
        path: &str,
        title: impl Into<String>,
        detail: impl Into<String>,
        outputs: usize,
        incoming: &[WorkflowGraphTail],
        address: Option<WorkflowStepAddress>,
    ) -> egui_graph::NodeId {
        let id = egui_graph::NodeId::new(path);
        for (input, tail) in incoming.iter().enumerate() {
            self.edges.push(WorkflowGraphEdge {
                from: tail.from,
                output: tail.output,
                to: id,
                input,
                insertion: tail.insertion.clone(),
            });
        }
        self.nodes.push(WorkflowGraphNode {
            id,
            title: title.into(),
            detail: detail.into(),
            inputs: incoming.len(),
            outputs,
            address,
        });
        id
    }
}
