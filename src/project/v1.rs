use std::path::PathBuf;

use egui::{Color32, FontId, Id, Pos2, Rect, Vec2};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    dependencies::{Dependency, SingletonFor},
    id::{next_page_id, set_min_layer_id, LayerId, PageId},
    model::{
        edit_state::EditablePage, page::Page as AppPage, scale_mode::ScaleMode as AppScaleMode,
        unit::Unit as AppUnit,
    },
    photo::{Photo as AppPhoto, PhotoRating as AppPhotoRating},
    photo_manager::{PhotoManager, PhotosGrouping as AppPhotosGrouping},
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
            CanvasText as AppCanvasText, CanvasTextEditState, Layer as AppLayer,
            LayerContent as AppLayerContent, LayerTransformEditState,
            TextHorizontalAlignment as AppTextHorizontalAlignment,
            TextVerticalAlignment as AppTextVerticalAlignment,
        },
        transformable::{ResizeMode, TransformHandleMode::Resize, TransformableState},
    },
};

#[derive(Error, Debug)]
pub enum ProjectError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serde error: {0}")]
    SerdeError(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub photos: Vec<Photo>,
    pub pages: Vec<CanvasPage>,
    pub group_by: PhotosGrouping,
    pub project_settings: ProjectSettings,
}

impl Project {
    pub fn new(root_scene: &OrganizeEditScene, photo_manager: &PhotoManager) -> Project {
        let photos = photo_manager
            .photos
            .iter()
            .map(|photo| Photo {
                path: photo.0.clone(),
                rating: photo.1.rating.into(),
            })
            .collect();

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
                                            path: canvas_photo.photo.path,
                                            rating: canvas_photo.photo.rating.into(),
                                        },
                                        crop: canvas_photo.crop,
                                    })
                                }
                                AppLayerContent::Text(canvas_text) => {
                                    LayerContent::Text(CanvasText {
                                        text: canvas_text.text,
                                        font_size: canvas_text.font_size,
                                        font_id: canvas_text.font_id,
                                        color: canvas_text.color,
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
                                        relative_position: region.relative_position,
                                        relative_size: region.relative_size,
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
                                            path: canvas_photo.photo.path,
                                            rating: canvas_photo.photo.rating.into(),
                                        },
                                        crop: canvas_photo.crop,
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
                                            relative_position: region.relative_position,
                                            relative_size: region.relative_size,
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
                                            font_id: text.font_id,
                                            color: text.color,
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
                            },
                            name: layer.name.clone(),
                            visible: layer.visible,
                            locked: layer.locked,
                            selected: layer.selected,
                            id: layer.id,
                            rect: layer.transform_state.rect,
                            rotation: layer.transform_state.rotation,
                        }
                    })
                    .collect();

                let template = canvas_state.template.clone();
                CanvasPage {
                    layers,
                    page: Page {
                        size: canvas_state.page.size(),
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
                            size: template.page.size(),
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
                                relative_position: region.relative_position,
                                relative_size: region.relative_size,
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

        let group_by = photo_manager.photo_grouping();

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

    pub fn save(
        path: &PathBuf,
        root_scene: &OrganizeEditScene,
        photo_manager: &PhotoManager,
    ) -> Result<(), ProjectError> {
        let project = Project::new(root_scene, photo_manager);

        let project_data = serde_json::to_string_pretty(&project)?;

        std::fs::write(path, project_data)?;

        Ok(())
    }

    pub fn load(path: &PathBuf) -> Result<OrganizeEditScene, ProjectError> {
        let file = std::fs::File::open(path)?;
        let project: Project = serde_json::from_reader(file)?;

        println!("Loaded project: {:?}", project);

        Ok(project.into())
    }
}

impl Into<OrganizeEditScene> for Project {
    fn into(self) -> OrganizeEditScene {
        Dependency::<ProjectSettingsManager>::get().with_lock_mut(|settings| {
            settings.project_settings = self.project_settings.into();
        });

        Dependency::<PhotoManager>::get().with_lock(|photo_manager| {
            photo_manager.load_photos(
                self.photos
                    .into_iter()
                    .map(|photo| (photo.path, Some(photo.rating.into())))
                    .collect(),
            );
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
                            rect: layer.rect,
                            active_handle: None,
                            is_moving: false,
                            handle_mode: Resize(ResizeMode::Free),
                            rotation: layer.rotation,
                            last_frame_rotation: layer.rotation,
                            change_in_rotation: None,
                            id: Id::random(),
                        };

                        let layer = AppLayer {
                            content: match layer.content {
                                LayerContent::Photo(photo) => {
                                    /// TODO: Don't unwrap
                                    AppLayerContent::Photo(AppCanvasPhoto {
                                        photo: AppPhoto::with_rating(
                                            photo.photo.path,
                                            photo.photo.rating.into(),
                                        )
                                        .unwrap(),
                                        crop: photo.crop,
                                    })
                                }
                                LayerContent::Text(text) => AppLayerContent::Text(AppCanvasText {
                                    text: text.text,
                                    font_size: text.font_size,
                                    font_id: text.font_id,
                                    color: text.color,
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
                                        relative_position: region.relative_position,
                                        relative_size: region.relative_size,
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
                                        photo: AppPhoto::with_rating(
                                            photo.photo.path,
                                            photo.photo.rating.into(),
                                        )
                                        .unwrap(), // TODO: Don't unwrap
                                        crop: photo.crop,
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
                                            relative_position: region.relative_position,
                                            relative_size: region.relative_size,
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
                                            font_id: text.font_id,
                                            color: text.color,
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
                        page.page.size,
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
                            template.page.size,
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
                                relative_position: region.relative_position,
                                relative_size: region.relative_size,
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

        let edit_scene = match pages.first().map(|(id, _)| *id) { Some(first_page_id) => {
            Some(CanvasScene::with_state(CanvasSceneState::with_pages(
                pages,
                first_page_id,
            )))
        } _ => {
            None
        }};

        let organize_scene = GalleryScene::new();

        let organize_edit_scene = OrganizeEditScene::new(organize_scene, edit_scene);

        //photo_manager.group_photos_by(project.group_by.into());

        organize_edit_scene
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CanvasPage {
    pub layers: Vec<Layer>,
    pub page: Page,
    pub template: Option<Template>,
    pub quick_layout_order: Vec<LayerId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Page {
    size: Vec2,
    ppi: i32,
    unit: Unit,
}

#[derive(Debug, Clone, PartialEq, Copy, Serialize, Deserialize)]
enum Unit {
    Pixels,
    Inches,
    Centimeters,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Photo {
    pub path: PathBuf,
    pub rating: PhotoRating,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Layer {
    pub content: LayerContent,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub selected: bool,
    pub id: LayerId,
    pub rect: Rect,
    pub rotation: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum ScaleMode {
    Fit,
    Fill,
    Stretch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub name: String,
    pub page: Page,
    pub regions: Vec<TemplateRegion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TemplateRegion {
    pub relative_position: Pos2,
    pub relative_size: Vec2,
    pub kind: TemplateRegionKind,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
enum TemplateRegionKind {
    Image,
    Text { sample_text: String, font_size: f32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CanvasPhoto {
    pub photo: Photo,
    pub crop: Rect,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CanvasText {
    pub text: String,
    pub font_size: f32,
    pub font_id: FontId,
    pub color: Color32,
    pub horizontal_alignment: TextHorizontalAlignment,
    pub vertical_alignment: TextVerticalAlignment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum LayerContent {
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
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TextHorizontalAlignment {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TextVerticalAlignment {
    Top,
    Center,
    Bottom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PhotoRating {
    Yes,
    No,
    Maybe,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PhotosGrouping {
    Rating,
    Date,
}

impl Into<AppPhotosGrouping> for PhotosGrouping {
    fn into(self) -> AppPhotosGrouping {
        match self {
            PhotosGrouping::Rating => AppPhotosGrouping::Rating,
            PhotosGrouping::Date => AppPhotosGrouping::Date,
        }
    }
}

impl Into<PhotosGrouping> for AppPhotosGrouping {
    fn into(self) -> PhotosGrouping {
        match self {
            AppPhotosGrouping::Rating => PhotosGrouping::Rating,
            AppPhotosGrouping::Date => PhotosGrouping::Date,
        }
    }
}

impl Into<Page> for AppPage {
    fn into(self) -> Page {
        Page {
            size: self.size(),
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
            self.size,
            self.ppi,
            match self.unit {
                Unit::Pixels => AppUnit::Pixels,
                Unit::Inches => AppUnit::Inches,
                Unit::Centimeters => AppUnit::Centimeters,
            },
        )
    }
}
