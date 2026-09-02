<script lang="ts">
  import { appState, FUNNEL_STAGES, STAGE_HINT } from '../store'
  import { page as reportsPage } from './Reports.svelte'
  import { dday, ddayLabel, ddayTone, gradeTone, statusTone } from '../lib/fmt'

  const OPEN = ['관심', '지원', '상담', '미팅']

  const upcoming = $derived(
    ($appState?.applications ?? [])
      .filter((a) => a.deadline && OPEN.includes(a.status))
      .map((a) => ({ ...a, d: dday($appState!.today, a.deadline!) }))
      .sort((a, b) => (a.d ?? 9999) - (b.d ?? 9999)),
  )

  const st = $derived($appState?.stats)

  // 각 단계 도달 수 + 직전 단계 대비 전환율. 어디서 새는지 보이게.
  const funnel = $derived(
    FUNNEL_STAGES.map((stage, i) => {
      const n = st?.funnel[stage] ?? 0
      const prev = i === 0 ? n : (st?.funnel[FUNNEL_STAGES[i - 1]] ?? 0)
      return {
        stage,
        n,
        pct: prev > 0 ? Math.round((n / prev) * 100) : null,
        width: (st?.funnel[FUNNEL_STAGES[0]] ?? 0) > 0
          ? Math.max(4, Math.round((n / st!.funnel[FUNNEL_STAGES[0]]) * 100))
          : 0,
      }
    }),
  )
</script>

<div class="page-head">
  <h1>대시보드</h1>
  <span class="sub">마지막 스캔 {$appState?.last_scan?.slice(0, 16).replace('T', ' ') ?? '—'}</span>
</div>

{#if !$appState || !st}
  <div class="empty">불러오는 중…</div>
{:else}
  <div class="cards">
    <div class="card">
      <div class="num">{st.open}</div>
      <div class="lbl">진행 중</div>
      <div class="hint">관심~미팅</div>
    </div>
    <div class="card">
      <div class="num" style="color: var(--info)">{st.by_status['미팅'] ?? 0}</div>
      <div class="lbl">미팅</div>
      <div class="hint">삼자 미팅 단계</div>
    </div>
    <div class="card">
      <div class="num" style="color: var(--good)">{st.won}</div>
      <div class="lbl">체결 이상</div>
      <div class="hint">체결·진행·완료</div>
    </div>
    <div class="card">
      <div class="num" style="color: var(--bad)">{st.lost}</div>
      <div class="lbl">불발</div>
      <div class="hint">미체결·탈락</div>
    </div>
    <div class="card">
      <div class="num">{st.samples >= 5 ? `${st.win_rate}%` : '—'}</div>
      <div class="lbl">수주율</div>
      <div class="hint">{st.samples >= 5 ? `표본 ${st.samples}건` : `통계 부족 (${st.samples}/5)`}</div>
    </div>
    <div class="card">
      <div class="num">{$appState.seen_count}</div>
      <div class="lbl">스캔 누적</div>
      <div class="hint">seen 캐시</div>
    </div>
  </div>

  <h2>수주 퍼널</h2>
  <div class="panel funnel">
    {#if (st.funnel['지원'] ?? 0) === 0}
      <div class="empty">
        <strong>아직 지원 이력이 없습니다</strong>
        지원한 공고의 상태를 파이프라인에서 바꾸면 단계별 전환율이 보입니다.
      </div>
    {:else}
      {#each funnel as f, i}
        <div class="frow">
          <div class="fname">
            {f.stage}
            <span class="dim" style="font-size: 0.7rem">{STAGE_HINT[f.stage]}</span>
          </div>
          <div class="fbar"><div class="ffill" style:width="{f.width}%"></div></div>
          <div class="fnum">{f.n}</div>
          <div class="fpct">
            {#if i > 0 && f.pct !== null}
              <span class="badge {f.pct >= 50 ? 'good' : f.pct >= 25 ? 'warn' : 'bad'}">{f.pct}%</span>
            {/if}
          </div>
        </div>
      {/each}
    {/if}
  </div>

  <h2>마감 임박</h2>
  <div class="panel" style="margin-bottom: 2rem">
    {#if upcoming.length === 0}
      <div class="empty">
        <strong>진행 중인 마감 없음</strong>
인박스에서 관심 표시한 공고의 마감이 여기에 뜹니다.
      </div>
    {:else}
      <table>
        <thead>
          <tr><th style="width: 5.5rem">D-DAY</th><th>공고</th><th style="width: 7rem">마감</th><th style="width: 6rem">상태</th></tr>
        </thead>
        <tbody>
          {#each upcoming as a (a.id)}
            <tr>
              <td><span class="badge {ddayTone(a.d)}">{ddayLabel(a.d)}</span></td>
              <td>
                <a href="#/pipeline/{a.id}">{a.title}</a>
                {#if a.grade}<span class="badge {gradeTone(a.grade)}" style="margin-left: 0.4rem">{a.grade}</span>{/if}
              </td>
              <td class="mono dim">{a.deadline}</td>
              <td><span class="badge {statusTone(a.status)}">{a.status}</span></td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>

  <h2>최근 리포트</h2>
  <div class="panel">
    {#if $appState.reports.length === 0}
      <div class="empty">아직 스캔 리포트가 없습니다.</div>
    {:else}
      <ul class="filelist">
        {#each $appState.reports as r (r.name)}
          <li>
            <a class="filebtn" href="#/reports" onclick={() => reportsPage.set(r.name)}>{r.name}</a>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
{/if}

<style>
  .funnel { padding: 0.4rem 0.9rem; margin-bottom: 2rem; }
  .frow {
    display: grid;
    grid-template-columns: 13rem 1fr 2.5rem 3.5rem;
    align-items: center;
    gap: 0.75rem;
    padding: 0.5rem 0;
    border-bottom: 1px solid var(--border);
  }
  .frow:last-child { border-bottom: none; }
  .fname { display: flex; flex-direction: column; font-size: 0.83rem; }
  .fbar { height: 8px; background: var(--inset); border-radius: 999px; overflow: hidden; }
  .ffill {
    height: 100%;
    background: linear-gradient(90deg, var(--brand), var(--brand-200));
    border-radius: 999px;
    transition: width 220ms ease;
  }
  .fnum { text-align: right; font-variant-numeric: tabular-nums; font-weight: 600; }
  .fpct { text-align: right; }
  @media (max-width: 760px) {
    .frow { grid-template-columns: 8rem 1fr 2rem 3rem; }
    .fname span { display: none; }
  }
</style>
