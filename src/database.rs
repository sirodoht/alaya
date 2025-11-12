use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use sqlx::{Pool, Row, Sqlite, SqlitePool, migrate::MigrateDatabase};
use std::{fs, path::Path};

pub struct Database {
    pub pool: Pool<Sqlite>,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        // Create database if it doesn't exist
        if !Sqlite::database_exists(database_url).await.unwrap_or(false) {
            println!("Creating database {}", database_url);
            match Sqlite::create_database(database_url).await {
                Ok(_) => println!("Successfully created database"),
                Err(error) => panic!("Error creating database: {}", error),
            }
        } else {
            println!("Database already exists");
        }

        // Connect to database
        let pool = SqlitePool::connect(database_url).await?;

        // Disable WAL mode to avoid -shm and -wal files
        sqlx::query("PRAGMA journal_mode = DELETE")
            .execute(&pool)
            .await?;

        Ok(Database { pool })
    }

    pub async fn run_migrations(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Running database migrations...");

        // Create migrations table if it doesn't exist
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS _migrations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                filename TEXT NOT NULL UNIQUE,
                executed_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Get all migration files
        let migrations_dir = Path::new("migrations");
        if !migrations_dir.exists() {
            println!("Migrations directory not found");
            return Ok(());
        }

        let mut entries: Vec<_> = fs::read_dir(migrations_dir)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext == "sql")
                    .unwrap_or(false)
            })
            .collect();

        entries.sort_by_key(|entry| entry.file_name());

        for entry in entries {
            let filename = entry.file_name().to_string_lossy().to_string();

            // Check if migration has already been executed
            let executed = sqlx::query("SELECT filename FROM _migrations WHERE filename = ?")
                .bind(&filename)
                .fetch_optional(&self.pool)
                .await?
                .is_some();

            if executed {
                println!("Migration {} already executed, skipping", filename);
                continue;
            }

            println!("Executing migration: {}", filename);

            // Read and execute migration file
            let migration_sql = fs::read_to_string(entry.path())?;

            // Execute the migration in a transaction
            let mut tx = self.pool.begin().await?;

            // Split by semicolons and execute each statement
            for statement in migration_sql.split(';') {
                let statement = statement.trim();
                if !statement.is_empty() {
                    sqlx::query(statement).execute(&mut *tx).await?;
                }
            }

            // Record the migration as executed
            sqlx::query(
                "INSERT INTO _migrations (filename, executed_at) VALUES (?, datetime('now'))",
            )
            .bind(&filename)
            .execute(&mut *tx)
            .await?;

            tx.commit().await?;

            println!("Successfully executed migration: {}", filename);
        }

        println!("All migrations completed");
        Ok(())
    }

    // User-related database methods
    pub async fn get_all_users(&self) -> Result<Vec<crate::User>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, username, password_hash, created_at FROM users ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        let users = rows
            .into_iter()
            .map(|row| crate::User {
                id: row.get("id"),
                username: row.get("username"),
                password_hash: row.get("password_hash"),
                created_at: row.get("created_at"),
            })
            .collect();

        Ok(users)
    }

    pub async fn create_user(
        &self,
        username: &str,
        password: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // Check if username already exists
        let existing_user = sqlx::query("SELECT id FROM users WHERE username = ?")
            .bind(username)
            .fetch_optional(&self.pool)
            .await?;

        if existing_user.is_some() {
            return Err("Username already exists".into());
        }

        // Hash the password
        let password_hash = self.hash_password(password)?;

        // Generate new user ID
        let user_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        // Insert user into database
        sqlx::query(
            "INSERT INTO users (id, username, password_hash, created_at, updated_at) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&user_id)
        .bind(username)
        .bind(&password_hash)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(user_id)
    }

    pub async fn verify_user(
        &self,
        username: &str,
        password: &str,
    ) -> Result<Option<crate::User>, Box<dyn std::error::Error>> {
        let user_row = sqlx::query(
            "SELECT id, username, password_hash, created_at FROM users WHERE username = ?",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = user_row {
            let stored_hash: String = row.get("password_hash");

            if self.verify_password(password, &stored_hash)? {
                let user = crate::User {
                    id: row.get("id"),
                    username: row.get("username"),
                    password_hash: stored_hash,
                    created_at: row.get("created_at"),
                };
                Ok(Some(user))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    fn hash_password(&self, password: &str) -> Result<String, Box<dyn std::error::Error>> {
        let argon2 = Argon2::default();
        let salt = SaltString::generate(&mut OsRng);
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| format!("Password hashing failed: {}", e))?
            .to_string();
        Ok(password_hash)
    }

    fn verify_password(
        &self,
        password: &str,
        hash: &str,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let argon2 = Argon2::default();
        let parsed_hash =
            PasswordHash::new(hash).map_err(|e| format!("Invalid password hash: {}", e))?;

        Ok(argon2
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    }

    // Session management methods
    pub async fn create_session(
        &self,
        user_id: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // Generate a simple session token (UUID)
        let token = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let session_id = uuid::Uuid::new_v4().to_string();

        // Insert session into database (no expiration)
        sqlx::query("INSERT INTO sessions (id, user_id, token, created_at) VALUES (?, ?, ?, ?)")
            .bind(&session_id)
            .bind(user_id)
            .bind(&token)
            .bind(&now)
            .execute(&self.pool)
            .await?;

        Ok(token)
    }

    pub async fn validate_session(
        &self,
        token: &str,
    ) -> Result<Option<crate::User>, Box<dyn std::error::Error>> {
        let session_row = sqlx::query(
            "SELECT s.user_id, u.username, u.password_hash, u.created_at
             FROM sessions s
             JOIN users u ON s.user_id = u.id
             WHERE s.token = ?",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = session_row {
            let user = crate::User {
                id: row.get("user_id"),
                username: row.get("username"),
                password_hash: row.get("password_hash"),
                created_at: row.get("created_at"),
            };
            Ok(Some(user))
        } else {
            Ok(None)
        }
    }
}
