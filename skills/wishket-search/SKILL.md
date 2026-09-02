---
name: wishket-search
description: Ad-hoc search for Wishket projects without recording to seen cache. Use for keyword searches and browsing project listings. 위시켓 프로젝트 임시 검색 (캐시 기록 없음, 일상 조회용). "위시켓 검색", "위시켓에 flutter 프로젝트 있어?", "외주 찾아줘", "외주 프로젝트 검색해줘" 등 키워드 검색·페이지 이동에 사용. 신규 diff는 wishket-scan, 심층 리포트는 wishket-scout.
---

Ad-hoc search for Wishket projects. Does not record results into the `seen` cache (intended for routine queries, not scan diff reports).

1. Parse search intent from the user request and invoke the `search_projects` MCP tool. Pass keywords in `keyword`, and set categories, form factors, and page numbers if mentioned.
2. Present results in a summary table ordered by match score. Columns: Title, Score, Applicants, Comments, Likes, Views, Budget, Deadline, URL.
   - Score: Initial list match score (`match.score`). Format as `"N (matched skills summary)"`.
   - Applicants from `applicants` ("지원자 N명" or "비공개"), comments from `comments`, likes from `likes`, views from `views`. Comments and likes are public metrics that do not require login.
3. If precise score computation is required (initial list scores are calculated only from title/role/skills text), call `get_project` to recompute score including full description text.
4. If in-depth analysis of a specific project is needed, suggest using `get_project` + the `wishket-analyst` agent.
