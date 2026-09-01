# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-09-01

### Added
- Multi-host agent plugin integration for Claude Code, Codex, agy (Antigravity), and Hermes.
- High-performance Rust MCP server (`wishket`) implementing reverse-engineered Wishket search API, HTML/JSON-LD parser, and deterministic keyword matching.
- Orchestration skills: `wishket-scout`, `wishket-scan`, `wishket-search`, `wishket-profile`, `wishket-onboard`.
- Specialized subagent: `wishket-analyst` for deep single-project fit analysis.
- Multi-platform prebuilt binaries distribution via `cargo-dist` (macOS arm64/x86_64, Linux arm64/x86_64, Windows x64).
- One-line installer scripts (`install.sh`, `install.ps1`).
- Unified runtime and profile storage under `~/.wishket-radar/`.
