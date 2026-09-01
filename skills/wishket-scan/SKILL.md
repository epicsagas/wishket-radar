---
name: wishket-scan
description: 위시켓 신규 프로젝트 diff 스캔 (마지막 스캔 이후 신규만). "위시켓 스캔", "새 프로젝트 있어?", "새 외주 올라온 거 있나?" 등에 사용. 심층 분석·리포트는 wishket-scout.
---

위시켓 신규 프로젝트 diff 스캔. `scan_new` MCP 도구 1회 호출로 마지막 스캔 이후 신규 프로젝트만 반환 (신규는 seen 캐시에 기록).

1. 요청에 키워드·카테고리 언급이 있으면 인자로 반영, 없으면 기본값 (development, web,pc,android,ios, 3페이지).
2. 결과 요약: 신규 N건 (`new_count`), 스코어 순 목록. `baseline: true`면 베이스라인 스캔임을 안내.
3. `new_count == 0`이면 마지막 스캔 시각(`~/.wishket-radar/state.json`의 `last_scan`)과 함께 "신규 없음"만 응답.
4. 심층 분석·리포트가 필요하면 wishket-scout 스킬로 확장 제안.
