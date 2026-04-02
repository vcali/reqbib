use crate::Result;
use serde::Serialize;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tempfile::NamedTempFile;

const HTTP_CODE_MARKER: &str = "__SHELLSHELF_HTTP_CODE__:";
const MAX_STORED_BODIES: usize = 24;
const FORBIDDEN_OPTIONS: &[&str] = &[
    "-D",
    "--dump-header",
    "-i",
    "--include",
    "-I",
    "--head",
    "-K",
    "--config",
    "-o",
    "--output",
    "-O",
    "--remote-name",
    "--remote-name-all",
    "-w",
    "--write-out",
    "--next",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CommandAnalysis {
    pub(crate) runnable: bool,
    pub(crate) unsupported_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ResponseHeader {
    pub(crate) name: String,
    pub(crate) value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RequestDetails {
    pub(crate) method: String,
    pub(crate) url: Option<String>,
    pub(crate) headers: Vec<ResponseHeader>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ResponseBodyKind {
    Text,
    Image,
    Video,
    Binary,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CurlRunResponse {
    pub(crate) exit_code: i32,
    pub(crate) success: bool,
    pub(crate) request: RequestDetails,
    pub(crate) http_status: Option<u16>,
    pub(crate) content_type: Option<String>,
    pub(crate) headers: Vec<ResponseHeader>,
    pub(crate) stderr: String,
    pub(crate) body_kind: ResponseBodyKind,
    pub(crate) body_text: Option<String>,
    pub(crate) body_note: Option<String>,
    pub(crate) preview_url: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct StoredRunBody {
    pub(crate) content_type: String,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Debug, Default)]
pub(crate) struct RunStore {
    next_id: AtomicU64,
    bodies: Mutex<RunStoreInner>,
}

#[derive(Debug, Default)]
struct RunStoreInner {
    bodies: HashMap<u64, StoredRunBody>,
    order: VecDeque<u64>,
}

impl RunStore {
    pub(crate) fn store_body(&self, body: StoredRunBody) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let mut inner = self.bodies.lock().unwrap();
        inner.bodies.insert(id, body);
        inner.order.push_back(id);

        while inner.order.len() > MAX_STORED_BODIES {
            if let Some(oldest) = inner.order.pop_front() {
                inner.bodies.remove(&oldest);
            }
        }

        id
    }

    pub(crate) fn get_body(&self, id: u64) -> Option<StoredRunBody> {
        self.bodies.lock().unwrap().bodies.get(&id).cloned()
    }
}

pub(crate) fn analyze_command(command: &str) -> CommandAnalysis {
    let args = match shell_words::split(command) {
        Ok(args) => args,
        Err(error) => {
            return CommandAnalysis {
                runnable: false,
                unsupported_reason: Some(format!("Command parsing failed: {error}")),
            };
        }
    };

    if args.is_empty() {
        return CommandAnalysis {
            runnable: false,
            unsupported_reason: Some("Command is empty.".to_string()),
        };
    }

    if args[0] != "curl" {
        return CommandAnalysis {
            runnable: false,
            unsupported_reason: Some(
                "Only curl commands can run in the web interface.".to_string(),
            ),
        };
    }

    if let Some(option) = args.iter().skip(1).find(|arg| is_forbidden_option(arg)) {
        return CommandAnalysis {
            runnable: false,
            unsupported_reason: Some(format!(
                "This curl command uses unsupported option '{option}' in the web interface."
            )),
        };
    }

    CommandAnalysis {
        runnable: true,
        unsupported_reason: None,
    }
}

pub(crate) async fn run_curl_command(
    command: &str,
    run_store: &RunStore,
) -> Result<CurlRunResponse> {
    let analysis = analyze_command(command);
    if !analysis.runnable {
        return Err(analysis
            .unsupported_reason
            .unwrap_or_else(|| "Only curl commands can run in the web interface.".to_string())
            .into());
    }

    let args = shell_words::split(command)?;
    let request = parse_request_details(&args);
    let header_file = NamedTempFile::new()?;
    let body_file = NamedTempFile::new()?;
    let trace_file = NamedTempFile::new()?;
    let header_path = header_file.path().to_path_buf();
    let body_path = body_file.path().to_path_buf();
    let trace_path = trace_file.path().to_path_buf();

    let mut curl = tokio::process::Command::new("curl");
    curl.args(args.iter().skip(1))
        .arg("--silent")
        .arg("--show-error")
        .arg("--trace-ascii")
        .arg(&trace_path)
        .arg("--dump-header")
        .arg(&header_path)
        .arg("--output")
        .arg(&body_path)
        .arg("--write-out")
        .arg(format!("{HTTP_CODE_MARKER}%{{http_code}}"));

    let output = curl.output().await?;
    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let http_status = parse_http_status(&stdout);
    let header_bytes = std::fs::read(&header_path)?;
    let body_bytes = std::fs::read(&body_path)?;
    let trace_bytes = std::fs::read(&trace_path)?;
    let request = merge_request_details(request, parse_last_request_from_trace(&trace_bytes));
    let headers = parse_last_response_headers(&header_bytes);
    let content_type = header_value(&headers, "content-type").map(normalize_content_type);
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    let (body_kind, body_text, body_note, preview_url) =
        build_body_representation(run_store, &body_bytes, content_type.as_deref());

    Ok(CurlRunResponse {
        exit_code,
        success: output.status.success(),
        request,
        http_status,
        content_type,
        headers,
        stderr,
        body_kind,
        body_text,
        body_note,
        preview_url,
    })
}

fn merge_request_details(
    fallback: RequestDetails,
    traced: Option<RequestDetails>,
) -> RequestDetails {
    let Some(traced) = traced else {
        return fallback;
    };

    RequestDetails {
        method: if traced.method.is_empty() {
            fallback.method
        } else {
            traced.method
        },
        url: fallback.url.or(traced.url),
        headers: if traced.headers.is_empty() {
            fallback.headers
        } else {
            traced.headers
        },
    }
}

fn parse_request_details(args: &[String]) -> RequestDetails {
    let mut method = None::<String>;
    let mut url = None::<String>;
    let mut headers = Vec::new();
    let mut saw_body = false;
    let mut index = 1;

    while index < args.len() {
        let arg = &args[index];

        if let Some((value, consumed_next)) =
            request_option_value(arg, args, index, &["-H", "--header"])
        {
            if let Some((name, value)) = value.split_once(':') {
                headers.push(ResponseHeader {
                    name: name.trim().to_string(),
                    value: value.trim().to_string(),
                });
            }
            index += if consumed_next { 2 } else { 1 };
            continue;
        }

        if let Some((value, consumed_next)) =
            request_option_value(arg, args, index, &["-X", "--request"])
        {
            method = Some(value.to_ascii_uppercase());
            index += if consumed_next { 2 } else { 1 };
            continue;
        }

        if let Some((value, consumed_next)) = request_option_value(arg, args, index, &["--url"]) {
            url = Some(value);
            index += if consumed_next { 2 } else { 1 };
            continue;
        }

        if is_body_option(arg) {
            saw_body = true;
            if option_consumes_value(arg)
                && index + 1 < args.len()
                && !args[index + 1].starts_with('-')
            {
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }

        if arg == "--head" || arg == "-I" {
            method = Some("HEAD".to_string());
            index += 1;
            continue;
        }

        if !arg.starts_with('-') && url.is_none() {
            url = Some(arg.clone());
        }

        index += 1;
    }

    let method = method.unwrap_or_else(|| {
        if saw_body {
            "POST".to_string()
        } else {
            "GET".to_string()
        }
    });

    RequestDetails {
        method,
        url,
        headers,
    }
}

fn request_option_value(
    arg: &str,
    args: &[String],
    index: usize,
    option_names: &[&str],
) -> Option<(String, bool)> {
    for option in option_names {
        if arg == *option {
            if index + 1 < args.len() {
                return Some((args[index + 1].clone(), true));
            }
            return None;
        }

        if option.starts_with("--") {
            if let Some(value) = arg.strip_prefix(&format!("{option}=")) {
                return Some((value.to_string(), false));
            }
        } else if let Some(value) = arg.strip_prefix(option) {
            if !value.is_empty() {
                return Some((value.to_string(), false));
            }
        }
    }

    None
}

fn is_body_option(arg: &str) -> bool {
    matches!(
        arg,
        "-d" | "--data"
            | "--data-raw"
            | "--data-binary"
            | "--data-urlencode"
            | "-F"
            | "--form"
            | "--json"
    ) || arg.starts_with("-d")
        || arg.starts_with("-F")
        || arg.starts_with("--data=")
        || arg.starts_with("--data-raw=")
        || arg.starts_with("--data-binary=")
        || arg.starts_with("--data-urlencode=")
        || arg.starts_with("--form=")
        || arg.starts_with("--json=")
}

fn option_consumes_value(arg: &str) -> bool {
    matches!(
        arg,
        "-d" | "--data"
            | "--data-raw"
            | "--data-binary"
            | "--data-urlencode"
            | "-F"
            | "--form"
            | "--json"
    )
}

fn build_body_representation(
    run_store: &RunStore,
    body_bytes: &[u8],
    content_type: Option<&str>,
) -> (
    ResponseBodyKind,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    if body_bytes.is_empty() {
        return (ResponseBodyKind::Empty, None, None, None);
    }

    if let Some(content_type) = content_type {
        if content_type.starts_with("image/") {
            let id = run_store.store_body(StoredRunBody {
                content_type: content_type.to_string(),
                bytes: body_bytes.to_vec(),
            });
            return (
                ResponseBodyKind::Image,
                None,
                None,
                Some(format!("/api/runs/{id}/body")),
            );
        }

        if content_type.starts_with("video/") {
            let id = run_store.store_body(StoredRunBody {
                content_type: content_type.to_string(),
                bytes: body_bytes.to_vec(),
            });
            return (
                ResponseBodyKind::Video,
                None,
                None,
                Some(format!("/api/runs/{id}/body")),
            );
        }
    }

    if is_text_like(content_type, body_bytes) {
        return (
            ResponseBodyKind::Text,
            Some(String::from_utf8_lossy(body_bytes).into_owned()),
            None,
            None,
        );
    }

    (
        ResponseBodyKind::Binary,
        None,
        Some("Binary response captured. Inline preview is only available for image and video responses.".to_string()),
        None,
    )
}

fn parse_http_status(stdout: &str) -> Option<u16> {
    stdout
        .split(HTTP_CODE_MARKER)
        .last()
        .and_then(|value| value.trim().parse::<u16>().ok())
        .filter(|value| *value > 0)
}

fn parse_last_response_headers(bytes: &[u8]) -> Vec<ResponseHeader> {
    let text = String::from_utf8_lossy(bytes);
    let normalized = text.replace("\r\n", "\n");
    let mut blocks = normalized
        .split("\n\n")
        .filter(|block| block.lines().any(|line| line.starts_with("HTTP/")))
        .collect::<Vec<_>>();

    let Some(last_block) = blocks.pop() else {
        return Vec::new();
    };

    last_block
        .lines()
        .skip(1)
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some(ResponseHeader {
                name: name.trim().to_string(),
                value: value.trim().to_string(),
            })
        })
        .collect()
}

fn parse_last_request_from_trace(bytes: &[u8]) -> Option<RequestDetails> {
    let text = String::from_utf8_lossy(bytes);
    let mut blocks = Vec::new();
    let mut current_block = Vec::new();
    let mut collecting = false;

    for line in text.lines() {
        if line.starts_with("=> Send header") {
            if !current_block.is_empty() {
                blocks.push(std::mem::take(&mut current_block));
            }
            collecting = true;
            continue;
        }

        if !collecting {
            continue;
        }

        let Some((_, payload)) = line.split_once(": ") else {
            continue;
        };

        if payload.is_empty() {
            if !current_block.is_empty() {
                blocks.push(std::mem::take(&mut current_block));
            }
            collecting = false;
            continue;
        }

        current_block.push(payload.to_string());
    }

    if !current_block.is_empty() {
        blocks.push(current_block);
    }

    let last_block = blocks.pop()?;
    let request_line = last_block.first()?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or_default().to_string();
    let headers = last_block
        .iter()
        .skip(1)
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some(ResponseHeader {
                name: name.trim().to_string(),
                value: value.trim().to_string(),
            })
        })
        .collect();

    Some(RequestDetails {
        method,
        url: None,
        headers,
    })
}

fn header_value(headers: &[ResponseHeader], target: &str) -> Option<String> {
    headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case(target))
        .map(|header| header.value.clone())
}

fn normalize_content_type(value: String) -> String {
    value
        .split(';')
        .next()
        .unwrap_or(value.as_str())
        .trim()
        .to_ascii_lowercase()
}

fn is_text_like(content_type: Option<&str>, body_bytes: &[u8]) -> bool {
    if let Some(content_type) = content_type {
        if content_type.starts_with("text/")
            || content_type.contains("json")
            || content_type.contains("xml")
            || content_type.contains("javascript")
            || content_type.contains("x-www-form-urlencoded")
        {
            return true;
        }
    }

    std::str::from_utf8(body_bytes).is_ok()
}

fn is_forbidden_option(arg: &str) -> bool {
    FORBIDDEN_OPTIONS.iter().any(|option| {
        arg == *option
            || arg.strip_prefix(option).is_some_and(|suffix| {
                !suffix.is_empty() && option.starts_with("--") && suffix.starts_with('=')
            })
            || matches!(*option, "-o" | "-D" | "-I" | "-K" | "-w")
                && arg.starts_with(option)
                && arg.len() > option.len()
    })
}

#[cfg(test)]
mod tests {
    use super::{
        analyze_command, merge_request_details, parse_last_request_from_trace,
        parse_last_response_headers, parse_request_details, run_curl_command, RequestDetails,
        ResponseBodyKind, ResponseHeader, RunStore, StoredRunBody, MAX_STORED_BODIES,
    };
    use axum::{routing::get, Router};

    #[test]
    fn test_analyze_command_rejects_non_curl() {
        let analysis = analyze_command("git status");
        assert!(!analysis.runnable);
        assert!(analysis
            .unsupported_reason
            .unwrap()
            .contains("Only curl commands"));
    }

    #[test]
    fn test_analyze_command_rejects_capture_conflicts() {
        let analysis = analyze_command("curl -o response.json https://example.com");
        assert!(!analysis.runnable);
        assert!(analysis
            .unsupported_reason
            .unwrap()
            .contains("unsupported option"));
    }

    #[test]
    fn test_parse_last_response_headers_uses_last_block() {
        let headers = parse_last_response_headers(
            b"HTTP/1.1 301 Moved Permanently\r\nLocation: /next\r\n\r\nHTTP/1.1 200 OK\r\nContent-Type: image/gif\r\nCache-Control: max-age=60\r\n\r\n",
        );

        assert_eq!(headers.len(), 2);
        assert_eq!(headers[0].name, "Content-Type");
        assert_eq!(headers[0].value, "image/gif");
    }

    #[test]
    fn test_parse_last_request_from_trace_uses_last_send_header_block() {
        let request = parse_last_request_from_trace(
            br#"== Info: first
=> Send header, 39 bytes (0x27)
0000: GET /redirect HTTP/1.1
0018: Host: example.com
002a: User-Agent: curl/8.7.1
0045:
== Info: second
=> Send header, 58 bytes (0x3a)
0000: POST /final HTTP/1.1
0019: Host: example.com
002b: User-Agent: curl/8.7.1
0046: Accept: */*
0054: Content-Type: application/json
0075:
"#,
        )
        .unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(
            request.headers,
            vec![
                ResponseHeader {
                    name: "Host".to_string(),
                    value: "example.com".to_string()
                },
                ResponseHeader {
                    name: "User-Agent".to_string(),
                    value: "curl/8.7.1".to_string()
                },
                ResponseHeader {
                    name: "Accept".to_string(),
                    value: "*/*".to_string()
                },
                ResponseHeader {
                    name: "Content-Type".to_string(),
                    value: "application/json".to_string()
                },
            ]
        );
    }

    #[test]
    fn test_merge_request_details_prefers_traced_headers() {
        let fallback = RequestDetails {
            method: "GET".to_string(),
            url: Some("https://example.com/anything".to_string()),
            headers: vec![ResponseHeader {
                name: "Authorization".to_string(),
                value: "Token 123".to_string(),
            }],
        };
        let traced = RequestDetails {
            method: "GET".to_string(),
            url: None,
            headers: vec![
                ResponseHeader {
                    name: "Host".to_string(),
                    value: "example.com".to_string(),
                },
                ResponseHeader {
                    name: "User-Agent".to_string(),
                    value: "curl/8.7.1".to_string(),
                },
            ],
        };

        let merged = merge_request_details(fallback, Some(traced));

        assert_eq!(merged.method, "GET");
        assert_eq!(merged.url.as_deref(), Some("https://example.com/anything"));
        assert_eq!(merged.headers.len(), 2);
        assert_eq!(merged.headers[0].name, "Host");
        assert_eq!(merged.headers[1].name, "User-Agent");
    }

    #[test]
    fn test_parse_request_details_extracts_method_url_and_headers() {
        let args = shell_words::split(
            "curl -X POST https://example.com/upload -H 'Authorization: Token 123' -H'Accept: application/json' -d '{}'",
        )
        .unwrap();

        let request = parse_request_details(&args);

        assert_eq!(request.method, "POST");
        assert_eq!(request.url.as_deref(), Some("https://example.com/upload"));
        assert_eq!(request.headers.len(), 2);
        assert_eq!(request.headers[0].name, "Authorization");
        assert_eq!(request.headers[0].value, "Token 123");
        assert_eq!(request.headers[1].name, "Accept");
        assert_eq!(request.headers[1].value, "application/json");
    }

    #[test]
    fn test_parse_request_details_defaults_to_post_for_body_requests() {
        let args =
            shell_words::split("curl https://example.com/upload --data 'hello world'").unwrap();

        let request = parse_request_details(&args);

        assert_eq!(request.method, "POST");
        assert_eq!(request.url.as_deref(), Some("https://example.com/upload"));
    }

    #[tokio::test]
    async fn test_run_curl_command_returns_text_body() {
        async fn handler() -> &'static str {
            "hello from shellshelf"
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, Router::new().route("/", get(handler)))
                .await
                .unwrap();
        });

        let run_store = RunStore::default();
        let response = run_curl_command(&format!("curl http://{address}/"), &run_store)
            .await
            .unwrap();

        assert!(response.success);
        assert_eq!(response.request.method, "GET");
        assert_eq!(response.body_kind, ResponseBodyKind::Text);
        assert_eq!(response.body_text.as_deref(), Some("hello from shellshelf"));
    }

    #[tokio::test]
    async fn test_run_curl_command_returns_image_preview_url() {
        async fn handler() -> impl axum::response::IntoResponse {
            (
                [("content-type", "image/gif")],
                vec![71_u8, 73, 70, 56, 57, 97],
            )
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, Router::new().route("/", get(handler)))
                .await
                .unwrap();
        });

        let run_store = RunStore::default();
        let response = run_curl_command(&format!("curl http://{address}/"), &run_store)
            .await
            .unwrap();

        assert!(response.success);
        assert_eq!(response.body_kind, ResponseBodyKind::Image);
        assert!(response.preview_url.is_some());
    }

    #[test]
    fn test_run_store_evicts_old_bodies() {
        let store = RunStore::default();

        let first = store.store_body(StoredRunBody {
            content_type: "image/gif".to_string(),
            bytes: vec![1],
        });

        for index in 0..MAX_STORED_BODIES {
            store.store_body(StoredRunBody {
                content_type: "image/gif".to_string(),
                bytes: vec![index as u8],
            });
        }

        assert!(store.get_body(first).is_none());
    }
}
