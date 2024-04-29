use egui::{Pos2, Vec2};
use once_cell::sync::Lazy;

pub const BUILT_IN: Lazy<Vec<Template>> = Lazy::new(|| {
    vec![
        // 12x8 Single
        Template {
            name: "12x8 Single".to_string(),
            aspect_ratio: 12.0 / 8.0,
            regions: vec![TemplateRegion {
                relative_position: Pos2::new(0.0, 0.0),
                relative_size: Vec2::new(1.0, 1.0),
                kind: TemplateRegionKind::Image,
            }],
        },
        // 12x8 Split
        Template {
            name: "12x8 Split".to_string(),
            aspect_ratio: 12.0 / 8.0,
            regions: vec![
                TemplateRegion {
                    relative_position: Pos2::new(0.05, 1.0 - (0.6 * 1.5) / 2.0),
                    relative_size: Vec2::new(0.4, 0.6 * 1.5),
                    kind: TemplateRegionKind::Image,
                },
                TemplateRegion {
                    relative_position: Pos2::new(0.55, 0.1),
                    relative_size: Vec2::new(0.4, 0.1),
                    kind: TemplateRegionKind::Text {
                        sample_text: "Title".to_string(),
                    },
                },
                TemplateRegion {
                    relative_position: Pos2::new(0.55, 0.2),
                    relative_size: Vec2::new(0.4, 0.7),
                    kind: TemplateRegionKind::Text {
                        sample_text: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Integer tempor libero eros, vel scelerisque quam fringilla et. Mauris libero augue, tempus vel eros ut, semper finibus arcu. Pellentesque pellentesque sit amet augue a laoreet. Integer eget feugiat ex, vel efficitur ante. Nullam sed mi imperdiet turpis varius scelerisque id eu dolor. Nulla sollicitudin vehicula interdum. Nunc diam libero, ullamcorper at feugiat eget, dapibus in ante.".to_string(),
                    },
                },
            ],
        },
        // 12x8 Double
        Template {
            name: "12x8 Double".to_string(),
            aspect_ratio: 12.0 / 8.0,
            regions: vec![
                TemplateRegion {
                    relative_position: Pos2::new(0.0, 0.0),
                    relative_size: Vec2::new(0.5, 1.0),
                    kind: TemplateRegionKind::Image,
                },
                TemplateRegion {
                    relative_position: Pos2::new(0.5, 0.0),
                    relative_size: Vec2::new(0.5, 1.0),
                    kind: TemplateRegionKind::Image,
                },
            ],
        },
        // 12x8 Triple
        Template {
            name: "12x8 Triple".to_string(),
            aspect_ratio: 12.0 / 8.0,
            regions: vec![
                TemplateRegion {
                    relative_position: Pos2::new(0.0, 0.0),
                    relative_size: Vec2::new(0.333, 1.0),
                    kind: TemplateRegionKind::Image,
                },
                TemplateRegion {
                    relative_position: Pos2::new(0.333, 0.0),
                    relative_size: Vec2::new(0.333, 1.0),
                    kind: TemplateRegionKind::Image,
                },
                TemplateRegion {
                    relative_position: Pos2::new(0.666, 0.0),
                    relative_size: Vec2::new(0.333, 1.0),
                    kind: TemplateRegionKind::Image,
                },
            ],
        },
    ]
});

#[derive(Debug, PartialEq, Clone)]
pub struct Template {
    pub name: String,
    pub aspect_ratio: f32,
    pub regions: Vec<TemplateRegion>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TemplateRegion {
    pub relative_position: Pos2,
    pub relative_size: Vec2,
    pub kind: TemplateRegionKind,
}

#[derive(Debug, PartialEq, Clone)]
pub enum TemplateRegionKind {
    Image,
    Text { sample_text: String },
}
