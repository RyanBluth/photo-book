use egui::{RichText, Ui};

use crate::widget::tree_list::{SelectionStyle, TreeList, TreeListRow, ROW_HEIGHT};

#[derive(Debug, Clone, Default)]
pub struct BookListState;

#[derive(Debug, Clone)]
pub struct BookListEntry {
    pub id: String,
    pub name: String,
    pub page_count: usize,
}

#[derive(Debug, Clone)]
pub struct BookListResponse {
    pub selected: Option<String>,
}

pub struct BookList<'a> {
    state: &'a mut BookListState,
    books: &'a [BookListEntry],
    selected_book_id: Option<&'a str>,
}

impl<'a> BookList<'a> {
    pub fn new(
        state: &'a mut BookListState,
        books: &'a [BookListEntry],
        selected_book_id: Option<&'a str>,
    ) -> Self {
        Self {
            state,
            books,
            selected_book_id,
        }
    }

    pub fn show(&mut self, ui: &mut Ui) -> BookListResponse {
        let _state = &mut self.state;
        let mut selected = None;

        if self.books.is_empty() {
            ui.label(RichText::new("No books").weak());
        } else {
            TreeList::new(ui).id_salt("book_list_scroll").body(|body| {
                body.rows(ROW_HEIGHT, self.books.len(), |mut row| {
                    let Some(book) = self.books.get(row.index()) else {
                        return;
                    };

                    let page_label = match book.page_count {
                        1 => "1 page".to_string(),
                        page_count => format!("{page_count} pages"),
                    };

                    let response = row.add(
                        TreeListRow::header(book.id.clone(), 0, book.name.clone(), false, false)
                            .selected(self.selected_book_id == Some(book.id.as_str()))
                            .selection_style(SelectionStyle::BoldText)
                            .trailing(page_label),
                    );

                    if response.clicked() {
                        selected = Some(book.id.clone());
                    }
                });
            });
        }

        BookListResponse { selected }
    }
}
