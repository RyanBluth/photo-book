pub mod grid_layout;
pub mod stack_layout;
pub mod template;

use eframe::egui::{Pos2, Rect, Vec2};
use indexmap::IndexMap;

use self::{
    grid_layout::{GridDistribution, GridLayout},
    stack_layout::{
        StackCrossAxisAlignment, StackLayout, StackLayoutDirection, StackLayoutDistribution,
    },
};

use crate::widget::canvas::CanvasState;

#[derive(Debug, Clone)]
pub struct LayoutItem {
    pub aspect_ratio: f32,
    pub id: usize,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Margin {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Margin {
    pub fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    pub fn none() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SplitChild {
    pub weight: f32,
    pub node: LayoutNode,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LayoutNode {
    Split {
        direction: SplitDirection,
        children: Vec<SplitChild>,
    },
    Leaf {
        layout: LeafLayout,
        photo_count: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
pub enum LeafLayout {
    Grid,
    CenteredWeightedGrid,
    VerticalStack,
    HorizontalStack,
}

impl LayoutNode {
    pub fn leaf(layout: LeafLayout, photo_count: usize) -> Self {
        LayoutNode::Leaf {
            layout,
            photo_count,
        }
    }

    pub fn hsplit(children: Vec<(f32, LayoutNode)>) -> Self {
        LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            children: children
                .into_iter()
                .map(|(weight, node)| SplitChild { weight, node })
                .collect(),
        }
    }

    pub fn vsplit(children: Vec<(f32, LayoutNode)>) -> Self {
        LayoutNode::Split {
            direction: SplitDirection::Vertical,
            children: children
                .into_iter()
                .map(|(weight, node)| SplitChild { weight, node })
                .collect(),
        }
    }

    pub fn total_photo_count(&self) -> usize {
        match self {
            LayoutNode::Leaf { photo_count, .. } => *photo_count,
            LayoutNode::Split { children, .. } => {
                children.iter().map(|c| c.node.total_photo_count()).sum()
            }
        }
    }

    pub fn all_leaves_valid(&self) -> bool {
        match self {
            LayoutNode::Leaf { photo_count, .. } => *photo_count > 0,
            LayoutNode::Split { children, .. } => {
                children.iter().all(|c| c.node.all_leaves_valid())
            }
        }
    }

    pub fn apply(
        &self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        gap: f32,
        items: &mut &[LayoutItem],
    ) -> IndexMap<usize, Rect> {
        match self {
            LayoutNode::Split {
                direction,
                children,
            } => {
                let total_weight: f32 = children.iter().map(|c| c.weight).sum();
                if total_weight <= 0.0
                    || !total_weight.is_finite()
                    || children
                        .iter()
                        .any(|child| child.weight <= 0.0 || !child.weight.is_finite())
                {
                    let skipped = self.total_photo_count().min(items.len());
                    *items = &items[skipped..];
                    return IndexMap::new();
                }

                let total_gaps = gap * (children.len().saturating_sub(1)) as f32;
                let is_horizontal = *direction == SplitDirection::Horizontal;

                let available = if is_horizontal {
                    width - total_gaps
                } else {
                    height - total_gaps
                }
                .max(0.0);

                let mut results = IndexMap::new();
                let mut pos = if is_horizontal { x } else { y };

                for child in children {
                    let size = available * (child.weight / total_weight);
                    let (cx, cy, cw, ch) = if is_horizontal {
                        (pos, y, size, height)
                    } else {
                        (x, pos, width, size)
                    };
                    results.extend(child.node.apply(cx, cy, cw, ch, gap, items));
                    pos += size + gap;
                }

                results
            }
            LayoutNode::Leaf {
                layout,
                photo_count,
            } => {
                let count = (*photo_count).min(items.len());
                let (leaf_items, rest) = items.split_at(count);
                *items = rest;

                // Choose grid direction based on region shape:
                // tall regions use horizontal rows, wide regions use vertical columns
                let grid_direction = if height > width {
                    StackLayoutDirection::Horizontal
                } else {
                    StackLayoutDirection::Vertical
                };

                match layout {
                    LeafLayout::Grid => GridLayout::new(width, height, gap, 0.0, grid_direction)
                        .with_x(x)
                        .with_y(y)
                        .layout(leaf_items),
                    LeafLayout::CenteredWeightedGrid => {
                        GridLayout::new(width, height, gap, 0.0, grid_direction)
                            .with_distribution(GridDistribution::CenterWeighted)
                            .with_x(x)
                            .with_y(y)
                            .layout(leaf_items)
                    }
                    LeafLayout::VerticalStack => StackLayout {
                        width,
                        height,
                        gap,
                        margin: Margin::none(),
                        direction: StackLayoutDirection::Vertical,
                        alignment: StackCrossAxisAlignment::Center,
                        distribution: StackLayoutDistribution::Center,
                        x,
                        y,
                    }
                    .layout(leaf_items),
                    LeafLayout::HorizontalStack => StackLayout {
                        width,
                        height,
                        gap,
                        margin: Margin::none(),
                        direction: StackLayoutDirection::Horizontal,
                        alignment: StackCrossAxisAlignment::Center,
                        distribution: StackLayoutDistribution::Center,
                        x,
                        y,
                    }
                    .layout(leaf_items),
                }
            }
        }
    }
}

pub(crate) fn fit_aspect_ratio_in_rect(aspect_ratio: f32, rect: Rect) -> Rect {
    if aspect_ratio <= 0.0
        || !aspect_ratio.is_finite()
        || rect.width() <= 0.0
        || rect.height() <= 0.0
    {
        return Rect::from_center_size(rect.center(), Vec2::ZERO);
    }

    let rect_aspect_ratio = rect.width() / rect.height();
    let size = if aspect_ratio > rect_aspect_ratio {
        Vec2::new(rect.width(), rect.width() / aspect_ratio)
    } else {
        Vec2::new(rect.height() * aspect_ratio, rect.height())
    };

    Rect::from_min_size(
        Pos2::new(
            rect.center().x - size.x / 2.0,
            rect.center().y - size.y / 2.0,
        ),
        size,
    )
}

pub fn apply_layout_node(node: &LayoutNode, canvas_state: &mut CanvasState, gap: f32, margin: f32) {
    let page_size = canvas_state.page.value.size_pixels();
    let items = canvas_state.quick_layout_items();
    let mut slice = &items[..];
    let regions = node.apply(
        margin,
        margin,
        page_size.x - 2.0 * margin,
        page_size.y - 2.0 * margin,
        gap,
        &mut slice,
    );
    for layer_id in canvas_state.quick_layout_order.iter() {
        if let Some(rect) = regions.get(layer_id)
            && let Some(layer) = canvas_state.layers.get_mut(layer_id)
            && CanvasState::is_layer_editable(layer)
        {
            layer.transform_state.rect = *rect;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use indexmap::indexmap;

    use super::*;
    use crate::{
        model::{edit_state::EditablePage, page::Page},
        photo::{MetadataCollection, Photo, PhotoMetadata, PhotoMetadataField},
        widget::canvas_info::layers::Layer,
    };

    fn photo_layer(name: &str) -> Layer {
        let mut fields = MetadataCollection::new();
        fields.insert(PhotoMetadataField::Width(400));
        fields.insert(PhotoMetadataField::Height(300));
        fields.insert(PhotoMetadataField::RotatedWidth(400));
        fields.insert(PhotoMetadataField::RotatedHeight(300));
        Layer::with_photo(Photo::with_metadata(
            PathBuf::from(name),
            PhotoMetadata { fields },
        ))
    }

    #[test]
    fn quick_layout_does_not_move_locked_layers() {
        let mut locked = photo_layer("locked.jpg");
        locked.locked = true;
        locked.transform_state.rect =
            Rect::from_min_size(Pos2::new(17.0, 23.0), Vec2::new(111.0, 79.0));
        let locked_id = locked.id;
        let locked_rect = locked.transform_state.rect;

        let mut editable = photo_layer("editable.jpg");
        editable.transform_state.rect =
            Rect::from_min_size(Pos2::new(31.0, 37.0), Vec2::new(91.0, 61.0));
        let editable_id = editable.id;
        let editable_rect = editable.transform_state.rect;

        let mut canvas_state = CanvasState::with_layers(
            indexmap! { locked_id => locked, editable_id => editable },
            EditablePage::new(Page::default()),
            None,
            vec![locked_id, editable_id],
        );

        apply_layout_node(
            &LayoutNode::leaf(LeafLayout::HorizontalStack, 2),
            &mut canvas_state,
            10.0,
            20.0,
        );

        assert_eq!(
            canvas_state.layers[&locked_id].transform_state.rect,
            locked_rect
        );
        assert_ne!(
            canvas_state.layers[&editable_id].transform_state.rect,
            editable_rect
        );
    }
}
