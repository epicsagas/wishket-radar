# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.1] - 2026-09-04

### Added
- 예산·기간 수치 지표 기록 — 카드/상세의 원문("예상 금액 1,000,000원", "1,500만~2,000만원", "월 금액 X원 /월", baseSalary 수치형)에서 월/총액(원)·기간(일)·**일 단위 금액**을 계산해 `state.json`에 저장한다. 월 금액은 ÷30, 총액은 ÷기간. 세션 재시작·다른 머신에서도 동일 표시. 파이프라인 상세 좌측 요약에 예산·예상 기간 다음 "일 단위" 행(계산식 병기) 추가. 상세 조회 시 예산이 비어 있으면 JSON-LD baseSalary로 보강, 구 state 항목은 첫 `/api/state` 조회 때 1회 셀프힐.

### Fixed
- 프라이빗 매칭 오탐 — 상세 페이지의 "모집 중인 다른 프로젝트" 추천 카드에 프라이빗 배지가 있으면 현재 공고까지 비공개로 표시되던 문제. 추천 섹션 내부의 배지는 무시한다.
- 상세 조회 시 스캔 카드가 준 제목을 사이트 태그라인("대한민국 대표 IT 프로젝트 플랫폼")으로 덮어쓰던 문제 — 제목은 이제 비어 있을 때만 채우고, 태그라인은 제목 후보에서 제외한다.

## [0.3.0] - 2026-09-04

### Added
- 스카우트 AI 평가 점수·평가 모델 기록 — `wishket-analyst`가 등급과 함께 0-100 적합도 점수를 반환(A=80-100, B=50-79, C=0-49 밴드 고정으로 변동 억제). 리포트에 `- AI 평가: {점수}점 · 모델: {model}` 라인이 남고, 모델명은 디스패처(세션)가 스탬프한다(analyst는 `model: inherit`이라 자가보고 대신). 대시보드 파서가 점수·모델을 독립 추출해 인박스 카드·상세의 등급·매칭 배지 다음에 `AI {점수}` 배지(툴팁에 모델명)로 표시. 해당 라인 없는 기존 리포트도 그대로 파싱된다.
- 지원 파이프라인 종결 건 기본 숨김 — 단계를 종결 상태(완료·미체결·탈락·철회)로 바꾸면 표·칸반에서 즉시 사라진다. 툴바 "종결 N건" 토글로 다시 표시. 수주율·퍼널 통계는 전체 건 기준을 유지한다.

### Fixed
- applications.yaml에 직접 등록된 공고(챗 스킬 `wishket-pipeline` 경로)가 인박스에도 계속 노출되던 중복 — 인박스는 최초 단계이므로 파이프라인(yaml)에 있는 공고는 미분류여도 인박스 목록·카운트에서 제외한다.

## [0.2.3] - 2026-09-03

### Fixed
- 파이프라인 표 모바일 가로 스크롤이 표 내부가 아니라 페이지 전체를 밀던 문제 — grid item(`main`)에 `min-width: 0` 누락으로 nav까지 잘려 보이던 레이아웃 붕괴 수정.

## [0.2.2] - 2026-09-03

### Changed
- 대시보드 스탯 카드 재구성 — "진행 중"(관심~미팅 합산) 카드를 파이프라인 순서대로 분리: 관심 / 지원 중(지원·상담·미팅) / 미팅 / 체결·진행 / 완료. 지원한 공고와 검토 중 공고를 따로 모니터링 가능.
- 대시보드 카드 데스크탑 4열 고정, 모바일은 기존 auto-fit 유지.
- 파이프라인 표 뷰 모바일 가로 스크롤 지원.

## [0.2.1] - 2026-09-02

### Added
- 프라이빗 매칭 표시 — PRIME·PRO·BOOST 파트너 전용 공고를 홈페이지에 가지 않고 인박스 리스트·인박스 상세·파이프라인 상세에서 바로 확인(주황 배지, 툴팁에 안내 문구). `state.json` seen 항목에 `private_matching` 저장.
- 예상 금액·예상 기간 표시 보강 — 파이프라인 상세 좌측 요약 패널에 예상 기간 행 추가, 예산·기간이 상세 캐시 없어도 카드 정보만으로 표시.

### Fixed
- 구 버전이 스캔한 인박스 항목의 예산·기간·마감·스킬이 영구히 비어 있던 문제 — 재스캔에서 다시 만나면 카드 정보로 백필(기존 값은 덮지 않음).

## [0.2.0] - 2026-09-02

### Added
- `wishket-mcp dashboard` 서브커맨드 — `~/.wishket-radar/` 상태(프로필·매치 리스트·지원 파이프라인·제안서·포트폴리오·리포트·마감)를 브라우저에서 보고 편집하는 로컬 webui. 0.0.0.0 바인드 + 랜덤 토큰 인증(`~/.wishket-radar/dashboard-token`, 0600). 프론트엔드(Svelte 5 + TS + vite)는 rust-embed로 바이너리에 임베드.
- 프레이밍 스킬 5종: `wishket-apply`(지원서·제안서 + 첨부 포트폴리오 추천), `wishket-pipeline`(applications.yaml 지원 추적·수주율 퍼널), `wishket-feedback`(지원 결과 기반 profile.yaml 가중치 보정 제안), `wishket-deadline`(RFC 5545 .ics 마감 캘린더 등록), `wishket-quote`(기능 분해 휴리스틱 견적 3안), `wishket-portfolio`(위시켓 포트폴리오 폼 초안), `wishket-dashboard`(webui 실행).
- dashboard API: applications.yaml 상태 PATCH(10단계 검증), matches.md·profile.yaml 원문 편집(Profile 파싱 검증 + 빈 프로필 덮어쓰기 거부), reports/proposals/portfolios/deadlines 파일 조회·편집. 전 쓰기 원자(tmp+rename) + 1뎁스 `.bak`, 경로 순회 차단.

- 위시켓 실제 수주 퍼널 10단계(관심·지원·상담·미팅·체결·진행 중·완료·미체결·탈락·철회)와 단계별 전환율. 수주율 = 체결 이상 / (체결 이상 + 미체결 + 탈락).
- 인박스 트리아지 — 스캔 결과가 `state.json`에 카드 정보(URL·점수·예산·기간·마감·스킬)와 함께 쌓이고, 대시보드 인박스에서 관심/스킵을 고르면 관심 항목만 파이프라인에 진입한다. `GET /api/inbox`, `POST /api/inbox/{id}/triage`.
- 인박스 공고 상세 `#/inbox/{id}` — "상세 불러오기"를 누를 때만 위시켓에서 설명·조건·매칭 기술을 가져오고 **본문까지 `state.json`에 캐시**한다. 두 번째부터는 네트워크 없이 즉시 렌더(1.0초 → 0.01초), 상단에 조회 시각과 갱신 버튼 표시. 자동 조회는 하지 않는다(robots Crawl-delay 준수). 판단 후 그 자리에서 관심/스킵.
- 캐시된 공고 본문을 파이프라인 상세(`#/pipeline/{id}`)에서도 재조회 없이 열람.
- 파이프라인 항목 상세 페이지 `#/pipeline/{id}` — 각 항목이 고유 URL을 가지며 단계·메모·다음 할 일을 한 화면에서 편집.
- 스카우트 리포트(`reports/*.md`)의 LLM 분석을 인박스·상세에 주입 — 적합도 등급(A/B/C), 적합도 판단, 주의점, 제안 방향을 공고 id에 붙인다. 기계 점수로는 못 내는 정보라 트리아지 판단 근거가 된다. 인박스는 등급 우선 정렬 + "리포트 분석만" 필터.

- 파이프라인 상세 ↔ 제안서 양방향 연결 — 상세의 "관련 문서"에 해당 공고의 제안서·포트폴리오를 나열하고, 누르면 제안서 화면에서 그 파일이 바로 열린다(`#/proposals?file=…&root=…`).
- 파이프라인 상세에서 공고 본문을 직접 불러오기 — 인박스를 거치지 않고 들어온 항목(예: matches.md 마이그레이션 건)은 캐시가 비어 있어 설명이 표시되지 않았다. 이제 그 자리에서 "공고 상세 불러오기"를 누를 수 있다.
- 프로필 구조화 편집 — 이름·한 줄 소개·기술(이름/가중치/키워드)·역할·메모를 각각의 입력 폼으로 나눠, YAML 문법을 몰라도 항목을 추가·수정·삭제·정렬할 수 있다. 서버가 구조체를 직렬화해 쓰므로 포맷이 깨지지 않는다(`PUT /api/profile/structured`, 빈 기술명·기술 0개는 거부). 원문 주석이 있으면 사라진다고 경고하며, YAML 탭에서 직접 편집도 가능.
- 제안서·프로필·포트폴리오 기본 뷰를 읽기 모드로 — 마크다운은 렌더링, 그 외는 원문 표시. "편집"을 눌러야 편집기가 열린다.
- 편집기와 목록이 화면 아래까지 채워지도록 높이 조정(`calc(100vh - …)`), 편집 영역은 내부 스크롤.
- 포트폴리오를 제안서 화면에서 분리해 "내 정보"(프로필) 탭으로 이동. 포트폴리오는 공고와 무관하게 축적·재사용하는 자산이라 프로필과 같은 층위이고, 제안서는 공고에 종속된 산출물이다. 파이프라인 상세의 "관련 문서"도 제안서만 보여준다.
- 제안서를 `proposals/<공고ID>/` 디렉터리로 분리. 소유자는 디렉터리가 정하며 파일명은 해석하지 않는다 — 파일명에서 ID를 추측하던 방식은 `2026-09-02-…`의 `202609` 같은 숫자에 걸릴 수 있었다. 루트에 남은 파일은 "기타"로 표시된다. 기존 파일은 자동 이전하지 않는다.
- 제안서를 공고별로 그룹핑 — 파일명에서 위시켓 공고 번호를 뽑아 묶고, 파이프라인의 공고 제목과 상세 링크를 함께 보여준다. 평면 나열이던 목록이 공고 단위로 정리된다.
- 대시보드 라이트/다크 테마. 사이드바 하단에서 버전과 같은 줄 맨 오른쪽 아이콘으로 전환, 선택은 브라우저에 저장.

### Removed
- `matches.md` 연동 제거 — 생성하는 스킬이 없는 수기 파일이었고, 인박스 트리아지(관심 표시)와 리포트 분석이 그 역할을 모두 대신한다. 파싱 코드·전용 메뉴·`/api/matches` 삭제. **기존 `matches.md`는 대시보드 첫 기동 시 표의 프로젝트 링크를 인박스 "관심"으로 1회 이관하고 `matches.md.migrated`로 이름을 바꿔 큐레이션이 유실되지 않게 한다.**

### Changed
- 파이프라인 소스가 applications.yaml + 인박스 관심 둘로 정리(applications.yaml 우선).
- 기본 진입 화면이 인박스로 변경.
- 구 상태명(검토중/면담/수주/거절)은 읽을 때 신규 단계로 자동 마이그레이션된다.
- `scripts/wishket-mcp` 런처가 하위 커맨드 인자를 바이너리로 전달(기존엔 인자 버림 — `dashboard` 실행 불가 문제의 근원).
- 릴리스 CI에 webui 빌드 job 추가(node 22 1회 빌드 → artifact → 전 cargo-dist 타깃에 다운로드).
- 스킬·analyst 문서를 영문으로 전환. 트리거 한국어와 위시켓 단계명은 유지.
- 제안서 산출물을 `draft.md`(대시보드 검토, 마크다운)와 `form.txt`(위시켓 폼 붙여넣기, 플레인 텍스트) 두 파일로 고정. `submit.md`는 만들지 않는다.

### Fixed
- 포트폴리오 탭으로 전환할 때 이전 탭의 선택이 남아 `404: portfolio-entries.md 없음`이 뜨던 버그. 탭 전환 시 선택을 비우고, 목록이 비면 안내 문구를 보여준다.
- applications.yaml로 승격된 뒤 상세를 불러와 매칭 점수·마감이 생겨도, yaml에 없으면 화면에 영영 표시되지 않던 문제. 비어 있는 표시용 필드(점수·마감·URL·제목)는 인박스 캐시 값으로 보강한다(상태·메모 등 사용자 입력은 yaml이 우선).
- 마감된 공고의 상세 페이지에 JobPosting JSON-LD가 없어 제목·설명이 빈 채로 저장되던 버그. `h1` → `og:title` → `<title>` 순으로 폴백한다. 이미 저장된 빈 제목은 목록에서 `(제목 없음 · 공고 N)`으로 표시된다.
- 카드 목록의 마감이 "마감 2주 2일 전" 같은 상대 표기라 D-day 계산·필터링이 불가능하던 문제. 스캔 시각 기준 날짜로 환산한다(주/일/개월/시간, 월·연·윤년 경계 처리).
- 마감이 지난 공고가 인박스에 계속 남던 문제. 기본으로 숨기고 "마감 지남 N" 토글로 볼 수 있다.
- 해시 라우터가 쿼리 문자열을 경로로 오인해 `#/proposals?file=…` 진입 시 인박스로 떨어지던 문제.
- 인박스에서 제목을 눌러도 상세로 가지 않고, 관심/스킵만 누를 수 있어 판단 근거 없이 분류해야 하던 문제. 제목은 상세로, 원문 링크는 별도 ↗ 아이콘으로 분리.
- 모든 외부 링크(카드·표·렌더된 마크다운 본문 포함)가 새 탭에서 열리도록 `target="_blank" rel="noopener noreferrer"` 적용. SPA라 같은 탭 이동은 상태를 날린다.
- 상세 페이지에 카드 DOM이 없어 마감일이 비던 문제 — 조건 행("2026년 09월 08일마감 …")에서 파싱해 보강.
- 정적 에셋 캐시 헤더 — `index.html`은 `no-cache, must-revalidate`, 해시 파일명인 `/assets/*`는 1년 immutable. 바이너리를 갱신했는데 브라우저가 옛 `index.html`을 들고 있어 사라진 에셋을 가리키던 문제.
- 대시보드 토큰 우선순위를 쿼리 > 쿠키로 교정. 쿠키는 포트를 구분하지 않아, 다른 포트로 띄운 대시보드에서 이전 쿠키가 URL 토큰을 덮어써 401이 나던 문제.

## [0.1.2] - 2026-09-01

### Fixed
- 상세 페이지(get_project)에서 `private_matching`이 누락되던 버그. 상세 페이지엔 카드 DOM(`project-info-box`)이 없어 `parse_cards`가 폴백되는데, 뱃지를 문서 레벨(`div.status-mark.private-mark`)에서 재확인하도록 수정.

## [0.1.1] - 2026-09-01

### Added
- `ProjectCard.private_matching` — 프라이빗 매칭(부스트 파트너 전용) 뱃지 여부. scan/search/get_project 모든 카드 출력에 노출.

### Changed
- MCP 래퍼가 설치된 바이너리 버전을 플러그인 매니페스트와 비교: 낮으면 최신 릴리즈에서 자동 갱신(install.sh), 갱신 실패 시 기존 바이너리로 폴백. 플러그인 업데이트가 릴리즈 바이너리까지 따라가도록 연결.
- README 설치 순서를 호스트 4종 → 온보딩 → 프리빌트(선택)로 바꾸고, 사용 표를 onboard → profile → scan/search/scout 순으로 맞춤.
- MCP 래퍼와 onboard는 한방 설치(`install.sh` / `install.ps1`)를 기본 폴백으로 쓰고, `cargo build`는 git 클론에서 설치가 실패했을 때만 실행한다.

## [0.1.0] - 2026-09-01

### Added
- Multi-host agent plugin integration for Claude Code, Codex, agy (Antigravity), and Hermes.
- High-performance Rust MCP server (`wishket`) implementing reverse-engineered Wishket search API, HTML/JSON-LD parser, and deterministic keyword matching.
- Orchestration skills: `wishket-scout`, `wishket-scan`, `wishket-search`, `wishket-profile`, `wishket-onboard`.
- Specialized subagent: `wishket-analyst` for deep single-project fit analysis.
- Multi-platform prebuilt binaries distribution via `cargo-dist` (macOS arm64/x86_64, Linux arm64/x86_64, Windows x64).
- One-line installer scripts (`install.sh`, `install.ps1`).
- Unified runtime and profile storage under `~/.wishket-radar/`.

### Changed
- Plugin manifests declare Apache-2.0 to match LICENSE.
- wishket-scan vs wishket-scout trigger phrases no longer overlap.
- `scan_new` / `search_projects` default to `development` and `web,pc,android,ios` on the server when omitted.
- Crawl-delay (5s) applies to every Wishket HTTP call, including `get_project`.
- English profile keywords match on word boundaries (`go` no longer hits `ongoing`).
- Hermes `provides_skills` lists all five skills.

### Fixed
- Profile and state directories fall back to `USERPROFILE` when `HOME` is unset (Windows).
- Search HTTP error bodies are not parsed as empty SSR pages.
- `wishket-mcp --version` and `--help` print instead of opening stdio MCP.
