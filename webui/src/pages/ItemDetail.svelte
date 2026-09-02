<script lang="ts">
  import { appState, STATUSES, STAGE_HINT, refresh, type FileEntry } from '../store'
  import { api } from '../api'
  import { go } from '../router'
  import { dday, ddayLabel, ddayTone, gradeTone, statusTone } from '../lib/fmt'

  let { id }: { id: string } = $props()

  let error = $state('')
  let saving = $state(false)
  let fetching = $state(false)
  let noteDraft = $state<string | null>(null)

  /// 공고 본문이 캐시에 없을 때 여기서 바로 불러온다.
  /// (인박스를 거치지 않고 들어온 항목은 캐시가 비어 있다)
  async function fetchDetail() {
    fetching = true
    error = ''
    try {
      await api(`/api/inbox/${encodeURIComponent(id)}/fetch`, { method: 'POST' })
      await refresh()
    } catch (e) {
      error = String(e)
    } finally {
      fetching = false
    }
  }

  const item = $derived($appState?.applications.find((a) => a.id === id))
  // 인박스에서 이미 불러온 공고 본문이 있으면 재조회 없이 보여준다
  const cached = $derived($appState?.details?.[id] ?? null)

  // 이 공고에 딸린 제안서·포트폴리오 (파일명에 공고 ID가 들어간 것)
  // 이 공고에 딸린 제안서. 포트폴리오는 공고 종속이 아니라 내 정보에 있다.
  let docs = $state<FileEntry[]>([])
  $effect(() => {
    const cur = id
    docs = []
    api<{ files: FileEntry[] }>('/api/files/proposals')
      .then((r) => (docs = r.files.filter((f) => f.project_id === cur)))
      .catch(() => { /* 목록 실패는 조용히 무시 — 본문 표시가 우선 */ })
  })
  const d = $derived(
    item?.deadline && $appState ? dday($appState.today, item.deadline) : null,
  )

  async function patch(fields: Record<string, string>) {
    saving = true
    error = ''
    try {
      await api(`/api/applications/${encodeURIComponent(id)}`, {
        method: 'PATCH',
        body: JSON.stringify(fields),
      })
      await refresh()
    } catch (e) {
      error = String(e)
    } finally {
      saving = false
    }
  }
</script>

<div class="page-head">
  <div>
    <button class="ghost" onclick={() => go('/pipeline')} style="margin-bottom: 0.5rem">← 파이프라인</button>
    <h1>{item?.title ?? id}</h1>
  </div>
  {#if item}<span class="badge {statusTone(item.status)}">{item.status}</span>{/if}
</div>

{#if error}<div class="banner err">{error}</div>{/if}

{#if !$appState}
  <div class="panel"><div class="empty">불러오는 중…</div></div>
{:else if !item}
  <div class="panel">
    <div class="empty">
      <strong>항목을 찾을 수 없습니다</strong>
      인박스에서 스킵했거나 목록에서 제거된 공고입니다.
    </div>
  </div>
{:else}
  <div class="detail">
    <div class="panel meta">
      <dl>
        <dt>단계</dt>
        <dd>
          <select
            value={item.status}
            disabled={saving}
            onchange={(e) => patch({ status: (e.currentTarget as HTMLSelectElement).value })}
            title={STAGE_HINT[item.status] ?? ''}
          >
            {#each STATUSES as s}<option value={s}>{s}</option>{/each}
          </select>
          <div class="dim hint">{STAGE_HINT[item.status] ?? ''}</div>
        </dd>

        {#if item.grade}
          <dt>매칭</dt>
          <dd><span class="badge {gradeTone(item.grade)}">{item.grade}</span></dd>
        {/if}

        {#if item.deadline}
          <dt>마감</dt>
          <dd>
            <span class="badge {ddayTone(d)}">{ddayLabel(d)}</span>
            <span class="mono dim">{item.deadline}</span>
          </dd>
        {/if}

        {#if item.quote_manwon}
          <dt>제안 금액</dt>
          <dd>{item.quote_manwon.toLocaleString()}만원</dd>
        {/if}

        {#if item.applied_at}
          <dt>지원일</dt>
          <dd class="mono">{item.applied_at}</dd>
        {/if}

        {#if item.status_at}
          <dt>단계 변경</dt>
          <dd class="mono">{item.status_at}</dd>
        {/if}

        {#if cached?.budget}<dt>예산</dt><dd>{cached.budget}</dd>{/if}
        {#if cached?.role}<dt>역할</dt><dd>{cached.role}</dd>{/if}
        {#if item.url}
          <dt>공고</dt>
          <dd><a href={item.url} target="_blank" rel="noopener noreferrer">위시켓에서 열기 ↗</a></dd>
        {/if}
      </dl>

      {#if cached?.matched?.length}
        <div class="matched">
          <div class="dim" style="font-size: 0.72rem; margin: 0.9rem 0 0.3rem">매칭된 기술</div>
          {#each cached.matched as m}<span class="badge good">{m}</span>{/each}
        </div>
      {/if}
    </div>

    <div class="panel body">
      <h2>다음 할 일</h2>
      <input
        class="flush"
        type="text"
        value={item.next_action ?? ''}
        placeholder="예: 제안서 초안 검토"
        onblur={(e) => {
          const v = (e.currentTarget as HTMLInputElement).value
          if (v !== (item!.next_action ?? '')) void patch({ next_action: v })
        }}
      />

      <h2 style="margin-top: 1.4rem">
        제안서
        <span class="dim srcnote">{docs.length}건</span>
      </h2>
      {#if docs.length}
        <ul class="docs">
          {#each docs as f (f.name)}
            <li>
              <a href="#/proposals?file={encodeURIComponent(f.name)}">
                <span class="mono">{f.name.split('/').pop()}</span>
              </a>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="dim" style="font-size: 0.82rem; margin: 0">
          이 공고로 만든 제안서가 없습니다. 채팅에서 "이 공고 지원서 써줘"로 만들 수 있습니다.
        </p>
      {/if}

      <h2 style="margin-top: 1.4rem">공고 설명</h2>
      {#if cached?.description}
        <details class="desc">
          <summary class="dim">펼쳐 보기 ({cached.detail_fetched_at?.slice(0, 10)} 기준)</summary>
          <div class="descbody">{cached.description}</div>
        </details>
        <button class="ghost mini" style="margin-top: 0.5rem" onclick={fetchDetail} disabled={fetching}>
          {fetching ? '갱신 중…' : '갱신'}
        </button>
      {:else}
        <p class="dim" style="font-size: 0.82rem; margin: 0 0 0.6rem">
          아직 공고 본문을 불러오지 않았습니다. 위시켓에서 설명·조건·매칭 기술을 가져옵니다.
        </p>
        <button onclick={fetchDetail} disabled={fetching}>
          {fetching ? '불러오는 중…' : '공고 상세 불러오기'}
        </button>
      {/if}

      <h2 style="margin-top: 1.4rem">메모</h2>
      {#if noteDraft === null}
        <div class="note">{item.note || '—'}</div>
        <button class="ghost" style="margin-top: 0.6rem" onclick={() => (noteDraft = item!.note ?? '')}>
          편집
        </button>
      {:else}
        <textarea bind:value={noteDraft} style="min-height: 180px"></textarea>
        <div class="toolbar" style="margin-top: 0.5rem">
          <button
            disabled={saving}
            onclick={async () => {
              await patch({ note: noteDraft ?? '' })
              noteDraft = null
            }}
          >저장</button>
          <button class="ghost" onclick={() => (noteDraft = null)}>취소</button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .detail { display: grid; grid-template-columns: 300px 1fr; gap: 1.1rem; align-items: start; }
  .meta { padding: 1rem 1.15rem; }
  dl { margin: 0; display: grid; grid-template-columns: 5.5rem 1fr; gap: 0.55rem 0.8rem; align-items: baseline; }
  dt { color: var(--faint); font-size: 0.78rem; }
  dd { margin: 0; font-size: 0.86rem; }
  .hint { font-size: 0.72rem; margin-top: 0.25rem; }
  .body { padding: 1rem 1.15rem 1.3rem; }
  .note { white-space: pre-wrap; font-size: 0.86rem; color: var(--muted); }
  .matched .badge { margin: 0 0.25rem 0.25rem 0; }
  .mini { padding: 0.2rem 0.5rem; font-size: 0.72rem; }
  .docs { list-style: none; margin: 0; padding: 0; display: grid; gap: 0.3rem; }
  .docs a { display: flex; align-items: center; gap: 0.45rem; font-size: 0.8rem; }
  .docs a:hover .mono { color: var(--brand-200); }
  .srcnote { font-size: 0.7rem; font-weight: 400; text-transform: none; letter-spacing: 0; margin-left: 0.5rem; }
  .desc summary { cursor: pointer; font-size: 0.8rem; }
  .descbody { white-space: pre-wrap; font-size: 0.85rem; line-height: 1.75; color: var(--muted); margin-top: 0.7rem; }
  @media (max-width: 860px) { .detail { grid-template-columns: 1fr; } }
</style>
