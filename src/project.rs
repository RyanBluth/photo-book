use std::{collections::HashSet, path::PathBuf};

use indexmap::IndexMap;
use savefile_derive::Savefile;

use thiserror::Error;

use crate::{
    dependencies::{Dependency, SingletonFor},
    id::{LayerId, PageId, next_page_id, set_min_layer_id},
    model::{
        edit_state::EditablePage, page::Page as AppPage,
        photo_grouping::PhotoGrouping as AppPhotoGrouping, scale_mode::ScaleMode as AppScaleMode,
        unit::Unit as AppUnit,
    },
    photo::{Photo as AppPhoto, PhotoRating as AppPhotoRating},
    photo_manager::PhotoManager,
    project_settings::{ProjectSettings as AppProjectSettings, ProjectSettingsManager},
    scene::{
        canvas_scene::{CanvasScene, CanvasSceneState},
        organize_edit_scene::OrganizeEditScene,
        organize_scene::GalleryScene,
    },
    template::{
        Template as AppTemplate, TemplateRegion as AppTemplateRegion,
        TemplateRegionKind as AppTemplateRegionKind,
    },
    utils::IdExt,
    widget::{
        canvas::{CanvasPhoto as AppCanvasPhoto, CanvasState},
        canvas_info::layers::{
            CanvasShape as AppCanvasShape, CanvasShapeEditState,
            CanvasShapeKind as AppCanvasShapeKind, CanvasText as AppCanvasText,
            CanvasTextEditState, Layer as AppLayer, LayerContent as AppLayerContent,
            LayerTransformEditState, TextHorizontalAlignment as AppTextHorizontalAlignment,
            TextVerticalAlignment as AppTextVerticalAlignment,
        },
        transformable::{ResizeMode, TransformHandleMode::Resize, TransformableState},
    },
};

pub const PROJECT_VERSION: u32 = 3;

#[derive(Error, Debug)]
pub enum ProjectError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serde error: {0}")]
    SerdeError(#[from] serde_json::Error),

    #[error("Savefile error: {0}")]
    SavefileError(#[from] savefile::SavefileError),
}

#[derive(Debug, Clone, Savefile)]
pub struct Project {
    pub photos: Vec<Photo>,
    pub pages: Vec<CanvasPage>,
    pub group_by: ProjectPhotoGrouping,
    pub project_settings: ProjectSettings,
}

impl Project {
    pub fn new(root_scene: &OrganizeEditScene) -> Project {
        let photos = Dependency::<PhotoManager>::get().with_lock(|photo_manager| {
            photo_manager
                .photo_database
                .get_all_photo_paths()
                .iter()
                .map(|path| Photo {
                    path: path.clone(),
                    rating: photo_manager.get_photo_rating(path).into(),
                    tags: photo_manager.get_photo_tags(path).into(),
                })
                .collect()
        });

        let mut app_pages = match &root_scene.edit {
            Some(edit) => edit.read().unwrap().state.pages_state.pages.clone(),
            None => IndexMap::new(),
        };

        let pages: Vec<CanvasPage> = app_pages
            .values_mut()
            .map(|canvas_state| {
                let layers = canvas_state
                    .layers
                    .values_mut()
                    .map(|layer| {
                        layer.transform_edit_state.update(&layer.transform_state);

                        Layer {
                            content: match layer.content.clone() {
                                AppLayerContent::Photo(canvas_photo) => {
                                    LayerContent::Photo(CanvasPhoto {
                                        photo: Photo {
                                            path: canvas_photo.photo.path.clone(),
                                            rating: Dependency::<PhotoManager>::get().with_lock(
                                                |photo_manager| {
                                                    photo_manager
                                                        .get_photo_rating(&canvas_photo.photo.path)
                                                        .into()
                                                },
                                            ),
                                            tags: Dependency::<PhotoManager>::get().with_lock(
                                                |photo_manager| {
                                                    photo_manager
                                                        .get_photo_tags(&canvas_photo.photo.path)
                                                        .into()
                                                },
                                            ),
                                        },
                                        crop: canvas_photo.crop.into(),
                                    })
                                }
                                AppLayerContent::Text(canvas_text) => {
                                    LayerContent::Text(CanvasText {
                                        text: canvas_text.text,
                                        font_size: canvas_text.font_size,
                                        font_id: canvas_text.font_id.into(),
                                        color: canvas_text.color.into(),
                                        horizontal_alignment: match canvas_text.horizontal_alignment
                                        {
                                            AppTextHorizontalAlignment::Left => {
                                                TextHorizontalAlignment::Left
                                            }
                                            AppTextHorizontalAlignment::Center => {
                                                TextHorizontalAlignment::Center
                                            }
                                            AppTextHorizontalAlignment::Right => {
                                                TextHorizontalAlignment::Right
                                            }
                                        },
                                        vertical_alignment: match canvas_text.vertical_alignment {
                                            AppTextVerticalAlignment::Top => {
                                                TextVerticalAlignment::Top
                                            }
                                            AppTextVerticalAlignment::Center => {
                                                TextVerticalAlignment::Center
                                            }
                                            AppTextVerticalAlignment::Bottom => {
                                                TextVerticalAlignment::Bottom
                                            }
                                        },
                                    })
                                }
                                AppLayerContent::TemplatePhoto {
                                    region,
                                    photo,
                                    scale_mode,
                                } => LayerContent::TemplatePhoto {
                                    region: TemplateRegion {
                                        relative_position: region.relative_position.into(),
                                        relative_size: region.relative_size.into(),
                                        kind: match region.kind {
                                            AppTemplateRegionKind::Image => {
                                                TemplateRegionKind::Image
                                            }
                                            AppTemplateRegionKind::Text {
                                                sample_text,
                                                font_size,
                                            } => TemplateRegionKind::Text {
                                                sample_text,
                                                font_size,
                                            },
                                        },
                                    },
                                    photo: photo.map(|canvas_photo| CanvasPhoto {
                                        photo: Photo {
                                            path: canvas_photo.photo.path.clone(),
                                            rating: Dependency::<PhotoManager>::get().with_lock(
                                                |photo_manager| {
                                                    photo_manager
                                                        .get_photo_rating(&canvas_photo.photo.path)
                                                        .into()
                                                },
                                            ),
                                            tags: Dependency::<PhotoManager>::get().with_lock(
                                                |photo_manager| {
                                                    photo_manager
                                                        .get_photo_tags(&canvas_photo.photo.path)
                                                        .into()
                                                },
                                            ),
                                        },
                                        crop: canvas_photo.crop.into(),
                                    }),
                                    scale_mode: match scale_mode {
                                        AppScaleMode::Fit => ScaleMode::Fit,
                                        AppScaleMode::Fill => ScaleMode::Fill,
                                        AppScaleMode::Stretch => ScaleMode::Stretch,
                                    },
                                },
                                AppLayerContent::TemplateText { region, text } => {
                                    LayerContent::TemplateText {
                                        region: TemplateRegion {
                                            relative_position: region.relative_position.into(),
                                            relative_size: region.relative_size.into(),
                                            kind: match region.kind {
                                                AppTemplateRegionKind::Image => {
                                                    TemplateRegionKind::Image
                                                }
                                                AppTemplateRegionKind::Text {
                                                    sample_text,
                                                    font_size,
                                                } => TemplateRegionKind::Text {
                                                    sample_text,
                                                    font_size,
                                                },
                                            },
                                        },
                                        text: CanvasText {
                                            text: text.text,
                                            font_size: text.font_size,
                                            font_id: text.font_id.into(),
                                            color: text.color.into(),
                                            horizontal_alignment: match text.horizontal_alignment {
                                                AppTextHorizontalAlignment::Left => {
                                                    TextHorizontalAlignment::Left
                                                }
                                                AppTextHorizontalAlignment::Center => {
                                                    TextHorizontalAlignment::Center
                                                }
                                                AppTextHorizontalAlignment::Right => {
                                                    TextHorizontalAlignment::Right
                                                }
                                            },
                                            vertical_alignment: match text.vertical_alignment {
                                                AppTextVerticalAlignment::Top => {
                                                    TextVerticalAlignment::Top
                                                }
                                                AppTextVerticalAlignment::Center => {
                                                    TextVerticalAlignment::Center
                                                }
                                                AppTextVerticalAlignment::Bottom => {
                                                    TextVerticalAlignment::Bottom
                                                }
                                            },
                                        },
                                    }
                                }
                                AppLayerContent::Shape(canvas_shape) => {
                                    LayerContent::Shape(CanvasShape {
                                        kind: match canvas_shape.kind {
                                            AppCanvasShapeKind::Rectangle { corner_radius } => {
                                                CanvasShapeKind::Rectangle { corner_radius }
                                            }
                                            AppCanvasShapeKind::Ellipse => CanvasShapeKind::Ellipse,
                                            AppCanvasShapeKind::Line { start, end } => {
                                                CanvasShapeKind::Line {
                                                    start: start.into(),
                                                    end: end.into(),
                                                }
                                            }
                                        },
                                        fill_color: canvas_shape.fill_color.into(),
                                        stroke: canvas_shape
                                            .stroke
                                            .map(|(stroke, kind)| (stroke.into(), kind.into())),
                                    })
                                }
                            },
                            name: layer.name.clone(),
                            visible: layer.visible,
                            locked: layer.locked,
                            selected: layer.selected,
                            id: layer.id,
                            rect: layer.transform_state.rect.into(),
                            rotation: layer.transform_state.rotation,
                        }
                    })
                    .collect();

                let template = canvas_state.template.clone();
                CanvasPage {
                    layers,
                    page: Page {
                        width: canvas_state.page.size().x,
                        height: canvas_state.page.size().y,
                        ppi: canvas_state.page.ppi(),
                        unit: match canvas_state.page.unit() {
                            AppUnit::Pixels => Unit::Pixels,
                            AppUnit::Inches => Unit::Inches,
                            AppUnit::Centimeters => Unit::Centimeters,
                        },
                    },
                    template: template.map(|template| Template {
                        name: template.name,
                        page: Page {
                            width: template.page.width(),
                            height: template.page.height(),
                            ppi: template.page.ppi(),
                            unit: match template.page.unit() {
                                AppUnit::Pixels => Unit::Pixels,
                                AppUnit::Inches => Unit::Inches,
                                AppUnit::Centimeters => Unit::Centimeters,
                            },
                        },
                        regions: template
                            .regions
                            .iter()
                            .map(|region| TemplateRegion {
                                relative_position: region.relative_position.into(),
                                relative_size: region.relative_size.into(),
                                kind: match &region.kind {
                                    AppTemplateRegionKind::Image => TemplateRegionKind::Image,
                                    AppTemplateRegionKind::Text {
                                        sample_text,
                                        font_size,
                                    } => TemplateRegionKind::Text {
                                        sample_text: sample_text.clone(),
                                        font_size: *font_size,
                                    },
                                },
                            })
                            .collect(),
                    }),
                    quick_layout_order: canvas_state.quick_layout_order.clone(),
                }
            })
            .collect();

        let group_by = Dependency::<PhotoManager>::get()
            .with_lock(|photo_manager| photo_manager.photo_grouping());

        let project_settings: AppProjectSettings = Dependency::<ProjectSettingsManager>::get()
            .with_lock(|settings| settings.project_settings.clone());

        let project = Project {
            photos,
            pages,
            group_by: group_by.into(),
            project_settings: project_settings.into(),
        };

        project
    }

    pub fn save(path: &PathBuf, root_scene: &OrganizeEditScene) -> Result<(), ProjectError> {
        let project = Project::new(root_scene);

        // Use savefile with compression to serialize the project
        match savefile::save_file_compressed(path, PROJECT_VERSION, &project) {
            Ok(_) => Ok(()),
            Err(e) => {
                println!("Error saving project: {:?}", e);
                Err(ProjectError::SavefileError(e))
            }
        }
    }

    pub fn load(path: &PathBuf) -> Result<OrganizeEditScene, ProjectError> {
        // Use savefile to deserialize the project
        // Note: This will load any version up to PROJECT_VERSION
        match savefile::load_file::<Project, _>(path, PROJECT_VERSION) {
            Ok(project) => {
                println!("Loaded project: {:?}", project);
                Ok(project.into())
            }
            Err(e) => {
                println!("Error loading project: {:?}", e);
                Err(ProjectError::SavefileError(e))
            }
        }
    }
}

impl Into<OrganizeEditScene> for Project {
    fn into(self) -> OrganizeEditScene {
        Dependency::<ProjectSettingsManager>::get().with_lock_mut(|settings| {
            settings.project_settings = self.project_settings.into();
        });

        let photos_with_metadata: Vec<(PathBuf, AppPhotoRating, HashSet<String>)> = self
            .photos
            .iter()
            .map(|photo| {
                (
                    photo.path.clone(),
                    photo.rating.clone().into(),
                    photo.tags.clone(),
                )
            })
            .collect();

        Dependency::<PhotoManager>::get().with_lock(|photo_manager| {
            photo_manager.load_photos(
                self.photos
                    .into_iter()
                    .map(|photo| (photo.path, None))
                    .collect(),
            );
        });

        Dependency::<PhotoManager>::get().with_lock_mut(|photo_manager| {
            for (path, rating, tags) in photos_with_metadata {
                photo_manager.set_photo_rating(&path, rating);
                photo_manager.set_photo_tags(&path, tags);
            }
        });

        let pages: IndexMap<PageId, CanvasState> = self
            .pages
            .into_iter()
            .map(|page| {
                let layers: IndexMap<LayerId, AppLayer> = page
                    .layers
                    .into_iter()
                    .map(|layer| {
                        let transformable_state = TransformableState {
                            rect: layer.rect.into(),
                            active_handle: None,
                            is_moving: false,
                            handle_mode: Resize(ResizeMode::Free),
                            rotation: layer.rotation,
                            last_frame_rotation: layer.rotation,
                            change_in_rotation: None,
                            id: egui::Id::random(),
                        };

                        let layer = AppLayer {
                            content: match layer.content {
                                LayerContent::Photo(photo) => {
                                    // TODO: Don't unwrap
                                    AppLayerContent::Photo(AppCanvasPhoto {
                                        photo: AppPhoto::new(photo.photo.path).unwrap(),
                                        crop: photo.crop.into(),
                                    })
                                }
                                LayerContent::Text(text) => AppLayerContent::Text(AppCanvasText {
                                    text: text.text,
                                    font_size: text.font_size,
                                    font_id: text.font_id.into(),
                                    color: text.color.into(),
                                    edit_state: CanvasTextEditState::new(text.font_size),
                                    horizontal_alignment: match text.horizontal_alignment {
                                        TextHorizontalAlignment::Left => {
                                            AppTextHorizontalAlignment::Left
                                        }
                                        TextHorizontalAlignment::Center => {
                                            AppTextHorizontalAlignment::Center
                                        }
                                        TextHorizontalAlignment::Right => {
                                            AppTextHorizontalAlignment::Right
                                        }
                                    },
                                    vertical_alignment: match text.vertical_alignment {
                                        TextVerticalAlignment::Top => AppTextVerticalAlignment::Top,
                                        TextVerticalAlignment::Center => {
                                            AppTextVerticalAlignment::Center
                                        }
                                        TextVerticalAlignment::Bottom => {
                                            AppTextVerticalAlignment::Bottom
                                        }
                                    },
                                }),
                                LayerContent::TemplatePhoto {
                                    region,
                                    photo,
                                    scale_mode,
                                } => AppLayerContent::TemplatePhoto {
                                    region: AppTemplateRegion {
                                        relative_position: region.relative_position.into(),
                                        relative_size: region.relative_size.into(),
                                        kind: match region.kind {
                                            TemplateRegionKind::Image => {
                                                AppTemplateRegionKind::Image
                                            }
                                            TemplateRegionKind::Text {
                                                sample_text,
                                                font_size,
                                            } => AppTemplateRegionKind::Text {
                                                sample_text,
                                                font_size,
                                            },
                                        },
                                    },
                                    photo: photo.map(|photo| AppCanvasPhoto {
                                        photo: AppPhoto::new(photo.photo.path).unwrap(), // TODO: Don't unwrap
                                        crop: photo.crop.into(),
                                    }),
                                    scale_mode: match scale_mode {
                                        ScaleMode::Fit => AppScaleMode::Fit,
                                        ScaleMode::Fill => AppScaleMode::Fill,
                                        ScaleMode::Stretch => AppScaleMode::Stretch,
                                    },
                                },
                                LayerContent::TemplateText { region, text } => {
                                    AppLayerContent::TemplateText {
                                        region: AppTemplateRegion {
                                            relative_position: region.relative_position.into(),
                                            relative_size: region.relative_size.into(),
                                            kind: match region.kind {
                                                TemplateRegionKind::Image => {
                                                    AppTemplateRegionKind::Image
                                                }
                                                TemplateRegionKind::Text {
                                                    sample_text,
                                                    font_size,
                                                } => AppTemplateRegionKind::Text {
                                                    sample_text,
                                                    font_size,
                                                },
                                            },
                                        },
                                        text: AppCanvasText {
                                            text: text.text,
                                            font_size: text.font_size,
                                            font_id: text.font_id.into(),
                                            color: text.color.into(),
                                            edit_state: CanvasTextEditState::new(text.font_size),
                                            horizontal_alignment: match text.horizontal_alignment {
                                                TextHorizontalAlignment::Left => {
                                                    AppTextHorizontalAlignment::Left
                                                }
                                                TextHorizontalAlignment::Center => {
                                                    AppTextHorizontalAlignment::Center
                                                }
                                                TextHorizontalAlignment::Right => {
                                                    AppTextHorizontalAlignment::Right
                                                }
                                            },
                                            vertical_alignment: match text.vertical_alignment {
                                                TextVerticalAlignment::Top => {
                                                    AppTextVerticalAlignment::Top
                                                }
                                                TextVerticalAlignment::Center => {
                                                    AppTextVerticalAlignment::Center
                                                }
                                                TextVerticalAlignment::Bottom => {
                                                    AppTextVerticalAlignment::Bottom
                                                }
                                            },
                                        },
                                    }
                                }
                                LayerContent::Shape(canvas_shape) => {
                                    AppLayerContent::Shape(AppCanvasShape {
                                        kind: match canvas_shape.kind {
                                            CanvasShapeKind::Rectangle { corner_radius } => {
                                                AppCanvasShapeKind::Rectangle { corner_radius }
                                            }
                                            CanvasShapeKind::Ellipse => AppCanvasShapeKind::Ellipse,
                                            CanvasShapeKind::Line { start, end } => {
                                                AppCanvasShapeKind::Line {
                                                    start: start.into(),
                                                    end: end.into(),
                                                }
                                            }
                                        },
                                        fill_color: canvas_shape.fill_color.into(),
                                        stroke: canvas_shape.stroke.as_ref().map(
                                            |(stroke, kind)| {
                                                (stroke.clone().into(), kind.clone().into())
                                            },
                                        ),
                                        edit_state: CanvasShapeEditState::new(
                                            canvas_shape
                                                .stroke
                                                .as_ref()
                                                .map(|(stroke, _)| stroke.width)
                                                .unwrap_or(1.0),
                                        ),
                                    })
                                }
                            },
                            name: layer.name,
                            visible: layer.visible,
                            locked: layer.locked,
                            selected: layer.selected,
                            id: layer.id,
                            transform_edit_state: LayerTransformEditState::from(
                                &transformable_state,
                            ),
                            transform_state: transformable_state,
                        };

                        set_min_layer_id(layer.id);

                        (layer.id, layer)
                    })
                    .collect();

                let canvas_state = CanvasState::with_layers(
                    layers,
                    EditablePage::new(AppPage::new(
                        page.page.width,
                        page.page.height,
                        page.page.ppi,
                        match page.page.unit {
                            Unit::Pixels => AppUnit::Pixels,
                            Unit::Inches => AppUnit::Inches,
                            Unit::Centimeters => AppUnit::Centimeters,
                        },
                    )),
                    page.template.map(|template| AppTemplate {
                        name: template.name,
                        page: AppPage::new(
                            template.page.width,
                            template.page.height,
                            template.page.ppi,
                            match template.page.unit {
                                Unit::Pixels => AppUnit::Pixels,
                                Unit::Inches => AppUnit::Inches,
                                Unit::Centimeters => AppUnit::Centimeters,
                            },
                        ),
                        regions: template
                            .regions
                            .iter()
                            .map(|region| AppTemplateRegion {
                                relative_position: region.relative_position.clone().into(),
                                relative_size: region.relative_size.clone().into(),
                                kind: match &region.kind {
                                    TemplateRegionKind::Image => AppTemplateRegionKind::Image,
                                    TemplateRegionKind::Text {
                                        sample_text,
                                        font_size,
                                    } => AppTemplateRegionKind::Text {
                                        sample_text: sample_text.clone(),
                                        font_size: *font_size,
                                    },
                                },
                            })
                            .collect(),
                    }),
                    page.quick_layout_order,
                );

                (next_page_id(), canvas_state)
            })
            .collect();

        let edit_scene = match pages.first().map(|(id, _)| *id) {
            Some(first_page_id) => Some(CanvasScene::with_state(CanvasSceneState::with_pages(
                pages,
                first_page_id,
            ))),
            _ => None,
        };

        let organize_scene = GalleryScene::new();

        let organize_edit_scene = OrganizeEditScene::new(organize_scene, edit_scene);

        //photo_manager.group_photos_by(project.group_by.into());

        organize_edit_scene
    }
}

#[derive(Debug, Clone, Savefile)]
pub struct CanvasPage {
    pub layers: Vec<Layer>,
    pub page: Page,
    pub template: Option<Template>,
    pub quick_layout_order: Vec<LayerId>,
}

#[derive(Debug, Clone, Savefile)]
pub struct Template {
    pub name: String,
    pub page: Page,
    pub regions: Vec<TemplateRegion>,
}

#[derive(Debug, Clone, Savefile)]
pub struct Page {
    pub width: f32,
    pub height: f32,
    pub ppi: i32,
    pub unit: Unit,
}

#[derive(Debug, Clone, PartialEq, Copy, Savefile)]
pub enum Unit {
    Pixels,
    Inches,
    Centimeters,
}

#[derive(Debug, Clone, Savefile)]
pub struct Photo {
    pub path: PathBuf,
    pub rating: PhotoRating,
    #[savefile_versions = "2.."]
    pub tags: HashSet<String>,
}

#[derive(Debug, Clone, Savefile)]
pub struct Layer {
    pub content: LayerContent,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub selected: bool,
    pub id: LayerId,
    pub rect: Rect,
    pub rotation: f32,
}

#[derive(Debug, Clone, Savefile)]
pub enum ScaleMode {
    Fit,
    Fill,
    Stretch,
}

#[derive(Debug, Clone, Savefile)]
pub struct TemplateRegion {
    pub relative_position: Pos2,
    pub relative_size: Vec2,
    pub kind: TemplateRegionKind,
}

#[derive(Debug, PartialEq, Clone, Savefile)]
pub enum TemplateRegionKind {
    Image,
    Text { sample_text: String, font_size: f32 },
}

#[derive(Debug, Clone, Savefile)]
pub struct CanvasPhoto {
    pub photo: Photo,
    pub crop: Rect,
}

#[derive(Debug, Clone, Savefile)]
pub struct CanvasText {
    pub text: String,
    pub font_size: f32,
    pub font_id: FontId,
    pub color: Color32,
    pub horizontal_alignment: TextHorizontalAlignment,
    pub vertical_alignment: TextVerticalAlignment,
}

#[derive(Debug, Clone, Savefile)]
pub struct CanvasShape {
    pub kind: CanvasShapeKind,
    pub fill_color: Color32,
    pub stroke: Option<(Stroke, StrokeKind)>,
}

#[derive(Debug, Clone, Savefile)]
pub enum CanvasShapeKind {
    Rectangle { corner_radius: f32 },
    Ellipse,
    Line { start: Pos2, end: Pos2 },
}

#[derive(Debug, Clone, Savefile)]
pub enum LayerContent {
    Photo(CanvasPhoto),
    Text(CanvasText),
    TemplatePhoto {
        region: TemplateRegion,
        photo: Option<CanvasPhoto>,
        scale_mode: ScaleMode,
    },
    TemplateText {
        region: TemplateRegion,
        text: CanvasText,
    },
    #[savefile_versions = "3.."]
    Shape(CanvasShape),
}

#[derive(Debug, Clone, Copy, Savefile)]
pub enum TextHorizontalAlignment {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, Savefile)]
pub enum TextVerticalAlignment {
    Top,
    Center,
    Bottom,
}

#[derive(Debug, Clone, Savefile)]
pub enum PhotoRating {
    Yes,
    No,
    Maybe,
}

#[derive(Debug, Clone, Savefile)]
pub struct ProjectSettings {
    default_page: Option<Page>,
}

impl Into<AppProjectSettings> for ProjectSettings {
    fn into(self) -> AppProjectSettings {
        AppProjectSettings {
            default_page: self.default_page.map(Page::into),
        }
    }
}

impl Into<ProjectSettings> for AppProjectSettings {
    fn into(self) -> ProjectSettings {
        ProjectSettings {
            default_page: self.default_page.map(AppPage::into),
        }
    }
}

impl Into<AppPhotoRating> for PhotoRating {
    fn into(self) -> AppPhotoRating {
        match self {
            PhotoRating::Yes => AppPhotoRating::Yes,
            PhotoRating::No => AppPhotoRating::No,
            PhotoRating::Maybe => AppPhotoRating::Maybe,
        }
    }
}

impl Into<PhotoRating> for AppPhotoRating {
    fn into(self) -> PhotoRating {
        match self {
            AppPhotoRating::Yes => PhotoRating::Yes,
            AppPhotoRating::No => PhotoRating::No,
            AppPhotoRating::Maybe => PhotoRating::Maybe,
        }
    }
}

#[derive(Debug, Clone, Savefile)]
pub enum ProjectPhotoGrouping {
    Rating,
    Date,
}

impl Into<AppPhotoGrouping> for ProjectPhotoGrouping {
    fn into(self) -> AppPhotoGrouping {
        match self {
            ProjectPhotoGrouping::Rating => AppPhotoGrouping::Rating,
            ProjectPhotoGrouping::Date => AppPhotoGrouping::Date,
        }
    }
}

impl Into<ProjectPhotoGrouping> for AppPhotoGrouping {
    fn into(self) -> ProjectPhotoGrouping {
        match self {
            AppPhotoGrouping::Rating => ProjectPhotoGrouping::Rating,
            AppPhotoGrouping::Date => ProjectPhotoGrouping::Date,
            AppPhotoGrouping::Tag => ProjectPhotoGrouping::Date,
        }
    }
}

impl Into<Page> for AppPage {
    fn into(self) -> Page {
        Page {
            width: self.width(),
            height: self.height(),
            ppi: self.ppi(),
            unit: match self.unit() {
                AppUnit::Pixels => Unit::Pixels,
                AppUnit::Inches => Unit::Inches,
                AppUnit::Centimeters => Unit::Centimeters,
            },
        }
    }
}

impl Into<AppPage> for Page {
    fn into(self) -> AppPage {
        AppPage::new(
            self.width,
            self.height,
            self.ppi,
            match self.unit {
                Unit::Pixels => AppUnit::Pixels,
                Unit::Inches => AppUnit::Inches,
                Unit::Centimeters => AppUnit::Centimeters,
            },
        )
    }
}

#[derive(Debug, Clone, Savefile)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Into<Vec2> for egui::Vec2 {
    fn into(self) -> Vec2 {
        Vec2 {
            x: self.x,
            y: self.y,
        }
    }
}

impl Into<egui::Vec2> for Vec2 {
    fn into(self) -> egui::Vec2 {
        egui::Vec2 {
            x: self.x,
            y: self.y,
        }
    }
}

#[derive(Debug, Clone, Savefile)]
pub struct Rect {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl Into<Rect> for egui::Rect {
    fn into(self) -> Rect {
        Rect {
            min_x: self.min.x,
            min_y: self.min.y,
            max_x: self.max.x,
            max_y: self.max.y,
        }
    }
}

impl Into<egui::Rect> for Rect {
    fn into(self) -> egui::Rect {
        egui::Rect::from_min_max(
            egui::Pos2::new(self.min_x, self.min_y),
            egui::Pos2::new(self.max_x, self.max_y),
        )
    }
}

#[derive(Debug, Clone, Savefile)]
pub struct Color32 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Into<Color32> for egui::Color32 {
    fn into(self) -> Color32 {
        Color32 {
            r: self.r(),
            g: self.g(),
            b: self.b(),
            a: self.a(),
        }
    }
}

impl Into<egui::Color32> for Color32 {
    fn into(self) -> egui::Color32 {
        egui::Color32::from_rgba_premultiplied(self.r, self.g, self.b, self.a)
    }
}

#[derive(Debug, Clone, Savefile)]
pub struct Pos2 {
    pub x: f32,
    pub y: f32,
}

impl Into<Pos2> for egui::Pos2 {
    fn into(self) -> Pos2 {
        Pos2 {
            x: self.x,
            y: self.y,
        }
    }
}

impl Into<egui::Pos2> for Pos2 {
    fn into(self) -> egui::Pos2 {
        egui::Pos2::new(self.x, self.y)
    }
}

#[derive(Debug, Clone, Savefile)]
pub struct FontId {
    pub size: f32,
    pub family: String,
}

impl From<egui::FontId> for FontId {
    fn from(font: egui::FontId) -> Self {
        let family = match font.family {
            egui::FontFamily::Proportional => "Proportional".to_string(),
            egui::FontFamily::Monospace => "Monospace".to_string(),
            egui::FontFamily::Name(name) => name.to_string(),
        };

        FontId {
            size: font.size,
            family,
        }
    }
}

impl From<FontId> for egui::FontId {
    fn from(font: FontId) -> Self {
        let family = match font.family.as_str() {
            "Proportional" => egui::FontFamily::Proportional,
            "Monospace" => egui::FontFamily::Monospace,
            name => egui::FontFamily::Name(name.into()),
        };

        egui::FontId::new(font.size, family)
    }
}

#[derive(Debug, Clone, Savefile)]
pub struct Stroke {
    pub color: Color32,
    pub width: f32,
}

impl Into<Stroke> for egui::Stroke {
    fn into(self) -> Stroke {
        Stroke {
            color: self.color.into(),
            width: self.width,
        }
    }
}

impl Into<egui::Stroke> for Stroke {
    fn into(self) -> egui::Stroke {
        egui::Stroke {
            color: self.color.into(),
            width: self.width,
        }
    }
}

#[derive(Debug, Clone, Savefile)]
pub enum StrokeKind {
    Inside,
    Middle,
    Outside,
}

impl Into<StrokeKind> for egui::StrokeKind {
    fn into(self) -> StrokeKind {
        match self {
            egui::StrokeKind::Inside => StrokeKind::Inside,
            egui::StrokeKind::Middle => StrokeKind::Middle,
            egui::StrokeKind::Outside => StrokeKind::Outside,
        }
    }
}

impl Into<egui::StrokeKind> for StrokeKind {
    fn into(self) -> egui::StrokeKind {
        match self {
            StrokeKind::Inside => egui::StrokeKind::Inside,
            StrokeKind::Middle => egui::StrokeKind::Middle,
            StrokeKind::Outside => egui::StrokeKind::Outside,
        }
    }
}
