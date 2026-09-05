//! SQLite 백엔드 (v0.4) — `state.db` 단일 파일.
//!
//! 파일이 정규 소스라는 철학을 SQLite 단일 소스로 옮긴다. 행당 JSON 컬럼으로
//! SeenEntry/Application을 그대로 보관해 serde 하위호환(구버전 필드 누락 허용)을
//! 유지하고, 소비자는 전부 load-all 뒤 Rust에서 필터하므로 컬럼 정규화는 불필요하다.
//! 원본 파일은 `*.migrated`로 보존 — 삭제 전까지만 롤백 가능 (matches.md 선례 준용).

use std::io;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::state::{SeenEntry, State};

const SCHEMA: &str = "\
CREATE TABLE IF NOT EXISTS seen (
    id TEXT PRIMARY KEY,
    data TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS applications (
    idx INTEGER PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    data TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
-- v0.5 대화. usage 누적 컬럼(tokens_in/out)은 migrate_schema()가 붙인다 —
-- v1 db와 동일한 base 스키마를 유지해 이관 경로를 하나로.
CREATE TABLE IF NOT EXISTS conversations (
    id INTEGER PRIMARY KEY,
    project_id TEXT,
    title TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY,
    conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL
);
";
// user_version은 ensure()의 생성 분기에서만 세팅한다 — open마다 쓰면 3초 폴마다
// 불필요한 쓰기 잠금이 걸린다.

pub fn db_path(dir: &Path) -> PathBuf {
    dir.join("state.db")
}

pub fn present(dir: &Path) -> bool {
    db_path(dir).exists()
}

fn io_err(e: rusqlite::Error) -> io::Error {
    io::Error::other(e)
}

/// 연결마다 WAL·busy_timeout·NORMAL·foreign_keys를 깐다. 대시보드 3초 폴링과
/// 쓰기가 겹쳐도 읽기가 막히지 않는다. journal_mode는 파일 속성이라 이후 호출은
/// no-op. foreign_keys는 연결별 설정 — 스트리밍 중 대화가 삭제돼도 고아 메시지
/// 행이 재삽입되지 않는다(FK 위반은 append_message의 오류 무시 경로로 흡수).
/// WAL/-shm은 clean close 후 재생성될 때 umask 권한으로 돌아오므로 매 연결
/// 0600을 재적용한다 — 키가 settings에 들어간 뒤로는 권한 이완이 곧 노출.
fn open(dir: &Path) -> Result<Connection, io::Error> {
    let conn = Connection::open(db_path(dir)).map_err(io_err)?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000; PRAGMA synchronous=NORMAL;
         PRAGMA foreign_keys=ON;",
    )
    .map_err(io_err)?;
    conn.execute_batch(SCHEMA).map_err(io_err)?;
    restrict_perms(dir);
    Ok(conn)
}

/// ensure + open. 첫 접근이면 스키마 생성·legacy 흡수·스냅샷 점검을 마친 연결.
fn open_ready(dir: &Path) -> Result<Connection, io::Error> {
    ensure(dir)?;
    open(dir)
}

/// 첫 기동 1회: 디렉터리·스키마 생성, legacy 파일 흡수, 주간 스냅샷 점검.
/// DB 생성은 이 함수에서만 한다 — open()이 임의로 만들면 빈 db가 present()로
/// 보여 마이그레이션을 영구 우회한다.
pub fn ensure(dir: &Path) -> Result<(), io::Error> {
    std::fs::create_dir_all(dir)?;
    if present(dir) {
        migrate_schema(dir);
        weekly_snapshot(dir);
        return Ok(());
    }
    let conn = open(dir)?;
    migrate_schema(dir);
    migrate_legacy(&conn, dir)?;
    restrict_perms(dir);
    weekly_snapshot(dir);
    Ok(())
}

/// 스키마 버전 이관. 생성 분기와 기존 db 분기가 같은 경로를 탄다 —
/// fresh db는 user_version 0 → ALTER(멱등) → 2, v1 db는 1 → 2.
fn migrate_schema(dir: &Path) {
    let Ok(conn) = open(dir) else { return };
    let ver: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);
    if ver >= 2 {
        return;
    }
    // v0.5: 대화별 토큰 usage 누적. 동시 기동 경합으로 컬럼이 이미
    // 있으면 duplicate column 에러 — 무시해도 목표 상태에 도달한다.
    for col in ["tokens_in", "tokens_out"] {
        let _ = conn.execute(
            &format!("ALTER TABLE conversations ADD COLUMN {col} INTEGER NOT NULL DEFAULT 0"),
            [],
        );
    }
    // 실제로 두 컬럼이 존재할 때만 버전을 올린다. ALTER 실패(디스크·
    // busy_timeout 초과)를 삼키고 버전만 찍으면 재시도가 영구히 생략되고
    // conversations 쿼리가 전부 "no such column"으로 깨진다.
    let cols: Vec<String> = conn
        .prepare("PRAGMA table_info(conversations)")
        .and_then(|mut s| {
            s.query_map([], |r| r.get::<_, String>(1))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    if !cols.iter().any(|c| c == "tokens_in") || !cols.iter().any(|c| c == "tokens_out") {
        return;
    }
    // open마다 쓰지 않고 이관 시에만 — 불필요한 쓰기 잠금 방지 (v0.4 주석 준용).
    let _ = conn.execute_batch("PRAGMA user_version = 2;");
}

/// db 파일을 토큰 파일 선례(dashboard/mod.rs)에 맞춰 0600으로. 실패는 무시.
#[cfg(unix)]
fn restrict_perms(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    for name in ["state.db", "state.db-wal", "state.db-shm"] {
        let p = dir.join(name);
        if p.exists() {
            let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
        }
    }
}
#[cfg(not(unix))]
fn restrict_perms(_dir: &Path) {}

/// state.json / applications.yaml / profile.yaml을 흡수하고 원본을
/// `*.migrated`로 rename한다. 깨진 파일은 흡수하지 않고 원본 그대로 둔다
/// (파서의 parse_error 철학과 동일 — 내용을 지우지 않는다).
fn migrate_legacy(conn: &Connection, dir: &Path) -> Result<(), io::Error> {
    // state.json → seen + last_scan
    let json_path = dir.join("state.json");
    if let Ok(raw) = std::fs::read_to_string(&json_path) {
        // 파싱 불가면 원본 보존, 다음 기동에 다시 시도
        if let Ok(st) = serde_json::from_str::<State>(&raw) {
            for (id, e) in &st.seen {
                insert_seen(conn, id, e)?;
            }
            if let Some(ls) = st.last_scan {
                set_setting(conn, "last_scan", &ls)?;
            }
            let _ = std::fs::rename(&json_path, dir.join("state.json.migrated"));
        }
    }
    // applications.yaml → applications (Value로 보관해 스키마 변경에 무관).
    // id가 숫자로 온 yaml도 문자열로 정규화하고, 그래도 Application으로
    // 역직렬화가 안 되는 행이 있으면(타입 불량) 파일 채로 남겨 dashboard의
    // parse_error 경로가 맡는다 — 조용한 행 유실을 막는다.
    let yaml_path = dir.join("applications.yaml");
    if let Ok(raw) = std::fs::read_to_string(&yaml_path) {
        if let Ok(shim) = serde_yaml::from_str::<AppsShim>(&raw) {
            // 모든 행이 객체여야 id 정규화가 가능하다(IndexMut은 비-객체에
            // panic). 하나라도 객체가 아니면 파일 채로 보존 — 부분 흡수로
            // 행을 조용히 버리지 않는다.
            if shim.applications.iter().all(|a| a.is_object()) {
                let normalized: Vec<Value> = shim
                    .applications
                    .iter()
                    .map(|a| {
                        let mut a = a.clone();
                        a["id"] = Value::String(coerce_id(&a["id"]));
                        a
                    })
                    .collect();
                if !normalized.is_empty()
                    && normalized.iter().all(|a| {
                        serde_json::from_value::<crate::dashboard::apps::Application>(a.clone())
                            .is_ok()
                    })
                {
                    for (i, a) in normalized.iter().enumerate() {
                        upsert_application(conn, i, a)?;
                    }
                    let _ = std::fs::rename(&yaml_path, dir.join("applications.yaml.migrated"));
                }
            }
        }
    }
    // profile.yaml → settings["profile_yaml"] (원문 그대로 — 파싱 실패해도 편집기에서 고친다)
    let profile_path = dir.join("profile.yaml");
    if let Ok(raw) = std::fs::read_to_string(&profile_path) {
        if !raw.trim().is_empty() {
            set_setting(conn, "profile_yaml", &raw)?;
            let _ = std::fs::rename(&profile_path, dir.join("profile.yaml.migrated"));
        }
    }
    Ok(())
}

#[derive(Deserialize)]
struct AppsShim {
    #[serde(default)]
    applications: Vec<Value>,
}

/// yaml의 id는 따옴표 없는 숫자로도 온다(유효한 yaml) — 문자열로 강제한다.
/// 비워두면 UNIQUE 충돌로 행이 조용히 사라진다.
fn coerce_id(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

fn upsert_application(conn: &Connection, idx: usize, a: &Value) -> Result<(), io::Error> {
    conn.execute(
        "INSERT OR REPLACE INTO applications (idx, id, data) VALUES (?1, ?2, ?3)",
        rusqlite::params![
            idx as i64,
            coerce_id(&a["id"]),
            serde_json::to_string(a).unwrap()
        ],
    )
    .map_err(io_err)?;
    Ok(())
}

fn insert_seen(conn: &Connection, id: &str, e: &SeenEntry) -> Result<(), io::Error> {
    let data = serde_json::to_string(e).map_err(io::Error::other)?;
    conn.execute(
        "INSERT OR REPLACE INTO seen (id, data) VALUES (?1, ?2)",
        rusqlite::params![id, data],
    )
    .map_err(io_err)?;
    Ok(())
}

fn get_setting(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        rusqlite::params![key],
        |r| r.get(0),
    )
    .ok()
}

fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<(), io::Error> {
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![key, value],
    )
    .map_err(io_err)?;
    Ok(())
}

pub fn load_state(dir: &Path) -> Result<State, io::Error> {
    let conn = open_ready(dir)?;
    let mut st = State::default();
    let mut q = conn.prepare("SELECT id, data FROM seen").map_err(io_err)?;
    let rows = q
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(io_err)?;
    for row in rows {
        let (id, data) = row.map_err(io_err)?;
        if let Ok(e) = serde_json::from_str::<SeenEntry>(&data) {
            st.seen.insert(id, e);
        }
    }
    st.last_scan = get_setting(&conn, "last_scan");
    Ok(st)
}

pub fn save_state(dir: &Path, state: &State) -> Result<(), io::Error> {
    let mut conn = open_ready(dir)?;
    let tx = conn.transaction().map_err(io_err)?;
    // ponytail: save는 전체 맵 치환(last-writer-wins) — 파일 시절과 동일
    // 의미론. MCP 스캔 프로세스와 대시보드가 동시에 쓰면 나중 쓰기가 이기고,
    // 사이에 추가된 타 프로세스의 새 항목은 유실된다. WAL이 막아주는 건 잠금
    // 오류까지. 완전한 증분 저장은 호출부가 '이번에 건드린 id'를 알아야 하므로
    // State에 dirty set이 필요 — 유실이 실제로 관측되면 그때 추가.
    tx.execute("DELETE FROM seen", []).map_err(io_err)?;
    for (id, e) in &state.seen {
        insert_seen(&tx, id, e)?;
    }
    if let Some(ls) = &state.last_scan {
        set_setting(&tx, "last_scan", ls)?;
    }
    tx.commit().map_err(io_err)
}

pub fn load_applications(dir: &Path) -> Result<Vec<Value>, io::Error> {
    let conn = open_ready(dir)?;
    let mut q = conn
        .prepare("SELECT data FROM applications ORDER BY idx")
        .map_err(io_err)?;
    let rows = q.query_map([], |r| r.get::<_, String>(0)).map_err(io_err)?;
    let mut out = Vec::new();
    for row in rows {
        let data = row.map_err(io_err)?;
        if let Ok(v) = serde_json::from_str::<Value>(&data) {
            out.push(v);
        }
    }
    Ok(out)
}

pub fn save_applications(dir: &Path, apps: &[Value]) -> Result<(), io::Error> {
    let mut conn = open_ready(dir)?;
    let tx = conn.transaction().map_err(io_err)?;
    // save_state와 동일한 전체 치환 의미론 (ponytail 주석 참고).
    tx.execute("DELETE FROM applications", []).map_err(io_err)?;
    for (i, a) in apps.iter().enumerate() {
        upsert_application(&tx, i, a)?;
    }
    tx.commit().map_err(io_err)
}

/// DB의 profile.yaml 원문. 없어도 state_dir에 파일이 남아 있으면(미이관·깨짐) 그걸로 폴백.
/// open_ready — open()으로 db를 임의 생성하면 ensure의 마이그레이션을 영구 우회한다.
pub fn load_profile_yaml(dir: &Path) -> Option<String> {
    let conn = open_ready(dir).ok()?;
    let stored = get_setting(&conn, "profile_yaml");
    if stored.is_some() {
        return stored;
    }
    std::fs::read_to_string(dir.join("profile.yaml")).ok()
}

pub fn save_profile_yaml(dir: &Path, yaml: &str) -> Result<(), io::Error> {
    let conn = open_ready(dir)?;
    set_setting(&conn, "profile_yaml", yaml)
}

/// 프로필의 정규 소스가 DB인지. API의 profile_external 힌트 계산에 쓴다.
pub fn profile_db_backed(dir: &Path) -> bool {
    open_ready(dir)
        .ok()
        .and_then(|c| get_setting(&c, "profile_yaml"))
        .is_some()
}

// --- 범용 settings + 대화 DAO (v0.5 BYOK·AI) -------------------------------

pub fn load_setting(dir: &Path, key: &str) -> Option<String> {
    let conn = open_ready(dir).ok()?;
    get_setting(&conn, key)
}

pub fn save_setting(dir: &Path, key: &str, value: &str) -> Result<(), io::Error> {
    let conn = open_ready(dir)?;
    set_setting(&conn, key, value)
}

pub fn create_conversation(
    dir: &Path,
    project_id: Option<&str>,
    title: &str,
) -> Result<i64, io::Error> {
    let conn = open_ready(dir)?;
    conn.execute(
        "INSERT INTO conversations (project_id, title, created_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![project_id, title, crate::state::now_iso()],
    )
    .map_err(io_err)?;
    Ok(conn.last_insert_rowid())
}

/// 대화 목록 (최신 먼저) — usage 누적·메시지 수·연결 공고 제목 포함.
/// project_title은 seen 캐시(data JSON)에서 추출 — 캐시에서 사라진 공고는 NULL.
pub fn list_conversations(dir: &Path) -> Result<Vec<Value>, io::Error> {
    let conn = open_ready(dir)?;
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.project_id, c.title, c.created_at, c.tokens_in, c.tokens_out,
                    (SELECT COUNT(*) FROM messages m WHERE m.conversation_id = c.id),
                    NULLIF(json_extract(s.data, '$.title'), '')
             FROM conversations c
             LEFT JOIN seen s ON s.id = c.project_id
             ORDER BY c.id DESC",
        )
        .map_err(io_err)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "project_id": r.get::<_, Option<String>>(1)?,
                "title": r.get::<_, String>(2)?,
                "created_at": r.get::<_, String>(3)?,
                "tokens_in": r.get::<_, i64>(4)?,
                "tokens_out": r.get::<_, i64>(5)?,
                "messages": r.get::<_, i64>(6)?,
                "project_title": r.get::<_, Option<String>>(7)?,
            }))
        })
        .map_err(io_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(io_err)?;
    Ok(rows)
}

/// 대화 단위 삭제 — 메시지는 foreign_keys=ON의 CASCADE로 함께 지운다.
/// 삭제된 대화가 있으면 true, 없는 id면 false.
pub fn delete_conversation(dir: &Path, id: i64) -> Result<bool, io::Error> {
    let conn = open_ready(dir)?;
    let n = conn
        .execute("DELETE FROM conversations WHERE id = ?1", [id])
        .map_err(io_err)?;
    Ok(n > 0)
}

/// 대화 헤더 + 메시지 배열. 없는 id면 None.
pub fn get_conversation(dir: &Path, id: i64) -> Result<Option<Value>, io::Error> {
    let conn = open_ready(dir)?;
    let mut header = match conn.query_row(
        "SELECT id, project_id, title, created_at, tokens_in, tokens_out
         FROM conversations WHERE id = ?1",
        [id],
        |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "project_id": r.get::<_, Option<String>>(1)?,
                "title": r.get::<_, String>(2)?,
                "created_at": r.get::<_, String>(3)?,
                "tokens_in": r.get::<_, i64>(4)?,
                "tokens_out": r.get::<_, i64>(5)?,
            }))
        },
    ) {
        Ok(v) => v,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(e) => return Err(io_err(e)),
    };
    let mut stmt = conn
        .prepare(
            "SELECT role, content, created_at FROM messages
             WHERE conversation_id = ?1 ORDER BY id",
        )
        .map_err(io_err)?;
    // assistant 응답은 마크다운이므로 sanitize된 HTML을 함께 실는다 —
    // 클라에서 파서 없이 렌더(채팅 화면 최적화). user 메시지는 plain.
    let msgs = stmt
        .query_map([id], |r| {
            let role: String = r.get(0)?;
            let content: String = r.get(1)?;
            let html = if role == "assistant" && !content.is_empty() {
                crate::dashboard::api::render_markdown(&content)
            } else {
                String::new()
            };
            Ok(json!({
                "role": role,
                "content": content,
                "content_html": html,
                "created_at": r.get::<_, String>(2)?,
            }))
        })
        .map_err(io_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(io_err)?;
    header["messages"] = Value::Array(msgs);
    Ok(Some(header))
}

pub fn append_message(
    dir: &Path,
    conversation_id: i64,
    role: &str,
    content: &str,
) -> Result<(), io::Error> {
    let conn = open_ready(dir)?;
    conn.execute(
        "INSERT INTO messages (conversation_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![conversation_id, role, content, crate::state::now_iso()],
    )
    .map_err(io_err)?;
    Ok(())
}

/// 공급자 응답 usage 필드의 대화별 누적.
pub fn add_usage(
    dir: &Path,
    conversation_id: i64,
    tokens_in: u64,
    tokens_out: u64,
) -> Result<(), io::Error> {
    let conn = open_ready(dir)?;
    conn.execute(
        "UPDATE conversations SET tokens_in = tokens_in + ?1, tokens_out = tokens_out + ?2
         WHERE id = ?3",
        rusqlite::params![tokens_in as i64, tokens_out as i64, conversation_id],
    )
    .map_err(io_err)?;
    Ok(())
}

/// 주간 스냅샷: 최신 backups/state-*.db가 7일 경과 시 VACUUM INTO, 4세대 유지.
/// 실패는 무시한다 — 스냅샷 실패가 앱을 막으면 본말전도.
fn weekly_snapshot(dir: &Path) {
    let _ = std::fs::create_dir_all(dir.join("backups"));
    let today = crate::state::now_iso()
        .get(..10)
        .unwrap_or("")
        .replace('-', "");
    let target = dir.join("backups").join(format!("state-{today}.db"));
    let stale = std::fs::read_dir(dir.join("backups"))
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| {
                    let n = e.file_name();
                    let n = n.to_string_lossy();
                    n.starts_with("state-") && n.ends_with(".db")
                })
                .filter_map(|e| e.metadata().ok().and_then(|m| m.modified().ok()))
                .max()
        })
        .ok()
        .flatten();
    let week = std::time::Duration::from_secs(7 * 86400);
    let needs = match stale {
        None => true,
        Some(m) => m.elapsed().map(|e| e > week).unwrap_or(true),
    };
    if needs && !target.exists() {
        if let Ok(conn) = open(dir) {
            // 임시 이름에 VACUUM INTO 후 rename — 실패해 부분 파일이
            // "오늘 스냅샷"으로 기록되는 걸 막는다.
            let tmp = target.with_extension("db.tmp");
            if conn
                .execute(
                    "VACUUM INTO ?1",
                    rusqlite::params![tmp.display().to_string()],
                )
                .is_ok()
            {
                // 스냅샷은 settings(ai_config 키 포함)를 통째로 복사한다 —
                // 0600을 rename 전에: 잠깐이라도 0644 최종 경로를 남기지 않는다.
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
                }
                let _ = std::fs::rename(&tmp, &target);
            } else {
                let _ = std::fs::remove_file(&tmp);
            }
        }
    }
    // 4세대 초과분 삭제 (이름에 날짜가 있어 정렬 = 시간순). 오늘 스냅샷이
    // 이미 있어도 돌린다 — 새 스냅샷과 무관하게 오래된 세대는 정리 대상.
    if let Ok(gens) = std::fs::read_dir(dir.join("backups")) {
        let mut names: Vec<_> = gens
            .filter_map(|e| e.ok())
            .map(|e| e.file_name())
            .filter(|n| {
                let n = n.to_string_lossy();
                n.starts_with("state-") && n.ends_with(".db")
            })
            .collect();
        names.sort();
        names.reverse();
        for old in names.into_iter().skip(4) {
            let _ = std::fs::remove_file(dir.join("backups").join(old));
        }
    }
}

/// reset_cache — seen 캐시만 지운다(구 state.json 삭제와 동일 의미론).
/// applications·profile_yaml는 파이프라인·프로필 데이터라 건드리지 않는다.
/// 성공 여부를 돌려준다 — 실패를 "지워짐"으로 보고하지 않는다.
pub fn reset(dir: &Path) -> Result<(), io::Error> {
    let conn = open_ready(dir)?;
    conn.execute("DELETE FROM seen", []).map_err(io_err)?;
    conn.execute("DELETE FROM settings WHERE key = 'last_scan'", [])
        .map_err(io_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Triage;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("wk-sql-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn entry(triage: Option<Triage>) -> SeenEntry {
        SeenEntry {
            first_seen: "2026-09-01T10:00:00+09:00".into(),
            title: "테스트 공고".into(),
            triage,
            triaged_at: triage.map(|_| "2026-09-02".into()),
            url: Some("https://www.wishket.com/project/1/".into()),
            score: Some(42),
            budget: Some("월 500만".into()),
            duration: Some("예상 기간 90일".into()),
            budget_monthly_won: Some(5_000_000),
            budget_total_won: Some((1_000, 2_000)),
            duration_days: Some(90),
            daily_won: Some((10, 20)),
            deadline: Some("2026-09-08".into()),
            skills: vec!["rust".into()],
            private_matching: Some(false),
            description: Some("<p>본문</p>".into()),
            conditions: vec![("모집 마감일".into(), "2026-09-08".into())],
            role: Some("백엔드".into()),
            level: Some("시니어".into()),
            location: Some("서울".into()),
            matched: vec!["rust".into()],
            detail_fetched_at: Some("2026-09-02T09:00:00+09:00".into()),
        }
    }

    #[test]
    fn ensure_creates_schema_and_wal() {
        let dir = tmpdir("schema");
        ensure(&dir).unwrap();
        let conn = open(&dir).unwrap();
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
        let ver: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(ver, 2, "fresh db도 migrate_schema 경로로 v2까지 간다");
        // conversations/messages 선반영 확인
        conn.execute(
            "INSERT INTO conversations (project_id, title, created_at) VALUES ('1', 't', 'now')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO messages (conversation_id, role, content, created_at) VALUES (1, 'user', 'hi', 'now')",
            [],
        )
        .unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn migrates_legacy_files_and_renames() {
        let dir = tmpdir("migrate");
        std::fs::write(
            dir.join("state.json"),
            r#"{"seen":{"7":{"first_seen":"2026-09-01T10:00:00+09:00","title":"구버전","triage":"interested"}},"last_scan":"2026-09-03T09:00:00+09:00"}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("applications.yaml"),
            "applications:\n  - id: \"7\"\n    status: 미팅\n  - id: \"9\"\n    status: 관심\n",
        )
        .unwrap();
        std::fs::write(dir.join("profile.yaml"), "name: 테스터\nskills: []\n").unwrap();

        let st = load_state(&dir).unwrap();
        assert_eq!(st.seen["7"].title, "구버전");
        assert_eq!(st.seen["7"].triage, Some(Triage::Interested));
        assert_eq!(st.last_scan.as_deref(), Some("2026-09-03T09:00:00+09:00"));
        let apps = load_applications(&dir).unwrap();
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0]["status"], "미팅");
        assert_eq!(
            load_profile_yaml(&dir).as_deref(),
            Some("name: 테스터\nskills: []\n")
        );
        for f in ["state.json", "applications.yaml", "profile.yaml"] {
            assert!(!dir.join(f).exists(), "{f} 이관됨");
            assert!(
                dir.join(format!("{f}.migrated")).exists(),
                "{f}.migrated 보존"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn broken_legacy_file_is_not_absorbed_nor_deleted() {
        let dir = tmpdir("broken");
        std::fs::write(dir.join("state.json"), "{oops").unwrap();
        let st = load_state(&dir).unwrap();
        assert!(st.seen.is_empty(), "깨진 파일은 흡수 안 함");
        assert!(dir.join("state.json").exists(), "원본 보존");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn state_roundtrip_preserves_fields_and_order() {
        let dir = tmpdir("roundtrip");
        let mut st = State::default();
        st.seen.insert("1".into(), entry(Some(Triage::Interested)));
        st.seen.insert("2".into(), entry(None));
        st.last_scan = Some("2026-09-04T00:00:00+09:00".into());
        save_state(&dir, &st).unwrap();
        let back = load_state(&dir).unwrap();
        assert_eq!(back.seen.len(), 2);
        assert_eq!(back.seen["1"], st.seen["1"], "전 필드 보존");
        assert_eq!(back.seen["1"].conditions, st.seen["1"].conditions);
        assert_eq!(back.seen["1"].daily_won, Some((10, 20)));
        assert_eq!(back.last_scan, st.last_scan);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn applications_roundtrip_keeps_vec_order() {
        let dir = tmpdir("apps");
        let apps: Vec<Value> = vec![
            serde_json::json!({"id": "5", "status": "미팅"}),
            serde_json::json!({"id": "3", "status": "관심"}),
        ];
        save_applications(&dir, &apps).unwrap();
        let back = load_applications(&dir).unwrap();
        assert_eq!(back, apps, "순서 보존");
        // 전체 치환 — 지워진 항목은 사라진다 (파일 세이브와 동일 의미론)
        save_applications(&dir, &[apps[1].clone()]).unwrap();
        assert_eq!(load_applications(&dir).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_numeric_application_id_is_coerced() {
        // yaml에서 id를 따옴표 없이 쓴 파일 — 문자열로 강제하지 않으면
        // UNIQUE 충돌로 행이 조용히 사라진다.
        let dir = tmpdir("numid");
        std::fs::write(
            dir.join("applications.yaml"),
            "applications:\n  - id: 12345\n    status: 미팅\n",
        )
        .unwrap();
        let apps = load_applications(&dir).unwrap();
        assert_eq!(apps.len(), 1, "숫자 id도 이관된다");
        assert_eq!(apps[0]["id"], "12345");
        assert!(dir.join("applications.yaml.migrated").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bad_typed_application_row_blocks_migration() {
        // quote_manwon이 문자열인 등 Application으로 역직렬화 불가한 행이면
        // 파일 채로 남긴다(조용한 유실 금지) — parse_error는 yaml 경로가 보고.
        let dir = tmpdir("badrow");
        std::fs::write(
            dir.join("applications.yaml"),
            "applications:\n  - id: \"1\"\n    quote_manwon: 많이\n",
        )
        .unwrap();
        let apps = load_applications(&dir).unwrap();
        assert!(apps.is_empty(), "흡수 안 함");
        assert!(dir.join("applications.yaml").exists(), "원본 보존");
        assert!(!dir.join("applications.yaml.migrated").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reset_clears_seen_only_keeps_pipeline_and_profile() {
        // 구 state.json 삭제와 동일 의미론 — 파이프라인·프로필은 유지된다.
        let dir = tmpdir("reset2");
        let mut st = State::default();
        st.seen.insert("1".into(), entry(None));
        st.last_scan = Some("2026-09-04T00:00:00+09:00".into());
        save_state(&dir, &st).unwrap();
        save_applications(&dir, &[serde_json::json!({"id": "1", "status": "미팅"})]).unwrap();
        save_profile_yaml(&dir, "name: 유지\nskills: []\n").unwrap();

        reset(&dir).unwrap();
        let back = load_state(&dir).unwrap();
        assert!(back.seen.is_empty(), "seen은 지워진다");
        assert!(back.last_scan.is_none(), "last_scan도 지워진다");
        assert_eq!(load_applications(&dir).unwrap().len(), 1, "파이프라인 유지");
        assert_eq!(
            load_profile_yaml(&dir).as_deref(),
            Some("name: 유지\nskills: []\n"),
            "프로필 유지"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn weekly_snapshot_creates_and_caps_generations() {
        let dir = tmpdir("snap");
        ensure(&dir).unwrap();
        let b = dir.join("backups");
        let first = std::fs::read_dir(&b).unwrap().count();
        assert!(first >= 1, "첫 ensure에 스냅샷 생성");
        // 이전 날짜 스냅샷 5개를 심고 다시 스냅샷 → 4세대로 정리
        for d in 1..=5 {
            std::fs::write(b.join(format!("state-2026080{d}.db")), "x").unwrap();
        }
        weekly_snapshot(&dir);
        let mut left: Vec<_> = std::fs::read_dir(&b)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        left.sort();
        assert_eq!(left.len(), 4, "4세대 유지: {left:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stale_snapshots_trigger_fresh_one() {
        // 최신 스냅샷이 7일 넘었으면 오늘자 새 스냅샷을 만든다 (verifier 갭 보강).
        let dir = tmpdir("snapstale");
        ensure(&dir).unwrap();
        let b = dir.join("backups");
        let old = b.join("state-20260801.db");
        std::fs::write(&old, "x").unwrap();
        let week_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(8 * 86400);
        old.metadata().unwrap().modified().unwrap();
        // set_modified는 std 1.75+의 안정 API (파일 생성 후 mtime 되돌리기)
        let f = std::fs::File::options().write(true).open(&old).unwrap();
        f.set_modified(week_ago).unwrap();
        drop(f);
        // 오늘자 스냅샷이 없어야 stale 판정이 된다 — 지운다
        let today = crate::state::now_iso()
            .get(..10)
            .unwrap_or("")
            .replace('-', "");
        let _ = std::fs::remove_file(b.join(format!("state-{today}.db")));
        weekly_snapshot(&dir);
        assert!(
            b.join(format!("state-{today}.db")).exists(),
            "7일 경과 시 재스냅샷"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reset_clears_seen_but_keeps_db() {
        let dir = tmpdir("reset");
        save_state(&dir, &State::default()).unwrap();
        assert!(present(&dir));
        reset(&dir).unwrap();
        assert!(present(&dir), "reset은 db 파일을 유지한다 (seen만 지움)");
        assert!(load_state(&dir).unwrap().seen.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn non_object_application_row_blocks_migration_without_panic() {
        // `- 5` 같은 비-객체 행 — IndexMut panic 대신 게이트 거부로 파일 보존.
        let dir = tmpdir("scalar");
        std::fs::write(
            dir.join("applications.yaml"),
            "applications:\n  - 5\n  - id: \"7\"\n    status: 미팅\n",
        )
        .unwrap();
        let apps = load_applications(&dir).unwrap();
        assert!(apps.is_empty(), "비-객체 행 포함 시 미흡수");
        assert!(dir.join("applications.yaml").exists(), "원본 보존");
        assert!(!dir.join("applications.yaml.migrated").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn v1_db_gains_usage_columns_keeping_rows() {
        // v1 모양 db(usage 컬럼 없음)를 만들고 ensure() → v2 이관, 기존 행 보존.
        let dir = tmpdir("v2migrate");
        {
            let conn = Connection::open(dir.join("state.db")).unwrap();
            conn.execute_batch(
                "CREATE TABLE conversations (
                    id INTEGER PRIMARY KEY,
                    project_id TEXT,
                    title TEXT NOT NULL DEFAULT '',
                    created_at TEXT NOT NULL
                );
                INSERT INTO conversations (project_id, title, created_at)
                VALUES ('7', '옛 대화', '2026-09-01');
                PRAGMA user_version = 1;",
            )
            .unwrap();
        }
        ensure(&dir).unwrap();
        let list = list_conversations(&dir).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["title"], "옛 대화");
        assert_eq!(list[0]["tokens_in"], 0, "이관 행의 usage 기본값");
        let ver: i64 = {
            let conn = open(&dir).unwrap();
            conn.query_row("PRAGMA user_version", [], |r| r.get(0))
                .unwrap()
        };
        assert_eq!(ver, 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn setting_roundtrip() {
        let dir = tmpdir("setting");
        assert_eq!(load_setting(&dir, "ai_config"), None);
        save_setting(&dir, "ai_config", "{\"model\":\"m\"}").unwrap();
        assert_eq!(
            load_setting(&dir, "ai_config").as_deref(),
            Some("{\"model\":\"m\"}")
        );
        save_setting(&dir, "ai_config", "{\"model\":\"m2\"}").unwrap();
        assert_eq!(
            load_setting(&dir, "ai_config").as_deref(),
            Some("{\"model\":\"m2\"}")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn conversation_dao_and_usage_accumulation() {
        let dir = tmpdir("conv");
        let id = create_conversation(&dir, Some("158063"), "테스트 대화").unwrap();
        append_message(&dir, id, "user", "이 공고 어때?").unwrap();
        append_message(&dir, id, "assistant", "핵심 스택이 일치합니다.").unwrap();
        add_usage(&dir, id, 100, 50).unwrap();
        add_usage(&dir, id, 30, 20).unwrap();

        let got = get_conversation(&dir, id).unwrap().unwrap();
        assert_eq!(got["project_id"], "158063");
        assert_eq!(got["messages"].as_array().unwrap().len(), 2);
        assert_eq!(got["messages"][0]["role"], "user");
        assert_eq!(got["tokens_in"], 130, "usage는 누적");
        assert_eq!(got["tokens_out"], 70);

        let list = list_conversations(&dir).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["messages"], 2);
        assert!(get_conversation(&dir, 999).unwrap().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_conversation_removes_messages() {
        let dir = tmpdir("conv-del");
        let id = create_conversation(&dir, None, "지울 대화").unwrap();
        append_message(&dir, id, "user", "안녕?").unwrap();
        append_message(&dir, id, "assistant", "네.").unwrap();
        add_usage(&dir, id, 10, 5).unwrap();

        assert!(delete_conversation(&dir, id).unwrap());
        assert!(get_conversation(&dir, id).unwrap().is_none());
        assert!(!delete_conversation(&dir, id).unwrap(), "미존재 id는 false");
        let conn = Connection::open(db_path(&dir)).unwrap();
        let left: i64 = conn
            .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 0, "메시지도 함께 삭제");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn list_conversations_joins_project_title() {
        let dir = tmpdir("conv-title");
        // seen 캐시가 있는 공고 + 없는 공고 (create_conversation이 스키마 생성)
        create_conversation(&dir, Some("158"), "있는 공고 대화").unwrap();
        create_conversation(&dir, Some("99999"), "없는 공고 대화").unwrap();
        create_conversation(&dir, Some("777"), "빈 제목 공고 대화").unwrap();
        {
            let conn = Connection::open(db_path(&dir)).unwrap();
            conn.execute(
                "INSERT INTO seen (id, data) VALUES ('158', '{\"title\":\"AI 챗봇 개발\"}')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO seen (id, data) VALUES ('777', '{\"title\":\"\"}')",
                [],
            )
            .unwrap();
        }

        let list = list_conversations(&dir).unwrap();
        assert_eq!(list.len(), 3);
        let with = list.iter().find(|c| c["project_id"] == "158").unwrap();
        assert_eq!(with["project_title"], "AI 챗봇 개발");
        let gone = list.iter().find(|c| c["project_id"] == "99999").unwrap();
        assert!(
            gone["project_title"].is_null(),
            "캐시에서 사라진 공고는 NULL"
        );
        let empty = list.iter().find(|c| c["project_id"] == "777").unwrap();
        assert!(
            empty["project_title"].is_null(),
            "빈 제목도 NULL로 정규화 (NULLIF)"
        );
        let none = create_conversation(&dir, None, "무연결").unwrap();
        let _ = get_conversation(&dir, none).unwrap();
        let list2 = list_conversations(&dir).unwrap();
        let orphan = list2.iter().find(|c| c["title"] == "무연결").unwrap();
        assert!(orphan["project_title"].is_null());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
