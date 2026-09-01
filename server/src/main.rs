//! wishket-mcp — MCP server exposing wishket.com project search/analysis.
//!
//! Tools: search_projects, scan_new, get_project, list_filters, reset_cache.

mod profile;
mod state;
mod wishket;

use std::time::Duration;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::schemars;
use rmcp::tool;
use rmcp::transport::stdio;
use rmcp::{tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::time::sleep;

use wishket::{fetch_detail, fetch_search, ProjectCard};

fn err(msg: String) -> McpError {
    McpError::internal_error(msg, None)
}

fn json_result(v: Value) -> CallToolResult {
    CallToolResult::structured(v)
}

#[derive(Debug, Clone)]
struct Wishket {
    http: reqwest::Client,
}

#[tool_router]
impl Wishket {
    fn new() -> Self {
        let http = reqwest::Client::builder()
            .user_agent(wishket::UA)
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client");
        Self { http }
    }

    /// 공통: 카테고리/형태/키워드를 d= 파라미터 쌍으로 변환.
    /// category/form_factors/keyword는 시맨틱 검증된 키, raw는 미검증 패스스루.
    fn filter_pairs(
        category: Option<&str>,
        form_factors: Option<&str>,
        keyword: Option<&str>,
        raw: Option<&Value>,
        page: Option<u32>,
    ) -> Vec<(String, String)> {
        let mut pairs: Vec<(String, String)> = Vec::new();
        let mut push = |k: &str, v: String| {
            if !v.is_empty() {
                pairs.push((k.to_string(), v));
            }
        };
        if let Some(c) = category {
            push("c", c.to_string());
        }
        if let Some(f) = form_factors {
            push("ff", f.replace(' ', ""));
        }
        if let Some(s) = keyword {
            push("s", s.to_string());
        }
        if let Some(Value::Object(map)) = raw {
            for (k, v) in map {
                let v = match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                push(k, v);
            }
        }
        if let Some(p) = page {
            if p > 1 {
                push("page", p.to_string());
            }
        }
        pairs
    }

    async fn search_pages(
        &self,
        pairs: &[(String, String)],
        max_pages: u32,
    ) -> Result<(u32, Vec<ProjectCard>), String> {
        let mut total = 0u32;
        let mut cards = Vec::new();
        for p in 1..=max_pages {
            let mut page_pairs = pairs.to_vec();
            if p > 1 {
                page_pairs.push(("page".into(), p.to_string()));
            }
            let refs: Vec<(&str, String)> =
                page_pairs.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
            let (count, mut batch) = fetch_search(&self.http, &refs).await?;
            if batch.is_empty() {
                break;
            }
            if p == 1 {
                total = count;
            }
            cards.append(&mut batch);
            if p < max_pages {
                sleep(Duration::from_millis(wishket::REQUEST_DELAY_MS)).await;
            }
        }
        Ok((total, cards))
    }

    fn attach_match(cards: &mut [&mut ProjectCard], detail_text: Option<&str>) -> Value {
        let prof = match profile::load() {
            Ok(p) => p,
            Err(e) => return json!({ "profile_error": e }),
        };
        for c in cards.iter_mut() {
            let text = format!(
                "{} {} {} {}",
                c.title,
                c.role.as_deref().unwrap_or(""),
                c.skills.join(" "),
                detail_text.unwrap_or(""),
            );
            c.r#match = Some(profile::score_card(&prof, &text));
        }
        json!({ "profile": {
            "name": prof.name,
            "headline": prof.headline,
            "skills": prof.skills.iter().map(|s| &s.name).collect::<Vec<_>>(),
        }})
    }

    #[tool(description = "위시켓 프로젝트 검색 (캐시 기록 없음, 순수 조회). 페이지당 10건. 기본 카테고리 development.")]
    async fn search_projects(
        &self,
        Parameters(p): Parameters<SearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let pairs = Self::filter_pairs(
            p.category.as_deref(),
            p.form_factors.as_deref(),
            p.keyword.as_deref(),
            p.raw.as_ref(),
            p.page,
        );
        let refs: Vec<(&str, String)> =
            pairs.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        let (count, mut cards) = fetch_search(&self.http, &refs).await.map_err(err)?;
        let mut refs_mut: Vec<&mut ProjectCard> = cards.iter_mut().collect();
        let prof_info = Self::attach_match(&mut refs_mut, None);
        Ok(json_result(json!({
            "count": count,
            "returned": cards.len(),
            "profile": prof_info["profile"],
            "projects": cards,
        })))
    }

    #[tool(description = "마지막 스캔 이후 신규 프로젝트만 반환 (신규는 seen 캐시에 기록). 기본 최대 3페이지(30건) 조회, 요청 간 5초 지연 (robots Crawl-delay). 첫 실행은 전체가 신규(베이스라인).")]
    async fn scan_new(
        &self,
        Parameters(p): Parameters<ScanParams>,
    ) -> Result<CallToolResult, McpError> {
        let pairs = Self::filter_pairs(
            p.category.as_deref(),
            p.form_factors.as_deref(),
            p.keyword.as_deref(),
            None,
            None,
        );
        let max_pages = p.max_pages.unwrap_or(3).clamp(1, 10);
        let (count, mut cards) = self.search_pages(&pairs, max_pages).await.map_err(err)?;

        let scan_at = state::now_iso();
        let mut st = state::load();
        state::prune(&mut st);
        let known_before = st.seen.len();
        let mut already = 0u32;
        cards.retain(|c| match st.seen.get(&c.id) {
            Some(_) => {
                already += 1;
                false
            }
            None => {
                st.seen.insert(
                    c.id.clone(),
                    state::SeenEntry {
                        first_seen: scan_at.clone(),
                        title: c.title.clone(),
                    },
                );
                true
            }
        });
        let known_after = st.seen.len();
        st.last_scan = Some(scan_at.clone());
        state::save(&st).map_err(|e| err(format!("state save: {e}")))?;

        let mut card_refs: Vec<&mut ProjectCard> = cards.iter_mut().collect();
        let prof_info = Self::attach_match(&mut card_refs, None);
        drop(card_refs);
        // 점수 내림차순
        cards.sort_by(|a, b| {
            let sa = a.r#match.as_ref().and_then(|m| m.get("score")).and_then(Value::as_u64).unwrap_or(0);
            let sb = b.r#match.as_ref().and_then(|m| m.get("score")).and_then(Value::as_u64).unwrap_or(0);
            sb.cmp(&sa)
        });

        Ok(json_result(json!({
            "new": cards,
            "new_count": cards.len(),
            "already_known": already,
            "total_matching_filter": count,
            "seen_total_before": known_before,
            "seen_total_after": known_after,
            "baseline": known_before == 0,
            "scan_at": scan_at,
            "profile": prof_info["profile"],
        })))
    }

    #[tool(description = "프로젝트 상세 조회 (JSON-LD 전체 설명 포함). id는 숫자 문자열 (예: \"158063\").")]
    async fn get_project(
        &self,
        Parameters(p): Parameters<DetailParams>,
    ) -> Result<CallToolResult, McpError> {
        let id = p.id.trim().trim_matches('/').rsplit('/').next().unwrap_or("").to_string();
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
            return Err(McpError::invalid_params(
                format!("invalid project id: {:?}", p.id),
                None,
            ));
        }
        let mut d = fetch_detail(&self.http, &id).await.map_err(err)?;
        let prof_info = Self::attach_match(&mut [&mut d.card], d.description.as_deref());
        Ok(json_result(json!({
            "project": d,
            "profile": prof_info["profile"],
        })))
    }

    #[tool(description = "사용 가능한 검색 필터 키와 값 목록. verified_keys만 의미가 확인됨. unverified_keys는 raw 파라미터로 실험적 사용.")]
    async fn list_filters(&self) -> Result<CallToolResult, McpError> {
        Ok(json_result(wishket::filter_docs()))
    }

    #[tool(description = "seen 캐시 초기화 — 다음 스캔이 다시 베이스라인이 됨.")]
    async fn reset_cache(&self) -> Result<CallToolResult, McpError> {
        let path = state::state_path();
        let cleared = std::fs::remove_file(&path).is_ok();
        Ok(json_result(json!({
            "cleared": cleared,
            "path": path.display().to_string(),
        })))
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SearchParams {
    /// 카테고리: development / design / planning / marketing / etc (기본 development)
    #[serde(default)]
    category: Option<String>,
    /// 형태 콤마결합: "web,pc,android,ios" (기본 web,pc,android,ios는 명시 필요)
    #[serde(default)]
    form_factors: Option<String>,
    /// 키워드 검색어
    #[serde(default)]
    keyword: Option<String>,
    /// 1부터 (기본 1)
    #[serde(default)]
    page: Option<u32>,
    /// 미검증 위시켓 원본 필터 키/값 패스스루 (예: {"srt": "..."}). list_filters 참고.
    #[serde(default)]
    raw: Option<Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ScanParams {
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    form_factors: Option<String>,
    #[serde(default)]
    keyword: Option<String>,
    /// 최대 페이지 수 1~10 (기본 3)
    #[serde(default)]
    max_pages: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct DetailParams {
    /// 프로젝트 ID 숫자 또는 상세 URL
    id: String,
}

#[tool_handler(name = "wishket-mcp", version = "0.1.0")]
impl ServerHandler for Wishket {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = Wishket::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
