---
name: wishket-pipeline
description: Track Wishket application pipeline. Manages project stages across the 10 Wishket phases in applications.yaml, calculating conversion and win rates. 위시켓 지원 파이프라인 추적. 관심 공고 관리 및 10단계 상태 갱신, 수주율/전환율 계산. "위시켓 지원했어", "지원 현황", "파이프라인 보여줘", "미팅 잡혔어", "계약했어", "떨어졌어", "탈락" 등에 사용.
---

# wishket-pipeline — Wishket Application Pipeline Tracker

## Data Schema

`~/.wishket-radar/applications.yaml` (Canonical data source; create if not present):

```yaml
applications:
  - id: "12345"            # Wishket project ID
    title: Project Title
    url: https://wishket.com/project/12345
    grade: A               # Scout evaluation grade at application time
    quote_manwon: 3000     # Proposed amount in 10k KRW (null if negotiable)
    applied_at: 2026-09-02
    deadline: 2026-09-10   # Project deadline (if present)
    status: 지원           # One of the 10 official stages below
    status_at: 2026-09-02  # Last status change date
    next_action: Review follow-up message # Next action item (or null)
    note: |
      Freeform notes (meeting logs, feedback, etc.)
```

## Overall Workflow

```mermaid
flowchart LR
    A[Scan] --> B[Inbox: Untriaged]
    B -->|Interested| C[관심 (Interested)]
    B -->|Skip| Z[Excluded]
    C -->|Draft proposal & Apply| D[지원 (Applied)]
    D --> E{Screening}
    E -->|Rejected| X[탈락 (Rejected)]
    E -->|Passed| F[상담 (Consulting)] --> G[미팅 (Meeting)] --> H{Contract}
    H -->|Uncontracted| Y[미체결 (Uncontracted)]
    H -->|Contracted| I[체결 (Contracted)] --> J[진행 중 (In Progress)] --> K[완료 (Completed)]
```

Scan results accumulate in `seen` inside `state.json`. Items without a `triage` field remain in the Inbox. When the user marks an item as Interested in the dashboard Inbox, `triage: interested` is set, and **only Interested items** appear in the pipeline. Changing the stage to Applied or later promotes the item into `applications.yaml`.

## The 10 Official Wishket Stages

| Status | Official Wishket Stage | Meaning |
|---|---|---|
| 관심 | (Pre-application) | Marked interested in Inbox, not yet applied |
| 지원 | 1. 지원 (Applied) | Application submitted with proposed price/duration |
| 상담 | 2. 위시켓 상담 (Consulting) | Wishket manager is screening applicants with client |
| 미팅 | 3. 미팅 (Meeting) | 3-way meeting between client, partner, and manager |
| 체결 | 4. 체결 (Contracted) | Final contract signed |
| 진행 중 | 5. 진행 중 (In Progress) | Client escrow deposited; project started |
| 완료 | 6. 완료 (Completed) | Approved and payment settled; project closed |
| 미체결 | 4. 미체결 (Uncontracted) | Terms did not match; cancelled |
| 탈락 | Rejected at step 2-3 | Not selected during screening |
| 철회 | Withdrawn | Applicant withdrew their proposal |

Win rate = (체결 + 진행 중 + 완료) / (체결 + 진행 중 + 완료 + 미체결 + 탈락). Withdrawn (`철회`) is excluded from the denominator.

Examine stage conversion rates (Applied -> Consulting -> Meeting -> Contracted -> Completed) to identify funnel drop-offs.

### Rules:
- Triage in the dashboard (`wishket-dashboard`) is fastest. If triaging via chat ("관심 있어"), update the item in `state.json` with `triage: interested` and `triaged_at`.
- Promote items to `applications.yaml` when applying (via `wishket-apply` or direct user statement "지원했어").
- Detect state transitions from user conversation: "지원했어" -> 지원, "위시켓에서 연락 왔어" -> 상담, "미팅 잡혔어" -> 미팅, "계약했어" -> 체결, "착수했어" -> 진행 중, "끝났어/정산됐어" -> 완료, "떨어졌어" -> 탈락, "조건 안 맞아서 엎어졌어" -> 미체결. Clarify if ambiguous.
- Legacy status names (검토중/면담/수주/거절) are automatically mapped by the server. Always write using the new 10 stages.
- Keep only the latest `status_at` date per item.
- Use Edit tools to modify only specific items; do not overwrite the entire file.

## Presentation

When responding to "show pipeline" requests:
- Funnel: 지원 N -> 상담 a -> 미팅 b -> 체결 c -> 완료 d with conversion % per step.
- Win rate = Won / (Won + Uncontracted + Rejected). Note "Insufficient data" if sample size < 5.
- Active items table: Title, Status, Deadline, Next Action.
- Highlight imminent deadlines (within 3 days) and suggest `wishket-deadline`.

## Skill Integrations

- Suggest `wishket-deadline` (calendar export) when registering an application with a deadline.
- Suggest `wishket-feedback` once 5+ closed items (Won / Uncontracted / Rejected) accumulate.
- Items triaged as Interested in Inbox appear automatically in the pipeline without manual registration. Changing the stage to 지원 or later promotes the item into `applications.yaml`.
