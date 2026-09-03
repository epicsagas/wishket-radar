<script lang="ts">
  import { api } from '../api'
  import { appState, refresh, type InboxItem } from '../store'
  import { dday, ddayLabel, ddayTone, gradeTone } from '../lib/fmt'

  let items = $state<InboxItem[]>([])
  let loading = $state(true)
  let error = $state('')
  let busy = $state<string | null>(null)
  let minScore = $state(0)

  let onlyAnalyzed = $state(false)
  let showExpired = $state(false)
  const shown = $derived(
    items
      .filter((i) => (i.score ?? 0) >= minScore)
      .filter((i) => !onlyAnalyzed || i.analysis)
      // 마감 지난 공고는 기본으로 숨긴다 — 지원할 수 없는 건 판단 대상이 아니다
      .filter((i) => showExpired || !i.expired)
      // 리포트 분석이 있는 건(등급 A>B>C) 먼저, 그 다음 점수순
      .sort((a, b) => {
        const g = (x: InboxItem) => (x.analysis?.grade ? { A: 0, B: 1, C: 2 }[x.analysis.grade] ?? 3 : 4)
        return g(a) - g(b) || (b.score ?? 0) - (a.score ?? 0)
      }),
  )

  const expiredCount = $derived(items.filter((i) => i.expired).length)

  async function load() {
    try {
      const r = await api<{ inbox: InboxItem[] }>('/api/inbox')
      items = r.inbox
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }
  void load()

  async function triage(id: string, action: 'interested' | 'skipped') {
    busy = id
    error = ''
    try {
      await api(`/api/inbox/${encodeURIComponent(id)}/triage`, {
        method: 'POST',
        body: JSON.stringify({ action }),
      })
      items = items.filter((i) => i.id !== id) // 처리한 건 인박스에서 즉시 내린다
      await refresh()
    } catch (e) {
      error = String(e)
    } finally {
      busy = null
    }
  }
</script>

<div class="page-head">
  <h1>인박스</h1>
  <span class="sub">
    {shown.length}건 미분류 · 스캔 누적 {$appState?.seen_count ?? 0}
  </span>
</div>

{#if error}<div class="banner err">{error}</div>{/if}

<div class="toolbar">
  <span class="dim" style="font-size: 0.8rem; align-self: center">최소 매칭</span>
  {#each [0, 20, 30, 40] as v}
    <button class:ghost={minScore !== v} onclick={() => (minScore = v)}>{v === 0 ? '전체' : `${v}+`}</button>
  {/each}
  <button class:ghost={!onlyAnalyzed} onclick={() => (onlyAnalyzed = !onlyAnalyzed)} style="margin-left: 0.5rem">
    리포트 분석만
  </button>
  {#if expiredCount}
    <button class:ghost={!showExpired} onclick={() => (showExpired = !showExpired)}>
      마감 지남 {expiredCount}
    </button>
  {/if}
</div>

{#if loading}
  <div class="panel"><div class="empty">불러오는 중…</div></div>
{:else if shown.length === 0}
  <div class="panel">
    <div class="empty">
      <strong>{items.length === 0 ? '분류할 공고가 없습니다' : '조건에 맞는 공고가 없습니다'}</strong>
      {items.length === 0 ? '채팅에서 "위시켓 스캔"을 실행하면 새 공고가 여기에 쌓입니다.' : '최소 매칭 점수를 낮춰보세요.'}
    </div>
  </div>
{:else}
  <div class="inbox">
    {#each shown as it (it.id)}
      {@const d = it.deadline && $appState ? dday($appState.today, it.deadline) : null}
      <article class="item" style:opacity={busy === it.id ? 0.45 : 1}>
        <div class="head">
          {#if it.analysis?.grade}
            <span class="badge {gradeTone(it.analysis.grade)}" title="스카우트 리포트 판정">{it.analysis.grade}</span>
          {/if}
          <span class="badge muted" title="키워드 매칭 점수">{it.score ?? 0}</span>
          {#if it.analysis?.score != null}
            <span class="badge muted" title={`AI 평가 점수 · 모델: ${it.analysis.model ?? '미기록'}`}>AI {it.analysis.score}</span>
          {/if}
          {#if it.private_matching}
            <span class="badge warn" title="PRIME·PRO·BOOST 파트너에게만 공개되는 비공개 프로젝트">프라이빗 매칭</span>
          {/if}
          <a href="#/inbox/{it.id}" class="title" class:missing={it.title_missing}>{it.title}</a>
          {#if it.expired}<span class="badge bad">마감됨</span>{/if}
          {#if it.url}
            <a class="ext" href={it.url} target="_blank" rel="noopener noreferrer" aria-label="위시켓에서 열기">↗</a>
          {/if}
          {#if it.deadline}<span class="badge {ddayTone(d)}">{ddayLabel(d)}</span>{/if}
        </div>
        <div class="meta dim">
          {#if it.budget}<span>{it.budget}</span>{/if}
          {#if it.duration}<span>{it.duration}</span>{/if}
        </div>
        {#if it.analysis?.fit}
          <p class="fit">{it.analysis.fit}</p>
        {/if}
        {#if it.analysis?.caution}
          <p class="caution">⚠ {it.analysis.caution}</p>
        {/if}
        {#if it.skills.length}
          <div class="skills">
            {#each it.skills.slice(0, 8) as sk}<span class="badge muted">{sk}</span>{/each}
          </div>
        {/if}
        <div class="actions">
          <button onclick={() => triage(it.id, 'interested')} disabled={busy === it.id}>관심</button>
          <button class="ghost" onclick={() => triage(it.id, 'skipped')} disabled={busy === it.id}>스킵</button>
        </div>
      </article>
    {/each}
  </div>
{/if}

<style>
  .inbox { display: grid; gap: 0.7rem; }
  .item {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    padding: 0.9rem 1rem;
    transition: border-color 140ms, opacity 140ms;
  }
  .item:hover { border-color: var(--brand-ring); }
  .head { display: flex; align-items: baseline; gap: 0.5rem; flex-wrap: wrap; }
  .ext { color: var(--faint); font-size: 0.78rem; }
  .ext:hover { color: var(--brand-200); }
  .title { font-size: 0.95rem; font-weight: 500; }
  .title.missing { color: var(--faint); font-style: italic; }
  .meta { display: flex; gap: 0.8rem; font-size: 0.78rem; margin-top: 0.3rem; flex-wrap: wrap; }
  .skills { display: flex; gap: 0.3rem; flex-wrap: wrap; margin-top: 0.5rem; }
  .fit, .caution {
    margin: 0.5rem 0 0; font-size: 0.8rem; line-height: 1.6;
    display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
  }
  .fit { color: var(--muted); }
  .caution { color: var(--warn); }
  .actions { display: flex; gap: 0.45rem; margin-top: 0.75rem; }
</style>
