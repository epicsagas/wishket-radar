//! v0.5 BYOK AI — 설정·프록시·평가·대화.
//!
//! 키는 SQLite settings("ai_config")에서만 읽고 응답으로 재노출하지 않는다
//! (마스킹만). 브라우저 JS는 키를 모른 채 서버 프록시를 통해 공급자와
//! 대화한다. 외부 전송은 사용자가 지정한 공급자 API 호출뿐이다 (로드맵 원칙).

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Path as AxPath, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::AppState;
use crate::sqlite;
use crate::state::SeenEntry;

type ApiState = State<Arc<AppState>>;
type ApiError = (StatusCode, Json<Value>);

fn err(status: StatusCode, msg: impl Into<String>) -> ApiError {
    (status, Json(json!({"error": msg.into()})))
}

// ---------------------------------------------------------------------------
// 설정 (R1)
// ---------------------------------------------------------------------------

const SETTINGS_KEY: &str = "ai_config";
const TIMEOUT: Duration = Duration::from_secs(120);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Deserialize, Serialize)]
struct AiConfig {
    /// anthropic | openai | compatible
    provider: String,
    /// compatible 전용 (예: https://openrouter.ai/api/v1). openai/anthropic은 기본값.
    #[serde(default)]
    base_url: Option<String>,
    api_key: String,
    model: String,
    #[serde(default)]
    temperature: Option<f64>,
}

impl AiConfig {
    fn validate(&self) -> Result<(), String> {
        match self.provider.as_str() {
            "anthropic" | "openai" => Ok(()),
            "compatible" if self.base_url.as_deref().unwrap_or("").starts_with("http") => Ok(()),
            "compatible" => Err("compatible 공급자에는 base_url이 필요합니다".into()),
            _ => Err("provider는 anthropic|openai|compatible 중 하나".into()),
        }
    }

    /// base_url은 어떤 공급자에도 존중한다(게이트웨이·테스트 오버라이드).
    /// anthropic: 끝에 /v1/messages를 붙인다. openai·compatible: /v1까지 포함한
    /// base 뒤에 /chat/completions를 붙인다.
    fn api_base(&self) -> String {
        match &self.base_url {
            Some(b) if !b.trim().is_empty() => b.trim_end_matches('/').to_string(),
            _ => default_base(&self.provider).to_string(),
        }
    }

    fn body(&self, system: &str, history: &[(String, String)], stream: bool) -> Value {
        if self.provider == "anthropic" {
            let messages: Vec<_> = history
                .iter()
                .map(|(r, c)| json!({"role": r, "content": c}))
                .collect();
            json!({
                "model": self.model,
                "max_tokens": 4000,
                "system": system,
                "temperature": self.temperature,
                "messages": messages,
                "stream": stream,
            })
        } else {
            let mut messages = vec![json!({"role": "system", "content": system})];
            messages.extend(
                history
                    .iter()
                    .map(|(r, c)| json!({"role": r, "content": c})),
            );
            let mut body = json!({
                "model": self.model,
                "temperature": self.temperature,
                "messages": messages,
                "stream": stream,
            });
            // openai 공급자에만 usage 스트림 옵션 — compatible 엔드포인트는
            // 모르는 필드를 거부하는 곳이 있어 넣지 않는다 (usage 0 누적 가능).
            if self.provider == "openai" && stream {
                body["stream_options"] = json!({"include_usage": true});
            }
            body
        }
    }
}

fn default_base(provider: &str) -> &'static str {
    match provider {
        "anthropic" => "https://api.anthropic.com",
        _ => "https://api.openai.com/v1",
    }
}

fn load_config(dir: &std::path::Path) -> Option<AiConfig> {
    let raw = sqlite::load_setting(dir, SETTINGS_KEY)?;
    serde_json::from_str(&raw).ok()
}

/// 평문 키 재노출 금지 — 앞3 + *** + 뒤4.
fn mask_key(k: &str) -> String {
    let n = k.chars().count();
    if n <= 8 {
        return "***".into();
    }
    let head: String = k.chars().take(3).collect();
    let tail: String = k.chars().skip(n - 4).collect();
    format!("{head}***{tail}")
}

fn public_config(c: &AiConfig) -> Value {
    json!({
        "provider": c.provider,
        "base_url": c.base_url,
        "model": c.model,
        "temperature": c.temperature,
        "api_key_masked": mask_key(&c.api_key),
    })
}

async fn get_settings_ai(State(app): ApiState) -> Result<Json<Value>, ApiError> {
    Ok(match load_config(&app.state_dir) {
        Some(c) => {
            let mut v = public_config(&c);
            v["configured"] = json!(true);
            Json(v)
        }
        None => Json(json!({"configured": false})),
    })
}

/// 키 미수신(또는 마스킹 값 수신) 시 기존 키 보존 — 마스킹 값 저장 방지.
async fn put_settings_ai(
    State(app): ApiState,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let existing = load_config(&app.state_dir);
    let key_in = body.get("api_key").and_then(Value::as_str);
    let api_key = match key_in {
        Some(k) if !k.contains("***") => k.to_string(),
        _ => existing
            .as_ref()
            .map(|c| c.api_key.clone())
            .ok_or_else(|| {
                err(
                    StatusCode::BAD_REQUEST,
                    "api_key가 필요합니다 (저장된 키 없음)",
                )
            })?,
    };
    let cfg = AiConfig {
        provider: body
            .get("provider")
            .and_then(Value::as_str)
            .unwrap_or("anthropic")
            .to_string(),
        base_url: body
            .get("base_url")
            .and_then(Value::as_str)
            .map(String::from),
        api_key,
        model: body
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        temperature: body.get("temperature").and_then(Value::as_f64),
    };
    cfg.validate()
        .map_err(|m| err(StatusCode::BAD_REQUEST, m))
        .and_then(|_| {
            if cfg.model.is_empty() {
                Err(err(StatusCode::BAD_REQUEST, "model이 필요합니다"))
            } else {
                Ok(())
            }
        })?;
    let raw =
        serde_json::to_string(&cfg).map_err(|e| err(StatusCode::BAD_REQUEST, e.to_string()))?;
    sqlite::save_setting(&app.state_dir, SETTINGS_KEY, &raw)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(public_config(&cfg)))
}

// ---------------------------------------------------------------------------
// 공급자 호출 (R2)
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Usage(u64, u64);

struct Completion {
    text: String,
    usage: Usage,
}

/// AI 전용 클라이언트 — connect 타임아웃은 클라이언트 레벨 옵션이라
/// 대시보드 위시켓 http 클라이언트와 분리한다.
fn ai_client() -> &'static reqwest::Client {
    static C: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    C.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .unwrap_or_default()
    })
}

fn provider_request(
    cfg: &AiConfig,
    system: &str,
    history: &[(String, String)],
    stream: bool,
) -> reqwest::RequestBuilder {
    let url = if cfg.provider == "anthropic" {
        format!("{}/v1/messages", cfg.api_base())
    } else {
        format!("{}/chat/completions", cfg.api_base())
    };
    let mut b = ai_client()
        .post(url)
        .timeout(TIMEOUT)
        .json(&cfg.body(system, history, stream));
    b = if cfg.provider == "anthropic" {
        b.header("x-api-key", &cfg.api_key)
            .header("anthropic-version", "2023-06-01")
    } else {
        b.header(header::AUTHORIZATION, format!("Bearer {}", cfg.api_key))
    };
    b
}

/// 공급자 오류는 상태코드+body 그대로 전달 (429 포함). 키는 body에 없다.
async fn provider_error(resp: reqwest::Response) -> ApiError {
    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let body = resp.text().await.unwrap_or_default();
    let excerpt: String = body.chars().take(500).collect();
    err(status, format!("공급자 오류: {excerpt}"))
}

async fn complete(
    cfg: &AiConfig,
    system: &str,
    history: &[(String, String)],
) -> Result<Completion, ApiError> {
    let resp = provider_request(cfg, system, history, false)
        .send()
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, format!("공급자 연결 실패: {e}")))?;
    if !resp.status().is_success() {
        return Err(provider_error(resp).await);
    }
    let v: Value = resp.json().await.map_err(|e| {
        err(
            StatusCode::BAD_GATEWAY,
            format!("공급자 응답 파싱 실패: {e}"),
        )
    })?;
    let text = if cfg.provider == "anthropic" {
        v["content"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|c| c.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default()
    } else {
        v.pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let mut usage = Usage::default();
    harvest_usage(&v, &mut usage);
    Ok(Completion { text, usage })
}

/// 단일 JSON 응답에서 usage 필드 수집 (공급자별 이름 차이 흡수).
fn harvest_usage(v: &Value, usage: &mut Usage) {
    // {usage:{...}} (openai) · {message:{usage:{...}}} (anthropic message_start)
    for u in [v.get("usage"), v.pointer("/message/usage")]
        .into_iter()
        .flatten()
    {
        let input = u
            .get("input_tokens")
            .or_else(|| u.get("prompt_tokens"))
            .and_then(Value::as_u64);
        if let Some(n) = input {
            usage.0 = usage.0.max(n);
        }
        let output = u
            .get("output_tokens")
            .or_else(|| u.get("completion_tokens"))
            .and_then(Value::as_u64);
        if let Some(n) = output {
            usage.1 = usage.1.max(n);
        }
    }
}

// ---------------------------------------------------------------------------
// SSE 스트림 중계 — 통과시키면서 usage·어시스턴트 텍스트를 곁들여 수집한다.
// ---------------------------------------------------------------------------

struct RecvStream(tokio::sync::mpsc::Receiver<Result<axum::body::Bytes, io::Error>>);

impl Stream for RecvStream {
    type Item = Result<axum::body::Bytes, io::Error>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.0.poll_recv(cx)
    }
}

/// 완성된 SSE data 라인 하나에서 텍스트 델타·usage 수집 (공급자 형태 무관 시도).
fn harvest_line(line: &str, usage: &mut Usage, text: &mut String) {
    let Some(payload) = line.strip_prefix("data:") else {
        return;
    };
    let payload = payload.trim();
    if payload.is_empty() || payload == "[DONE]" {
        return;
    }
    let Ok(v) = serde_json::from_str::<Value>(payload) else {
        return;
    };
    harvest_usage(&v, usage);
    if let Some(t) = v.pointer("/delta/text").and_then(Value::as_str) {
        text.push_str(t);
    }
    if let Some(t) = v
        .pointer("/choices/0/delta/content")
        .and_then(Value::as_str)
    {
        text.push_str(t);
    }
}

async fn relay_stream(
    resp: reqwest::Response,
    tx: tokio::sync::mpsc::Sender<Result<axum::body::Bytes, io::Error>>,
    dir: PathBuf,
    conversation_id: i64,
) {
    let mut usage = Usage::default();
    let mut text = String::new();
    let mut line_buf: Vec<u8> = Vec::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(b) => {
                for &byte in b.iter() {
                    if byte == b'\n' {
                        let line = String::from_utf8_lossy(&line_buf).to_string();
                        line_buf.clear();
                        harvest_line(&line, &mut usage, &mut text);
                    } else if line_buf.len() < 64 * 1024 {
                        line_buf.push(byte);
                    }
                }
                if tx.send(Ok(b)).await.is_err() {
                    return; // 클라이언트 끊김
                }
            }
            Err(e) => {
                let _ = tx
                    .send(Err(io::Error::other(format!("공급자 스트림 오류: {e}"))))
                    .await;
                return;
            }
        }
    }
    // 스트림 정상 종료 시에만 영속 — 중간 끊김은 부분만 남는다.
    let _ = sqlite::add_usage(&dir, conversation_id, usage.0, usage.1);
    if !text.is_empty() {
        let _ = sqlite::append_message(&dir, conversation_id, "assistant", &text);
    }
}

// ---------------------------------------------------------------------------
// 평가 (R3)
// ---------------------------------------------------------------------------

/// agents/wishket-analyst.md를 단일 소스로 — frontmatter만 떼고 본문 그대로.
fn analyst_system() -> String {
    let raw = include_str!("../../../agents/wishket-analyst.md");
    raw.strip_prefix("---")
        .and_then(|r| r.split_once("\n---"))
        .map(|(_, rest)| rest.trim_start().to_string())
        .unwrap_or_else(|| raw.to_string())
}

struct AnalystOutput {
    grade: String,
    score: Option<u32>,
    fit: String,
    caution: String,
    proposal: String,
    condition: Option<String>,
}

/// 분석가 5줄 출력 파싱. 등급+근거+주의+제안이 있어야 성공으로 본다.
fn parse_analyst(text: &str) -> Option<AnalystOutput> {
    let mut grade = None;
    let mut score = None;
    let mut fields: [(&str, Option<String>); 4] = [
        ("근거", None),
        ("주의", None),
        ("제안", None),
        ("조건", None),
    ];
    for line in text.lines() {
        let t = line.trim().trim_start_matches(['-', '*', '•']).trim();
        let Some((label, val)) = t.split_once(':') else {
            continue;
        };
        let val = val.trim();
        let label = label.trim();
        if label.starts_with("등급") {
            let g = val
                .chars()
                .next()
                .map(|c| c.to_ascii_uppercase().to_string())
                .filter(|c| "ABC".contains(c.as_str()));
            if g.is_some() {
                grade = g;
            }
            // "A (85점)" / "A(85)"
            let digits: String = val.chars().skip_while(|c| !c.is_ascii_digit()).collect();
            if let Ok(s) = digits.split('점').next().unwrap_or("").trim().parse() {
                score = Some(s);
            }
        } else if let Some((_, slot)) = fields.iter_mut().find(|(l, _)| label.starts_with(l)) {
            if slot.is_none() && !val.is_empty() {
                *slot = Some(val.to_string());
            }
        }
    }
    let fit = fields[0].1.take()?;
    let caution = fields[1].1.take()?;
    let proposal = fields[2].1.take()?;
    let grade = grade?;
    Some(AnalystOutput {
        grade,
        score,
        fit,
        caution,
        proposal,
        condition: fields[3].1.take(),
    })
}

/// 리포트 파서(reports.rs)가 읽는 포맷 그대로 reports/ai-eval.md에 덧붙인다.
/// 같은 공고 재평가 시 나중 항목이 이긴다(parse의 뒤 행 우선).
fn append_report(
    dir: &std::path::Path,
    id: &str,
    e: &SeenEntry,
    out: &AnalystOutput,
    model: &str,
) -> std::io::Result<PathBuf> {
    let reports = dir.join("reports");
    std::fs::create_dir_all(&reports)?;
    let path = reports.join("ai-eval.md");
    if !path.exists() {
        let today = crate::state::now_iso();
        std::fs::write(
            &path,
            format!(
                "# AI 평가 리포트\n\n> webui BYOK AI 평가. 시작: {}\n\n",
                today.get(..10).unwrap_or("")
            ),
        )?;
    }
    let url = e
        .url
        .clone()
        .unwrap_or_else(|| format!("https://www.wishket.com/project/{id}/"));
    let score_part = match out.score {
        Some(s) => format!("{s}점"),
        None => "점수 없음".into(),
    };
    let entry = format!(
        "### 1. [{}] {}\n- URL: {url}\n- AI 평가: {score_part} · 모델: {model}\n- 적합도 판단: {}\n- 주의점: {}\n- 제안 방향: {}\n- 조건: {}\n\n",
        out.grade,
        e.title,
        out.fit,
        out.caution,
        out.proposal,
        out.condition.as_deref().unwrap_or("공고에 명시 없음"),
    );
    let mut f = std::fs::OpenOptions::new().append(true).open(&path)?;
    use std::io::Write;
    f.write_all(entry.as_bytes())?;
    Ok(path)
}

#[derive(Deserialize)]
struct EvaluateBody {
    id: String,
}

async fn evaluate(
    State(app): ApiState,
    Json(body): Json<EvaluateBody>,
) -> Result<Json<Value>, ApiError> {
    let cfg = load_config(&app.state_dir).ok_or_else(|| {
        err(
            StatusCode::CONFLICT,
            "AI 설정이 없습니다 — 내 정보 탭에서 설정하세요",
        )
    })?;
    let st = crate::state::load_in(&app.state_dir);
    let Some(e) = st.seen.get(&body.id) else {
        return Err(err(StatusCode::NOT_FOUND, "캐시된 공고가 없습니다"));
    };
    if e.description.is_none() {
        return Err(err(
            StatusCode::CONFLICT,
            "상세 캐시가 없습니다 — 먼저 상세를 불러오세요",
        ));
    }
    let profile = crate::state::load_profile_yaml(&app.state_dir).unwrap_or_default();
    let project = serde_json::to_value(e)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let user = format!(
        "[기술 프로필]\n{}\n\n[공고 데이터]\n{}",
        profile.chars().take(4000).collect::<String>(),
        serde_json::to_string_pretty(&project).unwrap_or_default()
    );
    let completion = complete(&cfg, &analyst_system(), &[("user".into(), user)]).await?;
    let Some(out) = parse_analyst(&completion.text) else {
        let excerpt: String = completion.text.chars().take(500).collect();
        return Err(err(
            StatusCode::BAD_GATEWAY,
            format!("분석 출력 파싱 실패 — 모델 응답이 5줄 포맷이 아닙니다: {excerpt}"),
        ));
    };
    let path = append_report(&app.state_dir, &body.id, e, &out, &cfg.model)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({
        "grade": out.grade,
        "score": out.score,
        "fit": out.fit,
        "caution": out.caution,
        "proposal": out.proposal,
        "condition": out.condition,
        "model": cfg.model,
        "report": path.file_name().and_then(|n| n.to_str()),
        "usage": {"input_tokens": completion.usage.0, "output_tokens": completion.usage.1},
    })))
}

// ---------------------------------------------------------------------------
// 대화 (R4·R5)
// ---------------------------------------------------------------------------

/// 대화용 시스템 프롬프트 — 공고 컨텍스트는 있을 때 덧붙인다.
const CHAT_SYSTEM: &str = "당신은 위시켓 외주 공고 분석 도우미입니다. 주어진 공고 컨텍스트와 기술 프로필에 근거해서만 답하고, 근거가 없으면 \"공고에 명시 없음\"이라고 답합니다. 한국어로 답합니다.";

#[derive(Deserialize)]
struct ChatBody {
    message: String,
    #[serde(default)]
    conversation_id: Option<i64>,
    #[serde(default)]
    project_id: Option<String>,
}

async fn chat(State(app): ApiState, Json(body): Json<ChatBody>) -> Result<Response, ApiError> {
    if body.message.trim().is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "message가 비었습니다"));
    }
    let cfg = load_config(&app.state_dir).ok_or_else(|| {
        err(
            StatusCode::CONFLICT,
            "AI 설정이 없습니다 — 내 정보 탭에서 설정하세요",
        )
    })?;
    let dir = app.state_dir.clone();
    let conversation_id = match body.conversation_id {
        Some(id) => {
            if sqlite::get_conversation(&dir, id)
                .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
                .is_none()
            {
                return Err(err(StatusCode::NOT_FOUND, "대화를 찾을 수 없습니다"));
            }
            id
        }
        None => {
            let title: String = body.message.chars().take(40).collect();
            sqlite::create_conversation(&dir, body.project_id.as_deref(), &title)
                .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        }
    };
    // 컨텍스트: 대화에 묶인 공고 캐시 + 프로필 (후속 질문이 같은 맥락을 유지)
    let mut system = CHAT_SYSTEM.to_string();
    let conv = sqlite::get_conversation(&dir, conversation_id)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let project_id = conv
        .as_ref()
        .and_then(|c| c.get("project_id"))
        .and_then(Value::as_str)
        .map(String::from);
    if let Some(pid) = &project_id {
        let st = crate::state::load_in(&dir);
        if let Some(e) = st.seen.get(pid) {
            let profile = crate::state::load_profile_yaml(&dir).unwrap_or_default();
            let project = serde_json::to_value(e).unwrap_or_default();
            system.push_str(&format!(
                "\n\n[기술 프로필]\n{}\n\n[공고 컨텍스트]\n{}",
                profile.chars().take(4000).collect::<String>(),
                serde_json::to_string(&project).unwrap_or_default()
            ));
        }
    }
    // 전체 메시지 배열을 공급자에 재전송한다 (로드맵: 이전 맥락 이어가기)
    let history: Vec<(String, String)> = conv
        .and_then(|c| c.get("messages").cloned())
        .and_then(|m| m.as_array().cloned())
        .map(|msgs| {
            msgs.iter()
                .filter_map(|m| {
                    Some((
                        m.get("role")?.as_str()?.to_string(),
                        m.get("content")?.as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    // 사용자 메시지 선영속 — 공급자 실패 시에도 질문은 남는다
    sqlite::append_message(&dir, conversation_id, "user", &body.message)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut history = history;
    history.push(("user".into(), body.message.clone()));

    let resp = provider_request(&cfg, &system, &history, true)
        .send()
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, format!("공급자 연결 실패: {e}")))?;
    if !resp.status().is_success() {
        return Err(provider_error(resp).await);
    }
    let ct = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .cloned()
        .unwrap_or_else(|| HeaderValue::from_static("text/event-stream"));
    let (tx, rx) = tokio::sync::mpsc::channel(32);
    tokio::spawn(relay_stream(resp, tx, dir.clone(), conversation_id));
    let mut response = Response::new(Body::from_stream(RecvStream(rx)));
    response.headers_mut().insert(header::CONTENT_TYPE, ct);
    // conversation_id는 헤더로 — SSE 본문은 공급자 프레임 그대로 (얇은 프록시)
    if let Ok(hv) = HeaderValue::from_str(&conversation_id.to_string()) {
        response.headers_mut().insert("x-conversation-id", hv);
    }
    Ok(response)
}

#[derive(Deserialize)]
struct ConversationBody {
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default)]
    title: Option<String>,
}

async fn create_conversation(
    State(app): ApiState,
    Json(body): Json<ConversationBody>,
) -> Result<Json<Value>, ApiError> {
    let id = sqlite::create_conversation(
        &app.state_dir,
        body.project_id.as_deref(),
        body.title.as_deref().unwrap_or(""),
    )
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({"id": id})))
}

async fn list_conversations(State(app): ApiState) -> Result<Json<Value>, ApiError> {
    let list = sqlite::list_conversations(&app.state_dir)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({"conversations": list})))
}

async fn get_conversation(
    State(app): ApiState,
    AxPath(id): AxPath<i64>,
) -> Result<Json<Value>, ApiError> {
    let conv = sqlite::get_conversation(&app.state_dir, id)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    match conv {
        Some(c) => Ok(Json(c)),
        None => Err(err(StatusCode::NOT_FOUND, "대화를 찾을 수 없습니다")),
    }
}

// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/settings/ai", get(get_settings_ai).put(put_settings_ai))
        .route("/ai/evaluate", post(evaluate))
        .route("/ai/chat", post(chat))
        .route(
            "/ai/conversations",
            get(list_conversations).post(create_conversation),
        )
        .route("/ai/conversations/{id}", get(get_conversation))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Request;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("wk-ai-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn app(dir: PathBuf) -> Arc<AppState> {
        Arc::new(AppState {
            token: "t".into(),
            state_dir: dir,
            http: reqwest::Client::new(),
        })
    }

    fn cfg(provider: &str, base: Option<&str>) -> AiConfig {
        AiConfig {
            provider: provider.into(),
            base_url: base.map(String::from),
            api_key: "sk-test-1234567890abcd".into(),
            model: "test-model".into(),
            temperature: Some(0.3),
        }
    }

    #[test]
    fn mask_key_hides_middle() {
        assert_eq!(mask_key("sk-ant-1234567890abcdef"), "sk-***cdef");
        assert_eq!(mask_key("short"), "***", "짧은 키는 전부 가린다");
    }

    #[test]
    fn settings_preserve_key_when_masked_sent() {
        let dir = tmpdir("putkey");
        // 빈 상태에서 마스킹 키만 보내면 거부
        let body = json!({"provider": "anthropic", "model": "m", "api_key": "sk-***cdef"});
        let res = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(put_settings_ai(State(app(dir.clone())), Json(body)));
        assert!(matches!(res, Err((StatusCode::BAD_REQUEST, _))));
        // 실제 키 저장
        let body = json!({"provider": "anthropic", "model": "m", "api_key": "sk-ant-real-9999"});
        let _ = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(put_settings_ai(State(app(dir.clone())), Json(body)));
        // GET 어디에도 평문 키 없음
        let got = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(get_settings_ai(State(app(dir.clone()))))
            .unwrap();
        let s = got.0.to_string();
        assert!(!s.contains("sk-ant-real"), "평문 키 노출: {s}");
        assert!(s.contains("api_key_masked"));
        // 마스킹 키로 재저장 — 기존 키 보존
        let body = json!({"provider": "anthropic", "model": "m2", "api_key": "sk-***9999"});
        let _ = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(put_settings_ai(State(app(dir.clone())), Json(body)));
        let stored = load_config(&dir).unwrap();
        assert_eq!(stored.api_key, "sk-ant-real-9999");
        assert_eq!(stored.model, "m2");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn compatible_requires_base_url() {
        let dir = tmpdir("compat");
        let body = json!({"provider": "compatible", "model": "m", "api_key": "k-1234567890"});
        let res = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(put_settings_ai(State(app(dir.clone())), Json(body)));
        assert!(matches!(res, Err((StatusCode::BAD_REQUEST, _))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_analyst_five_lines() {
        let text = "등급: A (85점)\n근거: 핵심 과업이 Rust 백엔드와 일치\n주의: 마감이 1주 남아 경쟁 예상\n제안: 실시간 파이프라인 경험 강조\n조건: 월 700만 · 3개월 · 원격";
        let out = parse_analyst(text).unwrap();
        assert_eq!(out.grade, "A");
        assert_eq!(out.score, Some(85));
        assert!(out.fit.contains("Rust"));
        assert!(out.caution.contains("마감"));
        assert!(out.proposal.contains("제안서") || out.proposal.contains("강조"));
        assert_eq!(out.condition.as_deref(), Some("월 700만 · 3개월 · 원격"));
        // 소문자 · 점수 없는 형태
        let out = parse_analyst("등급: b\n근거: x:1\n주의: y\n제안: z").unwrap();
        assert_eq!(out.grade, "B");
        assert_eq!(out.score, None);
        // 전혀 다른 출력
        assert!(parse_analyst("안녕하세요. 분석 결과입니다.").is_none());
        // 근거 누락
        assert!(parse_analyst("등급: A (90점)\n주의: y\n제안: z").is_none());
    }

    #[test]
    fn report_roundtrip_through_parser() {
        let dir = tmpdir("report");
        let mut e = crate::state::SeenEntry {
            first_seen: "2026-09-04".into(),
            title: "Rust 백엔드 개발".into(),
            ..crate::state::SeenEntry::default()
        };
        e.url = Some("https://www.wishket.com/project/158063/".into());
        let out = AnalystOutput {
            grade: "B".into(),
            score: Some(72),
            fit: "스택 부분 일치".into(),
            caution: "상주 요구".into(),
            proposal: "하이브리드 제안".into(),
            condition: Some("월 500만".into()),
        };
        let path = append_report(&dir, "158063", &e, &out, "test-model").unwrap();
        let md = std::fs::read_to_string(path).unwrap();
        let parsed = super::super::reports::parse(&md, "ai-eval.md");
        let a = parsed.get("158063").expect("파서가 id를 찾아야 함");
        assert_eq!(a.grade.as_deref(), Some("B"));
        assert_eq!(a.score, Some(72));
        assert_eq!(a.fit.as_deref(), Some("스택 부분 일치"));
        assert_eq!(a.caution.as_deref(), Some("상주 요구"));
        assert_eq!(a.proposal.as_deref(), Some("하이브리드 제안"));
        assert_eq!(a.model.as_deref(), Some("test-model"));
        // 재평가 — 뒤 항목이 이긴다
        let out2 = AnalystOutput {
            grade: "A".into(),
            score: Some(90),
            ..out_clone()
        };
        let _ = append_report(&dir, "158063", &e, &out2, "test-model").unwrap();
        let md = std::fs::read_to_string(dir.join("reports/ai-eval.md")).unwrap();
        let parsed = super::super::reports::parse(&md, "ai-eval.md");
        assert_eq!(parsed.get("158063").unwrap().grade.as_deref(), Some("A"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn out_clone() -> AnalystOutput {
        AnalystOutput {
            grade: "B".into(),
            score: Some(72),
            fit: "스택 부분 일치".into(),
            caution: "상주 요구".into(),
            proposal: "하이브리드 제안".into(),
            condition: Some("월 500만".into()),
        }
    }

    /// 로컬 목업 공급자 — 외부 호출 없이 요청 직렬화·응답 파싱·에러 전달 검증.
    async fn spawn_mock(routes: axum::Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, routes).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn complete_anthropic_mock() {
        let app_routes = axum::Router::new().route(
            "/v1/messages",
            axum::routing::post(|req: Request| async move {
                assert_eq!(req.headers().get("x-api-key").unwrap(), "sk-test-1234567890abcd");
                axum::Json(json!({
                    "content": [{"type": "text", "text": "등급: A (85점)\n근거: 일치\n주의: 마감\n제안: 강조\n조건: 원격"}],
                    "usage": {"input_tokens": 120, "output_tokens": 45}
                }))
            }),
        );
        let base = spawn_mock(app_routes).await;
        let c = complete(
            &cfg("anthropic", Some(&base)),
            "sys",
            &[("user".into(), "u".into())],
        )
        .await
        .unwrap();
        assert!(c.text.starts_with("등급: A"));
        assert_eq!(c.usage.0, 120);
        assert_eq!(c.usage.1, 45);
    }

    #[tokio::test]
    async fn complete_openai_mock_and_error_passthrough() {
        let app_routes = axum::Router::new()
            .route(
                "/chat/completions",
                axum::routing::post(|axum::Json(body): axum::Json<Value>| async move {
                    // 시스템 프롬프트가 messages 배열로 들어가는지
                    assert_eq!(body["messages"][0]["role"], "system");
                    axum::Json(json!({
                        "choices": [{"message": {"content": "응답 텍스트"}}],
                        "usage": {"prompt_tokens": 10, "completion_tokens": 3}
                    }))
                }),
            )
            .route(
                "/fail/chat/completions",
                axum::routing::post(|| async { (StatusCode::TOO_MANY_REQUESTS, "rate limited") }),
            );
        let base = spawn_mock(app_routes).await;
        let c = complete(
            &cfg("openai", Some(&base)),
            "sys",
            &[("user".into(), "u".into())],
        )
        .await
        .unwrap();
        assert_eq!(c.text, "응답 텍스트");
        assert_eq!(c.usage.0, 10);
        assert_eq!(c.usage.1, 3);

        let bad = cfg("openai", Some(&format!("{base}/fail")));
        let e = complete(&bad, "sys", &[("user".into(), "u".into())]).await;
        let Err((status, Json(v))) = e else {
            panic!("429여야 한다");
        };
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "429 그대로 전달");
        assert!(v["error"].as_str().unwrap().contains("rate limited"));
    }

    #[tokio::test]
    async fn chat_stream_persists_text_and_usage() {
        // SSE 프레임을 흉내 내는 공급자 — 텍스트 델타 + usage.
        // 받은 요청 본문을 기록해 AC3(컨텍스트 주입·전체 배열 재전송)를 단증한다.
        let frames = Arc::new(
            "data: {\"delta\":{\"text\":\"핵심 스택이 \"}}\n\n\
             data: {\"delta\":{\"text\":\"일치합니다.\"}}\n\n\
             data: {\"usage\":{\"input_tokens\":30,\"output_tokens\":12}}\n\n\
             data: [DONE]\n\n"
                .to_string(),
        );
        let captured: Arc<std::sync::Mutex<Vec<Value>>> = Arc::default();
        let cap = captured.clone();
        let app_routes = axum::Router::new().route(
            "/v1/messages",
            axum::routing::post(move |body: String| {
                let frames = frames.clone();
                let cap = cap.clone();
                async move {
                    if let Ok(v) = serde_json::from_str::<Value>(&body) {
                        cap.lock().unwrap().push(v);
                    }
                    let mut res = axum::response::Response::new(Body::from((*frames).clone()));
                    res.headers_mut().insert(
                        header::CONTENT_TYPE,
                        HeaderValue::from_static("text/event-stream"),
                    );
                    res
                }
            }),
        );
        let base = spawn_mock(app_routes).await;
        let dir = tmpdir("chat");
        // 공고 캐시 시드 — 컨텍스트 주입 검증용
        let mut st = crate::state::State::default();
        let mut e = crate::state::SeenEntry {
            title: "Rust 실시간 파이프라인 백엔드".into(),
            ..crate::state::SeenEntry::default()
        };
        e.description = Some("<p>Rust 기반 실시간 데이터 수집 파이프라인</p>".into());
        e.first_seen = "2026-09-04".into();
        e.url = Some("https://www.wishket.com/project/158063/".into());
        st.seen.insert("158063".into(), e);
        crate::state::save_in(&dir, &st).unwrap();
        let raw = serde_json::to_string(&cfg("anthropic", Some(&base))).unwrap();
        sqlite::save_setting(&dir, SETTINGS_KEY, &raw).unwrap();

        let a = app(dir.clone());
        let res = chat(
            State(a.clone()),
            Json(ChatBody {
                message: "이 공고 어때?".into(),
                conversation_id: None,
                project_id: Some("158063".into()),
            }),
        )
        .await
        .unwrap();
        assert!(
            res.headers().get("x-conversation-id").is_some(),
            "conversation_id 헤더"
        );
        let conv_id: i64 = res
            .headers()
            .get("x-conversation-id")
            .unwrap()
            .to_str()
            .unwrap()
            .parse()
            .unwrap();
        let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
            .await
            .unwrap();
        let s = std::string::String::from_utf8(bytes.to_vec()).unwrap();
        assert!(s.contains("핵심 스택이"), "SSE 원문 중계");
        // 릴레이 태스크 종료 대기 — 사용자 메시지는 즉시, 어시스턴트는 종료 시점
        for _ in 0..40 {
            if sqlite::get_conversation(&dir, conv_id)
                .unwrap()
                .map(|c| c["messages"].as_array().map(|m| m.len()).unwrap_or(0))
                .unwrap_or(0)
                >= 2
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        let conv = sqlite::get_conversation(&dir, conv_id).unwrap().unwrap();
        let msgs = conv["messages"].as_array().unwrap();
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(
            msgs[1]["content"], "핵심 스택이 일치합니다.",
            "델타 합쳐 영속"
        );
        assert_eq!(conv["tokens_in"], 30);
        assert_eq!(conv["tokens_out"], 12);

        // AC3: 후속 질문 — 시스템 컨텍스트 + 전체 배열(user/assistant/user) 재전송
        let res2 = chat(
            State(a.clone()),
            Json(ChatBody {
                message: "제안 방향 더 구체화해줘".into(),
                conversation_id: Some(conv_id),
                project_id: None,
            }),
        )
        .await
        .unwrap();
        let _ = axum::body::to_bytes(res2.into_body(), 1 << 20)
            .await
            .unwrap();
        for _ in 0..40 {
            if captured.lock().unwrap().len() >= 2 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        let bodies = captured.lock().unwrap();
        assert_eq!(bodies.len(), 2, "공급자 요청 2회 기록");
        let first = &bodies[0];
        assert!(
            first["system"]
                .as_str()
                .unwrap_or("")
                .contains("실시간 데이터 수집 파이프라인"),
            "AC3 공고 캐시 시스템 컨텍스트 주입"
        );
        let last = &bodies[1];
        let sent = last["messages"].as_array().unwrap();
        assert_eq!(sent.len(), 3, "AC3 전체 배열 재전송");
        assert_eq!(sent[0]["role"], "user");
        assert_eq!(sent[1]["role"], "assistant");
        assert_eq!(sent[1]["content"], "핵심 스택이 일치합니다.");
        assert_eq!(sent[2]["role"], "user");
        assert_eq!(sent[2]["content"], "제안 방향 더 구체화해줘");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
