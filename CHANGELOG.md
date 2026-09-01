# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
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
