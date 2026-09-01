# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
