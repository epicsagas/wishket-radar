---
name: wishket-profile
description: View or edit technical matching profile. Use when asked to view Wishket profile, change skill weights, add/remove skills, or update matching criteria. 매칭용 기술 프로필 조회/편집. "내 위시켓 프로필 보여줘", "rust 가중치 올려줘", "react 스킬 추가", "매칭 기준 바꿔줘" 등에 사용.
---

Manage the technical profile used for Wishket project matching. Target file: `~/.wishket-radar/profile.yaml` (overridden by `WISHKET_PROFILE` environment variable if set).

- **View requests**: Summarize and display `profile.yaml` contents as a table or list (skills, weights, keywords, roles, notes).
- **Edit requests** (e.g., "increase Rust weight", "add React", "remove Flutter"): Read `profile.yaml`, then update it via Edit/Write tools.
- Inform the user that modifications take effect immediately on the next scan (no server restart required).
- Formula: `score = 100 * sum(matched weights) / sum(total weights)`.
