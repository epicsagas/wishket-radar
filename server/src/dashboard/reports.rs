//! 스카우트 리포트(reports/*.md)에서 LLM 분석 결과를 추출한다.
//!
//! 리포트에는 기계 점수로는 못 내는 정보가 들어 있다 — 적합도 등급(A/B/C),
//! 주의점, 제안 방향. 이걸 공고 id에 붙여 인박스·파이프라인에서 바로 보게 한다.
//! 리포트는 사람이 읽는 문서이므로 파싱은 관대하게(형식이 흔들려도 부분 추출).

use std::collections::HashMap;

use serde::Serialize;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Analysis {
    /// A / B / C
    pub grade: Option<String>,
    pub title: Option<String>,
    /// "적합도 판단:" 뒤 서술
    pub fit: Option<String>,
    /// "주의점:" 뒤 서술
    pub caution: Option<String>,
    /// "제안 방향:" 뒤 서술
    pub proposal: Option<String>,
    /// 어느 리포트에서 왔는지 (파일명)
    pub report: Option<String>,
}

impl Analysis {
    fn is_empty(&self) -> bool {
        self.fit.is_none() && self.caution.is_none() && self.proposal.is_none()
    }
}

/// URL에서 위시켓 프로젝트 ID 추출.
fn id_from_url(s: &str) -> Option<String> {
    let after = s.split("/project/").nth(1)?;
    let id: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    (!id.is_empty()).then_some(id)
}

/// "### 1. [A] 제목" 에서 (등급, 제목).
fn parse_heading(line: &str) -> (Option<String>, Option<String>) {
    let full = line.trim_start_matches('#').trim();
    // 앞의 "1." 같은 번호 제거
    let rest = match full.split_once('.') {
        Some((n, r)) if n.trim().parse::<u32>().is_ok() => r.trim(),
        _ => full,
    };
    if rest.starts_with('[') {
        if let Some(close) = rest.find(']') {
            let grade = rest[1..close].trim().to_string();
            let title = rest[close + 1..].trim().to_string();
            return (
                (!grade.is_empty()).then_some(grade),
                (!title.is_empty()).then_some(title),
            );
        }
    }
    (None, (!rest.is_empty()).then(|| rest.to_string()))
}

/// "- 적합도 판단: ..." 같은 줄에서 라벨 뒤 본문.
fn field_after(line: &str, label: &str) -> Option<String> {
    let l = line.trim_start_matches(['-', '*', ' ']).trim();
    let rest = l.strip_prefix(label)?;
    let rest = rest.trim_start_matches([':', ' ']).trim();
    (!rest.is_empty()).then(|| rest.to_string())
}

fn flush(
    out: &mut HashMap<String, Analysis>,
    cur: &mut Option<Analysis>,
    cur_id: &mut Option<String>,
) {
    if let (Some(a), Some(id)) = (cur.take(), cur_id.take()) {
        if !a.is_empty() {
            out.insert(id, a);
        }
    }
}

/// 리포트 본문 하나를 파싱해 { 공고 id → 분석 } 으로.
pub fn parse(md: &str, report_name: &str) -> HashMap<String, Analysis> {
    let mut out = HashMap::new();
    let mut cur: Option<Analysis> = None;
    let mut cur_id: Option<String> = None;

    for line in md.lines() {
        let t = line.trim();
        // 새 항목 시작: "### N. [A] 제목"
        if t.starts_with("###") {
            flush(&mut out, &mut cur, &mut cur_id);
            let (grade, title) = parse_heading(t);
            cur = Some(Analysis {
                grade,
                title,
                report: Some(report_name.to_string()),
                ..Default::default()
            });
            continue;
        }
        // "## 그 외 신규" 같은 상위 섹션에서 끊는다
        if t.starts_with("##") {
            flush(&mut out, &mut cur, &mut cur_id);
            continue;
        }
        let Some(a) = cur.as_mut() else { continue };
        if cur_id.is_none() {
            if let Some(id) = id_from_url(t) {
                cur_id = Some(id);
            }
        }
        if let Some(v) = field_after(t, "적합도 판단") {
            a.fit = Some(v);
        } else if let Some(v) = field_after(t, "주의점") {
            a.caution = Some(v);
        } else if let Some(v) = field_after(t, "제안 방향") {
            a.proposal = Some(v);
        }
    }
    flush(&mut out, &mut cur, &mut cur_id);
    out
}

/// reports/ 전체를 읽어 병합. 최신 리포트가 이긴다.
pub fn load_all(reports_dir: &std::path::Path) -> HashMap<String, Analysis> {
    let mut merged = HashMap::new();
    // fsutil::list는 mtime 역순 — 오래된 것부터 넣어야 최신이 덮어쓴다
    let mut files = super::fsutil::list(reports_dir);
    files.reverse();
    for f in files {
        if !f.name.ends_with(".md") {
            continue;
        }
        let Ok(body) = std::fs::read_to_string(reports_dir.join(&f.name)) else {
            continue;
        };
        merged.extend(parse(&body, &f.name));
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &str = "\
# 위시켓 스캔 리포트 — 2026-09-02 11:17

신규 30건

## 분석 대상 (적합도 순)

### 1. [A] 게임 로열티 정산·보고서 자동화 시스템 구축
- URL: https://www.wishket.com/project/158052/ · 1,500만~2,000만원 · 90일
- 키워드 매칭: 48점 (matched: 결제/핀테크, AI/LLM/RAG)
- 적합도 판단: 정산·환율 과업이 결제 도메인 경험과 직결.
- 주의점: 마감 2026-09-07 (5일) — 즉시 제안 시작 필요.
- 제안 방향: 로우 데이터 처리 방안을 최중요로 구체 제시.

### 2. [B] 영림원 ERP DB 연동
- URL: https://www.wishket.com/project/157885/ · 800만원 · 40일
- 적합도 판단: 백엔드/DB 부분 겹침.
- 주의점: ⚠️ 마감 2026-09-04 (2일 남음). 지원자 104명.
- 제안 방향: APScheduler vs Celery 선택 기준 제시.

## 그 외 신규 (미분석, 27건)

| 제목 | 스코어 |
|---|---|
| 소개팅앱 마켓 등록 | 13 |
";

    #[test]
    fn extracts_grade_and_analysis_per_project() {
        let m = parse(REPORT, "2026-09-02-1117.md");
        assert_eq!(m.len(), 2, "분석 대상 2건만 (표는 제외): {m:?}");

        let a = m.get("158052").expect("158052");
        assert_eq!(a.grade.as_deref(), Some("A"));
        assert!(a.title.as_deref().unwrap().contains("게임 로열티"));
        assert!(a.fit.as_deref().unwrap().contains("결제 도메인"));
        assert!(a.caution.as_deref().unwrap().contains("2026-09-07"));
        assert!(a.proposal.as_deref().unwrap().contains("로우 데이터"));
        assert_eq!(a.report.as_deref(), Some("2026-09-02-1117.md"));

        assert_eq!(m.get("157885").unwrap().grade.as_deref(), Some("B"));
    }

    #[test]
    fn table_rows_are_not_parsed_as_items() {
        let m = parse(REPORT, "r.md");
        assert!(!m
            .values()
            .any(|a| a.title.as_deref().unwrap_or("").contains("소개팅앱")));
    }

    #[test]
    fn heading_without_grade_still_parses() {
        let md =
            "### 제목만 있음\n- URL: https://www.wishket.com/project/99/\n- 적합도 판단: 괜찮음\n";
        let m = parse(md, "r.md");
        let a = m.get("99").expect("99");
        assert!(a.grade.is_none());
        assert_eq!(a.fit.as_deref(), Some("괜찮음"));
    }

    #[test]
    fn item_without_url_is_dropped() {
        let md = "### 1. [A] 링크 없음\n- 적합도 판단: 내용\n";
        assert!(parse(md, "r.md").is_empty(), "id를 못 찾으면 버린다");
    }

    #[test]
    fn empty_analysis_is_dropped() {
        let md = "### 1. [A] 제목\n- URL: https://www.wishket.com/project/5/\n";
        assert!(parse(md, "r.md").is_empty(), "본문 없으면 버린다");
    }
}
