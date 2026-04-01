use std::net::SocketAddr;

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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

#[derive(Deserialize)]
struct CreateTodoBody {
    title: Option<String>,
    description: Option<String>,
    due_date: Option<String>,
}

// App state

#[derive(Clone)]
struct AppState {
    database: PgPool,
}

#[derive(Deserialize)]
struct TodosQuery {
    status: Option<String>,
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
        .route("/todos", get(todos))
        .route("/todos", post(create_todo))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn todos(
    State(state): State<AppState>,
    Query(query): Query<TodosQuery>,
) -> (StatusCode, Json<Vec<Todo>>) {
    let res = match query.status.as_deref() {
        Some("pending") | Some("done") => {
            sqlx::query_as::<_, Todo>(
                "SELECT * FROM todos WHERE status = $1 ORDER BY created_at DESC",
            )
            .bind(query.status.as_deref().unwrap_or_default())
            .fetch_all(&state.database)
            .await
        }
        Some(_) => return (StatusCode::BAD_REQUEST, vec![].into()),
        None => {
            sqlx::query_as::<_, Todo>("SELECT * FROM todos ORDER BY created_at DESC")
                .fetch_all(&state.database)
                .await
        }
    };

    match res {
        Ok(todos) => (StatusCode::OK, todos.into()),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, vec![].into()),
    }
}

async fn create_todo(
    State(state): State<AppState>,
    Json(body): Json<CreateTodoBody>,
) -> StatusCode {
    let title = body.title.unwrap_or_default().trim().to_string();
    if title.is_empty() {
        return StatusCode::BAD_REQUEST;
    }

    let res = sqlx::query_as::<_, Todo>(
        "INSERT INTO todos(title, description, due_date) VALUES ($1, $2, $3) RETURNING id, title, description, due_date, status, created_at",
    )
    .bind(title)
    .bind(body.description)
    .bind(body.due_date)
    .fetch_one(&state.database)
    .await;

    match res {
        Ok(_) => StatusCode::CREATED,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
