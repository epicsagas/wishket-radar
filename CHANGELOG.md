# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.0] - 2026-09-05

### Added
- **제안서 AI 초안** — 파이프라인 상세·제안서 페이지의 "AI 초안 생성". 공고 캐시+활성 프로필(+기존 초안)을 `agents/wishket-proposal.md` 프롬프트(단일 소스)로 돌려 `proposals/<공고ID>/draft.md`에 저장. 기존 편집본은 `atomic_write`의 `.bak` 1세대 보존, `form.txt`는 불변. 재생성 시 기존 초안이 맥락으로 들어가 전면 재작성 대신 보강한다.
- **키워드 가중치 AI 보정** — 설정 화면 "AI 보정" 버튼. 관심·스킵 트리아지 분포와 스킬 출현 통계를 근거로 조정안(스킬·현재→제안·근거)을 JSON으로 제시. 모델 출력은 범위(1~5) clamp와 파싱 방어(불량 JSON 502). "적용"은 양식에 반영만 하고, 저장 전까지 프로필은 불변.
- **다중 프로필 프리셋** — 설정 화면에서 프리셋 생성(현재 복제)·전환·삭제. `settings.profile_presets`+`profile_active`로 저장하고 `load_profile_yaml`은 활성 프리셋→기존 단일 키→파일 순으로 폴백해 기존 배포 무수정 동작. 프리셋 도입 후 저장은 활성 프리셋에만 반영. 활성·마지막 프리셋 삭제는 400.

### Fixed
- (v0.6 이후 보강) AI 대화가 대시보드 데이터(프로필·공고 캐시 요약·파이프라인 현황)를 보지 못하던 문제 — 모든 대화에 컨텍스트 주입.
- 플로팅 채팅이 마지막 대화를 기억하지 않던 문제 — localStorage 이어가기 + 삭제된 대화 폴백.
- AI 채팅 응답 마크다운 렌더링 — 서버 렌더·sanitize 재사용, 스트리밍 중 plain.

## [0.6.0] - 2026-09-05
- **AI 대화가 대시보드 데이터를 보지 못하던 문제** — 공고에 연결된 대화에만 컨텍스트를 주입해, 일반 대화("내 프로필에 맞는 공고 top 3")가 "공고에 명시 없음"으로 답했다. 이제 모든 대화에 기술 프로필·공고 캐시 요약(적합도순 상위 60건)·지원 파이프라인 현황(단계별 집계+항목)을 주입하고, 공고 연결 대화는 상세까지 유지. 공고 언급은 [공고ID] 제목 형태로 근거 표기.

## [0.6.0] - 2026-09-05

### Added
- **대화 화면** — `#/chats`: 대화 목록(연결 공고 배지·토큰 합계·최신순), 메시지 스트림, 입력창. 공고 상세(인박스·파이프라인)의 "AI 대화" 버튼으로 해당 공고를 맥락으로 시작하면 목록에 공고 제목으로 태그.
- **플로팅 채팅** — 전 페이지 우하단 플로팅 버튼 → 미니 모달로 즉시 질문. 모달 헤더의 확대 버튼으로 대화 화면 전환(#/chats/{id}). 두 곳이 같은 `ChatPanel` 컴포넌트를 재사용.
- **히스토리 영속 UI** — 대화·메시지는 v0.5 SQLite 스키마 그대로. 목록에서 재진입하면 이전 맥락 그대로 이어가고(서버가 전체 메시지 배열 재전송), 채팅 헤더에 누적 토큰(입력/출력) 표시 — v0.5에서 v0.6으로 연기된 화면 표시.
- **대화 삭제** — `DELETE /api/ai/conversations/{id}`(메시지 함께 삭제, 미존재 404) + 목록에서 2단 확인 후 삭제. 공고 캐시에서 사라진 대화는 "삭제된 공고" 배지로 유지 표시.
- **모바일 레이아웃** — 플로팅 모달은 좁은 뷰포트에서 전체화면이 기본, FAB와의 겹침 방지.

## [0.5.0] - 2026-09-04

### Added
- **BYOK AI 설정** — 내 정보 탭에 "AI 설정" 섹션. 공급자(Anthropic/OpenAI/호환 엔드포인트), API 키, 모델, 온도를 SQLite `settings`(`ai_config`)에 저장. 키는 API 응답에서 마스킹(`sk-***xxxx`)되며, 마스킹 값이나 빈 키로 저장하면 기존 키가 보존된다.
- **서버 AI 프록시** — `POST /api/ai/chat`: 키가 브라우저 JS에 닿지 않게 서버가 공급자 API를 호출하는 얇은 프록시. SSE 스트림을 원문 그대로 중계하면서 usage·어시스턴트 텍스트를 곁들여 수집. 공급자 무응답 120초·연결 10초 타임아웃, 429 등 오류는 상태코드+본문 그대로 전달. `base_url`은 게이트웨이 오버라이드로 모든 공급자에서 존중.
- **AI 평가** — 인박스 상세·파이프라인 상세의 "AI 평가" 버튼. 캐시된 공고 본문+프로필로 `agents/wishket-analyst.md` 프롬프트(단일 소스)를 실행하고 5줄 출력을 파싱해 리포트 파서 포맷 그대로 `reports/ai-eval.md`에 기록. 기존 파서·배지 파이프라인 무수정 재사용, 재평가 시 뒤 항목이 이긴다.
- **대화(컨텍스트) 연동** — `conversations`/`messages` 테이블 활성화. 대화 생성 시 공고 연결(project_id)하면 본문·조건·매칭+프로필이 시스템 컨텍스트로 주입되고 전체 메시지 배열이 공급자에 재전송된다. `POST /api/ai/conversations`, `GET /api/ai/conversations[/{id}]`.
- **토큰 usage 누적** — `user_version=2` 이관으로 conversations에 `tokens_in`/`tokens_out` 추가(v1 db는 기동 시 자동 ALTER, 행 보존). 공급자 응답 usage(anthropic/openai 명칭 모두)를 대화별 누적, 대화 API로 노출. 화면 표시는 v0.6 채팅 UI와 함께.

### Security
- API 키는 로컬 SQLite에만 저장(파일 0600 선례), 응답 JSON·로그에 평문 재노출 없음. 외부 전송은 사용자가 지정한 공급자 API 호출뿐.
- **리포트 렌더 sanitize (ammonia)** — 공고 본문(3자 작성)이 모델 출력 경유로 리포트에 흐르면서 기존 "전부 본인 파일" 전제가 깨졌다. raw HTML·`javascript:` 링크·원격 `<img>`(비콘)를 제거한다.
- **키 파일 권한 0600 고정** — 기동 시 `umask 0o077` + 연결마다 재적용 + 주간 스냅샷 chmod 후 rename. WAL 재생성·스냅샷으로 권한이 풀리는 경로 제거.
- **공급자 리다이렉트 차단** — 기본 정책은 커스텀 인증 헤더(`x-api-key`)를 리다이렉트 대상 호스트로 그대로 전송한다. `Policy::none`으로 끊었다.

### Fixed
- 공급자 실패로 응답 없는 user 턴이 남으면 다음 요청이 `[user, user]` 배열로 400을 받는 문제 — 연속 same-role 병합과 history 40개 상한으로 방어.
- 스키마 마이그레이션이 ALTER 실패를 무시하고 `user_version=2`를 찍어 conversations 쿼리가 영구히 깨질 수 있던 문제 — 컬럼 실존 확인 후에만 버전 기록.

## [0.4.0] - 2026-09-04

### Added
- **SQLite 저장 계층 (WAL)** — `state.json` 단일 파일을 `state.db`(SQLite, WAL 모드)로 이전. 대시보드 3초 폴링과 쓰기가 겹쳐도 읽기가 막히지 않는다(`busy_timeout=5000`, `synchronous=NORMAL`). 스키마: `seen`·`applications`·`settings` + v0.5 대화용 `conversations`·`messages` 선반영(`user_version=1`).
- **자동 이관** — 첫 기동 시 `state.json`·`applications.yaml`·`profile.yaml`을 흡수하고 원본을 `*.migrated`로 보존(matches.md 이관 선례 준용). 깨진 파일은 흡수하지 않고 원본 그대로 둔다. 롤백 경로: `*.migrated` 파일을 원래 이름으로 되돌리고 `state.db`를 삭제.
- **주간 백업** — 최신 스냅샷이 7일 경과 시 `VACUUM INTO`로 `backups/state-YYYYMMDD.db` 생성, 4세대 유지. 실패해도 앱 동작에는 영향 없음.
- **저장 계층 추상화** — `StateStore` trait(JSON/SQLite 백엔드) 도입. 대시보드는 `AppState`의 state_dir로 스코프를 한정(`load_in`/`save_in`)해 테스트가 임시 디렉터리에 닫힌다.
- **프로필 해석 순서** — `$WISHKET_PROFILE`(명시 오버라이드) > SQLite(`settings.profile_yaml`) > 기존 파일 탐색 체인. 대시보드 프로필 편집은 DB에 기록. DB가 정규 소스면 `profile_external` 힌트는 노출하지 않는다.

### Changed
- `reset_cache`가 `state.db`(+WAL 잔재)를 삭제한다. 응답 스키마(`cleared`/`path`) 유지.
- 모든 API(`/api/state` 등) 응답 스키마 불변 — webui 무수정.

## [0.3.3] - 2026-09-04

### Added
- 사이드바 브랜드 영역에 GitHub Star 링크 버튼 — 클릭 시 리포지토리를 새 탭으로 열고, 호버 시 별 아이콘 강조. 브랜드 제목은 클릭 시 인박스로 이동.

### Changed
- 사이드바 폭 232px→264px — Star 버튼 추가 후 좁은 폭에서 제목과 버튼이 겹치던 문제. 제목은 극단적으로 좁아질 때만 말줄임 처리.

## [0.3.2] - 2026-09-04

### Fixed
- **상세 갱신이 캐시를 지우던 치명 버그** — 위시켓이 차단·삭제 공고에 사이트 태그라인만 든 셸 페이지를 반환할 때, 파싱 결과가 텅 빈 채로 기존 본문·조건·역할·매칭점수를 통째로 덮어써 삭제했다. 이제 파싱 결과가 비면 실패(502)로 돌려 캐시를 유지하고, 부분 파싱은 값 있는 필드만 갱신한다.

### Changed
- 인박스 상세의 관심/스킵 버튼을 하단 패널에서 제목 행 우측으로 이동. 모바일에서는 제목·배지·버튼이 세로로 스택되어 화면 넘침 해소.
- 인박스 리스트에서 매칭점수가 0이거나 상세 캐시가 없는 카드에 "점수·상세 갱신" 버튼 추가 — 리스트에서 바로 위시켓 상세를 가져와 매칭 점수를 재계산한다.

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
