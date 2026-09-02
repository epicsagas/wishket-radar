---
name: wishket-onboard
description: Onboarding for Wishket plugin. Verifies binary installation, sets up tech profile, executes baseline scan, and provides action guides. 위시켓 플러그인 온보딩 (바이너리 설치 확인 + 프로필 생성 + 베이스라인 스캔 + 다음 행동 요령). "위시켓 시작할게", "위시켓 세팅해줘", "온보딩" 등에 사용.
---

Wishket plugin onboarding. Sets up a fresh environment so that wishket-radar runs smoothly, guiding the user from profile setup to actionable workflows in a single flow.

1. **Binary and Environment Check / Installation** (Strict order; do not skip one-liner install even if `cargo` is present):
   - Check if the MCP server (`wishket-mcp`) is executable (`list_filters` call or `which wishket-mcp`).
   - If missing or startup fails, run the **one-liner installer first**:
     - macOS / Linux: `curl --proto '=https' --tlsv1.2 -LsSf https://github.com/epicsagas/wishket-radar/releases/latest/download/install.sh | sh`
     - Windows: `powershell -c "irm https://github.com/epicsagas/wishket-radar/releases/latest/download/install.ps1 | iex"`
     - If `install.sh` / `install.ps1` exists in the plugin root, execute that script directly.
   - Only if the installer fails, the current directory is a git clone (`server/Cargo.toml` and `.git` exist), and `cargo` is on PATH, run `cargo build --release --manifest-path server/Cargo.toml`. Never run cargo build inside plugin cache directories.
   - Verify that the MCP tool responds.

2. **Check Current Profile Status**:
   - Read `~/.wishket-radar/profile.yaml` (or `WISHKET_PROFILE` override).
   - If a profile already exists, summarize it and ask the user via `AskUserQuestion` whether they wish to keep or reconfigure it.
     - Keep existing: Proceed directly to Step 4 (Baseline Scan).
     - Reconfigure or no profile: Proceed to Step 3.

3. **Profile Interview & profile.yaml Creation**:
   - Conduct an interactive interview via `AskUserQuestion` (up to 4 questions per turn):
     - Core technical stack (e.g., Rust, Svelte+TS, Flutter, AWS)
     - Relative importance per stack (High / Medium / Low -> weight 3 / 2 / 1)
     - Applicable roles (e.g., Backend Developer, Fullstack Developer)
     - Working conditions & notes (Remote/On-site, preferred domains)
   - Create `~/.wishket-radar/profile.yaml`: auto-expand Korean + English synonym keywords per skill (e.g., Rust -> rust, 러스트, cargo, axum / Flutter -> flutter, 플러터, dart, 모바일 앱).
   - Inform the user that profile updates take effect immediately on the next scan without restarting the server.

4. **Baseline Scan**:
   - Propose running `scan_new` once (defaults: `development`, 3 pages).
   - On the first run, all fetched projects are recorded in the baseline cache (`~/.wishket-radar/state.json`) — subsequent scans will only report newly posted projects.
   - Skip if the user declines.

5. **Completion & Next Steps Guide**:
   - **Check New Projects**: `wishket-scan` (e.g., *"위시켓 새 프로젝트 있어?"*, *"새 외주 올라온 거 있나?"*)
   - **Deep Analysis & Report**: `wishket-scout` (e.g., *"위시켓 분석해줘"*, *"스카우트 리포트"*)
   - **Real-time Filtered Search**: `wishket-search` (e.g., *"flutter 외주 찾아줘"*, *"파이썬 백엔드 검색해줘"*)
   - **Profile & Weight Tuning**: `wishket-profile` (e.g., *"Rust 가중치 올려줘"*, *"FastAPI 키워드 추가"*)
   - **Triage & Dashboard**: `wishket-dashboard` (e.g., *"대시보드"*). Triage projects in Inbox (Interested/Skip); interested items flow into the pipeline.
   - **Portfolio Drafting**: `wishket-portfolio` (e.g., *"이 프로젝트로 포트폴리오 써줘"*).
   - **Application & Proposal**: `wishket-apply` (e.g., *"이 공고 지원서 써줘"*). Use `wishket-quote` for estimation, `wishket-pipeline` for stage tracking, and `wishket-deadline` for calendar reminders.
   - If the host environment supports scheduled tasks, suggest scheduling a daily morning scan routine.
