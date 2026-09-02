---
name: wishket-dashboard
description: Launch the Wishket local dashboard web UI. View and manage inbox triage (interested/skip), pipeline stages, proposals, profiles, portfolios, and reports. 위시켓 로컬 대시보드 webui 실행. "대시보드", "웹 UI 켜줘", "지원 현황 화면으로 보여줘", "공고 분류할래", "dashboard" 등에 사용.
---

# wishket-dashboard — Launch Local Web UI

The `dashboard` subcommand of `wishket-mcp` serves a local web UI reading and writing files under `~/.wishket-radar/`. Binds to `0.0.0.0` with token authentication. The token is stored in `~/.wishket-radar/dashboard-token`.

## Step 1: Start Server

Run in background via Bash:

```bash
sh "${CLAUDE_PLUGIN_ROOT:-.}/scripts/wishket-mcp" dashboard
```

- Stdout prints `URL: http://127.0.0.1:8787?token=...` and LAN addresses.
- If port conflict occurs (already running), advance directly to Step 2.
- Support custom ports via `--port N` or disable auto browser launch via `--no-open`.

## Step 2: Verification

```bash
sleep 1
curl -fsS -H "Authorization: Bearer $(tr -d '\n' < ~/.wishket-radar/dashboard-token)" http://127.0.0.1:8787/api/state
```

- HTTP 200 confirms success. If it fails, check token file and port status.

## Step 3: User Guide

Provide connection links:
- Local: `http://127.0.0.1:8787?token=<token>` (auto-opens on macOS).
- LAN / Mobile: `http://<LAN-IP>:8787?token=<token>` (use LAN IP from stdout or `ipconfig getifaddr en0` / `hostname -I`).
- Token reset: Delete `~/.wishket-radar/dashboard-token` and restart to generate a new token.

## Feature Overview

- **Inbox**: Triage newly scanned projects into Interested or Skip. Filter by score. Only Interested items proceed to pipeline.
- **Inbox Detail (`#/inbox/{id}`)**: Open via the title. Shows stored data first. Does not auto-fetch (robots Crawl-delay). "Fetch Details" loads description and conditions from Wishket on demand, then the user triages Interested/Skip.
- **Dashboard**: Visualizes application funnel, stage conversion rates, D-day deadlines, and recent reports.
- **Pipeline**: Inline stage editing and next actions. Detail view (`#/pipeline/{id}`) manages stage, notes, and actions on one page.
- **My Info**: Manage matching profile (`profile.yaml`) and portfolio assets (`portfolios/`) in one unified view.
- **Proposals**: Grouped per project directory and linked with pipeline details.
- **Atomic Saves**: Server verifies and performs atomic writes, maintaining a 1-generation `.bak` backup.
- **Reports**: View scout reports. Analyst fit ratings (A/B/C), cautions, and recommendations link directly to project cards.

## Caution

- Edits in the web UI synchronize with files read by CLI/chat skills (`wishket-pipeline`, etc.) with last-write-wins semantics.
- Default port is 8787. Terminate server by stopping the process.
