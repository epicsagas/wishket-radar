---
name: wishket-feedback
description: Calibrate matching profile using historical application outcomes. Analyzes win rates by grade in the pipeline store (state.db applications) to suggest profile.yaml adjustments. 지원 결과 데이터로 매칭 프로필 보정. "매칭 잘 안 되네", "프로필 점검해줘", "수주율 높여줘", "피드백 분석" 등에 사용.
---

# wishket-feedback — Profile Calibration via Application Outcomes

## Flow

```mermaid
flowchart LR
    A[pipeline store (state.db)] --> B{Sufficient sample?}
    B -- Under 5 items --> Z[Notify insufficient data]
    B -- 5+ items --> C[Cross-analyze win rates by grade/stack]
    C --> D[Detect anomalies]
    D --> E[Propose adjustments]
    E --> F{User approval}
    F -- Approved --> G[Edit profile.yaml]
```

## Step 1: Gather Data

- Read applications from `~/.wishket-radar/state.db` (SQLite — `sqlite3 ~/.wishket-radar/state.db 'select data from applications'`, 행마다 Application JSON). 구 `applications.yaml`은 자동 이관된다. If there are fewer than 5 closed items (`체결`, `진행 중`, `완료`, `미체결`, `탈락`), terminate analysis and advise collecting more outcome data first.
- Analyze only closed items; exclude in-progress states (`관심`, `지원`, `상담`, `미팅`).
- Check step-by-step conversion rates before overall win rates.

## Step 2: Anomaly Detection

Identify discrepancies between scout fit grades and actual outcomes:

- **Grade A win rate < Grade B win rate**: Matching score misaligned. Over-weighted keywords in Grade A announcements may be false positives. Suggest lowering weights or splitting keywords.
- **Consistent rejections on specific stack**: Weak experience or competitive market rate issues. Suggest lowering weight or removing from `roles`.
- **Consistent wins on specific keyword**: Core strength. Suggest increasing weight or adding related synonyms.
- **Applications stalled before manager consulting**: Wishket manager screening issue. Proposed price/duration may be unrealistic or proposals need strengthening. Check `wishket-apply` and `wishket-quote` (not a profile issue).
- **Stalled at client meetings**: Terms negotiation issue. Re-evaluate quoting strategy via `wishket-quote` (not a profile issue).

## Step 3: Apply Adjustments

- Detail each proposed adjustment: Current value, Proposed value, Evidence (count/pattern in data), Expected outcome.
- Never propose adjustments without data evidence. Treat <=2 occurrences as observations only.
- Only edit `profile.yaml` upon explicit user confirmation.
- Inform that changes take effect immediately on next scan without server restart.

## Caution

- Highlight that external factors (pricing, market timing, competition) may be the root cause; do not assume matching logic flaws alone.
- Preserve header comments in `profile.yaml`.
