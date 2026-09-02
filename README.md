# wishket-radar

<p align="center">
  <a href="https://github.com/epicsagas/wishket-radar/releases"><img alt="Version" src="https://img.shields.io/github/v/release/epicsagas/wishket-radar?style=for-the-badge&labelColor=0d1117&color=fc8d62&logo=github&logoColor=white" /></a>
  <a href="https://github.com/epicsagas/wishket-radar/actions"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/epicsagas/wishket-radar/ci.yml?style=for-the-badge&labelColor=0d1117&color=58a6ff&logo=github-actions&logoColor=white" /></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/license-Apache--2.0-3fb950?style=for-the-badge&labelColor=0d1117" /></a>
</p>

위시켓(wishket.com) 프로젝트를 검색·분석하고 내 기술 프로필과 매칭하는 멀티호스트 에이전트 플러그인 (Claude Code · Codex · Antigravity · Hermes).

- **Rust MCP 서버** (`wishket`): 위시켓 비공식 검색 API(리버스 엔지니어링) 호출, HTML/JSON-LD 파싱, 신규 diff 캐시, 결정론적 키워드 점수 계산
- **온보딩** `wishket-onboard`: 플러그인 설치 후 바이너리 점검부터 매칭 프로필까지 한 번에 설정
- **스킬** `wishket-profile` · `wishket-scan` · `wishket-search` · `wishket-scout`: 문장 요청 시 자동 실행 (슬래시 커맨드 불필요)
- **서브에이전트** `wishket-analyst`: 프로젝트 단건 심층 분석 (적합도 A/B/C). scout가 호출한다

## 설치

호스트에 플러그인을 설치한 뒤, 채팅에서 온보딩을 한 번 돌리면 MCP 바이너리 확인부터 매칭 프로필까지 끝난다. 프리빌트 바이너리는 온보딩이 못 찾거나 MCP만 따로 쓸 때의 선택 사항이다.

### Claude Code

```bash
claude plugin marketplace add epicsagas/wishket-radar
claude plugin install wishket-radar@wishket-radar -y
```

로컬 개발은 클론 디렉터리에서 `claude plugin marketplace add .` 후 동일하게 설치한다.

### Codex

```bash
codex plugin marketplace add epicsagas/wishket-radar
codex plugin add wishket-radar@wishket-radar
```

`.codex-plugin/plugin.json`과 `.codex-plugin/agents/*.toml`을 로드한다.

### agy (Antigravity)

```bash
agy plugin install https://github.com/epicsagas/wishket-radar
agy plugin enable wishket-radar
```

루트 `plugin.json`과 `mcp_config.json`을 인식한다. MCP 실행 파일은 `${CLAUDE_PLUGIN_ROOT}`(Claude), 현재 디렉터리(agy), PATH의 `wishket-mcp` 순으로 찾는다.

### Hermes

```bash
hermes plugins install https://github.com/epicsagas/wishket-radar
hermes plugins enable wishket-radar
```

루트 `plugin.yaml`과 `__init__.py`의 `register(ctx)`를 로드한다. skills_guard에 막히면 hermes 설정에서 `plugins.scan_on_install: false`로 둔다.

### 첫 설정 (온보딩)

플러그인을 켠 호스트 채팅에서 한 번 요청한다.

```text
위시켓 세팅해줘
```

`wishket-onboard`가 이어서 한다.

1. `wishket-mcp` 구동을 확인하고, 없으면 프리빌트(`install.sh` / `install.ps1`)를 깐다. `cargo`가 있어도 한방 설치가 먼저다. git 클론에서 한방 설치가 실패했을 때만 `cargo build --release`를 한다.
2. 주력 스택, 가중치, 역할, 근무 조건을 물어 `~/.wishket-radar/profile.yaml`을 만든다. 키워드 동의어(예: Rust → rust, 러스트, cargo, axum)도 채운다.
3. 원하면 `scan_new`로 베이스라인 스캔을 한 번 돌려 `~/.wishket-radar/state.json`에 현재 공고를 기록한다. 이후 스캔은 신규만 본다.

이미 프로필이 있으면 요약을 보여 주고 재설정 여부만 확인한다. 프로필은 저장소에 커밋하지 않는다.

### 프리빌트 바이너리 (선택)

온보딩이 바이너리를 못 찾거나, MCP만 호스트 없이 쓰거나, Rust 툴체인 없이 미리 깔고 싶을 때:

```bash
# macOS / Linux
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/epicsagas/wishket-radar/releases/latest/download/install.sh | sh
```

```powershell
# Windows (PowerShell)
irm https://github.com/epicsagas/wishket-radar/releases/latest/download/install.ps1 | iex
```

래퍼(`scripts/wishket-mcp`, Windows는 `scripts/wishket-mcp.cmd`)는 로컬 릴리스 바이너리, PATH의 `wishket-mcp`, 한방 설치 순으로 찾고, git 클론에서만 마지막에 `cargo build`를 한다. `cargo install wishket-mcp --git https://github.com/epicsagas/wishket-radar`도 된다. Windows에서 MCP `command: sh`가 없으면 인스톨러가 PATH에 넣은 바이너리를 쓴다.

## 사용

별도 슬래시 명령 없이 문장으로 요청하면 된다. 권장 순서는 온보딩 → 프로필 확인 → 일상 조회다.

| 순서 | 스킬 | 트리거 예시 | 동작 |
|---|---|---|---|
| 1 | `wishket-onboard` | "위시켓 세팅해줘", "온보딩" | 바이너리 점검, 프로필 생성, 베이스라인 스캔 |
| 2 | `wishket-profile` | "프로필 보여줘", "rust 가중치 올려" | `~/.wishket-radar/profile.yaml` 조회/편집. 다음 스캔에 바로 반영 |
| 3 | `wishket-scan` | "위시켓 스캔", "새 프로젝트 있어?" | 마지막 스캔 이후 신규만 diff |
| 4 | `wishket-search` | "위시켓 검색", "flutter 프로젝트 있어?", "외주 찾아줘" | 임시 검색 (캐시 기록 없음) |
| 5 | `wishket-scout` | "위시켓 분석", "스카우트", "리포트 뽑아줘" | 신규 스캔 + 상위 후보 심층 분석 + 리포트 |
| 6 | `wishket-portfolio` | "이 프로젝트로 포트폴리오 써줘" | 위시켓 포트폴리오 폼 초안 (일반 텍스트) |
| 7 | `wishket-apply` | "이 공고 지원서 써줘" | 제안서 작성 + 첨부 포트폴리오 추천. 견적은 `wishket-quote` |
| 8 | `wishket-pipeline` | "지원했어", "미팅 잡혔어", "지원 현황" | 위시켓 10단계 추적, 단계별 전환율·수주율 |
| 9 | `wishket-deadline` | "마감 캘린더에 넣어줘" | .ics 생성 → macOS/구글 캘린더 등록 |
| 10 | `wishket-feedback` | "수주율 높여줘" | 지원 결과 데이터로 프로필 가중치 보정 제안 |
| 11 | `wishket-dashboard` | "대시보드", "웹 UI 켜줘", "공고 분류할래" | 로컬 webui 실행 — 인박스 트리아지가 여기서 (아래 참고) |

온보딩을 건너뛰고 프로필만 손으로 만들 때는 예시를 복사한다.

```bash
mkdir -p ~/.wishket-radar
cp profile.example.yaml ~/.wishket-radar/profile.yaml
```

리포트는 `~/.wishket-radar/reports/`, seen 캐시는 `~/.wishket-radar/state.json`이다.

## 대시보드 (webui)

같은 바이너리의 `dashboard` 서브커맨드가 `~/.wishket-radar/` 상태 전체를 브라우저에서 보여준다. 채팅 스킬과 같은 파일을 공유하므로 어느 쪽에서 편집해도 즉시 반영된다.

```bash
scripts/wishket-mcp dashboard          # 기본 8787 포트, 브라우저 자동 오픈
scripts/wishket-mcp dashboard --port 8790 --no-open
```

- 첫 기동 시 랜덤 토큰을 `~/.wishket-radar/dashboard-token`(0600)에 생성하고 접속 URL을 출력한다. 폰 등 LAN 기기 접속을 위해 0.0.0.0에 바인드되며, 모든 요청은 토큰 인증을 통과한다.
- 기능: 인박스 트리아지(관심/스킵), 지원 퍼널·단계별 전환율·마감 D-day(대시보드), 단계 드롭다운 편집과 공고별 상세(파이프라인), profile.yaml·제안서·포트폴리오 직접 편집, 리포트 조회.
- 스카우트 리포트의 적합도 판정(A/B/C)·주의점·제안 방향이 인박스 카드에 자동으로 붙어 트리아지 판단 근거가 된다.
- 편집 저장은 원자 쓰기(tmp+rename) 후 이전 본문을 `.bak`으로 1세대 보관한다. profile.yaml은 저장 시 스키마 검증을 거친다.

## MCP 도구

| 도구 | 설명 |
|---|---|
| `scan_new` | 신규 diff 스캔 (기본 3페이지 30건, 요청 간 5초) |
| `search_projects` | 순수 검색 (category/form_factors/keyword/page/raw) |
| `get_project` | 상세 조회 (JSON-LD 전체 설명 포함) |
| `list_filters` | 필터 키 문서 (검증/미검증 구분) |
| `reset_cache` | seen 캐시 초기화 |

## 동작 원리

검색 필터는 `k=v&k=v` 문자열을 LZString(base64)으로 압축해 `?d=` 파라미터로 전송하고, `X-Requested-With` 헤더와 함께 호출하면 `{result, count}` JSON을 반환한다 (`server/src/wishket.rs` 참조). 상세 페이지는 schema.org JobPosting JSON-LD에서 전체 설명을 추출한다.

매칭은 2단계로 동작한다. 서버가 `WISHKET_PROFILE`(기본 `~/.wishket-radar/profile.yaml`) 키워드로 결정론적 점수(0~100)를 계산하고, LLM(analyst)이 공고 본문을 근거로 적합도 A/B/C를 판정한다.

## 디렉터리 구조 및 상태

```
~/.wishket-radar/
├── profile.yaml        # 매칭 프로필 (기술 스택, 가중치, 역할 등)
├── state.json          # seen 프로젝트 + 인박스 트리아지 + 상세 캐시 (90일 후 정리)
├── applications.yaml   # 지원 파이프라인 (wishket-pipeline/webui)
├── dashboard-token     # webui 접근 토큰 (자동 생성, 0600)
├── reports/            # 스캔 리포트 (한국어 markdown)
├── proposals/          # 지원서·제안서 초안
├── portfolios/         # 포트폴리오 폼 초안
└── deadlines/          # 마감 .ics
```

## 개발

```bash
cargo test --manifest-path server/Cargo.toml    # LZString 왕복·파서 단위 테스트
```

위시켓 마크업이 바뀌면 `server/src/wishket.rs`의 셀렉터와 테스트 픽스처를 함께 갱신한다.

## 주의 (법적 고지 포함, 사용 전 필독)

- **위시켓 서비스약관 제10조**는 "프로젝트의 정보 및 파트너의 정보를 수집하기 위해 크롤링을 하는 행위"를 회원 의무 위반으로 금지하고, 같은 조에서 리버스 엔지니어링 금지를 명시한다. 제재는 서면경고, 이용 제한, 영구 정지 순으로 강화되며, 약관은 비회원을 이유로 한 면책 주장도 배제한다. 위시켓 회원(파트너) 계정으로 사용하면 계정 제재 위험이 있다. **본 플러그인은 개인적 검토 목적의 소량 조회 용도이며, 사용 책임은 이용자에게 있다.**
- robots.txt는 `/project/` 크롤링을 허용(사이트맵 제공)하되 `Crawl-delay: 5`를 요구한다. 서버는 검색·상세를 포함한 모든 HTTP 요청 사이에 5초를 둔다.
- 요청 UA는 `wishket-radar/<버전> (+repo)`로 정체성을 밝힌다. 로그인·인증 우회 없이 공개 페이지만 조회하며, 회원 전용 영역(`/partners/`, `/media/` 등 robots.txt 비허용 경로)은 호출하지 않는다.
- 비공식 API 기반이므로 위시켓 측 변경에 깨질 수 있다 (SSR 폴백 내장).
- 과도한 스캔 금지. 수집 데이터는 seen 캐시(90일)와 로컬 리포트뿐이며 재배포하지 않는다.

## 기여 (Contributing)

기여 가이드 및 개발 환경 설정은 [CONTRIBUTING.md](CONTRIBUTING.md)를 참고해 주세요. 버그 제보와 기능 제안은 이슈를 통해 환영합니다.

## 라이선스 (License)

[Apache-2.0](LICENSE) © 2026 epicsagas
