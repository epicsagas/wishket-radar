//! /api 핸들러. 요청마다 state_dir 파일을 다시 읽는다(파일이 정규 소스).

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use pulldown_cmark::{html, Options, Parser};
use serde_json::{json, Value};

use super::apps::{self, Application};
use super::fsutil::{self, FileEntry};
use super::matches;
use super::reports;
use super::AppState;
use crate::profile::Profile;
use crate::state;

type ApiState = State<Arc<AppState>>;
type ApiError = (StatusCode, Json<Value>);

fn err(status: StatusCode, msg: impl Into<String>) -> ApiError {
    (status, Json(json!({"error": msg.into()})))
}

fn today() -> String {
    state::now_iso().get(..10).unwrap_or("").to_string()
}

fn render_markdown(src: &str) -> String {
    // ponytail: sanitize 없음 — 내용은 전부 본인 state-dir 파일. 비신뢰 편집 생기면 ammonia 추가.
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    let mut out = String::new();
    html::push_html(&mut out, Parser::new_ext(src, opts));
    // 렌더된 본문의 외부 링크는 전부 새 탭으로. 대시보드가 SPA라 같은 탭
    // 이동은 상태를 날린다.
    out.replace(
        "<a href=\"http",
        "<a target=\"_blank\" rel=\"noopener noreferrer\" href=\"http",
    )
}

/// "2026년 09월 08일마감 6일 8시간 전" → "2026-09-08".
/// 상세 페이지 조건 행 전용(카드 DOM이 없을 때의 폴백).
fn parse_korean_date(s: &str) -> Option<String> {
    // 각 단위 문자 바로 앞에 붙은 숫자를 딴다
    let digits = |unit: &str| -> Option<u32> {
        let i = s.find(unit)?;
        s[..i]
            .rsplit(|c: char| !c.is_ascii_digit())
            .next()
            .filter(|d| !d.is_empty())
            .and_then(|d| d.parse().ok())
    };
    let y = digits("년")?;
    let m = digits("월")?;
    let d = digits("일")?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

/// "마감 2주 2일 전" / "마감 6일 8시간 전" → today + N일.
/// 카드 목록은 절대 날짜가 아니라 상대 표기를 준다.
/// 오늘(YYYY-MM-DD) 기준으로 환산한다. 남은 일수를 못 읽으면 None.
pub(crate) fn relative_deadline_to_date(s: &str, today: &str) -> Option<String> {
    if !s.contains('전') {
        return None;
    }
    let mut days: i64 = 0;
    let mut saw = false;
    let b: Vec<char> = s.chars().collect();
    let mut num = String::new();
    for (i, c) in b.iter().enumerate() {
        if c.is_ascii_digit() {
            num.push(*c);
            continue;
        }
        if num.is_empty() {
            continue;
        }
        let n: i64 = num.parse().unwrap_or(0);
        num.clear();
        // "주" 뒤에 "일"이 붙는 "2주 2일" 형태를 모두 더한다
        match c {
            '주' => {
                days += n * 7;
                saw = true
            }
            '일' => {
                days += n;
                saw = true
            }
            '개' if b.get(i + 1) == Some(&'월') => {
                days += n * 30;
                saw = true
            }
            // 시간/분은 당일로 취급
            '시' | '분' => saw = true,
            _ => {}
        }
    }
    if !saw {
        return None;
    }
    add_days(today, days)
}

/// YYYY-MM-DD + n일. 윤년 포함 그레고리력.
fn add_days(date: &str, n: i64) -> Option<String> {
    let y: i64 = date.get(..4)?.parse().ok()?;
    let m: i64 = date.get(5..7)?.parse().ok()?;
    let d: i64 = date.get(8..10)?.parse().ok()?;
    // days from civil (Howard Hinnant)
    let yy = if m <= 2 { y - 1 } else { y };
    let era = if yy >= 0 { yy } else { yy - 399 } / 400;
    let yoe = yy - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let z = era * 146097 + doe - 719468 + n;
    // civil from days
    let z2 = z + 719468;
    let era2 = if z2 >= 0 { z2 } else { z2 - 146096 } / 146097;
    let doe2 = z2 - era2 * 146097;
    let yoe2 = (doe2 - doe2 / 1460 + doe2 / 36524 - doe2 / 146096) / 365;
    let y2 = yoe2 + era2 * 400;
    let doy2 = doe2 - (365 * yoe2 + yoe2 / 4 - yoe2 / 100);
    let mp2 = (5 * doy2 + 2) / 153;
    let d2 = doy2 - (153 * mp2 + 2) / 5 + 1;
    let m2 = if mp2 < 10 { mp2 + 3 } else { mp2 - 9 };
    let y2 = if m2 <= 2 { y2 + 1 } else { y2 };
    Some(format!("{y2:04}-{m2:02}-{d2:02}"))
}

fn root_dir(base: &std::path::Path, name: &str) -> Option<PathBuf> {
    match name {
        "reports" | "proposals" | "deadlines" | "portfolios" => Some(base.join(name)),
        _ => None,
    }
}

fn read_text(path: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// GET /api/state — 대시보드 집계 뷰.
async fn get_state(State(app): ApiState) -> Result<Json<Value>, ApiError> {
    let dir = &app.state_dir;
    let scan = state::load();
    let profile_summary = read_text(&dir.join("profile.yaml")).and_then(|raw| {
        serde_yaml::from_str::<Profile>(&raw)
            .ok()
            .map(|p| {
                json!({
                    "name": p.name,
                    "headline": p.headline,
                    "skills": p.skills.iter().map(|s| json!({"name": s.name, "weight": s.weight})).collect::<Vec<_>>(),
                })
            })
    });
    // 매칭이 state_dir 밖 프로필을 쓰고 있으면 힌트
    let state_profile = dir.join("profile.yaml");
    let profile_external = crate::profile::profile_path()
        .and_then(|p| p.canonicalize().ok())
        .filter(|p| {
            state_profile
                .canonicalize()
                .map(|sp| *p != sp)
                .unwrap_or(true)
        })
        .map(|p| p.display().to_string());
    let loaded = apps::load(dir);
    let today_s = today();
    // 파이프라인(yaml)에 이미 들어간 공고 id — merge로 소유권이 옮겨지기 전에 뽑는다
    let pipeline_ids: std::collections::HashSet<String> = loaded
        .file
        .applications
        .iter()
        .map(|a| a.id.clone())
        .collect();
    // 파이프라인 소스: applications.yaml > 인박스 관심.
    let applications = matches::merge(loaded.file.applications, matches::interested(&scan.seen));
    // 리포트의 LLM 분석(등급·주의점·제안 방향) — 기계 점수로는 못 내는 정보
    let analyses = reports::load_all(&dir.join("reports"));
    let inbox_count = scan
        .seen
        .iter()
        .filter(|(id, e)| inbox_visible(id, e, &pipeline_ids))
        .count();
    // 캐시된 공고 상세 — 파이프라인 상세 화면이 재조회 없이 본문을 띄운다.
    // 상세 본문이 없어도 카드 정보(예산·기간·프라이빗)만 있으면 포함한다 —
    // 좌측 요약 패널이 상세 불러오기 전에도 조건을 보여줘야 한다.
    let details: serde_json::Map<String, Value> = scan
        .seen
        .iter()
        .filter(|(_, e)| {
            e.detail_fetched_at.is_some()
                || e.budget.is_some()
                || e.duration.is_some()
                || e.private_matching.unwrap_or(false)
        })
        .map(|(id, e)| {
            (
                id.clone(),
                json!({
                    "description": e.description,
                    "conditions": e.conditions,
                    "role": e.role,
                    "level": e.level,
                    "location": e.location,
                    "matched": e.matched,
                    "skills": e.skills,
                    "budget": e.budget,
                    "duration": e.duration,
                    "private_matching": e.private_matching,
                    "detail_fetched_at": e.detail_fetched_at,
                }),
            )
        })
        .collect();
    let reports: Vec<FileEntry> = fsutil::list(&dir.join("reports"))
        .into_iter()
        .take(5)
        .collect();
    Ok(Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "last_scan": scan.last_scan,
        "seen_count": scan.seen.len(),
        "inbox_count": inbox_count,
        "details": details,
        "analyses": analyses,
        "today": today_s,
        "profile": profile_summary,
        "profile_external": profile_external,
        "stats": apps::stats(&applications),
        "applications": applications,
        "applications_parse_error": loaded.parse_error,
        "reports": reports,
    })))
}

/// GET /api/profile — 원문 + 파싱된 구조. 구조가 있으면 UI가 폼으로 편집한다.
async fn get_profile(State(app): ApiState) -> Json<Value> {
    let content = read_text(&app.state_dir.join("profile.yaml"));
    let parsed = content
        .as_deref()
        .and_then(|c| serde_yaml::from_str::<Profile>(c).ok());
    Json(json!({
        "content": content,
        "profile": parsed,
        // 원문에 주석이 있으면 폼 저장 시 사라진다는 걸 UI가 경고할 수 있게
        "has_comments": content
            .as_deref()
            .map(|c| c.lines().any(|l| l.trim_start().starts_with('#')))
            .unwrap_or(false),
    }))
}

/// PUT /api/profile/structured — 폼에서 온 구조체를 yaml로 직렬화해 저장.
/// 사람이 yaml 문법을 건드리지 않으므로 포맷이 깨질 일이 없다.
async fn put_profile_structured(
    State(app): ApiState,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let profile: Profile = serde_json::from_value(body)
        .map_err(|e| err(StatusCode::BAD_REQUEST, format!("형식 오류: {e}")))?;
    if profile.skills.is_empty() {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "기술을 최소 1개 이상 남겨야 합니다",
        ));
    }
    if let Some(bad) = profile.skills.iter().find(|s| s.name.trim().is_empty()) {
        let _ = bad;
        return Err(err(StatusCode::BAD_REQUEST, "기술 이름이 비어 있습니다"));
    }
    let yaml = serde_yaml::to_string(&profile)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let body = format!(
        "# 위시켓 프로젝트 매칭용 기술 프로필\n\
         # 대시보드 프로필 화면에서 편집됨. weight: 상대적 중요도(1~5).\n\
         # score = 100 * 매칭 weight 합 / 전체 weight 합.\n\n{yaml}"
    );
    fsutil::atomic_write(&app.state_dir.join("profile.yaml"), body.as_bytes())
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({"ok": true})))
}

async fn put_profile(
    State(app): ApiState,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let content = body
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| err(StatusCode::BAD_REQUEST, "content 필드 필요"))?;
    let parsed: Profile =
        serde_yaml::from_str(content).map_err(|e| err(StatusCode::BAD_REQUEST, e.to_string()))?;
    if parsed.skills.is_empty() {
        // 빈 yaml도 Profile으로 파싱되므로 의도치 않은 덮어쓰기를 막는 최소 가드
        return Err(err(
            StatusCode::BAD_REQUEST,
            "skills가 비어 있음 — 프로필 전체 삭제로 판단해 거절",
        ));
    }
    fsutil::atomic_write(&app.state_dir.join("profile.yaml"), content.as_bytes())
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({"ok": true})))
}

async fn get_applications(State(app): ApiState) -> Json<Value> {
    let loaded = apps::load(&app.state_dir);
    let scan = state::load();
    let applications = matches::merge(loaded.file.applications, matches::interested(&scan.seen));
    Json(json!({
        "applications": applications,
        "parse_error": loaded.parse_error,
    }))
}

/// PATCH /api/applications/{id} — status/next_action/note 갱신.
async fn patch_application(
    State(app): ApiState,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let mut loaded = apps::load(&app.state_dir);
    // matches.md에만 있는 항목을 수정하면 그 시점에 applications.yaml로 승격한다
    // (UI에서 상태를 바꿨는데 저장할 곳이 없어 404 나는 걸 막는 근본 처리)
    if !loaded.file.applications.iter().any(|a| a.id == id) {
        let scan = state::load();
        let promoted = matches::interested(&scan.seen)
            .into_iter()
            .find(|a| a.id == id)
            .ok_or_else(|| err(StatusCode::NOT_FOUND, format!("unknown id: {id}")))?;
        loaded.file.applications.push(promoted);
    }
    let entry: &mut Application = loaded
        .file
        .applications
        .iter_mut()
        .find(|a| a.id == id)
        .expect("just ensured present");
    let field = |k: &str| body.get(k).and_then(|v| v.as_str()).map(String::from);
    if let Some(status) = field("status") {
        if !apps::STATUSES.contains(&status.as_str()) {
            return Err(err(
                StatusCode::BAD_REQUEST,
                format!("status는 {:?} 중 하나여야 함", apps::STATUSES),
            ));
        }
        if status != entry.status {
            entry.status = status;
            entry.status_at = Some(today());
        }
    }
    if let Some(v) = field("next_action") {
        entry.next_action = if v.is_empty() { None } else { Some(v) };
    }
    if let Some(v) = field("note") {
        entry.note = if v.is_empty() { None } else { Some(v) };
    }
    apps::save(&app.state_dir, &loaded.file)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({"ok": true})))
}

async fn list_files(
    State(app): ApiState,
    Path(root): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let dir = root_dir(&app.state_dir, &root)
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "unknown root"))?;
    Ok(Json(json!({"files": fsutil::list(&dir)})))
}

fn file_kind(name: &str) -> Option<&'static str> {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "md" => Some("md"),
        "txt" | "ics" | "yaml" => Some("text"),
        "pdf" => Some("pdf"),
        _ => None,
    }
}

async fn get_file(
    Path((root, name)): Path<(String, String)>,
    State(app): ApiState,
) -> Result<Json<Value>, ApiError> {
    let dir = root_dir(&app.state_dir, &root)
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "unknown root"))?;
    let path = fsutil::resolve(&dir, &name).map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    match file_kind(&name) {
        Some("pdf") => Ok(Json(json!({
            "name": name, "kind": "pdf", "url": format!("/api/raw/{root}/{name}")
        }))),
        Some(kind) => {
            let content = read_text(&path)
                .ok_or_else(|| err(StatusCode::NOT_FOUND, format!("{name} 없음")))?;
            let html_out = if kind == "md" {
                Value::String(render_markdown(&content))
            } else {
                Value::Null
            };
            Ok(Json(
                json!({"name": name, "kind": kind, "content": content, "html": html_out}),
            ))
        }
        None => Err(err(StatusCode::FORBIDDEN, "지원하지 않는 확장자")),
    }
}

async fn put_file(
    State(app): ApiState,
    Path((root, name)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let dir = root_dir(&app.state_dir, &root)
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "unknown root"))?;
    let path = fsutil::resolve(&dir, &name).map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    match file_kind(&name) {
        Some("pdf") | None => Err(err(
            StatusCode::FORBIDDEN,
            "편집 불가 (pdf/알 수 없는 확장자)",
        )),
        Some(_) => {
            let content = body
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or_else(|| err(StatusCode::BAD_REQUEST, "content 필드 필요"))?;
            fsutil::atomic_write(&path, content.as_bytes())
                .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            Ok(Json(json!({"ok": true})))
        }
    }
}

/// 바이트 그대로 (pdf 인라인 열기용).
async fn get_raw(
    State(app): ApiState,
    Path((root, name)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let dir = root_dir(&app.state_dir, &root)
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "unknown root"))?;
    let path = fsutil::resolve(&dir, &name).map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    let bytes =
        std::fs::read(&path).map_err(|_| err(StatusCode::NOT_FOUND, format!("{name} 없음")))?;
    let mut res = (StatusCode::OK, bytes).into_response();
    let ct = super::content_type(&name);
    if let Ok(v) = HeaderValue::from_str(ct) {
        res.headers_mut().insert(header::CONTENT_TYPE, v);
    }
    if let Ok(v) = HeaderValue::from_str(&format!("inline; filename=\"{name}\"")) {
        res.headers_mut().insert(header::CONTENT_DISPOSITION, v);
    }
    Ok(res)
}

/// 인박스에 남는 조건 — 미분류이면서 파이프라인(applications.yaml)에도 없는 건.
/// 스킬이 yaml에 직접 등록한 건은 triage가 비어 있어도 인박스에 띄우면
/// 파이프라인과 중복으로 보인다. 인박스는 최초 단계 — 파이프라인에 있으면 끝.
fn inbox_visible(
    id: &str,
    e: &state::SeenEntry,
    pipeline_ids: &std::collections::HashSet<String>,
) -> bool {
    e.triage.is_none() && !pipeline_ids.contains(id)
}

/// GET /api/inbox — 아직 트리아지하지 않은 스캔 결과 (최신 발견순).
async fn get_inbox(State(app): ApiState) -> Json<Value> {
    let scan = state::load();
    let today_s = today();
    let analyses = reports::load_all(&app.state_dir.join("reports"));
    let pipeline_ids: std::collections::HashSet<String> = apps::load(&app.state_dir)
        .file
        .applications
        .into_iter()
        .map(|a| a.id)
        .collect();
    let mut items: Vec<Value> = scan
        .seen
        .iter()
        .filter(|(id, e)| inbox_visible(id, e, &pipeline_ids))
        .map(|(id, e)| {
            // 카드가 준 "마감 2주 2일 전" 같은 상대 표기를 날짜로 환산한다
            let deadline = e.deadline.as_deref().and_then(|d| {
                if d.len() == 10 && d.as_bytes()[4] == b'-' {
                    Some(d.to_string())
                } else {
                    relative_deadline_to_date(d, &today_s)
                }
            });
            json!({
                "id": id,
                // 제목이 비면(구 파서가 남긴 데이터) 최소한 식별 가능하게 표시한다
                "title": if e.title.trim().is_empty() {
                    format!("(제목 없음 · 공고 {id})")
                } else {
                    e.title.clone()
                },
                "title_missing": e.title.trim().is_empty(),
                "url": e.url,
                "score": e.score,
                "budget": e.budget,
                "duration": e.duration,
                "private_matching": e.private_matching,
                "deadline": deadline,
                "expired": deadline.as_deref().map(|d| d < today_s.as_str()).unwrap_or(false),
                "skills": e.skills,
                "first_seen": e.first_seen,
                "analysis": analyses.get(id),
            })
        })
        .collect();
    // 점수 높은 순, 동점이면 최근 발견순
    items.sort_by(|a, b| {
        let s = |v: &Value| v["score"].as_u64().unwrap_or(0);
        s(b).cmp(&s(a)).then_with(|| {
            b["first_seen"]
                .as_str()
                .unwrap_or("")
                .cmp(a["first_seen"].as_str().unwrap_or(""))
        })
    });
    Json(json!({ "inbox": items, "total_seen": scan.seen.len() }))
}

/// POST /api/inbox/{id}/fetch — 위시켓 상세를 지금 한 번 긁어 seen 항목을 채운다.
/// 자동 조회는 하지 않는다(Crawl-delay). 사용자가 버튼을 눌렀을 때만.
async fn fetch_inbox_detail(
    State(app): ApiState,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let detail = crate::wishket::fetch_detail(&app.http, &id)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    // 매칭 점수는 프로필 기준으로 다시 계산
    let score = crate::profile::load().ok().map(|prof| {
        let text = format!(
            "{} {} {} {}",
            detail.card.title,
            detail.card.role.as_deref().unwrap_or(""),
            detail.card.skills.join(" "),
            detail.description.as_deref().unwrap_or(""),
        );
        crate::profile::score_card(&prof, &text)
    });

    // 상세 페이지엔 카드 DOM이 없어 budget/deadline 등이 비는 경우가 있다.
    // 조건 행("모집 마감일: 2026년 09월 08일…")에서 보강한다.
    let cond = |key: &str| -> Option<String> {
        detail
            .conditions
            .iter()
            .find(|(k, _)| k.contains(key))
            .map(|(_, v)| v.clone())
    };
    let deadline = detail
        .card
        .deadline
        .clone()
        .or_else(|| cond("모집 마감").as_deref().and_then(parse_korean_date));

    let mut scan = state::load();
    if let Some(e) = scan.seen.get_mut(&id) {
        e.title = detail.card.title.clone();
        e.url = Some(detail.card.url.clone());
        if detail.card.budget.is_some() {
            e.budget = detail.card.budget.clone();
        }
        if detail.card.duration.is_some() {
            e.duration = detail.card.duration.clone();
        }
        if detail.card.private_matching.is_some() {
            e.private_matching = detail.card.private_matching;
        }
        if deadline.is_some() {
            e.deadline = deadline.clone();
        }
        if !detail.card.skills.is_empty() {
            e.skills = detail.card.skills.clone();
        }
        if let Some(n) = score
            .as_ref()
            .and_then(|m| m.get("score"))
            .and_then(Value::as_u64)
        {
            e.score = Some(n as u32);
        }
        // 본문까지 캐시한다 — 상세를 다시 열 때 재조회하지 않기 위해서.
        e.description = detail.description.clone();
        e.conditions = detail.conditions.clone();
        e.role = detail.card.role.clone();
        e.level = detail.card.level.clone();
        e.location = detail.card.location.clone();
        e.matched = score
            .as_ref()
            .and_then(|m| m.get("matched"))
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        e.detail_fetched_at = Some(state::now_iso());
        state::save(&scan).map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    Ok(Json(json!({
        "id": id,
        "title": detail.card.title,
        "url": detail.card.url,
        "budget": detail.card.budget,
        "duration": detail.card.duration,
        "private_matching": detail.card.private_matching,
        "deadline": deadline,
        "skills": detail.card.skills,
        "role": detail.card.role,
        "level": detail.card.level,
        "location": detail.card.location,
        "description": detail.description,
        "conditions": detail.conditions,
        "match": score,
    })))
}

/// GET /api/inbox/{id} — 저장된 정보만. 없으면 404.
async fn get_inbox_item(
    State(app): ApiState,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let scan = state::load();
    let e = scan
        .seen
        .get(&id)
        .ok_or_else(|| err(StatusCode::NOT_FOUND, format!("unknown id: {id}")))?;
    Ok(Json(json!({
        "id": id,
        "title": e.title,
        "url": e.url,
        "score": e.score,
        "budget": e.budget,
        "duration": e.duration,
        "private_matching": e.private_matching,
        "deadline": e.deadline,
        "skills": e.skills,
        "first_seen": e.first_seen,
        "triage": e.triage,
        "triaged_at": e.triaged_at,
        // 캐시된 상세 — 있으면 프론트가 재조회 없이 바로 렌더한다
        "description": e.description,
        "conditions": e.conditions,
        "role": e.role,
        "level": e.level,
        "location": e.location,
        "matched": e.matched,
        "detail_fetched_at": e.detail_fetched_at,
        "analysis": reports::load_all(&app.state_dir.join("reports")).get(&id),
    })))
}

/// POST /api/inbox/{id}/triage — {action: "interested"|"skipped"|"reset"}
async fn triage_inbox(
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let action = body
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or_else(|| err(StatusCode::BAD_REQUEST, "action 필드 필요"))?;
    let triage = match action {
        "interested" => Some(state::Triage::Interested),
        "skipped" => Some(state::Triage::Skipped),
        "reset" => None,
        other => {
            return Err(err(
                StatusCode::BAD_REQUEST,
                format!("action은 interested|skipped|reset 중 하나 (받음: {other})"),
            ))
        }
    };
    let mut scan = state::load();
    let entry = scan
        .seen
        .get_mut(&id)
        .ok_or_else(|| err(StatusCode::NOT_FOUND, format!("unknown id: {id}")))?;
    entry.triage = triage;
    entry.triaged_at = triage.map(|_| today());
    state::save(&scan).map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({"ok": true})))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/state", get(get_state))
        .route("/profile", get(get_profile).put(put_profile))
        .route(
            "/profile/structured",
            axum::routing::put(put_profile_structured),
        )
        .route("/applications", get(get_applications))
        .route("/inbox", get(get_inbox))
        .route("/inbox/{id}", get(get_inbox_item))
        .route("/inbox/{id}/fetch", axum::routing::post(fetch_inbox_detail))
        .route("/inbox/{id}/triage", axum::routing::post(triage_inbox))
        .route(
            "/applications/{id}",
            axum::routing::patch(patch_application),
        )
        .route("/files/{root}", get(list_files))
        .route("/files/{root}/{*name}", get(get_file).put(put_file))
        .route("/raw/{root}/{*name}", get(get_raw))
}

#[cfg(test)]
mod tests {
    use super::render_markdown;

    #[test]
    fn inbox_excludes_yaml_pipeline_items() {
        use super::inbox_visible;
        use crate::state::{SeenEntry, Triage};
        let mut ids = std::collections::HashSet::new();
        ids.insert("7".to_string());
        let e = SeenEntry::default();
        assert!(inbox_visible("1", &e, &ids), "미분류 + yaml 없음 = 인박스");
        assert!(
            !inbox_visible("7", &e, &ids),
            "미분류라도 yaml에 있으면 파이프라인 전용"
        );
        let mut skipped = SeenEntry::default();
        skipped.triage = Some(Triage::Skipped);
        assert!(
            !inbox_visible("1", &skipped, &ids),
            "스킵은 인박스에서 사라짐"
        );
    }

    #[test]
    fn external_links_open_in_new_tab() {
        let html = render_markdown("[위시켓](https://www.wishket.com/project/1/)");
        assert!(html.contains("target=\"_blank\""), "{html}");
        assert!(html.contains("rel=\"noopener noreferrer\""), "{html}");
    }

    #[test]
    fn relative_links_are_untouched() {
        // 내부 앵커까지 새 탭으로 열면 안 된다
        let html = render_markdown("[섹션](#anchor)");
        assert!(!html.contains("target="), "{html}");
    }

    #[test]
    fn relative_deadline_converts_to_date() {
        use super::relative_deadline_to_date as r;
        // 카드 목록은 절대 날짜가 아니라 "마감 N일 전" 상대 표기를 준다
        assert_eq!(
            r("마감 2주 2일 전", "2026-09-02").as_deref(),
            Some("2026-09-18")
        );
        assert_eq!(
            r("마감 6일 8시간 전", "2026-09-02").as_deref(),
            Some("2026-09-08")
        );
        assert_eq!(
            r("마감 1일 전", "2026-09-02").as_deref(),
            Some("2026-09-03")
        );
        assert_eq!(
            r("마감 3시간 전", "2026-09-02").as_deref(),
            Some("2026-09-02"),
            "당일"
        );
        assert_eq!(
            r("마감 1개월 전", "2026-09-02").as_deref(),
            Some("2026-10-02")
        );
        // 월/연 경계
        assert_eq!(
            r("마감 1일 전", "2026-12-31").as_deref(),
            Some("2027-01-01")
        );
        assert_eq!(
            r("마감 1일 전", "2028-02-28").as_deref(),
            Some("2028-02-29"),
            "윤년"
        );
        // 상대 표기가 아니면 None
        assert!(r("협의 후 결정", "2026-09-02").is_none());
        assert!(r("2026-09-08", "2026-09-02").is_none());
    }

    #[test]
    fn korean_date_from_condition_row() {
        use super::parse_korean_date;
        assert_eq!(
            parse_korean_date("2026년 09월 08일마감 6일 8시간 전").as_deref(),
            Some("2026-09-08")
        );
        assert_eq!(
            parse_korean_date("2026년 9월 8일").as_deref(),
            Some("2026-09-08")
        );
        assert!(parse_korean_date("계약 체결 이후, 즉시 시작").is_none());
        assert!(
            parse_korean_date("2026년 13월 08일").is_none(),
            "월 범위 검증"
        );
    }

    #[test]
    fn tables_render() {
        let html = render_markdown("| a | b |\n|---|---|\n| 1 | 2 |");
        assert!(html.contains("<table>"), "{html}");
    }
}
