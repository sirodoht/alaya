use askama::Template;
use axum::{
    extract::{Form, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::{Deserialize, Serialize};
use std::env;

use crate::AppState;
use crate::database::Database;
use crate::templates::{LoginTemplate, ProfileTemplate, SignupTemplate};

// User-related structures
#[derive(sqlx::FromRow, Serialize)]
pub struct User {
    pub id: String,
    pub username: String,
    #[serde(skip)] // Never serialize password hash
    pub password_hash: String,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct SignupForm {
    pub username: String,
    pub password: String,
    pub confirm_password: String,
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

    if signups_disabled() {
        return signup_disabled_response();
    }

    render_signup(String::new(), None)
}

pub async fn signup_submit(State(db): State<AppState>, Form(form): Form<SignupForm>) -> Response {
    if signups_disabled() {
        return signup_disabled_response();
    }

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

pub async fn profile_page(State(db): State<AppState>, headers: HeaderMap) -> Response {
    let user = current_user(&db, &headers).await;

    if user.is_none() {
        return Redirect::to("/login").into_response();
    }

    let book_count = db.get_book_count().await.unwrap_or(0);

    let template = ProfileTemplate {
        is_authenticated: true,
        signups_disabled: signups_disabled(),
        username: user.map(|u| u.username).unwrap_or_default(),
        book_count,
    };

    Html(template.render().unwrap()).into_response()
}

fn render_login(form_username: String, error_message: Option<String>) -> Response {
    let template = LoginTemplate {
        is_authenticated: false,
        signups_disabled: signups_disabled(),
        username: String::new(),
        form_username,
        error_message,
    };

    Html(template.render().unwrap()).into_response()
}

fn render_signup(form_username: String, error_message: Option<String>) -> Response {
    let template = SignupTemplate {
        is_authenticated: false,
        signups_disabled: signups_disabled(),
        username: String::new(),
        form_username,
        error_message,
    };

    Html(template.render().unwrap()).into_response()
}

pub async fn current_user(db: &Database, headers: &HeaderMap) -> Option<User> {
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

pub fn signups_disabled() -> bool {
    env::var("DISABLE_SIGNUPS")
        .map(|value| value.trim() == "1")
        .unwrap_or(false)
}

fn signup_disabled_response() -> Response {
    (StatusCode::FORBIDDEN, "signups are disabled.").into_response()
}
