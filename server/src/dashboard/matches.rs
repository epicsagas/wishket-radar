//! 인박스 트리아지 결과를 파이프라인 항목으로 변환한다.
//!
//! (0.2.0에서 matches.md 파싱은 제거됐다 — 생성 주체가 없는 수기 파일이라
//! 인박스·리포트와 역할이 겹쳤다. 관심 표시가 그 역할을 대신한다.)

use super::apps::Application;
use crate::state::{SeenEntry, Triage};

/// 구 matches.md를 1회 마이그레이션한다: 표의 프로젝트 링크를 관심 표시로 옮기고
/// 파일을 `matches.md.migrated`로 이름을 바꿔 다시 돌지 않게 한다.
/// (0.2.0에서 matches.md 연동이 빠지면서 기존 사용자의 큐레이션이 유실되는 걸 막는다.)
pub fn migrate_legacy_file(dir: &std::path::Path, today: &str) -> usize {
    let path = dir.join("matches.md");
    let Ok(md) = std::fs::read_to_string(&path) else {
        return 0;
    };
    let mut st = crate::state::load();
    let mut moved = 0;
    for line in md.lines() {
        let t = line.trim();
        if !t.starts_with('|') {
            continue;
        }
        let Some(id) = t
            .split("/project/")
            .nth(1)
            .map(|a| {
                a.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
            })
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let cells: Vec<&str> = t.trim_matches('|').split('|').map(str::trim).collect();
        let title = cells
            .get(1)
            .and_then(|c| c.split_once('['))
            .and_then(|(_, r)| r.split_once(']'))
            .map(|(t, _)| t.to_string())
            .unwrap_or_default();
        let e = st.seen.entry(id).or_insert_with(|| SeenEntry {
            first_seen: crate::state::now_iso(),
            title,
            ..Default::default()
        });
        if e.triage.is_none() {
            e.triage = Some(Triage::Interested);
            e.triaged_at = Some(today.to_string());
            moved += 1;
        }
    }
    if moved > 0 && crate::state::save(&st).is_ok() {
        let _ = std::fs::rename(&path, dir.join("matches.md.migrated"));
    }
    moved
}

/// 인박스에서 "관심" 표시한 항목을 파이프라인 항목으로 변환한다.
/// 아직 지원 전이므로 상태는 "관심".
pub fn from_triage(id: &str, e: &SeenEntry) -> Application {
    Application {
        id: id.to_string(),
        title: e.title.clone(),
        url: e.url.clone(),
        grade: e.score.map(|s| s.to_string()),
        deadline: e.deadline.clone(),
        status: "관심".into(),
        status_at: e.triaged_at.clone(),
        note: match (e.budget.as_deref(), e.duration.as_deref()) {
            (Some(b), Some(d)) => Some(format!("{b} · {d}")),
            (Some(b), None) => Some(b.to_string()),
            (None, Some(d)) => Some(d.to_string()),
            (None, None) => None,
        },
        ..Default::default()
    }
}

/// 관심 표시된 seen 항목들을 파이프라인 항목으로.
pub fn interested(seen: &std::collections::HashMap<String, SeenEntry>) -> Vec<Application> {
    let mut out: Vec<Application> = seen
        .iter()
        .filter(|(_, e)| e.triage == Some(Triage::Interested))
        .map(|(id, e)| from_triage(id, e))
        .collect();
    // 발견 순서 역순(최신 먼저)으로 안정 정렬
    out.sort_by(|a, b| b.id.cmp(&a.id));
    out
}

/// applications.yaml 항목이 우선. 없는 건 추가한다.
///
/// 단, yaml에 비어 있는 표시용 필드(매칭 점수·마감·URL·제목)는 인박스 쪽 값으로
/// 채운다. 승격 시점 이후에 상세를 불러와 점수가 생겨도 yaml은 갱신되지 않기
/// 때문에, 보강하지 않으면 화면에서 영영 비어 보인다.
pub fn merge(mut explicit: Vec<Application>, extra: Vec<Application>) -> Vec<Application> {
    for m in extra {
        match explicit.iter_mut().find(|a| a.id == m.id) {
            Some(a) => {
                if a.grade.is_none() {
                    a.grade = m.grade.clone();
                }
                if a.deadline.is_none() {
                    a.deadline = m.deadline.clone();
                }
                if a.url.is_none() {
                    a.url = m.url.clone();
                }
                if a.title.trim().is_empty() {
                    a.title = m.title.clone();
                }
            }
            None => explicit.push(m),
        }
    }
    explicit
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn entry(title: &str, triage: Option<Triage>) -> SeenEntry {
        SeenEntry {
            first_seen: "2026-09-01T10:00:00+09:00".into(),
            title: title.into(),
            triage,
            url: Some("https://www.wishket.com/project/1/".into()),
            score: Some(42),
            budget: Some("월 500만".into()),
            duration: Some("예상 기간 90일".into()),
            ..Default::default()
        }
    }

    #[test]
    fn only_interested_enters_pipeline() {
        let mut seen = HashMap::new();
        seen.insert("1".to_string(), entry("관심 건", Some(Triage::Interested)));
        seen.insert("2".to_string(), entry("스킵 건", Some(Triage::Skipped)));
        seen.insert("3".to_string(), entry("미분류", None));
        let apps = interested(&seen);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].id, "1");
        assert_eq!(apps[0].status, "관심");
        assert_eq!(apps[0].grade.as_deref(), Some("42"));
        assert_eq!(apps[0].note.as_deref(), Some("월 500만 · 예상 기간 90일"));
    }

    #[test]
    fn legacy_migration_moves_rows_to_interested() {
        let dir = std::env::temp_dir().join(format!("wk-mig-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("WISHKET_STATE_DIR", &dir);
        std::fs::write(
            dir.join("matches.md"),
            "| 상태 | 공고 | 조건 |\n|---|---|---|\n| 검토 | [제목](https://www.wishket.com/project/777/) | 월 500만 |\n",
        )
        .unwrap();

        assert_eq!(migrate_legacy_file(&dir, "2026-09-02"), 1);
        let st = crate::state::load();
        assert_eq!(st.seen["777"].triage, Some(Triage::Interested));
        assert!(dir.join("matches.md.migrated").exists(), "재실행 방지");
        assert_eq!(
            migrate_legacy_file(&dir, "2026-09-02"),
            0,
            "두 번 돌지 않는다"
        );

        std::env::remove_var("WISHKET_STATE_DIR");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_fields_are_backfilled_from_triage() {
        // 승격 후에 상세를 불러와 점수가 생기면, yaml에 없어도 화면엔 보여야 한다
        let explicit = vec![Application {
            id: "1".into(),
            title: "수동 입력".into(),
            status: "지원".into(),
            grade: None,
            deadline: None,
            ..Default::default()
        }];
        let mut seen = HashMap::new();
        let mut e = entry("관심 건", Some(Triage::Interested));
        e.deadline = Some("2026-09-08".into());
        seen.insert("1".to_string(), e);
        let merged = merge(explicit, interested(&seen));
        let a = &merged[0];
        assert_eq!(a.status, "지원", "상태는 yaml이 이긴다");
        assert_eq!(a.title, "수동 입력", "제목도 yaml이 이긴다");
        assert_eq!(a.grade.as_deref(), Some("42"), "빈 점수는 채운다");
        assert_eq!(
            a.deadline.as_deref(),
            Some("2026-09-08"),
            "빈 마감도 채운다"
        );
    }

    #[test]
    fn explicit_wins_over_triage() {
        let explicit = vec![Application {
            id: "1".into(),
            title: "수동 입력".into(),
            status: "미팅".into(),
            ..Default::default()
        }];
        let mut seen = HashMap::new();
        seen.insert("1".to_string(), entry("관심 건", Some(Triage::Interested)));
        seen.insert("9".to_string(), entry("다른 건", Some(Triage::Interested)));
        let merged = merge(explicit, interested(&seen));
        let a = merged.iter().find(|a| a.id == "1").unwrap();
        assert_eq!(a.status, "미팅", "applications.yaml이 이긴다");
        assert_eq!(a.title, "수동 입력");
        assert_eq!(merged.len(), 2, "관심에만 있는 건 추가됨");
    }
}
