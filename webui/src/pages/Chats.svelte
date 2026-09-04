<script lang="ts">
  // #/chats — 대화 목록 + 선택 시 ChatPanel. #/chats/{id}로 직접 진입, ?project=로
  // 공고 연결 새 대화. 공고 캐시에서 사라진 대화는 "삭제된 공고"로 표시.
  import { api } from '../api'
  import { route } from '../router'
  import { appState } from '../store'
  import ChatPanel from '../components/ChatPanel.svelte'

  interface Conv {
    id: number
    project_id: string | null
    project_title: string | null
    title: string
    created_at: string
    tokens_in: number
    tokens_out: number
    messages: number
  }

  let convs = $state<Conv[]>([])
  let loading = $state(true)
  let error = $state('')
  let confirmingDelete = $state<number | null>(null)

  let inboxTitles = $state<Record<string, string>>({})

  const seg = $derived($route.split('?')[0].split('/').filter(Boolean))
  const selectedId = $derived.by(() => {
    if (seg[0] !== 'chats' || !seg[1]) return null
    const n = Number(seg[1])
    return Number.isInteger(n) && n > 0 ? n : null
  })
  const query = $derived(new URLSearchParams($route.split('?')[1] ?? ''))
  const newProject = $derived(query.get('project') ?? null)
  const panelProject = $derived(
    selectedId == null ? newProject : null,
  )
  // 새 대화(?project=) 헤더 표시용 제목 — 인박스+파이프라인에서 해석
  // (/api/inbox는 미분류만 반환하므로 applications 제목을 합쳐야 파이프라인 공고도
  //  "삭제된 공고"로 오표시되지 않는다), 없으면 삭제된 공고
  const projectTitle = $derived.by(() => {
    if (!panelProject) return null
    const titles = { ...inboxTitles }
    for (const a of $appState?.applications ?? []) {
      if (a.title && titles[a.id] == null) titles[a.id] = a.title
    }
    return titles[panelProject] ?? `삭제된 공고 #${panelProject}`
  })
  async function loadList() {
    error = ''
    try {
      const r = await api<{ conversations: Conv[] }>('/api/ai/conversations')
      convs = r.conversations
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }

  $effect(() => {
    loadList()
    // 새 대화 헤더용 공고 제목 매핑 (1회)
    api<{ inbox: { id: string; title: string }[] }>('/api/inbox')
      .then((r) => {
        const m: Record<string, string> = {}
        for (const it of r.inbox) m[it.id] = it.title
        inboxTitles = m
      })
      .catch(() => {})
  })

  async function remove(id: number) {
    try {
      await api(`/api/ai/conversations/${id}`, { method: 'DELETE' })
      confirmingDelete = null
      await loadList()
      if (selectedId === id) location.hash = '/chats'
    } catch (e) {
      error = String(e)
      confirmingDelete = null
    }
  }

  function fmtDate(iso: string): string {
    return iso.slice(5, 16).replace('T', ' ')
  }
  function tok(c: Conv): string {
    return c.tokens_in + c.tokens_out > 0
      ? `${(c.tokens_in + c.tokens_out).toLocaleString()} tok`
      : ''
  }
</script>

<div class="page-head">
  <h1>AI 대화</h1>
</div>

{#if error}<div class="banner err">{error}</div>{/if}

<div class="chats">
  <aside class="panel list">
    <button
      class="newbtn"
      onclick={() => {
        if (selectedId != null) location.hash = '/chats'
      }}
      disabled={selectedId == null}
      title="새 대화"
    >
      ＋ 새 대화
    </button>
    {#if loading}
      <div class="empty">불러오는 중…</div>
    {:else if !convs.length}
      <div class="empty">대화가 없습니다</div>
    {:else}
      {#each convs as c (c.id)}
        <div class="convrow">
          <a
            class="conv"
            class:active={c.id === selectedId}
            href="#/chats/{c.id}"
          >
            <span class="ctitle">{c.title || `(제목 없음)`}</span>
            <span class="cmeta">
              {#if c.project_id}
                <span class="badge {c.project_title ? 'muted' : 'warn'}">
                  {c.project_title ?? '삭제된 공고'}
                </span>
              {/if}
              <span class="dim">{fmtDate(c.created_at)}</span>
              {#if tok(c)}<span class="mono dim">{tok(c)}</span>{/if}
            </span>
          </a>
          {#if confirmingDelete === c.id}
            <span class="confirm">
              삭제할까요?
              <button class="danger" onclick={() => void remove(c.id)}>삭제</button>
              <button class="ghost" onclick={() => (confirmingDelete = null)}>취소</button>
            </span>
          {:else}
            <button
              class="ghost del"
              title="대화 삭제"
              onclick={() => (confirmingDelete = c.id)}
            >×</button>
          {/if}
        </div>
      {/each}
    {/if}
  </aside>

  <section class="panel pane">
    {#if selectedId != null}
      {#key selectedId}
        <ChatPanel
          conversationId={selectedId}
          onActivity={loadList}
        />
      {/key}
    {:else if panelProject}
      <ChatPanel
        projectId={panelProject}
        projectTitle={projectTitle}
        onConversation={(id) => (location.hash = `/chats/${id}`)}
        onActivity={loadList}
      />
    {:else}
      <ChatPanel
        onConversation={(id) => (location.hash = `/chats/${id}`)}
        onActivity={loadList}
      />
    {/if}
  </section>
</div>

<style>
  .chats {
    display: grid;
    grid-template-columns: 300px 1fr;
    gap: 1rem;
    align-items: stretch;
    height: calc(100vh - 9.5rem);
    min-height: 22rem;
  }
  .list {
    overflow-y: auto;
    padding: 0.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }
  .newbtn {
    margin: 0.15rem 0.15rem 0.4rem;
  }
  .convrow {
    position: relative;
  }
  .conv {
    display: flex;
    flex-direction: column;
    gap: 0.28rem;
    padding: 0.55rem 1.9rem 0.55rem 0.65rem;
    border-radius: 10px;
    text-decoration: none;
    color: inherit;
    border: 1px solid transparent;
  }
  .conv:hover {
    background: var(--surface-hover);
  }
  .conv.active {
    background: var(--brand-soft);
    border-color: var(--brand-ring);
  }
  .ctitle {
    font-size: 0.82rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cmeta {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    font-size: 0.7rem;
    flex-wrap: wrap;
  }
  .del {
    position: absolute;
    top: 0.3rem;
    right: 0.35rem;
    padding: 0 0.3rem;
    font-size: 0.9rem;
    color: var(--faint);
  }
  .del:hover {
    color: var(--bad);
  }
  .confirm {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.3rem 0.55rem 0.45rem;
    font-size: 0.72rem;
    color: var(--warn);
  }
  .danger {
    background: var(--bad-bg);
    color: var(--bad);
    border: 1px solid var(--bad);
  }
  .pane {
    padding: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
  .pane :global(*) {
    min-height: 0;
  }
  @media (max-width: 860px) {
    .chats {
      grid-template-columns: 1fr;
      height: auto;
    }
    .list {
      max-height: 14rem;
    }
    .pane {
      height: calc(100vh - 18rem);
      min-height: 20rem;
    }
  }
</style>
