<script lang="ts">
  import {
    appState, STATUSES, ACTIVE_STAGES, CLOSED_STAGES, STAGE_HINT, refresh,
  } from '../store'
  import { api } from '../api'
  import type { Application } from '../store'
  import { dday, ddayLabel, ddayTone, gradeTone, statusTone } from '../lib/fmt'

  let view = $state<'table' | 'kanban'>('table')
  let expanded = $state<string | null>(null)
  let error = $state('')
  let busy = $state<string | null>(null)

  const apps = $derived($appState?.applications ?? [])
  const hasClosed = $derived(
    apps.some((a) => (CLOSED_STAGES as readonly string[]).includes(a.status)),
  )

  async function patch(id: string, fields: Record<string, string>) {
    busy = id
    error = ''
    try {
      await api(`/api/applications/${encodeURIComponent(id)}`, {
        method: 'PATCH',
        body: JSON.stringify(fields),
      })
      await refresh() // 폴링을 기다리지 않고 즉시 반영
    } catch (e) {
      error = String(e)
    } finally {
      busy = null
    }
  }

  function onStatus(a: Application, e: Event) {
    void patch(a.id, { status: (e.currentTarget as HTMLSelectElement).value })
  }

  function onAction(a: Application, e: Event) {
    const v = (e.currentTarget as HTMLInputElement).value
    if (v !== (a.next_action ?? '')) void patch(a.id, { next_action: v })
  }
</script>

<div class="page-head">
  <h1>지원 파이프라인</h1>
  <span class="sub">{apps.length}건 · 인박스 관심 + applications.yaml</span>
</div>

{#if error}<div class="banner err">{error}</div>{/if}

<div class="toolbar">
  <button class:ghost={view !== 'table'} onclick={() => (view = 'table')}>표</button>
  <button class:ghost={view !== 'kanban'} onclick={() => (view = 'kanban')}>칸반</button>
</div>

{#if apps.length === 0}
  <div class="panel">
    <div class="empty">
      <strong>지원 내역 없음</strong>
      <a href="#/inbox">인박스</a>에서 공고를 "관심"으로 표시하면 여기에 들어옵니다.
    </div>
  </div>
{:else if view === 'table'}
  <div class="panel">
    <table>
      <thead>
        <tr>
          <th style="min-width: 22rem">공고</th>
          <th style="width: 7rem">단계</th>
          <th style="width: 4rem">매칭</th>
          <th class="num" style="width: 6rem">금액(만)</th>
          <th style="width: 5.5rem">마감</th>
          <th style="width: 15rem">다음 할 일</th>
        </tr>
      </thead>
      <tbody>
        {#each apps as a (a.id)}
          {@const d = a.deadline ? dday($appState!.today, a.deadline) : null}
          <tr style:opacity={busy === a.id ? 0.5 : 1}>
            <td>
              <div class="row">
                <a href="#/pipeline/{a.id}">{a.title}</a>
                {#if a.url}
                  <a class="ext" href={a.url} target="_blank" rel="noopener noreferrer" aria-label="위시켓에서 열기">↗</a>
                {/if}
                {#if a.note}
                  <button
                    class="ghost notebtn"
                    onclick={() => (expanded = expanded === a.id ? null : a.id)}
                    aria-label="메모 보기"
                  >{expanded === a.id ? '−' : '메모'}</button>
                {/if}
              </div>
              {#if expanded === a.id && a.note}
                <div class="dim notebody">{a.note}</div>
              {/if}
            </td>
            <td>
              <select
                value={a.status}
                onchange={(e) => onStatus(a, e)}
                title={STAGE_HINT[a.status] ?? ''}
              >
                {#each STATUSES as s}<option value={s}>{s}</option>{/each}
              </select>
            </td>
            <td>{#if a.grade}<span class="badge {gradeTone(a.grade)}">{a.grade}</span>{/if}</td>
            <td class="num">{a.quote_manwon?.toLocaleString() ?? ''}</td>
            <td>
              {#if a.deadline}
                <span class="badge {ddayTone(d)}" title={a.deadline}>{ddayLabel(d)}</span>
              {/if}
            </td>
            <td>
              <input
                class="flush"
                type="text"
                value={a.next_action ?? ''}
                onblur={(e) => onAction(a, e)}
                placeholder="—"
              />
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{:else}
  <div class="kanban active">
    {#each ACTIVE_STAGES as s}
      <div class="kcol">
        <h3>
          <span class="badge {statusTone(s)}" title={STAGE_HINT[s]}>{s}</span>
          <span class="dim">{apps.filter((a) => a.status === s).length}</span>
        </h3>
        {#each apps.filter((a) => a.status === s) as a (a.id)}
          {@const d = a.deadline ? dday($appState!.today, a.deadline) : null}
          <div class="kcard">
            <a href="#/pipeline/{a.id}">{a.title}</a>
            {#if a.deadline || a.next_action}
              <div class="row" style="margin-top: 0.35rem; flex-wrap: wrap">
                {#if a.deadline}<span class="badge {ddayTone(d)}">{ddayLabel(d)}</span>{/if}
                {#if a.next_action}<span class="dim" style="font-size: 0.74rem">{a.next_action}</span>{/if}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/each}
  </div>

  {#if hasClosed}
    <h2 style="margin-top: 1.6rem">종결</h2>
    <div class="kanban closed">
      {#each CLOSED_STAGES as s}
        <div class="kcol">
          <h3>
            <span class="badge {statusTone(s)}" title={STAGE_HINT[s]}>{s}</span>
            <span class="dim">{apps.filter((a) => a.status === s).length}</span>
          </h3>
          {#each apps.filter((a) => a.status === s) as a (a.id)}
            <div class="kcard">
              <a href="#/pipeline/{a.id}">{a.title}</a>
            </div>
          {/each}
        </div>
      {/each}
    </div>
  {/if}
{/if}

<style>
  .notebtn {
    padding: 0 0.4rem;
    font-size: 0.68rem;
    line-height: 1.5;
    white-space: nowrap;
    flex: none;
  }
  .notebody {
    white-space: pre-wrap;
    margin-top: 0.35rem;
    font-size: 0.8rem;
  }
  .row { align-items: baseline; }
  .ext { color: var(--faint); font-size: 0.75rem; text-decoration: none; }
  .ext:hover { color: var(--brand-200); }
  /* 진행 6단계 + 종결 4단계 */
  .kanban.active { grid-template-columns: repeat(6, minmax(0, 1fr)); }
  .kanban.closed { grid-template-columns: repeat(4, minmax(0, 1fr)); }
  @media (max-width: 1100px) {
    .kanban.active, .kanban.closed { grid-template-columns: repeat(3, minmax(0, 1fr)); }
  }
  @media (max-width: 760px) {
    .kanban.active, .kanban.closed { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }
</style>
