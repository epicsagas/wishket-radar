//! Seen-project cache for new-only diff scans.
//! State lives in `~/.wishket-radar/state.json` (override: WISHKET_STATE_DIR)
//! so it survives plugin updates.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const PRUNE_DAYS: u64 = 90;

/// 스캔으로 발견된 공고 한 건. 인박스(트리아지 대기열)의 항목이기도 하다.
///
/// 기존 필드(first_seen/title)만 있는 구 state.json도 그대로 읽힌다
/// — 추가분은 전부 `#[serde(default)]`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SeenEntry {
    pub first_seen: String,
    pub title: String,
    /// 트리아지 결과. None이면 아직 미분류(인박스에 남아 있음).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub triage: Option<Triage>,
    /// 트리아지한 날짜 (YYYY-MM-DD)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub triaged_at: Option<String>,
    /// 카드에서 딴 표시용 정보 — 인박스를 렌더할 때 재조회가 필요 없게 한다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
    /// 프라이빗 매칭(PRIME·PRO·BOOST 파트너 전용) 여부 — 홈페이지에 가야
    /// 확인 가능한 정보라 인박스에서 바로 보이게 저장한다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private_matching: Option<bool>,

    // --- 상세 캐시 (인박스 "상세 불러오기"로 채워짐) -------------------
    /// JSON-LD 전체 설명. 있으면 상세 화면이 재조회 없이 바로 렌더한다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 조건 행 (모집 마감일/예상 시작일/관련 기술 ...)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<(String, String)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    /// 매칭된 프로필 기술 (상세 기준 재계산)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub matched: Vec<String>,
    /// 상세를 마지막으로 가져온 시각 (ISO). 재조회 판단용.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail_fetched_at: Option<String>,
}

/// 인박스 트리아지 결과.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Triage {
    /// 관심 — 파이프라인으로 넘어간다
    Interested,
    /// 스킵 — 인박스에서 내리고 다시 안 띄운다
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct State {
    pub seen: HashMap<String, SeenEntry>,
    pub last_scan: Option<String>,
}

pub(crate) fn home_dir_from(home: Option<&OsStr>, userprofile: Option<&OsStr>) -> PathBuf {
    home.or(userprofile)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn home_dir() -> PathBuf {
    home_dir_from(
        std::env::var_os("HOME").as_deref(),
        std::env::var_os("USERPROFILE").as_deref(),
    )
}

pub fn state_dir() -> PathBuf {
    std::env::var_os("WISHKET_STATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = home_dir();
            let dir = home.join(".wishket-radar");
            // 1회 마이그레이션: 구 플러그인명 시절 상태를 옮겨 seen 캐시 유지
            let legacy = home.join(".wishket-agents");
            if !dir.exists() && legacy.exists() {
                let _ = std::fs::rename(&legacy, &dir);
            }
            dir
        })
}

pub fn state_path() -> PathBuf {
    state_dir().join("state.json")
}

pub fn load() -> State {
    let Ok(raw) = std::fs::read_to_string(state_path()) else {
        return State::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

/// Atomic save: write to a tmp sibling then rename over the target.
pub fn save(state: &State) -> std::io::Result<()> {
    let path = state_path();
    std::fs::create_dir_all(path.parent().unwrap_or(&path))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(state).unwrap())?;
    std::fs::rename(tmp, path)
}

/// unix epoch seconds → ISO-8601 local-time-ish (KST fixed +09:00).
/// ponytail: fixed KST offset, no tz database — scans run on a KST machine.
pub fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let kst = secs + 9 * 3600;
    let days = kst / 86400;
    let rem = kst % 86400;
    let (y, mo, d) = civil_from_days(days as i64);
    format!(
        "{y:04}-{mo:02}-{d:02}T{:02}:{:02}:{:02}+09:00",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Drop entries older than PRUNE_DAYS (best-effort parse of our own format).
pub fn prune(state: &mut State) {
    let cutoff = now_epoch() - PRUNE_DAYS * 86400;
    state.seen.retain(|_, e| {
        parse_iso_epoch(&e.first_seen)
            .map(|t| t >= cutoff)
            .unwrap_or(true)
    });
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Parse "YYYY-MM-DDTHH:MM:SS+09:00" (our own writer) to epoch seconds.
/// ponytail: manual slice math, only our format — chrono not worth the dep.
fn parse_iso_epoch(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> {
        std::str::from_utf8(&b[r]).ok()?.trim().parse().ok()
    };
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, sec) = (num(11..13)?, num(14..16)?, num(17..19)?);
    // days from civil (Howard Hinnant)
    let y = if mo <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if mo > 2 { mo - 3 } else { mo + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    let secs = days * 86400 + h * 3600 + mi * 60 + sec - 9 * 3600;
    Some(secs.max(0) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn iso_epoch_roundtrip() {
        let s = now_iso();
        assert!(parse_iso_epoch(&s).is_some());
        assert_eq!(
            parse_iso_epoch("2026-08-31T18:00:00+09:00"),
            Some(1788166800)
        );
    }

    #[test]
    fn prune_keeps_recent() {
        let mut st = State::default();
        let recent = now_iso();
        st.seen.insert(
            "1".into(),
            SeenEntry {
                first_seen: recent.clone(),
                title: "t".into(),
                ..Default::default()
            },
        );
        st.seen.insert(
            "2".into(),
            SeenEntry {
                first_seen: "2020-01-01T00:00:00+09:00".into(),
                title: "old".into(),
                ..Default::default()
            },
        );
        prune(&mut st);
        assert!(st.seen.contains_key("1"));
        assert!(!st.seen.contains_key("2"));
    }

    #[test]
    fn legacy_state_json_without_new_fields_still_loads() {
        // 0.1.x가 쓴 state.json에는 triage/url/score 등이 없다 — 그래도 읽혀야 한다.
        let raw = r#"{"seen":{"1":{"first_seen":"2026-09-01T10:00:00+09:00","title":"구버전"}},"last_scan":null}"#;
        let st: State = serde_json::from_str(raw).expect("legacy state parses");
        let e = st.seen.get("1").expect("entry");
        assert_eq!(e.title, "구버전");
        assert!(e.triage.is_none(), "미분류 = 인박스에 남는다");
        assert!(e.url.is_none());
        assert!(e.skills.is_empty());
    }

    #[test]
    fn triage_roundtrips_through_json() {
        let mut st = State::default();
        st.seen.insert(
            "9".into(),
            SeenEntry {
                first_seen: now_iso(),
                title: "관심 건".into(),
                triage: Some(Triage::Interested),
                triaged_at: Some("2026-09-02".into()),
                score: Some(37),
                private_matching: Some(true),
                ..Default::default()
            },
        );
        let json = serde_json::to_string(&st).unwrap();
        assert!(json.contains("\"interested\""), "{json}");
        let back: State = serde_json::from_str(&json).unwrap();
        assert_eq!(back.seen["9"].triage, Some(Triage::Interested));
        assert_eq!(back.seen["9"].score, Some(37));
        assert_eq!(back.seen["9"].private_matching, Some(true));
    }

    #[test]
    fn home_dir_prefers_home_then_userprofile() {
        assert_eq!(
            home_dir_from(
                Some(OsStr::new("/Users/a")),
                Some(OsStr::new("C:\\Users\\a"))
            ),
            PathBuf::from("/Users/a")
        );
        assert_eq!(
            home_dir_from(None, Some(OsStr::new("C:\\Users\\a"))),
            PathBuf::from("C:\\Users\\a")
        );
        assert_eq!(home_dir_from(None, None), PathBuf::from("."));
    }
}
