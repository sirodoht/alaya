use askama::Template;
use axum::{
    extract::{Form, Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::{Deserialize, Serialize};
use std::env;

use crate::AppState;
use crate::auth::{current_user, signups_disabled};
use crate::templates::{BookDetailTemplate, BookFormTemplate, BookListTemplate};

// Book-related structures
#[derive(sqlx::FromRow, Serialize, Clone)]
pub struct Book {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub isbn: Option<String>,
    pub publication_year: Option<i32>,
    pub filepath: Option<String>,
    pub created_at: String,
}

impl Book {
    pub fn created_date(&self) -> &str {
        self.created_at
            .split('T')
            .next()
            .unwrap_or(&self.created_at)
    }
}

#[derive(Deserialize)]
pub struct CreateBookForm {
    pub title: String,
    pub author: String,
    pub isbn: String,
    pub publication_year: String,
}

pub async fn book_list(State(db): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let user = current_user(&db, &headers).await;
    let books = db.get_all_books().await.unwrap_or_default();

    let template = BookListTemplate {
        is_authenticated: user.is_some(),
        signups_disabled: signups_disabled(),
        username: user.map(|u| u.username).unwrap_or_default(),
        books,
    };

    Html(template.render().unwrap())
}

pub async fn book_form_page(State(db): State<AppState>, headers: HeaderMap) -> Response {
    let user = current_user(&db, &headers).await;

    if user.is_none() {
        return Redirect::to("/login").into_response();
    }

    let template = BookFormTemplate {
        is_authenticated: true,
        signups_disabled: signups_disabled(),
        username: user.map(|u| u.username).unwrap_or_default(),
        error_message: None,
    };

    Html(template.render().unwrap()).into_response()
}

pub async fn book_create(
    State(db): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<CreateBookForm>,
) -> Response {
    let user = current_user(&db, &headers).await;

    let Some(user) = user else {
        return Redirect::to("/login").into_response();
    };

    let title = form.title.trim();
    if title.is_empty() {
        let template = BookFormTemplate {
            is_authenticated: true,
            signups_disabled: signups_disabled(),
            username: user.username,
            error_message: Some("Title is required".to_string()),
        };
        return Html(template.render().unwrap()).into_response();
    }

    let author = if form.author.trim().is_empty() {
        None
    } else {
        Some(form.author.trim())
    };

    let isbn = if form.isbn.trim().is_empty() {
        None
    } else {
        Some(form.isbn.trim())
    };

    let publication_year = form.publication_year.trim().parse::<i32>().ok();

    match db.create_book(title, author, isbn, publication_year).await {
        Ok(_) => Redirect::to("/").into_response(),
        Err(error) => {
            eprintln!("Book creation error: {error}");
            let template = BookFormTemplate {
                is_authenticated: true,
                signups_disabled: signups_disabled(),
                username: user.username,
                error_message: Some("Could not create book. Please try again.".to_string()),
            };
            Html(template.render().unwrap()).into_response()
        }
    }
}

pub async fn book_detail(
    State(db): State<AppState>,
    headers: HeaderMap,
    Path(book_id): Path<String>,
) -> Response {
    let user = current_user(&db, &headers).await;

    match db.get_book_by_id(&book_id).await {
        Ok(Some(book)) => {
            let template = BookDetailTemplate {
                is_authenticated: user.is_some(),
                signups_disabled: signups_disabled(),
                username: user.map(|u| u.username).unwrap_or_default(),
                book,
            };
            Html(template.render().unwrap()).into_response()
        }
        Ok(None) => Redirect::to("/").into_response(),
        Err(error) => {
            eprintln!("Error fetching book: {error}");
            Redirect::to("/").into_response()
        }
    }
}

pub async fn book_delete(
    State(db): State<AppState>,
    headers: HeaderMap,
    Path(book_id): Path<String>,
) -> Response {
    let user = current_user(&db, &headers).await;

    if user.is_none() {
        return Redirect::to("/login").into_response();
    }

    match db.delete_book(&book_id).await {
        Ok(_) => Redirect::to("/").into_response(),
        Err(error) => {
            eprintln!("Error deleting book: {error}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Could not delete book").into_response()
        }
    }
}

pub async fn book_download(State(db): State<AppState>, Path(book_id): Path<String>) -> Response {
    let book = match db.get_book_by_id(&book_id).await {
        Ok(Some(book)) => book,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, "Book not found").into_response();
        }
        Err(error) => {
            eprintln!("Error fetching book: {error}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    };

    let Some(filepath) = &book.filepath else {
        return (StatusCode::NOT_FOUND, "No file associated with this book").into_response();
    };

    // Get library path from environment, default to current directory
    let library_path = env::var("LIBRARY_PATH").unwrap_or_else(|_| ".".to_string());
    let full_path = std::path::Path::new(&library_path).join(filepath);

    if !full_path.exists() {
        eprintln!("File not found: {}", full_path.display());
        return (StatusCode::NOT_FOUND, "File not found on disk").into_response();
    }

    let file_contents = match std::fs::read(&full_path) {
        Ok(contents) => contents,
        Err(error) => {
            eprintln!("Error reading file: {error}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Could not read file").into_response();
        }
    };

    // Determine content type based on extension
    let content_type = match full_path.extension().and_then(|e| e.to_str()) {
        Some("pdf") => "application/pdf",
        Some("epub") => "application/epub+zip",
        Some("mobi") => "application/x-mobipocket-ebook",
        Some("txt") => "text/plain",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        _ => "application/octet-stream",
    };

    // Get filename for Content-Disposition header
    let filename = full_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download");

    let headers = [
        (header::CONTENT_TYPE, content_type),
        (
            header::CONTENT_DISPOSITION,
            &format!("attachment; filename=\"{}\"", filename),
        ),
    ];

    (headers, file_contents).into_response()
}
