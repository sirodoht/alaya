use askama::Template;
use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Json},
    routing::get,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub mod database;
pub use database::Database;

// Application state
pub type AppState = Arc<Database>;

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {}

// User-related structures for API
#[derive(sqlx::FromRow, Serialize)]
pub struct User {
    pub id: String,
    pub username: String,
    #[serde(skip)] // Never serialize password hash
    pub password_hash: String,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    /// The username for the new account
    pub username: String,
    /// The password for the new account
    pub password: String,
}

#[derive(Serialize, Deserialize)]
pub struct RegisterResponse {}

#[derive(Deserialize)]
pub struct LoginRequest {
    /// The username to authenticate
    pub username: String,
    /// The password to authenticate
    pub password: String,
}

#[derive(Serialize, Deserialize)]
pub struct LoginResponse {
    /// Authentication token
    pub token: Option<String>,
}

#[derive(Serialize)]
pub struct UserInfo {
    /// Unique user ID
    pub id: String,
    /// Username
    pub username: String,
    /// Timestamp when the user was created
    pub created_at: String,
}

#[derive(Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Error message describing what went wrong
    pub error: String,
}

pub async fn dashboard(State(_db): State<AppState>) -> impl IntoResponse {
    let template = DashboardTemplate {};
    Html(template.render().unwrap())
}

// API Handler functions
pub async fn register_user(
    State(db): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validate input
    if request.username.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Username cannot be empty".to_string(),
            }),
        ));
    }

    // Attempt to create user
    match db.create_user(&request.username, &request.password).await {
        Ok(_) => Ok(Json(RegisterResponse {})),
        Err(e) => {
            if e.to_string().contains("already exists") {
                Err((
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: "Username already exists".to_string(),
                    }),
                ))
            } else {
                eprintln!("User registration error: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Internal server error".to_string(),
                    }),
                ))
            }
        }
    }
}

pub async fn login_user(
    State(db): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validate input
    if request.username.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Username cannot be empty".to_string(),
            }),
        ));
    }

    if request.password.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Password cannot be empty".to_string(),
            }),
        ));
    }

    // Authenticate user and create session
    let user = match db.verify_user(&request.username, &request.password).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "Invalid username or password".to_string(),
                }),
            ));
        }
        Err(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Authentication failed".to_string(),
                }),
            ));
        }
    };

    // Create and store session token in database
    let token = match db.create_session(&user.id).await {
        Ok(token) => token,
        Err(e) => {
            eprintln!("Session creation error: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Could not create session".to_string(),
                }),
            ));
        }
    };

    Ok(Json(LoginResponse { token: Some(token) }))
}

// Helper function to extract token from Authorization header
pub fn extract_token_from_headers(headers: &HeaderMap) -> Result<String, String> {
    let auth_header = headers
        .get("Authorization")
        .ok_or("Missing Authorization header")?
        .to_str()
        .map_err(|_| "Invalid Authorization header")?;

    if let Some(token) = auth_header.strip_prefix("Bearer ") {
        Ok(token.to_string())
    } else {
        Err("Invalid Authorization format. Expected 'Bearer <token>'".to_string())
    }
}

// App creation function
pub fn create_app(db: AppState) -> Router {
    Router::new().route("/", get(dashboard)).with_state(db)
}
