---
name: wishket-search
description: 위시켓 프로젝트 임시 검색 (캐시 기록 없음, 일상 조회용). "위시켓 검색", "위시켓에 flutter 프로젝트 있어?", "외주 찾아줘", "외주 프로젝트 검색해줘" 등 키워드 검색·페이지 이동에 사용. 신규 diff는 wishket-scan, 심층 리포트는 wishket-scout.
---

위시켓 임시 검색. seen 캐시에 기록하지 않는다 (일상 조회용, 스캔 리포트 아님).

1. 요청에서 검색 의도를 파싱해 `search_projects` MCP 도구 호출. 키워드는 keyword, 카테고리·형태·페이지가 언급되면 해당 인자로.
2. 결과를 스코어 순 표로 요약 출력. 컬럼: 제목, 스코어, 지원자, 댓글, 좋아요, 조회, 예산, 마감, URL.
   - 스코어: 검색 결과의 `match.score` (목록 초깃값). 표기는 "N (매칭 스킬 요약)" 형식.
   - 지원자는 `applicants` ("지원자 N명"/"비공개"), 댓글은 `comments`, 좋아요는 `likes`, 조회는 `views` 필드에서. 댓글·좋아요 수는 공개 데이터라 로그인 불필요.
3. 스코어 정밀값이 필요하면 (목록 초깃값은 title/role/skills 텍스트 기준이라 낮게 나옴) `get_project` 상세 조회로 description 포함 재계산 값 사용. get_project의 `comments`는 상세 페이지 카운트.
4. 특정 항목 심층 분석이 필요하면 `get_project` + wishket-analyst 에이전트 사용을 제안.
