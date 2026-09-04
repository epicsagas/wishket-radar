---
name: wishket-scan
description: Diff scan for new Wishket projects registered since the last scan. Use for quick new project checks. 위시켓 신규 프로젝트 diff 스캔 (마지막 스캔 이후 신규만). "위시켓 스캔", "새 프로젝트 있어?", "새 외주 올라온 거 있나?" 등에 사용. 심층 분석·리포트는 wishket-scout.
---

Diff scan for new Wishket projects. Calls the `scan_new` MCP tool once to return only projects posted since the last scan (new items are recorded to the `seen` cache).

1. If the request specifies keywords or categories, pass them as arguments; otherwise use defaults (`development`, `web,pc,android,ios`, `3` pages).
2. Summarize results: new count (`new_count`) and list sorted by match score. If `baseline: true`, notify the user that this was a baseline scan.
3. If `new_count == 0`, report "No new projects" along with the timestamp of the last scan (`last_scan` from `~/.wishket-radar/state.db` (SQLite, WAL)).
4. If deep analysis or a full report is needed, suggest expanding via the `wishket-scout` skill.

## Workflow After Scan

Scanned projects accumulate in `state.db` (SQLite, WAL) and remain in the dashboard **Inbox** until triaged. Triaging (Interested vs. Skip) is quickest via the web UI (`wishket-dashboard`), and only items marked as Interested proceed into the application pipeline.
