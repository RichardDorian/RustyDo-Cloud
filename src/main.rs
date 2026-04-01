use std::net::SocketAddr;

use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgPool, postgres::PgPoolOptions};

// Models

#[derive(Serialize, Clone, sqlx::FromRow)]
struct Todo {
    id: i32,
    title: String,
    description: Option<String>,
    due_date: Option<DateTime<Utc>>,
    status: String,
    created_at: DateTime<Utc>,
}

// App state

#[derive(Clone)]
struct AppState {
    database: PgPool,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .unwrap_or(3000);
    let database_url = std::env::var("POSTGRESQL_ADDON_URI").expect("Database URL not defined.");

    println!("{database_url}");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Unable to connect to database.");

    println!("Connected to database!");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS todos (
                id SERIAL PRIMARY KEY,
                title VARCHAR NOT NULL,
                description VARCHAR,
                due_date DATE,
                status VARCHAR CHECK (status IN ('pending', 'done')),
                created_at TIMESTAMPTZ DEFAULT NOW()
            )",
    )
    .execute(&pool)
    .await
    .expect("Cannot initialize database");

    let state = AppState { database: pool };

    let app = Router::new()
        .route("/", get(root))
        .route("/todos", get(todos))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn root() -> &'static str {
    "Hello, World!"
}

async fn todos(State(state): State<AppState>) -> (StatusCode, Json<Vec<Todo>>) {
    let res = sqlx::query_as::<_, Todo>("SELECT * FROM todos ORDER BY created_at DESC")
        .fetch_all(&state.database)
        .await;

    match res {
        Ok(todos) => (StatusCode::OK, todos.into()),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, vec![].into()),
    }
}
