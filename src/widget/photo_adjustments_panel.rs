use egui::Ui;

use crate::{
    dep_mut,
    histogram_manager::{HistogramLoadResult, HistogramManager},
    model::photo_adjustments::PhotoAdjustments,
    photo::Photo,
    widget::{
        edit_response::EditResponse,
        histogram::Histogram,
        photo_adjustments::{PhotoAdjustmentsEditor, PhotoAdjustmentsState},
    },
};

pub struct PhotoAdjustmentsPanel<'a> {
    photo: &'a Photo,
    adjustments: &'a mut PhotoAdjustments,
    state: &'a mut PhotoAdjustmentsState,
}

impl<'a> PhotoAdjustmentsPanel<'a> {
    pub fn new(
        photo: &'a Photo,
        adjustments: &'a mut PhotoAdjustments,
        state: &'a mut PhotoAdjustmentsState,
    ) -> Self {
        Self {
            photo,
            adjustments,
            state,
        }
    }

    pub fn show(self, ui: &mut Ui) -> EditResponse {
        let Self {
            photo,
            adjustments,
            state,
        } = self;

        Self::show_histogram(ui, photo, adjustments);
        ui.add_space(10.0);

        let original_adjustments = PhotoAdjustments::default();
        let histogram = dep_mut!(HistogramManager, |manager| {
            manager
                .get(&photo.path, &original_adjustments, true)
                .ready_data()
                .cloned()
        });

        PhotoAdjustmentsEditor::new(adjustments, state)
            .histogram(histogram.as_ref())
            .show(ui)
    }

    fn show_histogram(ui: &mut Ui, photo: &Photo, adjustments: &PhotoAdjustments) {
        let refresh_adjusted = !ui.input(|input| input.pointer.primary_down());
        let histogram = dep_mut!(HistogramManager, |manager| manager.get(
            &photo.path,
            adjustments,
            refresh_adjusted,
        ));

        match &histogram {
            HistogramLoadResult::Ready(data) => {
                ui.add(Histogram::new(data).height(82.0));
            }
            HistogramLoadResult::Pending(Some(data)) => {
                ui.add(Histogram::new(data).height(82.0).loading(true));
            }
            HistogramLoadResult::Unavailable(error, Some(data)) => {
                ui.add(Histogram::new(data).height(82.0))
                    .on_hover_text(error);
            }
            HistogramLoadResult::Pending(None) => {
                ui.add(Histogram::unavailable().height(82.0).loading(true));
            }
            HistogramLoadResult::Unavailable(error, None) => {
                ui.add(Histogram::unavailable().height(82.0))
                    .on_hover_text(error);
            }
        }
    }
}
