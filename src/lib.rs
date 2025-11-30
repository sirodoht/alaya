use askama::Template;
use axum::{
    Router,
    extract::{Form, Path, State},
    http::{HeaderMap, HeaderValue, header},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub mod database;
pub use database::Database;

// Application state
pub type AppState = Arc<Database>;

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub is_authenticated: bool,
    pub username: String,
    pub books: Vec<Book>,
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub is_authenticated: bool,
    pub username: String,
    pub form_username: String,
    pub error_message: Option<String>,
}

#[derive(Template)]
#[template(path = "signup.html")]
pub struct SignupTemplate {
    pub is_authenticated: bool,
    pub username: String,
    pub form_username: String,
    pub error_message: Option<String>,
}

#[derive(Template)]
#[template(path = "book_form.html")]
pub struct BookFormTemplate {
    pub is_authenticated: bool,
    pub username: String,
    pub error_message: Option<String>,
}

#[derive(Template)]
#[template(path = "book_detail.html")]
pub struct BookDetailTemplate {
    pub is_authenticated: bool,
    pub username: String,
    pub book: Book,
}

// User-related structures for API
#[derive(sqlx::FromRow, Serialize)]
pub struct User {
    pub id: String,
    pub username: String,
    #[serde(skip)] // Never serialize password hash
    pub password_hash: String,
    pub created_at: String,
}

// Book-related structures
#[derive(sqlx::FromRow, Serialize)]
pub struct Book {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub isbn: Option<String>,
    pub publication_year: Option<i32>,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    /// The username to authenticate
    pub username: String,
    /// The password to authenticate
    pub password: String,
}

pub async fn dashboard(State(db): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let user = current_user(&db, &headers).await;
    let books = db.get_all_books().await.unwrap_or_default();

    let template = DashboardTemplate {
        is_authenticated: user.is_some(),
        username: user.map(|u| u.username).unwrap_or_default(),
        books,
    };

    Html(template.render().unwrap())
}

pub async fn login_page(State(db): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if current_user(&db, &headers).await.is_some() {
        return Redirect::to("/").into_response();
    }

    render_login(String::new(), None)
}

pub async fn login_submit(State(db): State<AppState>, Form(form): Form<LoginRequest>) -> Response {
    let username = form.username.trim().to_string();
    let password = form.password;

    if username.is_empty() {
        return render_login(String::new(), Some("Username cannot be empty".to_string()));
    }

    if password.is_empty() {
        return render_login(
            username.clone(),
            Some("Password cannot be empty".to_string()),
        );
    }

    match db.verify_user(&username, &password).await {
        Ok(Some(user)) => match db.create_session(&user.id).await {
            Ok(token) => {
                let mut response = Redirect::to("/").into_response();
                if let Some(cookie) = build_session_cookie(&token) {
                    response.headers_mut().insert(header::SET_COOKIE, cookie);
                }
                response
            }
            Err(error) => {
                eprintln!("Session creation error: {error}");
                render_login(
                    username,
                    Some("Could not create session. Please try again.".to_string()),
                )
            }
        },
        Ok(None) => render_login(username, Some("Invalid username or password".to_string())),
        Err(error) => {
            eprintln!("Authentication error: {error}");
            render_login(username, Some("Authentication failed".to_string()))
        }
    }
}

pub async fn signup_page(State(db): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if current_user(&db, &headers).await.is_some() {
        return Redirect::to("/").into_response();
    }

    render_signup(String::new(), None)
}

#[derive(Deserialize)]
pub struct SignupForm {
    pub username: String,
    pub password: String,
    pub confirm_password: String,
}

#[derive(Deserialize)]
pub struct CreateBookForm {
    pub title: String,
    pub author: String,
    pub isbn: String,
    pub publication_year: String,
}

pub async fn signup_submit(State(db): State<AppState>, Form(form): Form<SignupForm>) -> Response {
    let username = form.username.trim().to_string();
    let password = form.password;
    let confirm_password = form.confirm_password;

    if username.is_empty() {
        return render_signup(String::new(), Some("Username cannot be empty".to_string()));
    }

    if password.len() < 8 {
        return render_signup(
            username.clone(),
            Some("Password must be at least 8 characters long".to_string()),
        );
    }

    if password != confirm_password {
        return render_signup(username.clone(), Some("Passwords do not match".to_string()));
    }

    match db.create_user(&username, &password).await {
        Ok(user_id) => match db.create_session(&user_id).await {
            Ok(token) => {
                let mut response = Redirect::to("/").into_response();
                if let Some(cookie) = build_session_cookie(&token) {
                    response.headers_mut().insert(header::SET_COOKIE, cookie);
                }
                response
            }
            Err(error) => {
                eprintln!("Session creation error: {error}");
                render_signup(
                    username,
                    Some("Could not create session. Please try again.".to_string()),
                )
            }
        },
        Err(error) => {
            if error.to_string().contains("already exists") {
                render_signup(username, Some("Username already exists".to_string()))
            } else {
                eprintln!("User registration error: {error}");
                render_signup(
                    username,
                    Some("Could not create account. Please try again.".to_string()),
                )
            }
        }
    }
}

pub async fn logout(State(db): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(token) = extract_session_token(&headers)
        && let Err(error) = db.delete_session(&token).await
    {
        eprintln!("Failed to delete session: {error}");
    }

    let mut response = Redirect::to("/login").into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, clear_session_cookie());
    response
}

pub async fn book_form_page(State(db): State<AppState>, headers: HeaderMap) -> Response {
    let user = current_user(&db, &headers).await;

    if user.is_none() {
        return Redirect::to("/login").into_response();
    }

    let template = BookFormTemplate {
        is_authenticated: true,
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

fn render_login(form_username: String, error_message: Option<String>) -> Response {
    let template = LoginTemplate {
        is_authenticated: false,
        username: String::new(),
        form_username,
        error_message,
    };

    Html(template.render().unwrap()).into_response()
}

fn render_signup(form_username: String, error_message: Option<String>) -> Response {
    let template = SignupTemplate {
        is_authenticated: false,
        username: String::new(),
        form_username,
        error_message,
    };

    Html(template.render().unwrap()).into_response()
}

async fn current_user(db: &Database, headers: &HeaderMap) -> Option<crate::User> {
    let token = extract_session_token(headers)?;
    db.validate_session(&token).await.ok()?
}

fn extract_session_token(headers: &HeaderMap) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;

    for cookie in cookie_header.split(';') {
        let trimmed = cookie.trim();
        if let Some(rest) = trimmed.strip_prefix("session_token=") {
            return Some(rest.to_string());
        }
    }

    None
}

fn build_session_cookie(token: &str) -> Option<HeaderValue> {
    HeaderValue::from_str(&format!(
        "session_token={token}; HttpOnly; Path=/; SameSite=Lax; Max-Age=604800"
    ))
    .ok()
}

fn clear_session_cookie() -> HeaderValue {
    HeaderValue::from_static("session_token=; Max-Age=0; Path=/; HttpOnly; SameSite=Lax")
}

// App creation function
pub fn create_app(db: AppState) -> Router {
    Router::new()
        .route("/", get(dashboard))
        .route("/login", get(login_page).post(login_submit))
        .route("/signup", get(signup_page).post(signup_submit))
        .route("/logout", post(logout))
        .route("/books/new", get(book_form_page).post(book_create))
        .route("/books/{id}", get(book_detail))
        .with_state(db)
}
