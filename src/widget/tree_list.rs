use std::{hash::Hash, path::PathBuf};

use egui::{Color32, Image, ImageSource, Rect, Response, RichText, Sense, Ui, UiBuilder, Vec2};
use egui_extras::{Column, TableBuilder};

use crate::{
    dependencies::{Dependency, Singleton, SingletonFor},
    photo_manager::PhotoManager,
    theme,
};

pub const ROW_HEIGHT: f32 = 24.0;
pub const INDENT_WIDTH: f32 = 20.0;
const DISCLOSURE_SIZE: f32 = 14.0;
const THUMBNAIL_SIZE: f32 = 16.0;
const DISCLOSURE_TITLE_SPACING: f32 = 4.0;
const PHOTO_TITLE_SPACING: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionStyle {
    Background,
    BoldText,
}

impl SelectionStyle {
    fn selected_fill(self, ui: &Ui, is_selected: bool) -> Option<Color32> {
        if is_selected && self == SelectionStyle::Background {
            Some(ui.visuals().selection.bg_fill)
        } else {
            None
        }
    }

    fn row_text(self, text: &str, is_selected: bool) -> RichText {
        let text = RichText::new(text);
        if is_selected && self == SelectionStyle::BoldText {
            text.strong()
        } else {
            text
        }
    }
}

struct HeaderRowResponse {
    row: Response,
    disclosure: Option<Response>,
}

#[derive(Debug, Clone)]
enum TreeListRowKind {
    Header {
        has_children: bool,
        is_expanded: bool,
        trailing: Option<String>,
    },
    Photo {
        path: PathBuf,
    },
}

#[derive(Debug, Clone)]
pub struct TreeListRow<Id> {
    id: Id,
    depth: usize,
    title: String,
    selected: bool,
    selection_style: SelectionStyle,
    sense: Sense,
    kind: TreeListRowKind,
}

impl<Id> TreeListRow<Id> {
    pub fn header(
        id: Id,
        depth: usize,
        title: String,
        has_children: bool,
        is_expanded: bool,
    ) -> Self {
        Self {
            id,
            depth,
            title,
            selected: false,
            selection_style: SelectionStyle::Background,
            sense: Sense::click(),
            kind: TreeListRowKind::Header {
                has_children,
                is_expanded,
                trailing: None,
            },
        }
    }

    pub fn photo(id: Id, depth: usize, title: String, path: PathBuf) -> Self {
        Self {
            id,
            depth,
            title,
            selected: false,
            selection_style: SelectionStyle::Background,
            sense: Sense::click(),
            kind: TreeListRowKind::Photo { path },
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn selection_style(mut self, selection_style: SelectionStyle) -> Self {
        self.selection_style = selection_style;
        self
    }

    pub fn sense(mut self, sense: Sense) -> Self {
        self.sense = sense;
        self
    }

    pub fn trailing(mut self, trailing: impl Into<String>) -> Self {
        if let TreeListRowKind::Header { trailing: t, .. } = &mut self.kind {
            *t = Some(trailing.into());
        }
        self
    }
}

impl<Id> TreeListRow<Id>
where
    Id: Hash,
{
    fn draw(&self, row_ui: &mut TreeListRowUi<'_>) -> HeaderRowResponse {
        match &self.kind {
            TreeListRowKind::Header {
                has_children,
                is_expanded,
                trailing,
            } => self.draw_header(row_ui, *has_children, *is_expanded, trailing.as_deref()),
            TreeListRowKind::Photo { path } => HeaderRowResponse {
                row: self.draw_photo(row_ui, path),
                disclosure: None,
            },
        }
    }

    fn draw_header(
        &self,
        row_ui: &mut TreeListRowUi<'_>,
        has_children: bool,
        is_expanded: bool,
        trailing: Option<&str>,
    ) -> HeaderRowResponse {
        let selected_fill = self.selection_style.selected_fill(row_ui.ui, self.selected);
        let mut disclosure_response = None;

        let row = row_ui.row_frame(selected_fill, self.sense, |ui| {
            TreeListRowUi::indent(ui, self.depth);

            if has_children {
                disclosure_response =
                    Some(TreeListRowUi::disclosure_button(ui, &self.id, is_expanded));
            } else {
                TreeListRowUi::disclosure_spacer(ui);
            }

            ui.add_space(DISCLOSURE_TITLE_SPACING);
            ui.label(self.selection_style.row_text(&self.title, self.selected));

            if let Some(trailing) = trailing {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(trailing).weak());
                });
            }
        });

        HeaderRowResponse {
            row,
            disclosure: disclosure_response,
        }
    }

    fn draw_photo(&self, row_ui: &mut TreeListRowUi<'_>, photo_path: &PathBuf) -> Response {
        let selected_fill = self.selection_style.selected_fill(row_ui.ui, self.selected);

        row_ui.row_frame(selected_fill, self.sense, |ui| {
            TreeListRowUi::indent(ui, self.depth);
            TreeListRowUi::photo_thumbnail(ui, photo_path);
            ui.add_space(PHOTO_TITLE_SPACING);
            ui.label(self.selection_style.row_text(&self.title, self.selected));
        })
    }
}

pub struct TreeList<'a> {
    ui: &'a mut Ui,
    id_salt: egui::Id,
    min_width: f32,
    scroll_to_row_top: Option<usize>,
}

impl<'a> TreeList<'a> {
    pub fn new(ui: &'a mut Ui) -> Self {
        Self {
            ui,
            id_salt: egui::Id::new("__tree_list"),
            min_width: 0.0,
            scroll_to_row_top: None,
        }
    }

    pub fn id_salt(mut self, id_salt: impl Hash) -> Self {
        self.id_salt = egui::Id::new(id_salt);
        self
    }

    pub fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = min_width;
        self
    }

    /// Scrolls the row to the top of the visible list.
    pub fn scroll_to_row_top(mut self, scroll_to_row_top: Option<usize>) -> Self {
        self.scroll_to_row_top = scroll_to_row_top;
        self
    }

    pub fn body(self, add_body_contents: impl for<'body> FnOnce(TreeListBody<'body>)) -> Response {
        self.ui.style_mut().interaction.selectable_labels = false;

        let outer_response = self
            .ui
            .allocate_response(self.ui.available_size(), Sense::click());
        let mut list_ui = self.ui.new_child(
            UiBuilder::new()
                .max_rect(outer_response.rect)
                .layout(*self.ui.layout()),
        );

        let body = TreeListBody {
            ui: &mut list_ui,
            id_salt: self.id_salt,
            min_width: self.min_width,
            scroll_to_row_top: self.scroll_to_row_top,
        };
        add_body_contents(body);

        outer_response
    }
}

pub struct TreeListBody<'a> {
    ui: &'a mut Ui,
    id_salt: egui::Id,
    min_width: f32,
    scroll_to_row_top: Option<usize>,
}

impl TreeListBody<'_> {
    pub fn rows(
        self,
        row_height: f32,
        row_count: usize,
        mut add_row: impl FnMut(TreeListRowUi<'_>),
    ) {
        let table_height = self.ui.available_height();
        let column_width = self.ui.available_width().max(self.min_width);

        let mut table = TableBuilder::new(self.ui)
            .id_salt(self.id_salt)
            .striped(false)
            .auto_shrink([false, false])
            .min_scrolled_height(table_height)
            .max_scroll_height(table_height)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::exact(column_width));

        if let Some(row) = self.scroll_to_row_top {
            table = table.scroll_to_row(row, Some(egui::Align::TOP));
        }

        table.body(|body| {
            body.rows(row_height, row_count, |mut row| {
                let row_index = row.index();
                row.col(|ui| {
                    add_row(TreeListRowUi {
                        ui,
                        row_index,
                        row_height,
                    });
                });
            });
        });
    }
}

pub struct TreeListRowUi<'a> {
    ui: &'a mut Ui,
    row_index: usize,
    row_height: f32,
}

impl TreeListRowUi<'_> {
    pub fn index(&self) -> usize {
        self.row_index
    }

    pub fn add<Id>(&mut self, row: TreeListRow<Id>) -> TreeListRowResponse<Id>
    where
        Id: Hash,
    {
        let row_response = row.draw(self);
        TreeListRowResponse {
            id: row.id,
            response: row_response.row,
            disclosure: row_response.disclosure,
        }
    }

    fn row_frame(
        &mut self,
        selected_fill: Option<Color32>,
        sense: Sense,
        add_contents: impl FnOnce(&mut Ui),
    ) -> Response {
        let (rect, response) = self
            .ui
            .allocate_at_least(Vec2::new(self.ui.available_width(), self.row_height), sense);

        if let Some(color) = selected_fill {
            self.ui.painter().rect_filled(rect, 0.0, color);
        }

        let mut content_ui = self.ui.new_child(
            UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );

        content_ui.horizontal(add_contents);

        response
    }

    fn indent(ui: &mut Ui, depth: usize) {
        ui.add_space(depth as f32 * INDENT_WIDTH);
    }

    fn disclosure_button(ui: &mut Ui, id_salt: impl Hash, is_expanded: bool) -> Response {
        let id = ui.make_persistent_id(id_salt);
        let openness = ui.ctx().animate_bool(id, is_expanded);
        let (_, response) = ui.allocate_exact_size(Vec2::splat(DISCLOSURE_SIZE), Sense::click());
        egui::collapsing_header::paint_default_icon(ui, openness, &response);
        response
    }

    fn disclosure_spacer(ui: &mut Ui) {
        ui.add_space(DISCLOSURE_SIZE);
    }

    fn photo_thumbnail(ui: &mut Ui, photo_path: &PathBuf) {
        let photo_manager: Singleton<PhotoManager> = Dependency::get();
        let photo_clone =
            photo_manager.with_lock(|pm| pm.photo_database.get_photo(photo_path).cloned());

        let texture_handle = if let Some(photo) = photo_clone {
            photo_manager.with_lock_mut(|pm| match pm.thumbnail_texture_for(&photo, ui.ctx()) {
                Ok(Some(texture)) => Some(texture),
                _ => None,
            })
        } else {
            None
        };

        if let Some(handle) = texture_handle {
            ui.add(Image::new(ImageSource::Texture(handle)).max_size(Vec2::splat(THUMBNAIL_SIZE)));
        } else {
            let next_pos = ui.next_widget_position();
            let rect = Rect::from_min_max(
                next_pos,
                next_pos + Vec2::new(THUMBNAIL_SIZE, THUMBNAIL_SIZE),
            );
            ui.allocate_rect(rect, Sense::hover());
            ui.painter()
                .rect_filled(rect, 0.0, theme::color::PLACEHOLDER);
        }
    }
}

pub struct TreeListRowResponse<Id> {
    id: Id,
    response: Response,
    disclosure: Option<Response>,
}

impl<Id> TreeListRowResponse<Id> {
    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn response(&self) -> &Response {
        &self.response
    }

    pub fn disclosure_clicked(&self) -> bool {
        self.disclosure
            .as_ref()
            .map(|response| response.clicked())
            .unwrap_or(false)
    }

    pub fn clicked(&self) -> bool {
        !self.disclosure_clicked() && self.response.clicked()
    }

    pub fn double_clicked(&self) -> bool {
        !self.disclosure_clicked() && self.response.double_clicked()
    }
}
