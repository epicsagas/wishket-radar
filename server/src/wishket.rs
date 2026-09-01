//! wishket.com HTML/API client and parsers.
//!
//! Search mechanism (reverse-engineered from the site's search.js):
//! filters are "k=v&k=v" pairs compressed with LZString.compressToBase64,
//! URL-encoded, and sent as `?d=`. With `X-Requested-With: XMLHttpRequest`
//! the endpoint returns JSON `{result: <html>, count: <total>}`; without it
//! the same URL returns the full server-rendered page (same card DOM).

use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const BASE: &str = "https://www.wishket.com";
/// 정체성을 밝히는 봇 UA. 위시켓은 UA 기반 차단을 하지 않고(nginx, 200 OK 확인) —
/// 브라우저 위장보다 연락처가 드러나는 쪽이 방어 가능하다.
pub const UA: &str = "wishket-radar/0.1.0 (+https://github.com/epicsagas/wishket-radar)";

/// Inter-request delay. robots.txt Crawl-delay: 5 준수.
pub const REQUEST_DELAY_MS: u64 = 5000;

// ---------------------------------------------------------------------------
// LZString filter encoding
// ---------------------------------------------------------------------------

/// Compress a `k=v&k=v` filter string into the `d=` parameter value.
pub fn build_d(pairs: &[(&str, String)]) -> String {
    let q = pairs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&");
    lz_str::compress_to_base64(&q)
}

fn decompress_d(d: &str) -> Option<String> {
    lz_str::decompress_from_base64(d).and_then(|v| String::from_utf16(&v).ok())
}

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Default)]
pub struct ClientInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ProjectCard {
    pub id: String,
    pub url: String,
    pub title: String,
    /// e.g. "모집 중"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// e.g. "월 금액 7,000,000원 /월" or "협의 후 결정"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget: Option<String>,
    /// e.g. "예상 기간 300일"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    /// "기간제" / "과제" 등
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_type: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posted_at: Option<String>,
    /// 남은 마감 (e.g. "마감 2주 2일 전")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applicants: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub views: Option<String>,
    /// 댓글(프로젝트 문의) 수 — 검색 카드 `i.comment-status[data-count]`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments: Option<String>,
    /// 좋아요(관심 프로젝트 추가) 수 — 검색 카드 `i.interest-status[data-count]`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub likes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<ClientInfo>,
    /// 결정론적 프로필 매칭 결과 (main.rs에서 채움)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#match: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectDetail {
    #[serde(flatten)]
    pub card: ProjectCard,
    /// JSON-LD JobPosting 전체 설명
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_posted: Option<String>,
    /// 모집 마감 ISO (validThrough)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_through: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub salary: Option<String>,
    /// 조건 행 (모집 마감일/예상 시작일/진행 분류/기획 상태/관련 기술 ...)
    pub conditions: Vec<(String, String)>,
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

fn sel(css: &str) -> Selector {
    Selector::parse(css).expect("valid selector")
}

/// Concatenate all descendant text nodes and collapse whitespace.
fn text_of(el: ElementRef) -> String {
    el.text()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn first_text(doc: &Html, css: &str) -> Option<String> {
    doc.select(&sel(css)).next().map(text_of)
}

/// "예상 기간300일" → "예상 기간 300일": 라벨 뒤 공백 보정.
fn label_value(text: String, label: &str) -> String {
    match text.strip_prefix(label) {
        Some(v) => format!("{label} {v}"),
        None => text,
    }
}

fn texts_of(doc: &Html, css: &str) -> Vec<String> {
    doc.select(&sel(css)).map(text_of).collect()
}

/// Parse project cards. Works on both the AJAX `result` fragment and the
/// full SSR list page: both use the `.project-info-box` card DOM.
pub fn parse_cards(html: &str) -> Vec<ProjectCard> {
    let doc = Html::parse_document(html);
    let card_sel = sel("div.project-info-box");
    let link = sel("a.project-link");
    let budget = sel("p.budget");
    let term = sel("p.term");
    let launch = sel("p.launch-date");
    let role = sel("p.project-category-or-role");
    let level = sel("p.project-level");
    let work_type = sel("div.status-mark.project-type-mark");
    let skill_chip = sel("div.project-skills-info span.skill-chip");
    let location = sel("p.location-data");
    let posted = sel("p.start-recruitment-data");
    let info_detail = sel("section.proposal-and-client-info p.info-detail");
    let views = sel("i.view-status");
    let comments = sel("i.comment-status");
    let likes = sel("i.interest-status");
    let username = sel("div.client-info p.username");
    let badge_span = sel("div.client-info .badge-box span");
    let verified = sel("div.client-info i.client-badge img[alt='인증된 클라이언트']");

    let mut cards = Vec::new();
    for card in doc.select(&card_sel) {
        let Some(a) = card.select(&link).next() else {
            continue;
        };
        let Some(href) = a.value().attr("href") else {
            continue;
        };
        let id = href
            .trim_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .to_string();
        let title = a.select(&sel("p")).next().map(text_of).unwrap_or_default();
        if id.is_empty() || title.is_empty() {
            continue;
        }

        let skills: Vec<String> = card.select(&skill_chip).map(text_of).collect();

        // proposal-info: "마감 2주 2일 전", "지원자 없음"
        let mut deadline = None;
        let mut applicants = None;
        for p in card.select(&info_detail) {
            let t = text_of(p);
            if t.starts_with("마감") {
                deadline = Some(t);
            } else if t.starts_with("지원자") {
                applicants = Some(t);
            }
        }

        let client = if card.select(&username).next().is_some() {
            Some(ClientInfo {
                name: card.select(&username).next().map(text_of),
                rating: card.select(&badge_span).last().map(text_of),
                verified: Some(card.select(&verified).next().is_some()),
            })
        } else {
            None
        };

        cards.push(ProjectCard {
            url: format!("{BASE}/project/{id}/"),
            id,
            title,
            status: card.select(&sel("div.status-mark.recruiting-mark")).next().map(text_of),
            budget: card.select(&budget).next().map(text_of),
            duration: card.select(&term).next().map(|el| label_value(text_of(el), "예상 기간")),
            start_date: card.select(&launch).next().map(|el| label_value(text_of(el), "근무 시작일")),
            role: card.select(&role).next().map(text_of),
            level: card.select(&level).next().map(text_of),
            work_type: card.select(&work_type).next().map(text_of),
            skills,
            location: card.select(&location).next().map(text_of),
            posted_at: card.select(&posted).next().map(|el| {
                text_of(el)
                    .trim_start_matches("·")
                    .trim_start()
                    .to_string()
            }),
            deadline,
            applicants,
            views: card
                .select(&views)
                .next()
                .and_then(|el| el.value().attr("data-count"))
                .map(str::to_string),
            comments: card
                .select(&comments)
                .next()
                .and_then(|el| el.value().attr("data-count"))
                .map(str::to_string),
            likes: card
                .select(&likes)
                .next()
                .and_then(|el| el.value().attr("data-count"))
                .map(str::to_string),
            client,
            r#match: None,
        });
    }
    cards
}

/// Minimal HTML entity decode for JSON-LD payloads that embed entities.
fn decode_entities(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

#[derive(Deserialize)]
struct JobPosting {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(rename = "datePosted", default)]
    date_posted: Option<String>,
    #[serde(rename = "validThrough", default)]
    valid_through: Option<String>,
    #[serde(rename = "baseSalary", default)]
    base_salary: Option<Value>,
    #[serde(rename = "jobLocation", default)]
    job_location: Option<Value>,
}

/// Extract the schema.org JobPosting JSON-LD block, if present.
fn find_job_posting(doc: &Html) -> Option<JobPosting> {
    for script in doc.select(&sel("script[type='application/ld+json']")) {
        let raw = script.text().collect::<String>();
        let parsed = serde_json::from_str::<Value>(&raw)
            .or_else(|_| serde_json::from_str::<Value>(&decode_entities(&raw)));
        let Ok(v) = parsed else { continue };
        let v = if v.is_array() {
            v.as_array()?.clone()
        } else {
            vec![v]
        };
        for item in v {
            if item.get("@type").and_then(Value::as_str) == Some("JobPosting") {
                if let Ok(jp) = serde_json::from_value::<JobPosting>(item) {
                    return Some(jp);
                }
            }
        }
    }
    None
}

/// Parse detail page. Combines JSON-LD JobPosting (title/description/dates)
/// with the condition rows and skill tags from the HTML body.
pub fn parse_detail(id: &str, html: &str) -> ProjectDetail {
    let doc = Html::parse_document(html);
    let jp = find_job_posting(&doc);

    let cond_row = sel("div.project-detail-condition-row");
    let mut conditions = Vec::new();
    for row in doc.select(&cond_row) {
        let mut parts = row
            .children()
            .filter_map(ElementRef::wrap)
            .map(text_of)
            .filter(|t| !t.is_empty());
        if let (Some(label), Some(value)) = (parts.next(), parts.next()) {
            conditions.push((label, value));
        }
    }

    let skill_tag = sel("span.skill-tag");

    let mut card = parse_cards(html)
        .into_iter()
        .next()
        .unwrap_or_else(|| ProjectCard {
            id: id.to_string(),
            url: format!("{BASE}/project/{id}/"),
            title: jp.as_ref().and_then(|j| j.title.clone()).unwrap_or_default(),
            ..Default::default()
        });
    // 상세 설명까지 매칭 텍스트로 쓸 수 있게 skills 보강
    for tag in doc.select(&skill_tag) {
        let t = text_of(tag);
        if !t.is_empty() && !card.skills.contains(&t) {
            card.skills.push(t);
        }
    }

    // 상세 페이지엔 상태 아이콘이 없고 `프로젝트 문의 N` 카운트가 렌더링됨
    if card.comments.is_none() {
        card.comments = first_text(&doc, "span.comment-layer-count");
    }

    let salary = jp.as_ref().and_then(|j| {
        j.base_salary
            .as_ref()
            .and_then(|v| salary_to_string(v))
    });

    ProjectDetail {
        card,
        description: jp.as_ref().and_then(|j| j.description.clone()),
        date_posted: jp.as_ref().and_then(|j| j.date_posted.clone()),
        valid_through: jp.as_ref().and_then(|j| j.valid_through.clone()),
        salary,
        conditions,
    }
}

/// baseSalary may be a plain string or a schema.org structured object.
fn salary_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Object(_) => {
            // {"@type": "MonetaryAmount", "value": {"value": 20000000, ...}, "currency": "KRW"}
            let val = v
                .pointer("/value/value")
                .or_else(|| v.pointer("/value"))
                .or_else(|| v.get("value"));
            match val {
                Some(Value::Number(n)) => Some(format!("{n}원")),
                Some(Value::String(s)) => Some(s.clone()),
                Some(Value::Object(o)) => {
                    let mut parts = Vec::new();
                    if let (Some(min), Some(max)) = (o.get("minValue"), o.get("maxValue")) {
                        parts.push(format!("{min}~{max}"));
                    }
                    if let Some(cur) = v.get("currency").and_then(Value::as_str) {
                        parts.push(cur.to_string());
                    }
                    if parts.is_empty() {
                        None
                    } else {
                        Some(parts.join(" "))
                    }
                }
                _ => None,
            }
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// HTTP
// ---------------------------------------------------------------------------

/// Fetch one search page. Returns (total_count, cards).
/// AJAX first; falls back to parsing the SSR page if the JSON shape changes.
pub async fn fetch_search(
    http: &reqwest::Client,
    pairs: &[(&str, String)],
) -> Result<(u32, Vec<ProjectCard>), String> {
    let d = build_d(pairs);
    let url = format!("{BASE}/project/");
    let resp = http
        .get(&url)
        .query(&[("d", d)])
        .header("X-Requested-With", "XMLHttpRequest")
        .header("User-Agent", UA)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    let body = resp.text().await.map_err(|e| format!("body read failed: {e}"))?;

    if let Ok(v) = serde_json::from_str::<Value>(&body) {
        let count = v.get("count").and_then(Value::as_u64).unwrap_or(0) as u32;
        let html = v
            .get("result")
            .and_then(Value::as_str)
            .unwrap_or_default();
        Ok((count, parse_cards(html)))
    } else {
        // SSR fallback: no count available; derive from pagination if present
        let cards = parse_cards(&body);
        Ok((cards.len() as u32, cards))
    }
}

/// Fetch a project detail page.
pub async fn fetch_detail(
    http: &reqwest::Client,
    id: &str,
) -> Result<ProjectDetail, String> {
    let url = format!("{BASE}/project/{id}/");
    let resp = http
        .get(&url)
        .header("User-Agent", UA)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| format!("body read failed: {e}"))?;
    if !status.is_success() {
        return Err(format!("HTTP {status} for {url}"));
    }
    Ok(parse_detail(id, &body))
}

/// Filter key documentation for the `list_filters` tool.
pub fn filter_docs() -> Value {
    serde_json::json!({
        "categories": {
            "development": "개발", "design": "디자인", "planning": "기획",
            "marketing": "마케팅", "etc": "기타"
        },
        "form_factors": ["web", "pc", "android", "ios", "etc"],
        "verified_keys": {
            "c": "카테고리 (development 등)",
            "ff": "형태 (콤마 결합: web,pc,android,ios)",
            "page": "페이지 (1부터)",
            "s": "키워드 검색"
        },
        "unverified_keys": {
            "srt": "정렬", "abi": "금액 min", "aba": "금액 max",
            "ati": "기간 min", "ata": "기간 max", "r": "지역", "l": "지역(기간제)",
            "eit": "업종", "mt": "SI/SM", "ewr": "재택", "cr": "평가 우수", "cv": "인증 완료"
        }
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Known vector: the `d=` param from a real wishket URL the user shared.
    const KNOWN_D: &str =
        "MYXgJgpgbhA2D2AHAthAdgFwGQDMcgHcIAjAGkWFIEM0wAneASzFMfgGcg==";
    const KNOWN_FILTER: &str = "c=development&ff=web,pc,android,ios";

    #[test]
    fn lzstring_roundtrip() {
        let dec = decompress_d(KNOWN_D).expect("decompress");
        assert_eq!(dec, KNOWN_FILTER);
        // lz-str의 base64 패딩이 원본과 자릿수가 다를 수 있으므로 바이트 동등성 대신
        // 재압축 결과가 다시 원본 필터로 풀리는지(의미 동등)로 게이트한다.
        let d = build_d(&[("c", "development".into()), ("ff", "web,pc,android,ios".into())]);
        assert_eq!(decompress_d(&d).as_deref(), Some(KNOWN_FILTER));
    }

    /// Real card markup from the live site (trimmed to one card).
    const CARD_HTML: &str = r#"
    <div class="project-info-box"><div class="project-info-box-wrapper">
    <section class="project-organic-info">
      <div class="project-status-label recruiting-status mb12"><div class="status-mark recruiting-mark">모집 중</div></div>
      <a class="subtitle-2-medium project-link" href="/project/157463/"><p class="subtitle-1-half-medium mb10">Databricks 기반 데이터 플랫폼 구축 데이터 엔지니어</p></a>
      <div class="project-core-info mb10">
        <p class="budget body-1 text700">월 금액 <span class="body-1-medium">7,000,000원 <span class="body-2-medium">/월</span></span></p>
        <span class="info-divider"></span>
        <p class="term body-1 text700">예상 기간<span class="body-1-medium">300일</span></p>
        <span class="info-divider"></span>
        <p class="launch-date body-1 text700">근무 시작일 <span class="body-1-medium">즉시 시작</span></p>
      </div>
      <div class="project-classification-info mb32"><p class="project-category-or-role body-2 text700">데이터 엔지니어</p><p class="project-level body-2 text700">시니어</p></div>
      <div class="project-minor-info">
        <div class="project-status-label recruiting-status"><div class="status-mark project-type-mark with-img"><img class="project-type-mark-img" src="/x.svg"/>기간제</div></div>
        <div class="divider"></div>
        <div class="project-skills-info"><span class="skill-chip body-3 text600">Databricks · 경력 무관</span></div>
        <p class="body-3 text500 location-data"><img src="/x.png"/>서울특별시 강남구</p>
        <p class="body-3 text300 start-recruitment-data">· 등록일자 2026.08.05.</p>
      </div>
    </section>
    <section class="proposal-and-client-info">
      <div class="proposal-info">
        <p class="info-detail body-2 mb8 text600"><img src="/x.png"/>마감 <span class="body-2-medium text600">2주 2일 전</span></p>
        <p class="info-detail body-2 text600"><img src="/x.png"/>지원자 <span class="body-2-medium text600">없음</span></p>
      </div>
      <div class="minor-info has-value">
        <div class="status-wrap"><i class="project-tooltip status-icon view-status" data-count="2,022"><span class="caption-1 text400">아주 높음</span></i></div>
        <div class="status-wrap"><i class="project-tooltip status-icon comment-status" data-count="16"><span class="caption-1 text400">16</span></i></div>
        <div class="status-wrap"><i class="project-tooltip status-icon interest-status" data-count="31"><span class="caption-1 text400">31</span></i></div>
      </div>
      <div class="client-info">
        <div class="profile-box mb12"><p class="username body-3-medium">to******</p></div>
        <div class="badge-box">
          <i class="project-tooltip client-badge"><img alt="인증된 클라이언트" src="/x.png"/><span class="caption-1 text600">인증 완료</span></i>
          <i class="client-badge"><span class="caption-1 text600">3.7</span></i>
        </div>
      </div>
    </section></div></div>
    "#;

    #[test]
    fn parses_card_fields() {
        let cards = parse_cards(CARD_HTML);
        assert_eq!(cards.len(), 1);
        let c = &cards[0];
        assert_eq!(c.id, "157463");
        assert_eq!(c.title, "Databricks 기반 데이터 플랫폼 구축 데이터 엔지니어");
        assert_eq!(c.url, "https://www.wishket.com/project/157463/");
        assert_eq!(c.status.as_deref(), Some("모집 중"));
        assert_eq!(c.budget.as_deref(), Some("월 금액 7,000,000원 /월"));
        assert_eq!(c.duration.as_deref(), Some("예상 기간 300일"));
        assert_eq!(c.role.as_deref(), Some("데이터 엔지니어"));
        assert_eq!(c.level.as_deref(), Some("시니어"));
        assert_eq!(c.work_type.as_deref(), Some("기간제"));
        assert_eq!(c.skills, vec!["Databricks · 경력 무관"]);
        assert_eq!(c.location.as_deref(), Some("서울특별시 강남구"));
        assert_eq!(c.posted_at.as_deref(), Some("등록일자 2026.08.05."));
        assert_eq!(c.deadline.as_deref(), Some("마감 2주 2일 전"));
        assert_eq!(c.applicants.as_deref(), Some("지원자 없음"));
        assert_eq!(c.views.as_deref(), Some("2,022"));
        assert_eq!(c.comments.as_deref(), Some("16"));
        assert_eq!(c.likes.as_deref(), Some("31"));
        assert_eq!(c.client.as_ref().unwrap().name.as_deref(), Some("to******"));
        assert_eq!(c.client.as_ref().unwrap().rating.as_deref(), Some("3.7"));
        assert_eq!(c.client.as_ref().unwrap().verified, Some(true));
    }

    #[test]
    fn parses_detail_jsonld() {
        let html = r#"
        <html><body>
        <div class="project-detail-condition-row"><div class="a body-2 text500">예상 기간</div><div class="b">30일</div></div>
        <div class="project-detail-condition-row"><div class="a body-2 text500">관련 기술</div><div class="b">Flutter, Rust</div></div>
        <div class="project-detail-skills-list mb16"><span class="skill-tag body-3 text600">flutter, rust, postgresql</span></div>
        <div class="project-comment-layer mb40"><p class="comment-layer-title">프로젝트 문의 <span class="comment-layer-count subtitle-2-medium">3</span></p><div class="comment-data-box"></div></div>
        <script type="application/ld+json">
        {"@context":"https://schema.org","@type":"JobPosting",
         "title":"테스트 프로젝트",
         "description":"[프로젝트 개요]\r\n- 상세 설명 본문",
         "datePosted":"2026-08-31","validThrough":"2026-09-07T23:59",
         "baseSalary":"2,000만원 (부가세 별도)"}
        </script>
        </body></html>"#;
        let d = parse_detail("999", html);
        assert_eq!(d.card.id, "999");
        assert_eq!(d.card.title, "테스트 프로젝트");
        assert!(d.description.as_deref().unwrap().contains("상세 설명 본문"));
        assert_eq!(d.date_posted.as_deref(), Some("2026-08-31"));
        assert_eq!(d.valid_through.as_deref(), Some("2026-09-07T23:59"));
        assert_eq!(d.salary.as_deref(), Some("2,000만원 (부가세 별도)"));
        assert_eq!(d.conditions.len(), 2);
        assert_eq!(d.conditions[0], ("예상 기간".to_string(), "30일".to_string()));
        assert!(d.card.skills.contains(&"flutter, rust, postgresql".to_string()));
        assert_eq!(d.card.comments.as_deref(), Some("3"));
    }
}
