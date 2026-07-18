use crate::{
    id::LayerId,
    utils::RectExt,
    widget::{canvas_info::layers::Layer, transformable::TransformableState},
};
use eframe::{
    emath::Rot2,
    epaint::{Pos2, Rect},
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
        let selected_ids = Self::selected_layer_ids(layers);
        let rect = Self::compute_rect(layers, &selected_ids);
        let transformable_state = TransformableState::new(rect);
        let selected_layers = Self::selected_children(layers, &transformable_state);

        Self {
            transformable_state,
            selected_layers,
        }
    }

    pub fn update_selected(&mut self, layers: &IndexMap<LayerId, Layer>) {
        let selected_layer_ids = Self::selected_layer_ids(layers);

        let selection_changed = !selected_layer_ids
            .iter()
            .copied()
            .eq(self.selected_layers.iter().map(|child| child.id));

        if !selection_changed {
            return;
        }

        // A changed selection starts a new, unrotated group around the visual bounds of
        // every child. Keeping the previous group rotation would rotate these already
        // axis-aligned bounds a second time.
        let rect = Self::compute_rect(layers, &selected_layer_ids);
        self.transformable_state = TransformableState::new(rect);

        self.selected_layers = Self::selected_children(layers, &self.transformable_state);
    }

    fn selected_layer_ids(layers: &IndexMap<LayerId, Layer>) -> Vec<LayerId> {
        layers
            .iter()
            .filter(|(_, layer)| layer.selected)
            .map(|(id, _)| *id)
            .collect()
    }

    fn selected_children(
        layers: &IndexMap<LayerId, Layer>,
        parent: &TransformableState,
    ) -> Vec<MultiSelectChild> {
        layers
            .iter()
            .filter(|(_, layer)| layer.selected)
            .map(|(id, layer)| MultiSelectChild {
                transformable_state: layer.transform_state.to_local_space(parent),
                id: *id,
            })
            .collect()
    }

    fn compute_rect(layers: &IndexMap<LayerId, Layer>, selected_layers: &[LayerId]) -> Rect {
        selected_layers.iter().fold(Rect::NOTHING, |bounds, id| {
            let state = &layers.get(id).unwrap().transform_state;
            bounds.union(state.rect.rotate_bb_around_center(state.rotation))
        })
    }

    pub(super) fn transform_child(
        child: &mut TransformableState,
        previous_group_rect: Rect,
        group: &TransformableState,
    ) {
        if previous_group_rect != group.rect {
            let previous_child_rect = child.rect;
            let remap = |value, old_min, old_size, new_min, new_size| {
                if old_size == 0.0 {
                    value + new_min - old_min
                } else {
                    new_min + (value - old_min) / old_size * new_size
                }
            };

            child.rect = Rect::from_min_max(
                Pos2::new(
                    remap(
                        previous_child_rect.left(),
                        previous_group_rect.left(),
                        previous_group_rect.width(),
                        group.rect.left(),
                        group.rect.width(),
                    ),
                    remap(
                        previous_child_rect.top(),
                        previous_group_rect.top(),
                        previous_group_rect.height(),
                        group.rect.top(),
                        group.rect.height(),
                    ),
                ),
                Pos2::new(
                    remap(
                        previous_child_rect.right(),
                        previous_group_rect.left(),
                        previous_group_rect.width(),
                        group.rect.left(),
                        group.rect.width(),
                    ),
                    remap(
                        previous_child_rect.bottom(),
                        previous_group_rect.top(),
                        previous_group_rect.height(),
                        group.rect.top(),
                        group.rect.height(),
                    ),
                ),
            );
        }

        let rotation = group.rotation - group.last_frame_rotation;
        if rotation != 0.0 {
            let relative_center = child.rect.center() - group.rect.center();
            child
                .rect
                .set_center(group.rect.center() + Rot2::from_angle(rotation) * relative_center);
            child.rotation += rotation;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::{FRAC_PI_4, FRAC_PI_6};

    use eframe::epaint::Vec2;

    use super::*;

    fn selected_layer(rect: Rect, rotation: f32) -> Layer {
        let mut layer = Layer::new_rectangle_shape_layer();
        layer.selected = true;
        layer.transform_state.rect = rect;
        layer.transform_state.rotation = rotation;
        layer
    }

    #[test]
    fn group_bounds_enclose_rotated_children() {
        let first = selected_layer(
            Rect::from_min_max(Pos2::new(10.0, 20.0), Pos2::new(110.0, 60.0)),
            FRAC_PI_4,
        );
        let second = selected_layer(
            Rect::from_min_max(Pos2::new(140.0, 80.0), Pos2::new(180.0, 180.0)),
            -FRAC_PI_6,
        );
        let layers = IndexMap::from([(first.id, first), (second.id, second)]);

        let multi_select = MultiSelect::new(&layers);

        for layer in layers.values() {
            for corner in layer
                .transform_state
                .rect
                .rotated_corners(layer.transform_state.rotation)
            {
                assert!(multi_select.transformable_state.rect.contains(corner));
            }
        }
    }

    #[test]
    fn unchanged_selection_preserves_transformed_group_bounds() {
        let first = selected_layer(
            Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 40.0)),
            0.0,
        );
        let second = selected_layer(
            Rect::from_min_max(Pos2::new(120.0, 20.0), Pos2::new(180.0, 80.0)),
            0.0,
        );
        let layers = IndexMap::from([(first.id, first), (second.id, second)]);
        let mut multi_select = MultiSelect::new(&layers);
        let transformed_rect = multi_select
            .transformable_state
            .rect
            .translate(Vec2::splat(25.0));
        multi_select.transformable_state.rect = transformed_rect;
        multi_select.transformable_state.rotation = FRAC_PI_4;

        multi_select.update_selected(&layers);

        assert_eq!(multi_select.transformable_state.rect, transformed_rect);
        assert_eq!(multi_select.transformable_state.rotation, FRAC_PI_4);
    }

    #[test]
    fn rotating_group_preserves_child_size() {
        let group_rect = Rect::from_min_max(Pos2::ZERO, Pos2::new(200.0, 100.0));
        let mut group = TransformableState::new(group_rect);
        group.last_frame_rotation = FRAC_PI_6;
        group.rotation = FRAC_PI_4;

        let mut child = TransformableState::new(Rect::from_min_max(
            Pos2::new(-20.0, 10.0),
            Pos2::new(60.0, 50.0),
        ));
        let original_size = child.rect.size();
        let original_center = child.rect.center();
        let rotation_delta = group.rotation - group.last_frame_rotation;
        let expected_center = group.rect.center()
            + Rot2::from_angle(rotation_delta) * (original_center - group.rect.center());

        MultiSelect::transform_child(&mut child, group_rect, &group);

        assert_eq!(child.rect.size(), original_size);
        assert!(child.rect.center().distance(expected_center) < 0.001);
        assert!((child.rotation - rotation_delta).abs() < f32::EPSILON);
    }
}
