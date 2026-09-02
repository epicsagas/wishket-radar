//! applications.yaml (wishket-pipeline 스킬이 정의한 스키마) 읽기/쓰기/통계.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::fsutil;

/// 위시켓 실제 수주 퍼널 순서.
/// 지원 → 위시켓 상담(매니저가 지원자 선발) → 삼자 미팅 → 체결/미체결 →
/// 선예치 후 진행 → 승인·대금 지급 완료. 관심은 지원 전 단계.
pub const STATUSES: [&str; 10] = [
    "관심",
    "지원",
    "상담",
    "미팅",
    "체결",
    "진행 중",
    "완료",
    "미체결",
    "탈락",
    "철회",
];

/// 계약이 성사된 단계 (수주율 분자).
pub const WON: [&str; 3] = ["체결", "진행 중", "완료"];

/// 성사 없이 끝난 단계 (수주율 분모에 포함).
pub const LOST: [&str; 2] = ["미체결", "탈락"];

/// 아직 살아있는 단계. 철회는 본인 포기라 어느 쪽에도 안 넣는다.
pub const OPEN: [&str; 4] = ["지원", "상담", "미팅", "관심"];

/// 퍼널 전환율을 볼 때 "이 단계까지 도달했다"고 보는 순서.
/// 예: 체결이면 지원·상담·미팅도 통과한 것으로 센다.
const FUNNEL: [&str; 5] = ["지원", "상담", "미팅", "체결", "완료"];

/// status가 퍼널 단계 `stage`에 도달했는지. 종결 상태는 마지막 도달 지점까지만 인정.
fn reached(status: &str, stage: &str) -> bool {
    let rank = |s: &str| FUNNEL.iter().position(|f| *f == s);
    // 종결 상태를 퍼널 위치로 환산
    let cur = match status {
        "관심" => return false,
        "진행 중" => Some(3), // 체결까지 확실히 통과
        "탈락" => Some(1),    // 최소 상담까지는 갔다
        "미체결" => Some(2),  // 미팅까지 갔다
        "철회" => return false,
        other => rank(other),
    };
    match (cur, rank(stage)) {
        (Some(c), Some(t)) => c >= t,
        _ => false,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApplicationsFile {
    #[serde(default)]
    pub applications: Vec<Application>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Application {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub grade: Option<String>,
    #[serde(default)]
    pub quote_manwon: Option<u32>,
    #[serde(default)]
    pub applied_at: Option<String>,
    #[serde(default)]
    pub deadline: Option<String>,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub status_at: Option<String>,
    #[serde(default)]
    pub next_action: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

fn default_status() -> String {
    "관심".into()
}

/// 구 상태명(0.2.0 이전) → 신 퍼널 상태. 기존 파일이 조용히 기본값으로
/// 떨어지는 걸 막는다. 읽을 때 변환하고, 저장 시점에 새 이름으로 굳는다.
fn migrate_status(s: &str) -> Option<&'static str> {
    match s {
        "검토중" => Some("관심"),
        "면담" => Some("미팅"),
        "수주" => Some("체결"),
        "거절" => Some("탈락"),
        _ => None,
    }
}

/// 로드 결과. parse_error는 파일이 깨졌을 때 내용을 지우지 않고 UI에 경고만 띄우기 위한 것.
pub struct Loaded {
    pub file: ApplicationsFile,
    pub parse_error: Option<String>,
}

pub fn load(dir: &Path) -> Loaded {
    let path = dir.join("applications.yaml");
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Loaded {
            file: ApplicationsFile::default(),
            parse_error: None,
        };
    };
    match serde_yaml::from_str::<ApplicationsFile>(&raw) {
        Ok(mut file) => {
            for a in &mut file.applications {
                if let Some(new) = migrate_status(&a.status) {
                    a.status = new.to_string();
                }
            }
            Loaded {
                file,
                parse_error: None,
            }
        }
        Err(e) => Loaded {
            file: ApplicationsFile::default(),
            parse_error: Some(e.to_string()),
        },
    }
}

pub fn save(dir: &Path, file: &ApplicationsFile) -> std::io::Result<()> {
    let body = serde_yaml::to_string(file).expect("applications serialize");
    fsutil::atomic_write(&dir.join("applications.yaml"), body.as_bytes())
}

/// 퍼널 통계. win_rate = 체결이상/(체결이상+미체결+탈락), 분모 0이면 null.
/// 단계별 도달 수도 같이 내보내 어디서 새는지 보이게 한다.
pub fn stats(apps: &[Application]) -> Value {
    let count = |s: &str| apps.iter().filter(|a| a.status == s).count();
    let won = WON.iter().map(|s| count(s)).sum::<usize>();
    let lost = LOST.iter().map(|s| count(s)).sum::<usize>();
    let settled = won + lost;
    let mut by_status = serde_json::Map::new();
    for s in STATUSES {
        by_status.insert(s.to_string(), json!(count(s)));
    }
    let mut funnel = serde_json::Map::new();
    for stage in FUNNEL {
        let n = apps.iter().filter(|a| reached(&a.status, stage)).count();
        funnel.insert(stage.to_string(), json!(n));
    }
    json!({
        "by_status": by_status,
        "funnel": funnel,
        "open": OPEN.iter().map(|s| count(s)).sum::<usize>(),
        "won": won,
        "lost": lost,
        "win_rate": if settled > 0 {
            json!((won as f64 / settled as f64 * 1000.0).round() / 10.0)
        } else {
            Value::Null
        },
        "samples": settled,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
applications:
  - id: \"12345\"
    title: 사내 시스템 재구축
    url: https://wishket.com/project/12345
    grade: A
    quote_manwon: 9500
    applied_at: 2026-09-02
    deadline: 2026-09-10
    status: 미팅
    status_at: 2026-09-03
    next_action: 기술 면담 자료 준비
    note: |
      첫 미팅에서 레거시 범위 협의.
      2차 제안 대기.
  - id: \"42\"
    title: 최소 필드만
    status: 이상한상태
";

    fn tmpdir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("wk-apps-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn roundtrip_preserves_fields_and_unknown_status() {
        let dir = tmpdir("roundtrip");
        save(&dir, &serde_yaml::from_str(SAMPLE).unwrap()).unwrap();
        let Loaded { file, parse_error } = load(&dir);
        assert!(parse_error.is_none());
        assert_eq!(file.applications.len(), 2);
        let a = &file.applications[0];
        assert_eq!(a.id, "12345");
        assert_eq!(a.quote_manwon, Some(9500));
        assert!(a.note.as_deref().unwrap().contains("2차 제안"));
        assert_eq!(file.applications[1].status, "이상한상태");
        // 재직렬화 후에도 동일
        save(&dir, &file).unwrap();
        let again = load(&dir);
        assert_eq!(again.file.applications, file.applications);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_fields_default() {
        let file: ApplicationsFile =
            serde_yaml::from_str("applications:\n  - id: \"1\"\n").unwrap();
        assert_eq!(file.applications[0].status, "관심");
        assert_eq!(file.applications[0].title, "");
        assert!(file.applications[0].deadline.is_none());
    }

    #[test]
    fn missing_file_is_empty_not_error() {
        let dir = tmpdir("missing");
        let Loaded { file, parse_error } = load(&dir);
        assert!(file.applications.is_empty());
        assert!(parse_error.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn broken_yaml_reports_parse_error() {
        let dir = tmpdir("broken");
        std::fs::write(dir.join("applications.yaml"), "applications: [").unwrap();
        let Loaded { file, parse_error } = load(&dir);
        assert!(file.applications.is_empty());
        assert!(parse_error.is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stats_counts_and_win_rate() {
        let mk = |id: &str, status: &str| Application {
            id: id.into(),
            status: status.into(),
            ..Default::default()
        };
        let s = stats(&[
            mk("1", "체결"),
            mk("2", "완료"),
            mk("3", "탈락"),
            mk("4", "미체결"),
            mk("5", "지원"),
            mk("6", "철회"),
        ]);
        assert_eq!(s["won"], json!(2), "체결+완료");
        assert_eq!(s["lost"], json!(2), "탈락+미체결");
        assert_eq!(s["samples"], json!(4));
        assert_eq!(s["win_rate"], json!(50.0));
        assert_eq!(s["open"], json!(1), "철회는 open이 아니다");
        assert_eq!(s["by_status"]["체결"], json!(1));

        let empty = stats(&[]);
        assert!(empty["win_rate"].is_null());
        assert_eq!(empty["samples"], json!(0));
    }

    #[test]
    fn funnel_counts_cumulative_reach() {
        let mk = |id: &str, status: &str| Application {
            id: id.into(),
            status: status.into(),
            ..Default::default()
        };
        let s = stats(&[
            mk("1", "관심"), // 퍼널 진입 전
            mk("2", "지원"),
            mk("3", "미팅"),
            mk("4", "완료"),
            mk("5", "탈락"), // 상담까지 도달로 인정
        ]);
        let f = &s["funnel"];
        assert_eq!(f["지원"], json!(4), "관심 제외 전부 지원은 했다");
        assert_eq!(f["상담"], json!(3), "미팅/완료/탈락");
        assert_eq!(f["미팅"], json!(2), "미팅/완료");
        assert_eq!(f["체결"], json!(1), "완료만");
        assert_eq!(f["완료"], json!(1));
    }

    #[test]
    fn legacy_status_names_migrate_on_load() {
        let dir = tmpdir("migrate");
        std::fs::write(
            dir.join("applications.yaml"),
            "applications:\n  - id: \"1\"\n    status: 수주\n  - id: \"2\"\n    status: 검토중\n  - id: \"3\"\n    status: 면담\n  - id: \"4\"\n    status: 거절\n",
        )
        .unwrap();
        let got: Vec<String> = load(&dir)
            .file
            .applications
            .iter()
            .map(|a| a.status.clone())
            .collect();
        assert_eq!(got, vec!["체결", "관심", "미팅", "탈락"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
