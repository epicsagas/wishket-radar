# wishket-radar

위시켓(wishket.com) 프로젝트를 검색·분석하고 내 기술 프로필과 매칭하는 멀티호스트 에이전트 플러그인 (Claude Code · Codex · Antigravity · Hermes).

- **Rust MCP 서버** (`wishket`): 위시켓 비공식 검색 API(리버스 엔지니어링) 호출, HTML/JSON-LD 파싱, 신규 diff 캐시, 결정론적 키워드 점수 계산
- **스킬** `wishket-scout`: 스캔부터 후보 선별, 병렬 분석, 한국어 리포트 작성까지 전 과정을 조율
- **서브에이전트** `wishket-analyst`: 프로젝트 단건 심층 분석 (적합도 A/B/C)
- **스킬** `wishket-scan` · `wishket-search` · `wishket-profile` · `wishket-onboard`: 문장 요청 시 자동 실행 (슬래시 커맨드 불필요)

## 설치

### 프리빌트 바이너리 설치 (Rust 툴체인 불필요)

릴리스에서 플랫폼별 프리빌트 바이너리를 제공한다. MCP 서버만 따로 쓰거나 Rust 툴체인 없이 플러그인을 쓰려면 먼저 실행:

```bash
# macOS / Linux
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/epicsagas/wishket-radar/releases/latest/download/install.sh | sh
```

```powershell
# Windows (PowerShell)
irm https://github.com/epicsagas/wishket-radar/releases/latest/download/install.ps1 | iex
```

설치된 `wishket-mcp` 바이너리는 플러그인 래퍼(`scripts/wishket-mcp`)가 자동으로 찾는다. 이후 플러그인 설치 과정만 남는다. 소스에서 직접 설치할 때는 `cargo install wishket-mcp --git https://github.com/epicsagas/wishket-radar`도 가능하다.

Rust 툴체인이 있다면 프리빌트 없이도 동작한다. 래퍼가 첫 실행 시 `cargo build --release`를 자동으로 수행하며, 첫 MCP 기동에 1분 가량 걸린다.

### Claude Code

```bash
claude plugin marketplace add epicsagas/wishket-radar
claude plugin install wishket-radar@wishket-radar -y
```

로컬(비공개 개발)은 클론 디렉터리에서 `claude plugin marketplace add .`를 실행한 뒤 동일하게 설치한다. 설치 캐시(`~/.claude/plugins/cache/wishket-radar/wishket-radar/<버전>/`)에 바이너리가 없으므로 캐시 디렉터리에서 한 번 더 빌드하거나 래퍼 자동 빌드에 맡긴다.

### Codex

```bash
codex plugin marketplace add epicsagas/wishket-radar
codex plugin add wishket-radar@wishket-radar
```

`.codex-plugin/plugin.json`(interface 블록)과 `.codex-plugin/agents/*.toml`을 로드한다.

### agy (Antigravity)

```bash
agy plugin install https://github.com/epicsagas/wishket-radar
agy plugin enable wishket-radar
```

루트의 `plugin.json`과 `mcp_config.json`을 자동으로 인식한다. `mcp_config.json`의 명령 경로는 호스트와 무관하게 자동으로 해석되며, `${CLAUDE_PLUGIN_ROOT}`(Claude), 현재 디렉터리(agy), PATH의 `wishket-mcp`(인스톨러 설치본) 순서로 찾는다.

### Hermes

```bash
hermes plugins install https://github.com/epicsagas/wishket-radar
hermes plugins enable wishket-radar
```

`plugin.yaml`(루트)과 `__init__.py`의 `register(ctx)`를 로드한다. skills_guard에 막히면 hermes 설정에서 `plugins.scan_on_install: false`로 지정한다.

## 사용

각 스킬은 별도 명령 없이 문장으로 요청하면 자동으로 실행된다.

| 스킬 | 트리거 예시 | 동작 |
|---|---|---|
| `wishket-scan` | "위시켓 스캔", "새 프로젝트 있어?" | 마지막 스캔 이후 신규만 diff 조회 |
| `wishket-search` | "위시켓 검색", "flutter 프로젝트 있어?" | 임시 검색 (캐시 기록 없음) |
| `wishket-onboard` | "위시켓 세팅해줘" | 온보딩: 인터뷰로 프로필 생성 후 베이스라인 스캔 |
| `wishket-profile` | "프로필 보여줘", "rust 가중치 올려" | `~/.wishket/profile.yaml` 조회/편집 |

리포트: `~/.wishket-radar/reports/` · 캐시: `~/.wishket-radar/state.json`

매칭 프로필은 저장소의 `profile.example.yaml`을 복사해 만든다. 실제 프로필은 gitignore되며 커밋하지 않는다.

```bash
mkdir -p ~/.wishket
cp profile.example.yaml ~/.wishket/profile.yaml
```

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

매칭은 2단계로 동작한다. 서버가 `WISHKET_PROFILE`(기본 `~/.wishket/profile.yaml`) 키워드로 결정론적 점수(0~100)를 계산하고, LLM(analyst)이 공고 본문을 근거로 적합도 A/B/C를 판정한다.

## 상태

```
~/.wishket-radar/
├── state.json    # seen 프로젝트 ID (90일 후 자동 정리)
└── reports/      # 스캔 리포트 (한국어 markdown)
```

## 개발

```bash
cargo test --manifest-path server/Cargo.toml    # LZString 왕복·파서 단위 테스트
```

위시켓 마크업이 바뀌면 `server/src/wishket.rs`의 셀렉터와 테스트 픽스처를 함께 갱신한다.

## 주의 (법적 고지 포함, 사용 전 필독)

- **위시켓 서비스약관 제10조**는 "프로젝트의 정보 및 파트너의 정보를 수집하기 위해 크롤링을 하는 행위"를 회원 의무 위반으로 금지하고, 같은 조에서 리버스 엔지니어링 금지를 명시한다. 제재는 서면경고, 이용 제한, 영구 정지 순으로 강화되며, 약관은 비회원을 이유로 한 면책 주장도 배제한다. 위시켓 회원(파트너) 계정으로 사용하면 계정 제재 위험이 있다. **본 플러그인은 개인적 검토 목적의 소량 조회 용도이며, 사용 책임은 이용자에게 있다.**
- robots.txt는 `/project/` 크롤링을 허용(사이트맵 제공)하되 `Crawl-delay: 5`를 요구한다. 서버는 이를 준수한다 (요청 간 5초 지연, 순차 요청).
- 요청 UA는 `wishket-radar/<버전> (+repo)`로 정체성을 밝힌다. 로그인·인증 우회 없이 공개 페이지만 조회하며, 회원 전용 영역(`/partners/`, `/media/` 등 robots.txt 비허용 경로)은 호출하지 않는다.
- 비공식 API 기반이므로 위시켓 측 변경에 깨질 수 있다 (SSR 폴백 내장).
- 과도한 스캔 금지. 수집 데이터는 seen 캐시(90일)와 로컬 리포트뿐이며 재배포하지 않는다.
