use std::net::SocketAddr;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, patch, post},
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, postgres::PgPoolOptions};

// Models

#[derive(Serialize, Clone, sqlx::FromRow)]
struct Todo {
    id: i32,
    title: String,
    description: Option<String>,
    due_date: Option<NaiveDate>,
    status: String,
    created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
struct CreateTodoBody {
    title: Option<String>,
    description: Option<String>,
    due_date: Option<NaiveDate>, // NaiveDate == Postgres' Date
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateTodoBody {
    title: Option<String>,
    description: Option<Option<String>>,
    due_date: Option<Option<NaiveDate>>,
    status: Option<String>,
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
                status VARCHAR CHECK (status IN ('pending', 'done')) DEFAULT 'pending',
                created_at TIMESTAMPTZ DEFAULT NOW()
            )",
    )
    .execute(&pool)
    .await
    .expect("Cannot initialize database");

    let state = AppState { database: pool };

    let app = Router::new()
        .route("/todos", get(todos))
        .route("/todos/overdue", get(overdue_todos))
        .route("/todos", post(create_todo))
        .route("/todos/{id}", patch(update_todo))
        .route("/todos/{id}", delete(delete_todo))
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
        Err(err) => {
            println!("{err}");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

async fn overdue_todos(State(state): State<AppState>) -> (StatusCode, Json<Vec<Todo>>) {
    let res = sqlx::query_as::<_, Todo>(
        "SELECT * FROM todos WHERE due_date <= CURRENT_DATE AND status = 'pending' ORDER BY due_date ASC, created_at DESC",
    )
    .fetch_all(&state.database)
    .await;

    match res {
        Ok(todos) => (StatusCode::OK, todos.into()),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, vec![].into()),
    }
}

async fn update_todo(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(body): Json<UpdateTodoBody>,
) -> StatusCode {
    let title_set = body.title.is_some();
    let description_set = body.description.is_some();
    let due_date_set = body.due_date.is_some();
    let status_set = body.status.is_some();

    if !title_set && !description_set && !due_date_set && !status_set {
        return StatusCode::BAD_REQUEST;
    }

    let title = match body.title {
        Some(title) => {
            let trimmed = title.trim().to_string();
            if trimmed.is_empty() {
                return StatusCode::BAD_REQUEST;
            }
            Some(trimmed)
        }
        None => None,
    };

    let status = match body.status {
        Some(status) => {
            if status != "pending" && status != "done" {
                return StatusCode::BAD_REQUEST;
            }
            Some(status)
        }
        None => None,
    };

    let res = sqlx::query(
        "UPDATE todos
         SET title = CASE WHEN $1 THEN $2 ELSE title END,
             description = CASE WHEN $3 THEN $4 ELSE description END,
             due_date = CASE WHEN $5 THEN $6 ELSE due_date END,
             status = CASE WHEN $7 THEN $8 ELSE status END
         WHERE id = $9",
    )
    .bind(title_set)
    .bind(title)
    .bind(description_set)
    .bind(body.description.unwrap_or(None))
    .bind(due_date_set)
    .bind(body.due_date.unwrap_or(None))
    .bind(status_set)
    .bind(status)
    .bind(id)
    .execute(&state.database)
    .await;

    match res {
        Ok(res) if res.rows_affected() == 0 => StatusCode::NOT_FOUND,
        Ok(_) => StatusCode::OK,
        Err(err) => {
            println!("{err}");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

async fn delete_todo(State(state): State<AppState>, Path(id): Path<i32>) -> StatusCode {
    let res = sqlx::query("DELETE FROM todos WHERE id = $1")
        .bind(id)
        .execute(&state.database)
        .await;

    match res {
        Ok(res) if res.rows_affected() == 0 => StatusCode::NOT_FOUND,
        Ok(_) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
