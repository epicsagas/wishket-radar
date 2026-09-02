//! `wishket-mcp dashboard` — 로컬 webui. ~/.wishket-radar 상태 파일을 읽고(일부 편집) 서빙한다.
//! 파일이 정규 소스고 UI는 파생 뷰다. 0.0.0.0 바인드 + 토큰 인증.

pub mod api;
pub mod apps;
pub mod fsutil;
pub mod matches;
pub mod reports;

use std::io::Write;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use rust_embed::RustEmbed;
use serde_json::json;

use crate::state;

pub const DEFAULT_PORT: u16 = 8787;
const TOKEN_FILE: &str = "dashboard-token";

pub struct AppState {
    pub token: String,
    pub state_dir: PathBuf,
    /// 인박스 상세 "불러오기" 버튼 전용. 자동 조회는 하지 않는다
    /// (robots Crawl-delay 5초 — 사용자가 누를 때만 나간다).
    pub http: reqwest::Client,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cli {
    pub port: u16,
    pub no_open: bool,
}

impl Default for Cli {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            no_open: false,
        }
    }
}

impl Cli {
    /// `--port N` / `--port=N` / `--no-open`. 무효 토큰이면 Err(사용법).
    pub fn parse_from(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut cli = Self::default();
        let mut it = args.into_iter();
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--no-open" => cli.no_open = true,
                "--port" => {
                    let v = it.next().ok_or("--port 값 누락")?;
                    cli.port = v.parse().map_err(|_| format!("잘못된 포트: {v}"))?;
                }
                other => {
                    if let Some(v) = other.strip_prefix("--port=") {
                        cli.port = v.parse().map_err(|_| format!("잘못된 포트: {v}"))?;
                    } else {
                        return Err(format!("알 수 없는 인자: {other}\n사용법: wishket-mcp dashboard [--port N] [--no-open]"));
                    }
                }
            }
        }
        Ok(cli)
    }
}

fn load_or_create_token(dir: &std::path::Path) -> std::io::Result<String> {
    let path = dir.join(TOKEN_FILE);
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let t = existing.trim().to_string();
        if !t.is_empty() {
            return Ok(t);
        }
    }
    let bytes: [u8; 24] = rand::random();
    let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let mut f = std::fs::File::create(&path)?;
    writeln!(f, "{token}")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(token)
}

/// 쿠키/쿼리/헤더에서 토큰 추출. 토큰은 hex 고정 길이라 디코딩 불필요.
///
/// 우선순위: Authorization > 쿼리 > 쿠키.
/// 쿼리가 쿠키보다 앞서야 한다 — 쿠키는 포트를 구분하지 않으므로 다른 포트의
/// 대시보드(다른 state dir)를 열면 이전 쿠키가 새 토큰을 덮어써 401이 난다.
fn extract_token(req: &Request) -> (Option<String>, bool) {
    // (token, from_query)
    if let Some(auth) = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(t) = auth.strip_prefix("Bearer ") {
            return (Some(t.trim().to_string()), false);
        }
    }
    if let Some(q) = req.uri().query() {
        for pair in q.split('&') {
            if let Some(t) = pair.strip_prefix("token=") {
                if !t.is_empty() {
                    return (Some(t.to_string()), true);
                }
            }
        }
    }
    if let Some(cookie) = req
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
    {
        for pair in cookie.split(';') {
            let pair = pair.trim();
            if let Some(t) = pair.strip_prefix("wk_token=") {
                return (Some(t.to_string()), false);
            }
        }
    }
    (None, false)
}

/// 모든 요청(정적 포함) 게이트. ponytail: == 비교 — 랜덤 토큰 LAN 위협 모델에서
/// 타이밍 안전 비교는 과잉. 외부 노출이면 subtle로 교체.
async fn auth(State(app): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let (token, from_query) = extract_token(&req);
    match token {
        Some(t) if t == app.token => {
            let mut res = next.run(req).await;
            if from_query {
                if let Ok(v) = HeaderValue::from_str(&format!(
                    "wk_token={}; Path=/; SameSite=Lax; HttpOnly; Max-Age=2592000",
                    app.token
                )) {
                    res.headers_mut().insert(header::SET_COOKIE, v);
                }
            }
            res
        }
        _ => (
            StatusCode::UNAUTHORIZED,
            axum::Json(json!({"error": "token required"})),
        )
            .into_response(),
    }
}

#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../webui/dist"]
struct Assets;

pub(crate) fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "json" | "map" => "application/json",
        "woff2" => "font/woff2",
        "txt" => "text/plain; charset=utf-8",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

async fn static_handler(req: Request) -> Response {
    let path = req.uri().path().trim_start_matches('/').to_string();
    let path = if path.is_empty() {
        "index.html".into()
    } else {
        path
    };
    match Assets::get(&path) {
        Some(asset) => {
            // index.html은 절대 캐시하지 않는다 — 여기에 해시된 에셋 경로가 박혀
            // 있어서, 캐시되면 바이너리를 업데이트해도 옛 UI가 계속 뜬다.
            // 해시 파일명인 /assets/* 는 반대로 영구 캐시해도 안전하다.
            let cache = if path == "index.html" {
                "no-cache, must-revalidate"
            } else if path.starts_with("assets/") {
                "public, max-age=31536000, immutable"
            } else {
                "no-cache"
            };
            let mut res = (StatusCode::OK, asset.data.into_owned()).into_response();
            if let Ok(v) = HeaderValue::from_str(content_type(&path)) {
                res.headers_mut().insert(header::CONTENT_TYPE, v);
            }
            if let Ok(v) = HeaderValue::from_str(cache) {
                res.headers_mut().insert(header::CACHE_CONTROL, v);
            }
            res
        }
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

/// LAN IP 최선 추정 (UDP connect 트릭 — 패킷 안 나감). 실패 시 None.
fn lan_ip() -> Option<Ipv4Addr> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("8.8.8.8:80").ok()?;
    match s.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(v4) => Some(v4),
        _ => None,
    }
}

pub fn build_router(app: Arc<AppState>) -> Router {
    Router::new()
        .nest("/api", api::router())
        .fallback(get(static_handler))
        .layer(middleware::from_fn_with_state(app.clone(), auth))
        .with_state(app)
}

pub async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let dir = state::state_dir();
    std::fs::create_dir_all(&dir)?;
    let token = load_or_create_token(&dir)?;
    // 구 matches.md가 남아 있으면 관심 표시로 1회 이관 (0.2.0 마이그레이션)
    let migrated = matches::migrate_legacy_file(&dir, &state::now_iso()[..10]);
    if migrated > 0 {
        println!("matches.md {migrated}건을 인박스 '관심'으로 이관했습니다.");
    }
    let addr = SocketAddr::from(([0, 0, 0, 0], cli.port));
    let listener = tokio::net::TcpListener::bind(addr).await?; // 바인드 먼저 — 실패면 브라우저 안 엶
    let app = Arc::new(AppState {
        token: token.clone(),
        state_dir: dir.clone(),
        http: reqwest::Client::builder()
            .user_agent(crate::wishket::UA)
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("reqwest client"),
    });

    println!("wishket-mcp dashboard {}", env!("CARGO_PKG_VERSION"));
    println!("URL:   http://127.0.0.1:{}?token={token}", cli.port);
    if let Some(ip) = lan_ip() {
        println!("LAN:   http://{ip}:{}?token={token}", cli.port);
    }
    println!("token: {}", dir.join(TOKEN_FILE).display());

    if !cli.no_open {
        // 최선 노력: 실패해도 서버는 계속
        let _ = webbrowser::open(&format!("http://127.0.0.1:{}?token={token}", cli.port));
    }
    axum::serve(listener, build_router(app)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use tower::ServiceExt;

    fn test_app(tag: &str) -> (Arc<AppState>, PathBuf) {
        let dir = std::env::temp_dir().join(format!("wk-auth-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        (
            Arc::new(AppState {
                token: "test-token".into(),
                state_dir: dir.clone(),
                http: reqwest::Client::new(),
            }),
            dir,
        )
    }

    #[tokio::test]
    async fn no_token_is_401() {
        let (app, _dir) = test_app("no-token");
        let res = build_router(app)
            .oneshot(Request::get("/api/state").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn wrong_token_is_401() {
        let (app, _dir) = test_app("wrong");
        let res = build_router(app)
            .oneshot(
                Request::get("/api/state")
                    .header(header::AUTHORIZATION, "Bearer nope")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn bearer_ok_on_api() {
        let (app, _dir) = test_app("bearer");
        let res = build_router(app)
            .oneshot(
                Request::get("/api/state")
                    .header(header::AUTHORIZATION, "Bearer test-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn query_token_on_root_sets_cookie_and_static_gated() {
        let (app, _dir) = test_app("cookie");
        let router = build_router(app);
        // 정적도 게이트
        let res = router
            .clone()
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
        // 쿼리 토큰 → 200 + Set-Cookie (플레이스홀더 index.html이 임베드됨)
        let res = router
            .clone()
            .oneshot(
                Request::get("/?token=test-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let cookie = res
            .headers()
            .get(header::SET_COOKIE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        assert!(cookie.contains("wk_token=test-token"), "{cookie}");
        drop(res);
        // 쿠키로 /api/state
        let res = router
            .oneshot(
                Request::get("/api/state")
                    .header(header::COOKIE, "wk_token=test-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn query_token_beats_stale_cookie() {
        // 쿠키는 포트를 구분하지 않는다 — 다른 대시보드의 낡은 쿠키가 붙어도
        // URL의 토큰이 이겨야 한다.
        let (app, _dir) = test_app("stale-cookie");
        let res = build_router(app)
            .oneshot(
                Request::get("/?token=test-token")
                    .header(header::COOKIE, "wk_token=someoldtoken")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let cookie = res
            .headers()
            .get(header::SET_COOKIE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        assert!(
            cookie.contains("wk_token=test-token"),
            "쿠키를 갱신해야 함: {cookie}"
        );
    }

    #[tokio::test]
    async fn index_is_not_cached_but_assets_are() {
        // 바이너리를 갱신했는데 브라우저가 옛 index.html을 들고 있으면
        // 사라진 에셋을 가리켜 옛 UI가 뜬다.
        let (app, _dir) = test_app("cache-headers");
        let res = build_router(app)
            .oneshot(
                Request::get("/?token=test-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let cc = res
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        assert!(cc.contains("no-cache"), "index.html must revalidate: {cc}");
    }

    #[test]
    fn cli_parse() {
        let c = Cli::parse_from([] as [String; 0]).unwrap();
        assert_eq!(
            c,
            Cli {
                port: 8787,
                no_open: false
            }
        );
        let c = Cli::parse_from(["--port".into(), "9000".into()]).unwrap();
        assert_eq!(c.port, 9000);
        let c = Cli::parse_from(["--port=9001".into(), "--no-open".into()]).unwrap();
        assert_eq!(c.port, 9001);
        assert!(c.no_open);
        assert!(Cli::parse_from(["--bogus".into()]).is_err());
        assert!(Cli::parse_from(["--port".into()]).is_err());
        assert!(Cli::parse_from(["--port".into(), "x".into()]).is_err());
    }
}
