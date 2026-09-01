//! Seen-project cache for new-only diff scans.
//! State lives in `~/.wishket-radar/state.json` (override: WISHKET_STATE_DIR)
//! so it survives plugin updates.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const PRUNE_DAYS: u64 = 90;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SeenEntry {
    pub first_seen: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct State {
    pub seen: HashMap<String, SeenEntry>,
    pub last_scan: Option<String>,
}

pub fn state_dir() -> PathBuf {
    std::env::var_os("WISHKET_STATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| ".".into()));
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
            },
        );
        st.seen.insert(
            "2".into(),
            SeenEntry {
                first_seen: "2020-01-01T00:00:00+09:00".into(),
                title: "old".into(),
            },
        );
        prune(&mut st);
        assert!(st.seen.contains_key("1"));
        assert!(!st.seen.contains_key("2"));
    }
}
