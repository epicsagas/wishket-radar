---
name: wishket-onboard
description: 위시켓 플러그인 온보딩 (바이너리 설치 확인 + 프로필 생성 + 베이스라인 스캔 + 다음 행동 요령). "위시켓 시작할게", "위시켓 세팅해줘", "온보딩" 등에 사용.
---

위시켓 플러그인 온보딩. 새 환경/사용자에서 wishket-radar가 정상 구동 가능한 상태를 만들고, 사용자 프로필 설정부터 다음 행동 요령까지 원스톱으로 완료한다.

1. **바이너리 및 실행 환경 점검/설치** (순서 고정. `cargo`가 있어도 한방 설치를 건너뛰지 않는다):
   - MCP 서버(`wishket-mcp`)가 정상 구동 가능한지 확인 (`list_filters` 호출 또는 `which wishket-mcp`).
   - 없거나 기동 실패면 **한방 설치를 먼저** 실행한다. PATH에 `cargo`가 있어도 이 단계를 생략하지 않는다.
     - macOS / Linux: `curl --proto '=https' --tlsv1.2 -LsSf https://github.com/epicsagas/wishket-radar/releases/latest/download/install.sh | sh`
     - Windows: `powershell -c "irm https://github.com/epicsagas/wishket-radar/releases/latest/download/install.ps1 | iex"`
     - 플러그인 루트에 `install.sh` / `install.ps1`이 있으면 그걸 실행해도 된다.
   - 한방 설치가 실패했고, **현재 위치가 git 클론**(`server/Cargo.toml`과 `.git`)이며 `cargo`가 있을 때만 `cargo build --release --manifest-path server/Cargo.toml`. 플러그인 캐시 디렉터리에서는 cargo 빌드를 하지 않는다.
   - MCP 도구가 응답하는지 최종 확인.

2. **현재 프로필 상태 확인**:
   - `~/.wishket-radar/profile.yaml` (`WISHKET_PROFILE` 오버라이드 시 그 경로) 읽기.
   - 이미 프로필이 채워져 있으면 기존 프로필을 요약해 보여주고, 재설정 여부만 AskUserQuestion으로 확인.
     - 기존 프로필 유지 선택 시: 4단계(베이스라인 스캔)로 바로 이동.
     - 재설정 선택 또는 프로필이 없는 경우: 3단계로 진행.

3. **프로필 인터뷰 & profile.yaml 생성**:
   - AskUserQuestion으로 인터뷰 진행 (한 번에 최대 4개):
     - 주력 기술 스택 (예: Rust, Svelte+TS, Flutter, AWS)
     - 스택별 상대 중요도 (높음/중간/낮음 → weight 3/2/1)
     - 수행 가능한 모집 역할 (예: 백엔드 개발자, 풀스택 개발자)
     - 근무 조건 notes (원격/출근/지역, 선호 도메인)
   - `~/.wishket-radar/profile.yaml` 생성: 스킬마다 한국어+영어 동의어 keywords 자동 확장 (예: Rust → rust, 러스트, cargo, axum / Flutter → flutter, 플러터, dart, 모바일 앱).
   - 프로필은 수정 즉시 다음 스캔에 반영됨(서버 재시작 불필요)을 안내.

4. **베이스라인 스캔**:
   - `scan_new` MCP 도구 1회 실행 제안 (기본 development 3페이지).
   - 첫 실행 시 검색된 프로젝트 전체가 베이스라인 캐시(`~/.wishket-radar/state.json`)에 기록됨 — 이후 스캔부터는 신규 등록 공고만 보고됨.
   - 사용자가 스킵을 원하면 생략.

5. **완료 및 다음 행동 요령 안내**:
   - **신규 목록**: wishket-scan. 예: *"위시켓 새 프로젝트 있어?"*, *"새 외주 올라온 거 있나?"*
   - **심층 분석/리포트**: wishket-scout. 예: *"위시켓 분석해줘"*, *"스카우트 리포트"*
   - **조건별 실시간 검색**: wishket-search. 예: *"flutter 외주 찾아줘"*, *"파이썬 백엔드 검색해줘"*
   - **프로필/가중치 조정**: wishket-profile. 예: *"Rust 가중치 올려줘"*, *"FastAPI 키워드 추가"*
   - **공고 분류·현황 보기**: wishket-dashboard. 예: *"대시보드"*. 스캔된 공고를 인박스에서 관심/스킵으로 분류하면 관심 건만 파이프라인으로 넘어간다.
   - **포트폴리오 작성**: wishket-portfolio. 예: *"이 프로젝트로 포트폴리오 써줘"*. 공고와 무관한 재사용 자산이라 프로필과 함께 "내 정보"에 쌓인다.
   - **지원서·제안서**: wishket-apply. 예: *"이 공고 지원서 써줘"*. 견적은 wishket-quote, 단계 추적은 wishket-pipeline, 마감 알림은 wishket-deadline.
   - 호스트가 스케줄 기능을 제공하면 매일 아침 스캔 루틴 등록을 안내한다.
