use crate::browse::{
    load_browse_data_from_root, local_shelves_root, BrowseData, CommandEntry, ShelfData, TeamData,
};
use crate::config::{get_team_data_file_path, SharedStorageContext, WebTheme};
use crate::curl_runner::{analyze_command, run_curl_command, CurlRunResponse, RunStore};
use crate::database::{CommandDatabase, SaveCommandOutcome};
use crate::Result;
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::path::{Path as StdPath, PathBuf};
use std::sync::Arc;

const DEFAULT_WEB_PORT: u16 = 4812;
const INDEX_HTML: &str = include_str!("../assets/web/index.html");

#[derive(Clone)]
struct WebState {
    local_shelves_root: PathBuf,
    shared_context: Option<SharedStorageContext>,
    run_store: Arc<RunStore>,
    theme: WebTheme,
}

#[derive(Debug, Serialize)]
struct BrowseResponse {
    local: Vec<WebShelfData>,
    shared: Vec<WebTeamData>,
}

#[derive(Debug, Serialize)]
struct WebTeamData {
    team: String,
    shelves: Vec<WebShelfData>,
}

#[derive(Debug, Serialize)]
struct WebShelfData {
    shelf: String,
    runnable_command_count: usize,
    commands: Vec<WebCommandEntry>,
}

#[derive(Debug, Serialize)]
struct WebCommandEntry {
    command: String,
    preview_command: String,
    description: Option<String>,
    runnable: bool,
    unsupported_reason: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum ShelfScope {
    Local,
    Shared,
}

#[derive(Debug, Deserialize)]
struct RunRequest {
    command: String,
}

#[derive(Debug, Deserialize)]
struct CreateShelfRequest {
    scope: ShelfScope,
    team: Option<String>,
    shelf: String,
}

#[derive(Debug, Deserialize)]
struct SaveCommandRequest {
    scope: ShelfScope,
    team: Option<String>,
    shelf: String,
    original_command: Option<String>,
    command: String,
    description: Option<String>,
}

#[derive(Debug, Serialize)]
struct MutationResponse {
    message: String,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug)]
struct WebError {
    status: StatusCode,
    message: String,
}

impl WebError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorResponse {
                error: self.message,
            }),
        )
            .into_response()
    }
}

pub(crate) fn run_web_server(
    shared_context: Option<SharedStorageContext>,
    requested_port: Option<u16>,
    theme: WebTheme,
) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(run_web_server_async(shared_context, requested_port, theme))
}

async fn run_web_server_async(
    shared_context: Option<SharedStorageContext>,
    requested_port: Option<u16>,
    theme: WebTheme,
) -> Result<()> {
    let app = build_router(WebState {
        local_shelves_root: local_shelves_root(),
        shared_context,
        run_store: Arc::new(RunStore::default()),
        theme,
    });

    let port = requested_port.unwrap_or(DEFAULT_WEB_PORT);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    let address = listener.local_addr()?;
    println!("Shellshelf web UI running at http://{address}");
    axum::serve(listener, app).await?;
    Ok(())
}

fn build_router(state: WebState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/assets/styles.css", get(styles))
        .route("/assets/app.js", get(app_js))
        .route("/api/browse", get(browse))
        .route("/api/shelves", post(create_shelf))
        .route("/api/commands", post(save_command))
        .route("/api/run", post(run_command))
        .route("/api/runs/{id}/body", get(run_body))
        .with_state(state)
}

async fn index(State(state): State<WebState>) -> Html<String> {
    Html(INDEX_HTML.replace("__WEB_THEME__", state.theme.as_str()))
}

async fn styles() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/css; charset=utf-8"),
        )],
        include_str!("../assets/web/styles.css"),
    )
}

async fn app_js() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/javascript; charset=utf-8"),
        )],
        include_str!("../assets/web/app.js"),
    )
}

async fn browse(
    State(state): State<WebState>,
) -> std::result::Result<Json<BrowseResponse>, WebError> {
    let data = load_browse_data_from_root(&state.local_shelves_root, state.shared_context.as_ref())
        .map_err(|error| WebError::internal(format!("Failed to load browse data: {error}")))?;

    Ok(Json(map_browse_response(data)))
}

async fn create_shelf(
    State(state): State<WebState>,
    Json(payload): Json<CreateShelfRequest>,
) -> std::result::Result<Json<MutationResponse>, WebError> {
    let data_file = resolve_data_file_path(
        &state,
        payload.scope,
        payload.team.as_deref(),
        &payload.shelf,
    )?;

    if data_file.exists() {
        return Err(WebError::bad_request(format!(
            "Shelf '{}' already exists.",
            payload.shelf
        )));
    }

    CommandDatabase::new()
        .save_to_file(&data_file)
        .map_err(|error| WebError::internal(format!("Failed to create shelf: {error}")))?;

    Ok(Json(MutationResponse {
        message: match payload.scope {
            ShelfScope::Local => format!("Created shelf '{}'.", payload.shelf),
            ShelfScope::Shared => format!(
                "Created shelf '{}' for team '{}'.",
                payload.shelf,
                payload.team.unwrap_or_default()
            ),
        },
    }))
}

async fn save_command(
    State(state): State<WebState>,
    Json(payload): Json<SaveCommandRequest>,
) -> std::result::Result<Json<MutationResponse>, WebError> {
    let data_file = resolve_data_file_path(
        &state,
        payload.scope,
        payload.team.as_deref(),
        &payload.shelf,
    )?;
    let mut database = CommandDatabase::load_from_file(&data_file)
        .map_err(|error| WebError::internal(format!("Failed to load shelf: {error}")))?;
    let description = payload
        .description
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let outcome = database.save_command(
        payload.original_command.as_deref(),
        payload.command.clone(),
        description,
    );

    if outcome == SaveCommandOutcome::Duplicate {
        return Err(WebError::bad_request(format!(
            "Command already exists in shelf '{}'.",
            payload.shelf
        )));
    }

    database
        .save_to_file(&data_file)
        .map_err(|error| WebError::internal(format!("Failed to save command: {error}")))?;

    Ok(Json(MutationResponse {
        message: match outcome {
            SaveCommandOutcome::Added => format!("Saved command to shelf '{}'.", payload.shelf),
            SaveCommandOutcome::Updated => {
                format!("Updated command in shelf '{}'.", payload.shelf)
            }
            SaveCommandOutcome::Duplicate => unreachable!("duplicate handled above"),
        },
    }))
}

async fn run_command(
    State(state): State<WebState>,
    Json(payload): Json<RunRequest>,
) -> std::result::Result<Json<CurlRunResponse>, WebError> {
    let analysis = analyze_command(&payload.command);
    if !analysis.runnable {
        return Err(WebError::bad_request(
            analysis
                .unsupported_reason
                .unwrap_or_else(|| "Only curl commands can run in the web interface.".to_string()),
        ));
    }

    let response = run_curl_command(&payload.command, &state.run_store)
        .await
        .map_err(|error| WebError::bad_request(error.to_string()))?;

    Ok(Json(response))
}

async fn run_body(
    Path(id): Path<u64>,
    State(state): State<WebState>,
) -> std::result::Result<Response, WebError> {
    let Some(body) = state.run_store.get_body(id) else {
        return Err(WebError::not_found(
            "Run body was not found or is no longer available.",
        ));
    };

    let content_type = HeaderValue::from_str(&body.content_type)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));

    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
        ],
        body.bytes,
    )
        .into_response())
}

fn map_browse_response(data: BrowseData) -> BrowseResponse {
    BrowseResponse {
        local: data.local.into_iter().map(map_shelf).collect(),
        shared: data.shared.into_iter().map(map_team).collect(),
    }
}

fn map_team(team: TeamData) -> WebTeamData {
    WebTeamData {
        team: team.team,
        shelves: team.shelves.into_iter().map(map_shelf).collect(),
    }
}

fn map_shelf(shelf: ShelfData) -> WebShelfData {
    let commands: Vec<WebCommandEntry> = shelf.commands.into_iter().map(map_command).collect();
    let runnable_command_count = commands.iter().filter(|command| command.runnable).count();

    WebShelfData {
        shelf: shelf.shelf,
        runnable_command_count,
        commands,
    }
}

fn map_command(command: CommandEntry) -> WebCommandEntry {
    let analysis = analyze_command(&command.command);
    WebCommandEntry {
        preview_command: format_command_preview(&command.command),
        command: command.command,
        description: command.description,
        runnable: analysis.runnable,
        unsupported_reason: analysis.unsupported_reason,
    }
}

fn resolve_data_file_path(
    state: &WebState,
    scope: ShelfScope,
    team: Option<&str>,
    shelf: &str,
) -> std::result::Result<PathBuf, WebError> {
    crate::config::validate_shelf_name(shelf)
        .map_err(|error| WebError::bad_request(error.to_string()))?;

    match scope {
        ShelfScope::Local => Ok(local_data_file_path(&state.local_shelves_root, shelf)),
        ShelfScope::Shared => {
            let shared_context = state.shared_context.as_ref().ok_or_else(|| {
                WebError::bad_request("Shared repository is not configured for the web interface.")
            })?;
            let team = team.ok_or_else(|| {
                WebError::bad_request("Shared shelf operations require a team name.")
            })?;
            get_team_data_file_path(
                &shared_context.repository_root,
                &shared_context.teams_dir,
                team,
                shelf,
            )
            .map_err(|error| WebError::bad_request(error.to_string()))
        }
    }
}

fn local_data_file_path(local_shelves_root: &StdPath, shelf: &str) -> PathBuf {
    local_shelves_root.join(format!("{shelf}.json"))
}

fn format_command_preview(command: &str) -> String {
    let Ok(tokens) = shell_words::split(command) else {
        return fallback_command_preview(command);
    };

    let Some(first) = tokens.first() else {
        return String::new();
    };

    let mut lines = vec![first.clone()];
    let mut index = 1;

    while index < tokens.len() {
        let token = &tokens[index];

        if let Some((option, value, consumed_next)) = preview_option(token, &tokens, index) {
            lines.push(format!(
                "  {} {}",
                option,
                shell_words::quote(value.as_str())
            ));
            index += if consumed_next { 2 } else { 1 };
            continue;
        }

        if token.starts_with('-') {
            lines.push(format!("  {token}"));
        } else {
            lines.push(format!("  {}", shell_words::quote(token.as_str())));
        }
        index += 1;
    }

    lines.join("\n")
}

fn preview_option(token: &str, tokens: &[String], index: usize) -> Option<(String, String, bool)> {
    const VALUE_OPTIONS: &[&str] = &[
        "-H",
        "--header",
        "-X",
        "--request",
        "-d",
        "--data",
        "--data-raw",
        "--data-binary",
        "--data-urlencode",
        "-F",
        "--form",
        "--json",
        "--url",
        "-u",
        "--user",
        "-A",
        "--user-agent",
    ];

    for option in VALUE_OPTIONS {
        if token == *option {
            if index + 1 < tokens.len() {
                return Some((option.to_string(), tokens[index + 1].clone(), true));
            }
            return None;
        }

        if option.starts_with("--") {
            if let Some(value) = token.strip_prefix(&format!("{option}=")) {
                return Some((option.to_string(), value.to_string(), false));
            }
        } else if let Some(value) = token.strip_prefix(option) {
            if !value.is_empty() {
                return Some((option.to_string(), value.to_string(), false));
            }
        }
    }

    None
}

fn fallback_command_preview(command: &str) -> String {
    command.replace(" --", "\n  --").replace(" -", "\n  -")
}

#[cfg(test)]
mod tests {
    use super::{build_router, format_command_preview, WebState};
    use crate::config::{SharedStorageContext, WebTheme};
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tower::ServiceExt;

    fn build_test_app(temp_dir: &TempDir) -> axum::Router {
        build_router(WebState {
            local_shelves_root: temp_dir.path().join("local-shelves"),
            shared_context: None,
            run_store: Arc::new(crate::curl_runner::RunStore::default()),
            theme: WebTheme::SolarizedDark,
        })
    }

    #[tokio::test]
    async fn test_index_route_serves_html() {
        let temp_dir = TempDir::new().unwrap();
        let app = build_test_app(&temp_dir);

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()["content-type"],
            "text/html; charset=utf-8"
        );
    }

    #[tokio::test]
    async fn test_index_route_injects_theme_name() {
        let temp_dir = TempDir::new().unwrap();
        let app = build_router(WebState {
            local_shelves_root: temp_dir.path().join("local-shelves"),
            shared_context: None,
            run_store: Arc::new(crate::curl_runner::RunStore::default()),
            theme: WebTheme::Giphy,
        });

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let html = String::from_utf8(body.to_vec()).unwrap();

        assert!(html.contains("data-theme=\"giphy\""));
    }

    #[tokio::test]
    async fn test_run_route_rejects_non_curl_commands() {
        let temp_dir = TempDir::new().unwrap();
        let app = build_test_app(&temp_dir);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/run")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"command":"git status"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_missing_run_body_returns_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let app = build_test_app(&temp_dir);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/runs/999/body")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_existing_run_body_returns_media_bytes() {
        let temp_dir = TempDir::new().unwrap();
        let run_store = Arc::new(crate::curl_runner::RunStore::default());
        let id = run_store.store_body(crate::curl_runner::StoredRunBody {
            content_type: "image/gif".to_string(),
            bytes: vec![71_u8, 73, 70],
        });

        let app = build_router(WebState {
            local_shelves_root: temp_dir.path().join("local-shelves"),
            shared_context: None,
            run_store,
            theme: WebTheme::SolarizedDark,
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/runs/{id}/body"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "image/gif");
    }

    #[tokio::test]
    async fn test_create_local_shelf_route_writes_file() {
        let temp_dir = TempDir::new().unwrap();
        let app = build_test_app(&temp_dir);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/shelves")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"scope":"local","shelf":"media"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(temp_dir
            .path()
            .join("local-shelves")
            .join("media.json")
            .exists());
    }

    #[tokio::test]
    async fn test_save_command_route_persists_non_curl_command() {
        let temp_dir = TempDir::new().unwrap();
        let app = build_test_app(&temp_dir);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/commands")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "scope":"local",
                            "shelf":"media",
                            "command":"git status",
                            "description":"Repository status"
                        }"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let saved =
            fs::read_to_string(temp_dir.path().join("local-shelves").join("media.json")).unwrap();
        assert!(saved.contains("git status"));
        assert!(saved.contains("Repository status"));
    }

    #[tokio::test]
    async fn test_create_shared_shelf_route_writes_team_file() {
        let temp_dir = TempDir::new().unwrap();
        let app = build_router(WebState {
            local_shelves_root: temp_dir.path().join("local-shelves"),
            shared_context: Some(SharedStorageContext {
                repository_root: temp_dir.path().join("shared-repo"),
                teams_dir: PathBuf::from("teams"),
            }),
            run_store: Arc::new(crate::curl_runner::RunStore::default()),
            theme: WebTheme::SolarizedDark,
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/shelves")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"scope":"shared","team":"platform","shelf":"media"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(temp_dir
            .path()
            .join("shared-repo")
            .join("teams")
            .join("platform")
            .join("shelves")
            .join("media.json")
            .exists());
    }

    #[test]
    fn test_format_command_preview_breaks_flags_into_lines() {
        let preview = format_command_preview(
            "curl --request POST https://example.com -H'X-API-Key: 123' --form 'file=@cat.gif'",
        );

        assert!(preview.starts_with("curl"));
        assert!(preview.contains("\n  --request 'POST'") || preview.contains("\n  --request POST"));
        assert!(preview.contains("\n  -H 'X-API-Key: 123'"));
        assert!(preview.contains("\n  --form 'file=@cat.gif'"));
    }
}
