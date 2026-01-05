use crate::{
    id::LayerId,
    utils::IdExt,
    widget::{
        canvas_info::layers::Layer,
        transformable::{TransformHandleMode, TransformableState},
    },
};
use eframe::{
    egui::Id,
    epaint::{Rect, Vec2},
};
use indexmap::IndexMap;

#[derive(Debug, Clone, PartialEq)]
pub struct MultiSelect {
    pub transformable_state: TransformableState,
    pub selected_layers: Vec<MultiSelectChild>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MultiSelectChild {
    pub transformable_state: TransformableState,
    pub id: LayerId,
}

impl MultiSelect {
    pub fn new(layers: &IndexMap<LayerId, Layer>) -> Self {
        let selected_ids = layers
            .iter()
            .filter(|(_, layer)| layer.selected)
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();

        let rect = Self::compute_rect(layers, &selected_ids);
        let transformable_state = TransformableState {
            rect,
            active_handle: None,
            is_moving: false,
            handle_mode: TransformHandleMode::default(),
            rotation: 0.0,
            last_frame_rotation: 0.0,
            change_in_rotation: None,
            id: Id::random(),
        };

        let res = Self {
            transformable_state: transformable_state.clone(),
            selected_layers: layers
                .iter()
                .filter(|(_, layer)| layer.selected)
                .map(|(id, transform)| MultiSelectChild {
                    transformable_state: transform
                        .transform_state
                        .to_local_space(&transformable_state),
                    id: *id,
                })
                .collect(),
        };
        res
    }

    pub fn update_selected<'a>(&'a mut self, layers: &'a IndexMap<LayerId, Layer>) {
        let selected_layer_ids = layers
            .iter()
            .filter(|(_, layer)| layer.selected)
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();

        let added_layers = layers
            .iter()
            .filter(|(_, layer)| layer.selected)
            .filter(|(layer_id, _)| {
                !self
                    .selected_layers
                    .iter()
                    .any(|child| child.id == **layer_id)
            })
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();

        let removed_layers = self
            .selected_layers
            .iter()
            .filter(|child| !selected_layer_ids.iter().any(|id| child.id == *id))
            .map(|child| child.id)
            .collect::<Vec<_>>();

        for layer_id in removed_layers {
            self.selected_layers.retain(|child| child.id != layer_id);
        }

        let joined_selected_ids: Vec<usize> = selected_layer_ids
            .iter()
            .chain(self.selected_layers.iter().map(|child| &child.id))
            .copied()
            .collect();

        let new_rect = Self::compute_rect(layers, &joined_selected_ids);

        self.transformable_state.rect = new_rect;

        for layer in added_layers {
            self.selected_layers.push(MultiSelectChild {
                transformable_state: layers
                    .get(&layer)
                    .unwrap()
                    .transform_state
                    .to_local_space(&self.transformable_state),
                id: layer,
            });
        }
    }

    fn compute_rect(layers: &IndexMap<LayerId, Layer>, selected_layers: &[usize]) -> Rect {
        let mut min = Vec2::splat(std::f32::MAX);
        let mut max = Vec2::splat(std::f32::MIN);

        for layer_id in selected_layers {
            let layer = &layers.get(layer_id).unwrap();

            // TODO: we should be rotating the rect here as well, but that messes things up
            let rect = layer.transform_state.rect;

            min.x = min.x.min(rect.min.x);
            min.y = min.y.min(rect.min.y);

            max.x = max.x.max(rect.max.x);
            max.y = max.y.max(rect.max.y);
        }

        Rect::from_min_max(min.to_pos2(), max.to_pos2())
    }
}
