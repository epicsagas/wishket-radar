<script lang="ts">
  import { api } from '../api'
  import { appState, refresh } from '../store'
  import { go } from '../router'
  import { dday, ddayLabel, ddayTone, gradeTone } from '../lib/fmt'

  let { id }: { id: string } = $props()

  interface Item {
    id: string
    title: string
    url: string | null
    score: number | null
    budget: string | null
    duration: string | null
    private_matching: boolean | null
    deadline: string | null
    skills: string[]
    first_seen: string
    triage: string | null
    triaged_at: string | null
    description: string | null
    conditions: [string, string][]
    role: string | null
    level: string | null
    location: string | null
    matched: string[]
    detail_fetched_at: string | null
    analysis: {
      grade: string | null
      fit: string | null
      caution: string | null
      proposal: string | null
      score: number | null
      model: string | null
      report: string | null
    } | null
  }

  let item = $state<Item | null>(null)
  let error = $state('')
  let fetching = $state(false)
  let busy = $state(false)

  // AI 평가 (BYOK) — 결과는 reports/ai-eval.md로 저장되어 analysis로 재로딩된다
  let aiBusy = $state(false)
  let aiError = $state('')

  async function runAiEval() {
    aiBusy = true
    aiError = ''
    try {
      await api(`/api/ai/evaluate`, {
        method: 'POST',
        body: JSON.stringify({ id }),
      })
      item = await api<Item>(`/api/inbox/${encodeURIComponent(id)}`)
      await refresh()
    } catch (e) {
      aiError = String(e)
    } finally {
      aiBusy = false
    }
  }

  const d = $derived(
    item?.deadline && $appState ? dday($appState.today, item.deadline) : null,
  )

  // id가 바뀌면(다른 공고로 이동) 다시 읽는다 — 컴포넌트가 재사용되기 때문
  // 캐시된 상세가 있으면 바로 렌더 — 재조회하지 않는다
  const hasDetail = $derived(!!item?.detail_fetched_at)

  $effect(() => {
    const cur = id
    item = null
    error = ''
    api<Item>(`/api/inbox/${encodeURIComponent(cur)}`)
      .then((r) => (item = r))
      .catch((e) => (error = String(e)))
  })

  /// 위시켓에서 다시 긁어 캐시를 갱신한다. 캐시가 있으면 사용자가 명시적으로 누를 때만.
  async function fetchDetail() {
    fetching = true
    error = ''
    try {
      await api(`/api/inbox/${encodeURIComponent(id)}/fetch`, { method: 'POST' })
      item = await api<Item>(`/api/inbox/${encodeURIComponent(id)}`)
      await refresh()
    } catch (e) {
      error = String(e)
    } finally {
      fetching = false
    }
  }

  async function triage(action: 'interested' | 'skipped') {
    busy = true
    error = ''
    try {
      await api(`/api/inbox/${encodeURIComponent(id)}/triage`, {
        method: 'POST',
        body: JSON.stringify({ action }),
      })
      await refresh()
      go(action === 'interested' ? `/pipeline/${id}` : '/inbox')
    } catch (e) {
      error = String(e)
      busy = false
    }
  }
</script>

<div class="page-head">
  <div>
    <button class="ghost" onclick={() => go('/inbox')} style="margin-bottom: 0.5rem">← 인박스</button>
    <h1>{item?.title ?? id}</h1>
  </div>
  <div class="row">
    {#if item?.analysis?.grade}
      <span class="badge {gradeTone(item.analysis.grade)}">적합도 {item.analysis.grade}</span>
    {/if}
    {#if item?.private_matching}
      <span class="badge warn" title="PRIME·PRO·BOOST 파트너에게만 공개되는 비공개 프로젝트">프라이빗 매칭</span>
    {/if}
    {#if item?.score != null}
      <span class="badge muted">매칭 {item.score}</span>
    {/if}
    {#if item?.analysis?.score != null}
      <span class="badge muted" title={`AI 평가 점수 · 모델: ${item.analysis.model ?? '미기록'}`}>
        AI {item.analysis.score}
      </span>
    {/if}
    {#if item && !item.triage}
      <span class="triage" title="이 공고를 파이프라인에 넣을까요?">
        <button onclick={() => triage('interested')} disabled={busy}>관심 · 파이프라인</button>
        <button class="ghost" onclick={() => triage('skipped')} disabled={busy}>스킵</button>
      </span>
    {:else if item?.triage}
      <span class="badge muted">이미 {item.triage === 'interested' ? '관심' : '스킵'} 처리됨</span>
    {/if}
  </div>
</div>

{#if error}<div class="banner err">{error}</div>{/if}

{#if !item}
  <div class="panel"><div class="empty">불러오는 중…</div></div>
{:else}
  <div class="detail">
    <div class="panel meta">
      <dl>
        {#if item.budget}<dt>예산</dt><dd>{item.budget}</dd>{/if}
        {#if item.duration}<dt>기간</dt><dd>{item.duration}</dd>{/if}
        {#if item.deadline}
          <dt>마감</dt>
          <dd>
            <span class="badge {ddayTone(d)}">{ddayLabel(d)}</span>
            <span class="mono dim">{item.deadline}</span>
          </dd>
        {/if}
        {#if item.role}<dt>역할</dt><dd>{item.role}</dd>{/if}
        {#if item.level}<dt>레벨</dt><dd>{item.level}</dd>{/if}
        {#if item.location}<dt>장소</dt><dd>{item.location}</dd>{/if}
        <dt>발견</dt>
        <dd class="mono dim">{item.first_seen.slice(0, 10)}</dd>
        {#if item.url}
          <dt>공고</dt>
          <dd><a href={item.url} target="_blank" rel="noopener noreferrer">위시켓에서 열기 ↗</a></dd>
        {/if}
      </dl>

      {#if item.skills.length}
        <div class="skills">
          {#each item.skills as sk}<span class="badge muted">{sk}</span>{/each}
        </div>
      {/if}

      {#if item.matched.length}
        <div class="matched">
          <div class="dim" style="font-size: 0.72rem; margin-bottom: 0.3rem">매칭된 기술</div>
          {#each item.matched as m}<span class="badge good">{m}</span>{/each}
        </div>
      {/if}
    </div>

    <div class="panel body">
      {#if item.analysis}
        <h2>
          스카우트 분석
          <span class="dim srcnote">{item.analysis.report}</span>
          <button class="ghost mini" style="margin-left: auto" onclick={runAiEval} disabled={aiBusy || !hasDetail}>
            {aiBusy ? '평가 중…' : 'AI 재평가'}
          </button>
        </h2>
        <dl class="cond">
          {#if item.analysis.fit}<dt>적합도</dt><dd>{item.analysis.fit}</dd>{/if}
          {#if item.analysis.caution}<dt>주의점</dt><dd class="warn">{item.analysis.caution}</dd>{/if}
          {#if item.analysis.proposal}<dt>제안 방향</dt><dd>{item.analysis.proposal}</dd>{/if}
        </dl>
        <hr />
      {:else if hasDetail}
        <div class="aieval">
          <p class="dim" style="margin: 0 0 0.6rem">
            AI 분석가가 공고 본문과 기술 프로필을 대조해 등급(A/B/C)과 제안 방향을 5줄로 냅니다.
          </p>
          <button onclick={runAiEval} disabled={aiBusy}>
            {aiBusy ? '평가 중…' : 'AI 평가'}
          </button>
        </div>
      {/if}
      {#if aiError}<div class="banner err">{aiError}</div>{/if}
      {#if !hasDetail}
        <div class="empty">
          <strong>공고 상세를 아직 불러오지 않았습니다</strong>
          위시켓에서 설명·조건을 가져와 저장합니다. 한 번 불러오면 다음부터는 캐시에서 바로 열립니다.
          <div style="margin-top: 0.9rem">
            <button onclick={fetchDetail} disabled={fetching}>
              {fetching ? '불러오는 중…' : '상세 불러오기'}
            </button>
          </div>
        </div>
      {:else}
        <div class="bodyhead">
          <span class="dim" style="font-size: 0.74rem">
            {item.detail_fetched_at?.slice(0, 16).replace('T', ' ')} 기준 (캐시)
          </span>
          <button class="ghost" onclick={fetchDetail} disabled={fetching}>
            {fetching ? '갱신 중…' : '갱신'}
          </button>
        </div>
        {#if item.conditions.length}
          <h2>조건</h2>
          <dl class="cond">
            {#each item.conditions as [k, v]}
              <dt>{k}</dt><dd>{v}</dd>
            {/each}
          </dl>
        {/if}
        <h2 style="margin-top: 1.3rem">설명</h2>
        <div class="desc">{item.description ?? '(설명 없음)'}</div>
      {/if}
    </div>
  </div>

{/if}

<style>
  .detail { display: grid; grid-template-columns: 320px 1fr; gap: 1.1rem; align-items: start; }
  .meta { padding: 1rem 1.15rem; }
  dl { margin: 0; display: grid; grid-template-columns: 4.5rem 1fr; gap: 0.5rem 0.8rem; align-items: baseline; }
  dt { color: var(--faint); font-size: 0.78rem; }
  dd { margin: 0; font-size: 0.86rem; }
  dl.cond { grid-template-columns: 8rem 1fr; }
  .skills, .matched { display: flex; gap: 0.3rem; flex-wrap: wrap; margin-top: 0.9rem; }
  .matched { display: block; }
  .matched .badge { margin: 0 0.25rem 0.25rem 0; }
  .body { padding: 1rem 1.15rem 1.3rem; }
  .body h2 { display: flex; align-items: center; }
  .mini { padding: 0.2rem 0.45rem; font-size: 0.72rem; }
  .aieval { display: flex; flex-direction: column; align-items: flex-start; gap: 0.2rem; }
  .bodyhead {
    display: flex; align-items: center; justify-content: space-between;
    gap: 0.6rem; margin-bottom: 0.9rem; padding-bottom: 0.7rem;
    border-bottom: 1px solid var(--border);
  }
  .warn { color: var(--warn); }
  .srcnote { font-size: 0.7rem; font-weight: 400; text-transform: none; letter-spacing: 0; margin-left: 0.5rem; }
  hr { border: none; border-top: 1px solid var(--border); margin: 1.3rem 0; }
  .desc { white-space: pre-wrap; font-size: 0.86rem; line-height: 1.75; color: var(--muted); }
  .triage { display: inline-flex; gap: 0.35rem; margin-left: 0.4rem; }
  /* 모바일: 제목과 배지·트리아지 버튼을 세로로 스택 — 한 줄 baseline 정렬이
     긴 제목+버튼을 비좁게 눌러 화면을 넘치게 한다 */
  @media (max-width: 760px) {
    .page-head { flex-direction: column; align-items: stretch; gap: 0.55rem; }
    .page-head .row { flex-wrap: wrap; justify-content: flex-start; }
    .triage { width: 100%; margin-left: 0; }
    .triage button { flex: 1; }
  }
  @media (max-width: 860px) { .detail { grid-template-columns: 1fr; } }
</style>
