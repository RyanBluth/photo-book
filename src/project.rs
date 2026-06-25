use std::{collections::HashSet, path::PathBuf};

use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use savefile_derive::Savefile;

use thiserror::Error;

use crate::{
    dep, dep_mut,
    id::{LayerId, PageId, next_page_id, set_min_layer_id},
    model::{
        album::Album as AppAlbum, edit_state::EditablePage, page::Page as AppPage,
        photo_grouping::PhotoGrouping as AppPhotoGrouping, scale_mode::ScaleMode as AppScaleMode,
        unit::Unit as AppUnit,
    },
    photo::{
        MetadataCollection as AppMetadataCollection, Photo as AppPhoto,
        PhotoMetadata as AppPhotoMetadata, PhotoMetadataField as AppPhotoMetadataField,
        PhotoRating as AppPhotoRating, PhotoRotation as AppPhotoRotation, Rational as AppRational,
    },
    photo_manager::PhotoManager,
    project_settings::{ProjectSettings as AppProjectSettings, ProjectSettingsManager},
    scene::{
        canvas_scene::CanvasSceneState,
        gallery_scene::GalleryScene,
        organize_edit_scene::{Book as AppBook, OrganizeEditScene},
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
            LayerTransformEditState, LineSlope as AppLineSlope,
            TextHorizontalAlignment as AppTextHorizontalAlignment,
            TextVerticalAlignment as AppTextVerticalAlignment,
        },
        transformable::{ResizeMode, TransformHandleMode::Resize, TransformableState},
    },
};

pub const PROJECT_VERSION: u32 = 6;

#[derive(Error, Debug)]
pub enum ProjectError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serde error: {0}")]
    SerdeError(#[from] serde_json::Error),

    #[error("Savefile error: {0}")]
    SavefileError(#[from] savefile::SavefileError),
}

#[derive(Debug, Clone, PartialEq, Savefile)]
pub struct Project {
    pub photos: Vec<Photo>,
    pub books: Vec<ProjectBook>,
    pub group_by: ProjectPhotoGrouping,
    pub project_settings: ProjectSettings,
    pub albums: Vec<Album>,
}

impl Project {
    pub fn new(root_scene: &OrganizeEditScene) -> Project {
        let photos = dep!(PhotoManager, |photo_manager| {
            photo_manager
                .photo_database
                .all_photos_iter()
                // We do this instead of Into::into avoid deadlocks from photo.rating() and photo.tags()
                .map(|photo| Photo {
                    path: photo.path.clone(),
                    rating: photo_manager.get_photo_rating(&photo.path),
                    tags: photo_manager.get_photo_tags(&photo.path).into(),
                    metadata: photo.metadata.clone().into(),
                    last_modified: photo.last_modified.clone(),
                })
                .collect()
        });

        let books = root_scene
            .books_snapshot()
            .into_iter()
            .map(ProjectBook::from)
            .collect::<Vec<_>>();

        let group_by = dep!(PhotoManager, |photo_manager| photo_manager.photo_grouping());

        let project_settings: AppProjectSettings =
            dep!(ProjectSettingsManager, |settings| settings
                .project_settings
                .clone());

        let albums = dep!(PhotoManager, |photo_manager| {
            photo_manager
                .photo_database
                .albums_iter()
                .cloned()
                .collect::<Vec<_>>()
        })
        .into_iter()
        .map(|album| {
            dep_mut!(PhotoManager, |photo_manager| Album {
                id: album.id.clone(),
                name: album.name.clone(),
                photos: photo_manager
                    .photo_database
                    .album_photos_iter(&album.id)
                    .cloned()
                    .collect(),
            })
        })
        .collect::<Vec<_>>();

        Project {
            photos,
            books,
            group_by: group_by.into(),
            project_settings: project_settings.into(),
            albums,
        }
    }

    #[allow(dead_code)]
    pub fn save(path: &PathBuf, root_scene: &OrganizeEditScene) -> Result<(), ProjectError> {
        let project = Project::new(root_scene);
        Self::save_project(path, &project)
    }

    pub fn save_project(path: &PathBuf, project: &Project) -> Result<(), ProjectError> {
        match savefile::save_file_compressed(path, PROJECT_VERSION, project) {
            Ok(_) => Ok(()),
            Err(e) => {
                println!("Error saving project: {:?}", e);
                Err(ProjectError::SavefileError(e))
            }
        }
    }

    pub fn load_project(path: &PathBuf) -> Result<Project, ProjectError> {
        savefile::load_file::<Project, _>(path, PROJECT_VERSION)
            .map_err(ProjectError::SavefileError)
    }

    #[allow(dead_code)]
    pub fn load(path: &PathBuf) -> Result<OrganizeEditScene, ProjectError> {
        match Self::load_project(path) {
            Ok(project) => {
                println!("Loaded project: {:?}", project);
                Ok(project.into())
            }
            Err(e) => {
                println!("Error loading project: {:?}", e);
                Err(e)
            }
        }
    }
}

impl From<Project> for OrganizeEditScene {
    fn from(project: Project) -> Self {
        dep_mut!(ProjectSettingsManager, |settings| {
            settings.project_settings = project.project_settings.into();
        });

        let photos_with_metadata: Vec<(PathBuf, AppPhotoRating, HashSet<String>)> = project
            .photos
            .iter()
            .map(|photo| (photo.path.clone(), photo.rating, photo.tags.clone()))
            .collect();

        let photos = project
            .photos
            .iter()
            .map(|photo| AppPhoto::with_metadata(photo.path.clone(), photo.metadata.clone().into()))
            .collect();

        dep!(PhotoManager, |photo_manager| {
            photo_manager.load_photos(photos);
        });

        dep_mut!(PhotoManager, |photo_manager| {
            for (path, rating, tags) in photos_with_metadata {
                photo_manager.set_photo_rating(&path, rating);
                photo_manager.set_photo_tags(&path, tags);
            }
        });

        let albums: Vec<AppAlbum> = project.albums.into_iter().map(AppAlbum::from).collect();
        dep_mut!(PhotoManager, |photo_manager| {
            for album in albums {
                photo_manager.insert_album(album);
            }
        });

        let books = project
            .books
            .into_iter()
            .map(AppBook::from)
            .collect::<Vec<_>>();

        let scene = OrganizeEditScene::with_books(GalleryScene::new(), books);

        //photo_manager.group_photos_by(project.group_by.into());

        scene
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
pub struct ProjectBook {
    pub id: String,
    pub name: String,
    pub pages: Vec<CanvasPage>,
}

impl From<AppBook> for ProjectBook {
    fn from(book: AppBook) -> Self {
        Self {
            id: book.id,
            name: book.name,
            pages: book
                .state
                .pages_state
                .pages
                .into_values()
                .map(CanvasPage::from)
                .collect(),
        }
    }
}

impl From<ProjectBook> for AppBook {
    fn from(book: ProjectBook) -> Self {
        let id = if book.id.trim().is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            book.id
        };

        let name = if book.name.trim().is_empty() {
            "Book".to_string()
        } else {
            book.name
        };

        AppBook::with_state(id, name, book.pages.into())
    }
}

impl From<Vec<CanvasPage>> for CanvasSceneState {
    fn from(pages: Vec<CanvasPage>) -> Self {
        let pages: IndexMap<PageId, CanvasState> = pages
            .into_iter()
            .map(|page| (next_page_id(), page.into()))
            .collect();

        match pages.first().map(|(id, _)| *id) {
            Some(first_page_id) => CanvasSceneState::with_pages(pages, first_page_id),
            None => CanvasSceneState::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
pub struct CanvasPage {
    pub layers: Vec<Layer>,
    pub page: Page,
    pub template: Option<Template>,
    pub quick_layout_order: Vec<LayerId>,
}

impl From<CanvasState> for CanvasPage {
    fn from(canvas_state: CanvasState) -> Self {
        let layers = canvas_state.layers.into_values().map(Layer::from).collect();

        Self {
            layers,
            page: canvas_state.page.value.clone().into(),
            template: canvas_state.template.map(Template::from),
            quick_layout_order: canvas_state.quick_layout_order,
        }
    }
}

impl From<CanvasPage> for CanvasState {
    fn from(page: CanvasPage) -> Self {
        let layers: IndexMap<LayerId, AppLayer> = page
            .layers
            .into_iter()
            .map(|layer| {
                let layer = AppLayer::from(layer);
                set_min_layer_id(layer.id);
                (layer.id, layer)
            })
            .collect();

        CanvasState::with_layers(
            layers,
            EditablePage::new(page.page.into()),
            page.template.map(AppTemplate::from),
            page.quick_layout_order,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
pub struct Template {
    pub name: String,
    pub page: Page,
    pub regions: Vec<TemplateRegion>,
}

impl From<AppTemplate> for Template {
    fn from(template: AppTemplate) -> Self {
        Self {
            name: template.name,
            page: template.page.into(),
            regions: template
                .regions
                .into_iter()
                .map(TemplateRegion::from)
                .collect(),
        }
    }
}

impl From<Template> for AppTemplate {
    fn from(template: Template) -> Self {
        Self {
            name: template.name,
            page: template.page.into(),
            regions: template
                .regions
                .into_iter()
                .map(AppTemplateRegion::from)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
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

#[derive(Debug, Clone, PartialEq, Savefile)]
pub struct Photo {
    pub path: PathBuf,
    pub rating: PhotoRating,
    pub tags: HashSet<String>,
    pub metadata: PhotoMetadata,
    pub last_modified: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Savefile)]
pub struct PhotoMetadata {
    pub fields: Vec<PhotoMetadataField>,
}

impl From<PhotoMetadata> for AppPhotoMetadata {
    fn from(value: PhotoMetadata) -> Self {
        Self {
            fields: value
                .fields
                .iter()
                .map(|field| {
                    (
                        AppPhotoMetadataField::from(field.clone()).label(),
                        AppPhotoMetadataField::from(field.clone()),
                    )
                })
                .into(),
        }
    }
}

impl From<AppPhotoMetadata> for PhotoMetadata {
    fn from(value: AppPhotoMetadata) -> Self {
        PhotoMetadata {
            fields: value
                .iter()
                .map(|(_, field)| field.clone())
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<AppMetadataCollection> for Vec<PhotoMetadataField> {
    fn from(value: AppMetadataCollection) -> Self {
        value
            .iter()
            .map(|(_, field)| field.clone())
            .map(Into::into)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
pub enum PhotoMetadataField {
    Path(PathBuf),
    Width(usize),
    Height(usize),
    Rotation(PhotoRotation),
    RotatedWidth(usize),
    RotatedHeight(usize),
    Camera(String),
    DateTime(DateTime<Utc>),
    ISO(u32),
    ShutterSpeed(Rational),
    Aperture(Rational),
    FocalLength(Rational),
}

impl From<AppPhotoMetadataField> for PhotoMetadataField {
    fn from(value: AppPhotoMetadataField) -> Self {
        match value {
            AppPhotoMetadataField::Path(path_buf) => PhotoMetadataField::Path(path_buf),
            AppPhotoMetadataField::Width(width) => PhotoMetadataField::Width(width),
            AppPhotoMetadataField::Height(height) => PhotoMetadataField::Height(height),
            AppPhotoMetadataField::Rotation(photo_rotation) => {
                PhotoMetadataField::Rotation(photo_rotation.into())
            }
            AppPhotoMetadataField::RotatedWidth(width) => PhotoMetadataField::RotatedWidth(width),
            AppPhotoMetadataField::RotatedHeight(height) => {
                PhotoMetadataField::RotatedHeight(height)
            }
            AppPhotoMetadataField::Camera(camera) => PhotoMetadataField::Camera(camera),
            AppPhotoMetadataField::DateTime(date_time) => PhotoMetadataField::DateTime(date_time),
            AppPhotoMetadataField::ISO(iso) => PhotoMetadataField::ISO(iso),
            AppPhotoMetadataField::ShutterSpeed(speed) => {
                PhotoMetadataField::ShutterSpeed(speed.into())
            }
            AppPhotoMetadataField::Aperture(aperature) => {
                PhotoMetadataField::Aperture(aperature.into())
            }
            AppPhotoMetadataField::FocalLength(focal_length) => {
                PhotoMetadataField::FocalLength(focal_length.into())
            }
        }
    }
}

impl From<PhotoMetadataField> for AppPhotoMetadataField {
    fn from(value: PhotoMetadataField) -> Self {
        match value {
            PhotoMetadataField::Path(path_buf) => AppPhotoMetadataField::Path(path_buf),
            PhotoMetadataField::Width(width) => AppPhotoMetadataField::Width(width),
            PhotoMetadataField::Height(height) => AppPhotoMetadataField::Height(height),
            PhotoMetadataField::Rotation(photo_rotation) => {
                AppPhotoMetadataField::Rotation(photo_rotation.into())
            }
            PhotoMetadataField::RotatedWidth(width) => AppPhotoMetadataField::RotatedWidth(width),
            PhotoMetadataField::RotatedHeight(height) => {
                AppPhotoMetadataField::RotatedHeight(height)
            }
            PhotoMetadataField::Camera(camera) => AppPhotoMetadataField::Camera(camera),
            PhotoMetadataField::DateTime(date_time) => AppPhotoMetadataField::DateTime(date_time),
            PhotoMetadataField::ISO(iso) => AppPhotoMetadataField::ISO(iso),
            PhotoMetadataField::ShutterSpeed(speed) => {
                AppPhotoMetadataField::ShutterSpeed(speed.into())
            }
            PhotoMetadataField::Aperture(aperature) => {
                AppPhotoMetadataField::Aperture(aperature.into())
            }
            PhotoMetadataField::FocalLength(focal_length) => {
                AppPhotoMetadataField::FocalLength(focal_length.into())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
pub struct Rational {
    pub num: i32,
    pub denom: i32,
}

impl From<Rational> for AppRational {
    fn from(value: Rational) -> Self {
        Self {
            num: value.num,
            denom: value.denom,
        }
    }
}

impl From<AppRational> for Rational {
    fn from(value: AppRational) -> Self {
        Self {
            num: value.num,
            denom: value.denom,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
pub enum PhotoRotation {
    Normal,
    MirrorHorizontal,
    Rotate180,
    MirrorVerticalAndRotate180,
    MirrorHorizontalAndRotate90CW,
    Rotate90CW,
    MirrorHorizontalAndRotate270CW,
    Rotate270CW,
}

impl From<PhotoRotation> for AppPhotoRotation {
    fn from(value: PhotoRotation) -> Self {
        match value {
            PhotoRotation::Normal => AppPhotoRotation::Normal,
            PhotoRotation::MirrorHorizontal => AppPhotoRotation::MirrorHorizontal,
            PhotoRotation::Rotate180 => AppPhotoRotation::Rotate180,
            PhotoRotation::MirrorVerticalAndRotate180 => {
                AppPhotoRotation::MirrorVerticalAndRotate180
            }
            PhotoRotation::MirrorHorizontalAndRotate90CW => {
                AppPhotoRotation::MirrorHorizontalAndRotate90CW
            }
            PhotoRotation::Rotate90CW => AppPhotoRotation::Rotate90CW,
            PhotoRotation::MirrorHorizontalAndRotate270CW => {
                AppPhotoRotation::MirrorHorizontalAndRotate270CW
            }
            PhotoRotation::Rotate270CW => AppPhotoRotation::Rotate270CW,
        }
    }
}

impl From<AppPhotoRotation> for PhotoRotation {
    fn from(value: AppPhotoRotation) -> Self {
        match value {
            AppPhotoRotation::Normal => PhotoRotation::Normal,
            AppPhotoRotation::MirrorHorizontal => PhotoRotation::MirrorHorizontal,
            AppPhotoRotation::Rotate180 => PhotoRotation::Rotate180,
            AppPhotoRotation::MirrorVerticalAndRotate180 => {
                PhotoRotation::MirrorVerticalAndRotate180
            }
            AppPhotoRotation::MirrorHorizontalAndRotate90CW => {
                PhotoRotation::MirrorHorizontalAndRotate90CW
            }
            AppPhotoRotation::Rotate90CW => PhotoRotation::Rotate90CW,
            AppPhotoRotation::MirrorHorizontalAndRotate270CW => {
                PhotoRotation::MirrorHorizontalAndRotate270CW
            }
            AppPhotoRotation::Rotate270CW => PhotoRotation::Rotate270CW,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
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

impl From<AppLayer> for Layer {
    fn from(mut layer: AppLayer) -> Self {
        layer.transform_edit_state.update(&layer.transform_state);

        Self {
            content: layer.content.into(),
            name: layer.name,
            visible: layer.visible,
            locked: layer.locked,
            selected: layer.selected,
            id: layer.id,
            rect: layer.transform_state.rect.into(),
            rotation: layer.transform_state.rotation,
        }
    }
}

impl From<Layer> for AppLayer {
    fn from(layer: Layer) -> Self {
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

        Self {
            content: layer.content.into(),
            name: layer.name,
            visible: layer.visible,
            locked: layer.locked,
            selected: layer.selected,
            id: layer.id,
            transform_edit_state: LayerTransformEditState::from(&transformable_state),
            transform_state: transformable_state,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
pub enum ScaleMode {
    Fit,
    Fill,
    Stretch,
}

impl From<AppScaleMode> for ScaleMode {
    fn from(scale_mode: AppScaleMode) -> Self {
        match scale_mode {
            AppScaleMode::Fit => Self::Fit,
            AppScaleMode::Fill => Self::Fill,
            AppScaleMode::Stretch => Self::Stretch,
        }
    }
}

impl From<ScaleMode> for AppScaleMode {
    fn from(scale_mode: ScaleMode) -> Self {
        match scale_mode {
            ScaleMode::Fit => Self::Fit,
            ScaleMode::Fill => Self::Fill,
            ScaleMode::Stretch => Self::Stretch,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
pub struct TemplateRegion {
    pub relative_position: Pos2,
    pub relative_size: Vec2,
    pub kind: TemplateRegionKind,
}

impl From<AppTemplateRegion> for TemplateRegion {
    fn from(region: AppTemplateRegion) -> Self {
        Self {
            relative_position: region.relative_position.into(),
            relative_size: region.relative_size.into(),
            kind: region.kind.into(),
        }
    }
}

impl From<TemplateRegion> for AppTemplateRegion {
    fn from(region: TemplateRegion) -> Self {
        Self {
            relative_position: region.relative_position.into(),
            relative_size: region.relative_size.into(),
            kind: region.kind.into(),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Savefile)]
pub enum TemplateRegionKind {
    Image,
    Text { sample_text: String, font_size: f32 },
}

impl From<AppTemplateRegionKind> for TemplateRegionKind {
    fn from(kind: AppTemplateRegionKind) -> Self {
        match kind {
            AppTemplateRegionKind::Image => Self::Image,
            AppTemplateRegionKind::Text {
                sample_text,
                font_size,
            } => Self::Text {
                sample_text,
                font_size,
            },
        }
    }
}

impl From<TemplateRegionKind> for AppTemplateRegionKind {
    fn from(kind: TemplateRegionKind) -> Self {
        match kind {
            TemplateRegionKind::Image => Self::Image,
            TemplateRegionKind::Text {
                sample_text,
                font_size,
            } => Self::Text {
                sample_text,
                font_size,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
pub struct CanvasPhoto {
    pub photo: Photo,
    pub crop: Rect,
}

impl From<AppCanvasPhoto> for CanvasPhoto {
    fn from(canvas_photo: AppCanvasPhoto) -> Self {
        Self {
            // We do this instead of Into::into avoid deadlocks from photo.rating() and photo.tags()
            photo: Photo {
                path: canvas_photo.photo.path.clone(),
                rating: dep!(PhotoManager, |photo_manager| {
                    photo_manager.get_photo_rating(&canvas_photo.photo.path)
                }),
                tags: dep!(PhotoManager, |photo_manager| {
                    photo_manager
                        .get_photo_tags(&canvas_photo.photo.path)
                        .into()
                }),
                metadata: canvas_photo.photo.metadata.clone().into(),
                last_modified: canvas_photo.photo.last_modified.clone(),
            },
            crop: canvas_photo.crop.into(),
        }
    }
}

impl From<CanvasPhoto> for AppCanvasPhoto {
    fn from(canvas_photo: CanvasPhoto) -> Self {
        Self {
            photo: AppPhoto::new(canvas_photo.photo.path).unwrap(),
            crop: canvas_photo.crop.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
pub struct CanvasText {
    pub text: String,
    pub font_size: f32,
    pub font_id: FontId,
    pub color: Color32,
    pub horizontal_alignment: TextHorizontalAlignment,
    pub vertical_alignment: TextVerticalAlignment,
}

impl From<AppCanvasText> for CanvasText {
    fn from(text: AppCanvasText) -> Self {
        Self {
            text: text.text,
            font_size: text.font_size,
            font_id: text.font_id.into(),
            color: text.color.into(),
            horizontal_alignment: text.horizontal_alignment.into(),
            vertical_alignment: text.vertical_alignment.into(),
        }
    }
}

impl From<CanvasText> for AppCanvasText {
    fn from(text: CanvasText) -> Self {
        Self {
            text: text.text,
            font_size: text.font_size,
            font_id: text.font_id.into(),
            color: text.color.into(),
            edit_state: CanvasTextEditState::new(text.font_size),
            horizontal_alignment: text.horizontal_alignment.into(),
            vertical_alignment: text.vertical_alignment.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
pub struct CanvasShape {
    pub kind: CanvasShapeKind,
    pub fill_color: Color32,
    pub stroke: Option<(Stroke, StrokeKind)>,
}

impl From<AppCanvasShape> for CanvasShape {
    fn from(shape: AppCanvasShape) -> Self {
        Self {
            kind: shape.kind.into(),
            fill_color: shape.fill_color.into(),
            stroke: shape
                .stroke
                .map(|(stroke, kind)| (stroke.into(), kind.into())),
        }
    }
}

impl From<CanvasShape> for AppCanvasShape {
    fn from(shape: CanvasShape) -> Self {
        Self {
            kind: shape.kind.into(),
            fill_color: shape.fill_color.into(),
            stroke: shape
                .stroke
                .as_ref()
                .map(|(stroke, kind)| (stroke.clone().into(), kind.clone().into())),
            edit_state: CanvasShapeEditState::new(
                shape
                    .stroke
                    .as_ref()
                    .map(|(stroke, _)| stroke.width)
                    .unwrap_or(1.0),
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
pub enum CanvasShapeKind {
    Rectangle { corner_radius: f32 },
    Ellipse,
    Line { slope: LineSlope },
}

impl From<AppCanvasShapeKind> for CanvasShapeKind {
    fn from(kind: AppCanvasShapeKind) -> Self {
        match kind {
            AppCanvasShapeKind::Rectangle { corner_radius } => Self::Rectangle { corner_radius },
            AppCanvasShapeKind::Ellipse => Self::Ellipse,
            AppCanvasShapeKind::Line { slope } => Self::Line {
                slope: slope.into(),
            },
        }
    }
}

impl From<CanvasShapeKind> for AppCanvasShapeKind {
    fn from(kind: CanvasShapeKind) -> Self {
        match kind {
            CanvasShapeKind::Rectangle { corner_radius } => Self::Rectangle { corner_radius },
            CanvasShapeKind::Ellipse => Self::Ellipse,
            CanvasShapeKind::Line { slope } => Self::Line {
                slope: slope.into(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
pub enum LineSlope {
    Positive,
    Negative,
}

impl From<LineSlope> for AppLineSlope {
    fn from(slope: LineSlope) -> Self {
        match slope {
            LineSlope::Positive => Self::Positive,
            LineSlope::Negative => Self::Negative,
        }
    }
}

impl From<AppLineSlope> for LineSlope {
    fn from(slope: AppLineSlope) -> Self {
        match slope {
            AppLineSlope::Positive => Self::Positive,
            AppLineSlope::Negative => Self::Negative,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
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
    Shape(CanvasShape),
}

impl From<AppLayerContent> for LayerContent {
    fn from(content: AppLayerContent) -> Self {
        match content {
            AppLayerContent::Photo(photo) => Self::Photo(photo.into()),
            AppLayerContent::Text(text) => Self::Text(text.into()),
            AppLayerContent::TemplatePhoto {
                region,
                photo,
                scale_mode,
            } => Self::TemplatePhoto {
                region: region.into(),
                photo: photo.map(CanvasPhoto::from),
                scale_mode: scale_mode.into(),
            },
            AppLayerContent::TemplateText { region, text } => Self::TemplateText {
                region: region.into(),
                text: text.into(),
            },
            AppLayerContent::Shape(shape) => Self::Shape(shape.into()),
        }
    }
}

impl From<LayerContent> for AppLayerContent {
    fn from(content: LayerContent) -> Self {
        match content {
            LayerContent::Photo(photo) => Self::Photo(photo.into()),
            LayerContent::Text(text) => Self::Text(text.into()),
            LayerContent::TemplatePhoto {
                region,
                photo,
                scale_mode,
            } => Self::TemplatePhoto {
                region: region.into(),
                photo: photo.map(AppCanvasPhoto::from),
                scale_mode: scale_mode.into(),
            },
            LayerContent::TemplateText { region, text } => Self::TemplateText {
                region: region.into(),
                text: text.into(),
            },
            LayerContent::Shape(shape) => Self::Shape(shape.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Copy, Savefile)]
pub enum TextHorizontalAlignment {
    Left,
    Center,
    Right,
}

impl From<AppTextHorizontalAlignment> for TextHorizontalAlignment {
    fn from(alignment: AppTextHorizontalAlignment) -> Self {
        match alignment {
            AppTextHorizontalAlignment::Left => Self::Left,
            AppTextHorizontalAlignment::Center => Self::Center,
            AppTextHorizontalAlignment::Right => Self::Right,
        }
    }
}

impl From<TextHorizontalAlignment> for AppTextHorizontalAlignment {
    fn from(alignment: TextHorizontalAlignment) -> Self {
        match alignment {
            TextHorizontalAlignment::Left => Self::Left,
            TextHorizontalAlignment::Center => Self::Center,
            TextHorizontalAlignment::Right => Self::Right,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Copy, Savefile)]
pub enum TextVerticalAlignment {
    Top,
    Center,
    Bottom,
}

impl From<AppTextVerticalAlignment> for TextVerticalAlignment {
    fn from(alignment: AppTextVerticalAlignment) -> Self {
        match alignment {
            AppTextVerticalAlignment::Top => Self::Top,
            AppTextVerticalAlignment::Center => Self::Center,
            AppTextVerticalAlignment::Bottom => Self::Bottom,
        }
    }
}

impl From<TextVerticalAlignment> for AppTextVerticalAlignment {
    fn from(alignment: TextVerticalAlignment) -> Self {
        match alignment {
            TextVerticalAlignment::Top => Self::Top,
            TextVerticalAlignment::Center => Self::Center,
            TextVerticalAlignment::Bottom => Self::Bottom,
        }
    }
}

pub type PhotoRating = Option<u8>;

#[derive(Debug, Clone, PartialEq, Savefile)]
pub struct ProjectSettings {
    default_page: Option<Page>,
}

impl From<ProjectSettings> for AppProjectSettings {
    fn from(settings: ProjectSettings) -> Self {
        AppProjectSettings {
            default_page: settings.default_page.map(AppPage::from),
        }
    }
}

impl From<AppProjectSettings> for ProjectSettings {
    fn from(settings: AppProjectSettings) -> Self {
        ProjectSettings {
            default_page: settings.default_page.map(Page::from),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
pub enum ProjectPhotoGrouping {
    Rating,
    Date,
}

impl From<ProjectPhotoGrouping> for AppPhotoGrouping {
    fn from(grouping: ProjectPhotoGrouping) -> Self {
        match grouping {
            ProjectPhotoGrouping::Rating => Self::Rating,
            ProjectPhotoGrouping::Date => Self::Date,
        }
    }
}

impl From<AppPhotoGrouping> for ProjectPhotoGrouping {
    fn from(grouping: AppPhotoGrouping) -> Self {
        match grouping {
            AppPhotoGrouping::Rating => Self::Rating,
            AppPhotoGrouping::Date => Self::Date,
            AppPhotoGrouping::Tag => Self::Date,
        }
    }
}

impl From<AppPage> for Page {
    fn from(page: AppPage) -> Self {
        Self {
            width: page.width(),
            height: page.height(),
            ppi: page.ppi(),
            unit: match page.unit() {
                AppUnit::Pixels => Unit::Pixels,
                AppUnit::Inches => Unit::Inches,
                AppUnit::Centimeters => Unit::Centimeters,
            },
        }
    }
}

impl From<Page> for AppPage {
    fn from(page: Page) -> Self {
        AppPage::new(
            page.width,
            page.height,
            page.ppi,
            match page.unit {
                Unit::Pixels => AppUnit::Pixels,
                Unit::Inches => AppUnit::Inches,
                Unit::Centimeters => AppUnit::Centimeters,
            },
        )
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl From<egui::Vec2> for Vec2 {
    fn from(vec: egui::Vec2) -> Self {
        Self { x: vec.x, y: vec.y }
    }
}

impl From<Vec2> for egui::Vec2 {
    fn from(vec: Vec2) -> Self {
        egui::Vec2 { x: vec.x, y: vec.y }
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
pub struct Rect {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl From<egui::Rect> for Rect {
    fn from(rect: egui::Rect) -> Self {
        Self {
            min_x: rect.min.x,
            min_y: rect.min.y,
            max_x: rect.max.x,
            max_y: rect.max.y,
        }
    }
}

impl From<Rect> for egui::Rect {
    fn from(rect: Rect) -> Self {
        egui::Rect::from_min_max(
            egui::Pos2::new(rect.min_x, rect.min_y),
            egui::Pos2::new(rect.max_x, rect.max_y),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
pub struct Color32 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl From<egui::Color32> for Color32 {
    fn from(color: egui::Color32) -> Self {
        Self {
            r: color.r(),
            g: color.g(),
            b: color.b(),
            a: color.a(),
        }
    }
}

impl From<Color32> for egui::Color32 {
    fn from(color: Color32) -> Self {
        egui::Color32::from_rgba_premultiplied(color.r, color.g, color.b, color.a)
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
pub struct Pos2 {
    pub x: f32,
    pub y: f32,
}

impl From<egui::Pos2> for Pos2 {
    fn from(pos: egui::Pos2) -> Self {
        Self { x: pos.x, y: pos.y }
    }
}

impl From<Pos2> for egui::Pos2 {
    fn from(pos: Pos2) -> Self {
        egui::Pos2::new(pos.x, pos.y)
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
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

        Self {
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

#[derive(Debug, Clone, PartialEq, Savefile)]
pub struct Stroke {
    pub color: Color32,
    pub width: f32,
}

impl From<egui::Stroke> for Stroke {
    fn from(stroke: egui::Stroke) -> Self {
        Self {
            color: stroke.color.into(),
            width: stroke.width,
        }
    }
}

impl From<Stroke> for egui::Stroke {
    fn from(stroke: Stroke) -> Self {
        egui::Stroke {
            color: stroke.color.into(),
            width: stroke.width,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
pub enum StrokeKind {
    Inside,
    Middle,
    Outside,
}

impl From<egui::StrokeKind> for StrokeKind {
    fn from(kind: egui::StrokeKind) -> Self {
        match kind {
            egui::StrokeKind::Inside => Self::Inside,
            egui::StrokeKind::Middle => Self::Middle,
            egui::StrokeKind::Outside => Self::Outside,
        }
    }
}

impl From<StrokeKind> for egui::StrokeKind {
    fn from(kind: StrokeKind) -> Self {
        match kind {
            StrokeKind::Inside => Self::Inside,
            StrokeKind::Middle => Self::Middle,
            StrokeKind::Outside => Self::Outside,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
pub struct Album {
    name: String,
    photos: HashSet<PathBuf>,
    id: String,
}

impl From<AppAlbum> for Album {
    fn from(album: AppAlbum) -> Self {
        Self {
            name: album.name,
            photos: album.photos,
            id: album.id,
        }
    }
}

impl From<Album> for AppAlbum {
    fn from(album: Album) -> Self {
        let id = if album.id.trim().is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            album.id
        };

        Self {
            id,
            name: album.name,
            photos: album.photos,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas_scene_state(width: f32, height: f32) -> CanvasSceneState {
        let page_id = next_page_id();
        let mut pages = IndexMap::new();
        pages.insert(
            page_id,
            CanvasState::with_layers(
                IndexMap::new(),
                EditablePage::new(AppPage::new(width, height, 300, AppUnit::Inches)),
                None,
                Vec::new(),
            ),
        );
        CanvasSceneState::with_pages(pages, page_id)
    }

    #[test]
    fn test_album_project_conversion_preserves_id() {
        let photo_path = PathBuf::from("/test/photo1.jpg");
        let mut photos = HashSet::new();
        photos.insert(photo_path.clone());

        let app_album = AppAlbum {
            id: "album-1".to_string(),
            name: "Favorites".to_string(),
            photos,
        };

        let project_album: Album = app_album.into();
        let restored_album: AppAlbum = project_album.into();

        assert_eq!(restored_album.id, "album-1");
        assert_eq!(restored_album.name, "Favorites");
        assert!(restored_album.photos.contains(&photo_path));
    }

    #[test]
    fn project_new_preserves_multiple_books_independently() {
        let scene = OrganizeEditScene::with_books(
            GalleryScene::new(),
            vec![
                AppBook::with_state(
                    "book-1".to_string(),
                    "First".to_string(),
                    canvas_scene_state(6.0, 8.0),
                ),
                AppBook::with_state(
                    "book-2".to_string(),
                    "Second".to_string(),
                    canvas_scene_state(10.0, 12.0),
                ),
            ],
        );

        let project = Project::new(&scene);

        assert_eq!(project.books.len(), 2);
        assert_eq!(project.books[0].id, "book-1");
        assert_eq!(project.books[0].name, "First");
        assert_eq!(project.books[0].pages[0].page.width, 6.0);
        assert_eq!(project.books[1].id, "book-2");
        assert_eq!(project.books[1].name, "Second");
        assert_eq!(project.books[1].pages[0].page.width, 10.0);
    }
}
