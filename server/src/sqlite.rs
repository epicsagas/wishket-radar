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
use serde_json::Value;

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
-- v0.5 대화용 선반영 (아직 소비자 없음)
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
PRAGMA user_version = 1;
";

pub fn db_path(dir: &Path) -> PathBuf {
    dir.join("state.db")
}

pub fn present(dir: &Path) -> bool {
    db_path(dir).exists()
}

fn io_err(e: rusqlite::Error) -> io::Error {
    io::Error::other(e)
}

/// 연결마다 WAL·busy_timeout·NORMAL을 깐다. 대시보드 3초 폴링과 쓰기가 겹쳐도
/// 읽기가 막히지 않는다. journal_mode는 파일 속성이라 이후 호출은 no-op.
fn open(dir: &Path) -> Result<Connection, io::Error> {
    let conn = Connection::open(db_path(dir)).map_err(io_err)?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000; PRAGMA synchronous=NORMAL;",
    )
    .map_err(io_err)?;
    conn.execute_batch(SCHEMA).map_err(io_err)?;
    Ok(conn)
}

/// ensure + open. 첫 접근이면 스키마 생성·legacy 흡수·스냅샷 점검을 마친 연결.
fn open_ready(dir: &Path) -> Result<Connection, io::Error> {
    ensure(dir)?;
    open(dir)
}

/// 첫 기동 1회: 디렉터리·스키마 생성, legacy 파일 흡수, 주간 스냅샷 점검.
pub fn ensure(dir: &Path) -> Result<(), io::Error> {
    std::fs::create_dir_all(dir)?;
    if present(dir) {
        weekly_snapshot(dir);
        return Ok(());
    }
    let conn = open(dir)?;
    migrate_legacy(&conn, dir)?;
    weekly_snapshot(dir);
    Ok(())
}

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
    // applications.yaml → applications (Value로 보관해 스키마 변경에 무관)
    let yaml_path = dir.join("applications.yaml");
    if let Ok(raw) = std::fs::read_to_string(&yaml_path) {
        match serde_yaml::from_str::<AppsShim>(&raw) {
            Ok(shim) if !shim.applications.is_empty() => {
                for (i, a) in shim.applications.iter().enumerate() {
                    let id = a["id"].as_str().unwrap_or("").to_string();
                    conn.execute(
                        "INSERT OR REPLACE INTO applications (idx, id, data) VALUES (?1, ?2, ?3)",
                        rusqlite::params![i as i64, id, serde_json::to_string(a).unwrap()],
                    )
                    .map_err(io_err)?;
                }
                let _ = std::fs::rename(&yaml_path, dir.join("applications.yaml.migrated"));
            }
            _ => {}
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
    tx.execute("DELETE FROM applications", []).map_err(io_err)?;
    for (i, a) in apps.iter().enumerate() {
        let id = a["id"].as_str().unwrap_or("").to_string();
        tx.execute(
            "INSERT INTO applications (idx, id, data) VALUES (?1, ?2, ?3)",
            rusqlite::params![i as i64, id, serde_json::to_string(a).unwrap()],
        )
        .map_err(io_err)?;
    }
    tx.commit().map_err(io_err)
}

/// DB의 profile.yaml 원문. 없어도 state_dir에 파일이 남아 있으면(미이관·깨짐) 그걸로 폴백.
pub fn load_profile_yaml(dir: &Path) -> Option<String> {
    let conn = open(dir).ok()?;
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
    open(dir)
        .ok()
        .and_then(|c| get_setting(&c, "profile_yaml"))
        .is_some()
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
            let _ = conn.execute(
                "VACUUM INTO ?1",
                rusqlite::params![target.display().to_string()],
            );
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

/// reset_cache — DB와 WAL 잔재를 지운다. 다음 스캔이 다시 베이스라인.
pub fn reset(dir: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let mut name = db_path(dir).into_os_string();
        name.push(suffix);
        let _ = std::fs::remove_file(std::path::PathBuf::from(name));
    }
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
            budget_monthly_won: Some(5_000_000),
            budget_total_won: Some((1_000, 2_000)),
            duration_days: Some(90),
            daily_won: Some((10, 20)),
            conditions: vec![("모집 마감일".into(), "2026-09-08".into())],
            ..Default::default()
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
        assert_eq!(ver, 1);
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
    fn reset_removes_db_and_wal() {
        let dir = tmpdir("reset");
        save_state(&dir, &State::default()).unwrap();
        assert!(present(&dir));
        reset(&dir);
        assert!(!present(&dir));
        assert!(!dir.join("state.db-wal").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
