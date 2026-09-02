---
name: wishket-scout
description: 위시켓 신규 공고 심층 분석·리포트. scan_new로 신규를 조회한 뒤 상위 후보를 wishket-analyst로 분석해 한국어 리포트를 작성한다. "위시켓 분석", "스카우트", "리포트 뽑아줘", "심층 분석해줘" 등에 사용. 목록만 보려면 wishket-scan.
---

# wishket-scout — 위시켓 프로젝트 스카우트

## 흐름

```mermaid
flowchart LR
    A[scan_new MCP 호출] --> B{신규 있음?}
    B -- 없음 --> Z[요약만 응답]
    B -- 있음 --> C[점수 상위 N개 선별]
    C --> D[get_project 순차]
    D --> E[wishket-analyst]
    E --> F[reports/ 저장 + 채팅 요약]
```

## 1단계: 스캔

MCP 도구 `wishket` 서버의 `scan_new` 호출. 인자를 생략해도 서버가 `category=development`, `form_factors=web,pc,android,ios`, `max_pages=3`을 넣는다. 사용자가 지정한 값이 있으면 그걸 쓴다.

- 사용자가 키워드를 지정했으면 `keyword` 추가.
- 첫 실행(`baseline: true`)이면 모든 항목이 신규인 것이 정상. 리포트에 "베이스라인 스캔" 표시.
- `new_count == 0`이면: 마지막 스캔 시각(`~/.wishket-radar/state.json`의 `last_scan`)과 함께 "신규 없음"만 응답하고 종료.
- `total_matching_filter` 대비 조회 수가 현저히 적으면(30건 조회 제한) 리포트에 "상위 N페이지만 조회함" 명시.

## 2단계: 후보 선별

응답의 `new` 배열을 `match.score` 내림차순(이미 정렬됨) 기준으로:

- **score >= 40** 또는 상위 5개 중 큰 쪽을 분석 대상으로.
- score가 낮아도 제목에 명시적 키워드(예: rust, flutter, llm)가 있으면 포함.
- 분석 대상은 최대 5개. 나머지는 리포트 말미에 표로 간단 나열.

## 3단계: 상세 분석

각 후보에 대해 `get_project(id)`로 상세(JSON-LD 전체 설명 포함)를 **순차** 조회한다. 서버가 요청 사이 robots Crawl-delay 5초를 지키므로 `get_project`를 병렬 호출하지 않는다. 상세를 모은 뒤 `wishket-analyst` 서브에이전트는 후보별로 동시에 디스패치해도 된다.

- 서브에이전트 프롬프트: get_project 결과 JSON 전체 + 사용자 프로필 요약.
- 프로필 요약: `~/.wishket-radar/profile.yaml` (`WISHKET_PROFILE`이 있으면 그 경로)을 Read로 읽는다. 파일이 없으면 스택을 지어내지 말고, 프로필 없음과 wishket-onboard 안내를 적고 analyst에는 "프로필 없음"만 넘긴다.

## 4단계: 리포트

리포트 파일: `~/.wishket-radar/reports/YYYY-MM-DD-HHmm.md` (디렉터리 없으면 생성, 한국 시각 기준 파일명).

템플릿:

```markdown
# 위시켓 스캔 리포트 — YYYY-MM-DD HH:mm

신규 N건 (전체 M건 중 조회 K건) · 필터: development / web,pc,android,ios
> 베이스라인 스캔 (첫 실행) 또는 마지막 스캔: {last_scan}

## 분석 대상 (적합도 순)

### 1. [A] {제목}
- URL: {url} · {budget} · {duration} · {role}/{level} · {location}
- 키워드 매칭: {score}점 (matched: ..., missing: ...)
- 적합도 판단: {analyst 서술}
- 주의점: {analyst 서술}
- 제안 방향: {analyst 서술 1-2줄}

(후보마다 반복)

## 그 외 신규 (미분석)

| 제목 | 스코어 | 지원자 | 댓글 | 좋아요 | 예산 | 마감 |
|---|---|---|---|---|---|---|
```

적합도 등급은 analyst 판정(A/B/C) 사용. 등급 기준: A=핵심 스택 직접 매칭+조건 합리적, B=부분 매칭 또는 조건 불확실, C=스택 편차 큼.

## 5단계: 채팅 요약

리포트 전문 대신 요약 응답:

- 신규 N건, A/B/C 등급 분포
- A등급 프로젝트 제목+한줄 이유 (있으면)
- 리포트 파일 경로
- 마감 임박(마감 1주 이내) 항목 강조

## 리포트와 인박스

리포트의 적합도 판정(A/B/C)·주의점·제안 방향은 대시보드가 `reports/*.md`를 파싱해 해당 공고의 인박스 카드에 자동으로 붙인다. 따라서 **"### N. [A] 제목" 헤딩과 그 아래 `- URL:` 줄 형식을 지켜야** 연결된다. 형식이 깨지면 분석 결과가 공고에 붙지 않는다.

## 유지

- 스캔 기준 갱신: 사용자가 "매칭 기준 바꿔줘" 하면 wishket-profile로 `profile.yaml`을 편집한다.
- 분석한 공고를 바로 추적하려면 wishket-pipeline으로 "관심" 등록(또는 대시보드 인박스에서 분류).
- 캐시 리셋: `reset_cache` MCP 도구. 다음 스캔이 베이스라인이 됨.
- 필터 확인: `list_filters` 도구. 검증된 키(c/ff/page/s) 외에는 raw로만 전달.
