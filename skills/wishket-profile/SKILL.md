---
name: wishket-profile
description: 매칭용 기술 프로필 조회/편집. "내 위시켓 프로필 보여줘", "rust 가중치 올려줘", "react 스킬 추가", "매칭 기준 바꿔줘" 등에 사용.
---

매칭 기준 프로필 관리. 파일: `~/.wishket/profile.yaml` (`WISHKET_PROFILE` 환경변수로 오버라이드).

- 조회 요청: profile.yaml 내용을 표 혹은 목록으로 요약 출력 (스킬, weight, keywords, roles, notes).
- 변경 요청 ("rust 가중치 올려줘", "react 추가", "flutter 빼줘"): Read 후 Edit/Write로 profile.yaml 수정.
- 수정 즉시 다음 스캔에 반영됨(서버 재시작 불필요)을 안내.
- 산식: score = 100 × 매칭 weight 합 / 전체 weight 합.
