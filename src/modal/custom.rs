use std::marker::PhantomData;

use exif::Tag;
use parking_lot::Mutex;

use crate::modal::{Modal, ModalActionResponse};

pub struct CustomModal<Tag: Send + 'static> {
    pub title: String,
    pub body: Box<dyn Fn(&mut egui::Ui) + Send + Sync + 'static>,
    pub actions: Vec<Box<dyn Fn(&mut egui::Ui) -> ModalActionResponse + Send + Sync + 'static>>,
    pub footer: Option<Box<dyn Fn(&mut egui::Ui) + Send + Sync + 'static>>,
    phantom: PhantomData<Tag>,
}

impl<Tag: Send + 'static> CustomModal<Tag> {
    pub fn new<BodyFn, ActionFn, FooterFn>(
        title: String,
        body: BodyFn,
        actions: Vec<ActionFn>,
        footer: Option<FooterFn>,
    ) -> Self
    where
        BodyFn: Fn(&mut egui::Ui) + Send + Sync + 'static,
        ActionFn: Fn(&mut egui::Ui) -> ModalActionResponse + Send + Sync + 'static,
        FooterFn: Fn(&mut egui::Ui) + Send + Sync + 'static,
    {
        Self {
            title,
            body: Box::new(body),
            actions: actions
                .into_iter()
                .map(|f| {
                    Box::new(f)
                        as Box<dyn Fn(&mut egui::Ui) -> ModalActionResponse + Send + Sync + 'static>
                })
                .collect(),
            footer: footer
                .map(|f| Box::new(f) as Box<dyn Fn(&mut egui::Ui) + Send + Sync + 'static>),
            phantom: PhantomData,
        }
    }
}

impl<Tag: Send + 'static> Modal for CustomModal<Tag> {
    fn title(&self) -> String {
        self.title.clone()
    }

    fn body_ui(&mut self, ui: &mut egui::Ui) {
        (self.body)(ui);
    }

    fn actions_ui(&mut self, ui: &mut egui::Ui) -> ModalActionResponse {
        let mut response = ModalActionResponse::None;
        for action in &self.actions {
            let action_response = action(ui);
            if !matches!(action_response, super::ModalActionResponse::None) {
                response = action_response;
            }
        }

        if let Some(footer) = &self.footer {
            footer(ui);
        }

        response
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
