use axum::{
    Router,
    routing::{get, post},
};
use std::sync::Arc;

pub mod auth;
pub mod books;
pub mod database;
pub mod gpt;
pub mod templates;

pub use auth::User;
pub use books::Book;
pub use database::Database;

// Application state
pub type AppState = Arc<Database>;

// App creation function
pub fn create_app(db: AppState) -> Router {
    use auth::{login_page, login_submit, logout, profile_page, signup_page, signup_submit};
    use books::{
        book_create, book_delete, book_detail, book_download, book_edit_notes_page,
        book_edit_notes_submit, book_edit_page, book_edit_submit, book_form_page, book_list,
        quick_add_page, quick_add_submit,
    };

    Router::new()
        .route("/", get(book_list))
        .route("/login", get(login_page).post(login_submit))
        .route("/signup", get(signup_page).post(signup_submit))
        .route("/logout", post(logout))
        .route("/profile", get(profile_page))
        .route("/books/new", get(book_form_page).post(book_create))
        .route(
            "/books/quick-add",
            get(quick_add_page).post(quick_add_submit),
        )
        .route("/books/{id}", get(book_detail))
        .route(
            "/books/{id}/edit",
            get(book_edit_page).post(book_edit_submit),
        )
        .route(
            "/books/{id}/edit-notes",
            get(book_edit_notes_page).post(book_edit_notes_submit),
        )
        .route("/books/{id}/delete", post(book_delete))
        .route("/books/{id}/download", get(book_download))
        .with_state(db)
}
