<script lang="ts">
  // 대화 스트림 + 입력. #/chats 페이지와 플로팅 모달이 같은 컴포넌트를 재사용.
  import { api, ApiError } from '../api'
  import { streamChat, type ChatTurn } from '../lib/chat'

  let {
    conversationId = null,
    projectId = null,
    projectTitle = null,
    onConversation,
    onActivity,
  }: {
    conversationId?: number | null
    projectId?: string | null
    projectTitle?: string | null
    onConversation?: (id: number | null) => void
    onActivity?: () => void
  } = $props()

  interface ConvHeader {
    id: number
    tokens_in: number
    tokens_out: number
  }

  let turns = $state<ChatTurn[]>([])
  let input = $state('')
  let streaming = $state(false)
  let error = $state('')
  let tokens = $state<{ in: number; out: number } | null>(null)
  let activeId = $state<number | null>(null)
  let streamEl: HTMLElement | null = $state(null)
  let loadedFor: number | null | undefined = undefined // undefined = 아직 안 읽음
  let aborter: AbortController | null = null

  // 언마운트(모달 닫기·경로 이동) 시 진행 중 스트림을 끊는다 —
  // 서버 relay가 부분 텍스트를 영속하므로 재진입하면 이어서 보인다
  $effect(() => () => aborter?.abort())

  // conversationId가 바뀌면 대화를 읽는다 — 스트리밍 중엔 덮어쓰지 않는다.
  // activeId가 아닌 loadedFor로 중복을 막는다(새 대화 초기화가 effect를
  // 다시 트리거해 방금 지운 대화를 재로드하는 것을 방지).
  $effect(() => {
    const id = conversationId ?? null
    if (id == null || streaming) return
    if (loadedFor === id) return
    load(id)
  })

  async function load(id: number) {
    error = ''
    try {
      const c = await api<ConvHeader & { messages: ChatTurn[] }>(
        `/api/ai/conversations/${id}`,
      )
      turns = c.messages
      tokens = { in: c.tokens_in, out: c.tokens_out }
      activeId = id
      loadedFor = id
    } catch (e) {
      if ((e as ApiError).status === 401) return
      // 저장된 마지막 대화가 이미 삭제됐으면 조용히 새 대화로 폴백
      if ((e as ApiError).status === 404) {
        activeId = null
        turns = []
        tokens = null
        onConversation?.(null)
        return
      }
      error = String(e)
    }
  }

  function newConversation() {
    if (streaming) return
    // loadedFor는 그대로 — 되돌리면 effect가 방금 지운 대화를 재로드한다
    activeId = null
    turns = []
    tokens = null
    error = ''
    input = ''
  }

  async function send() {
    const message = input.trim()
    if (!message || streaming) return
    error = ''
    input = ''
    streaming = true
    aborter = new AbortController()
    turns.push({ role: 'user', content: message })
    turns.push({ role: 'assistant', content: '' })
    const assistant = turns[turns.length - 1]
    try {
      const r = await streamChat(
        {
          message,
          ...(activeId != null ? { conversation_id: activeId } : {}),
          ...(projectId ? { project_id: projectId } : {}),
        },
        (d) => {
          assistant.content += d
          scrollBottom()
        },
        aborter.signal,
      )
      if (r.conversationId != null && r.conversationId !== activeId) {
        activeId = r.conversationId
        onConversation?.(r.conversationId)
      }
      if (r.error) error = r.error
      // 스트림 종료 = 서버 영속 완료 — 재조회로 토큰 누적과 마크다운 렌더
      // 결과(content_html)를 한 번에 교체한다. 스트리밍 중엔 plain text,
      // 완료 후에만 HTML로 — delta마다 파싱하는 비용이 없다.
      let refreshed = false
      if (activeId != null) {
        const c = await api<ConvHeader & { messages: ChatTurn[] }>(
          `/api/ai/conversations/${activeId}`,
        ).catch(() => null)
        if (c) {
          tokens = { in: c.tokens_in, out: c.tokens_out }
          turns = c.messages
          refreshed = true
        }
      }
      // 재조회 실패 시에만 — 서버 기준 상태로 교체했으면 처리 불요
      if (!refreshed && !assistant.content) turns.splice(turns.indexOf(assistant), 1)
    } catch (e) {
      error = String(e)
      if (!assistant.content) turns.splice(turns.indexOf(assistant), 1)
    } finally {
      aborter = null
      streaming = false
      onActivity?.()
      scrollBottom()
    }
  }

  function scrollBottom() {
    requestAnimationFrame(() => {
      streamEl?.scrollTo({ top: streamEl.scrollHeight })
    })
  }
</script>

<div class="chat">
  <div class="chathead">
    <span class="chatlabel">
      {projectTitle ? `공고 · ${projectTitle}` : activeId ? `대화 #${activeId}` : '새 대화'}
    </span>
    {#if tokens}
      <span class="tok" title="누적 토큰 (입력 / 출력)">
        {tokens.in.toLocaleString()} / {tokens.out.toLocaleString()}
      </span>
    {/if}
    {#if activeId != null}
      <button class="ghost mini" onclick={newConversation} disabled={streaming}>
        새 대화
      </button>
    {/if}
  </div>

  <div class="stream" bind:this={streamEl}>
    {#if !turns.length}
      <div class="empty">
        {projectTitle
          ? '이 공고에 대해 바로 물어보세요.'
          : '무엇이든 물어보세요. 공고 맥락이 필요하면 공고 상세에서 대화를 시작하세요.'}
      </div>
    {/if}
    {#each turns as t, i (i)}
      <div class="turn {t.role}">
        <div class="bubble" class:md={!!t.content_html}>
          {#if t.content_html}
            <!-- 서버 render_markdown(pulldown-cmark + ammonia)이 sanitize한 값만 -->
            <div class="markdown">{@html t.content_html}</div>
          {:else}
            {t.content || (streaming ? '…' : '')}
          {/if}
        </div>
      </div>
    {/each}
  </div>

  {#if error}<div class="banner err">{error}</div>{/if}

  <form
    class="composer"
    onsubmit={(e) => {
      e.preventDefault()
      void send()
    }}
  >
    <textarea
      rows="2"
      placeholder="메시지 입력 — Enter로 전송"
      bind:value={input}
      onkeydown={(e) => {
        if (e.key === 'Enter' && !e.shiftKey) {
          e.preventDefault()
          void send()
        }
      }}
      disabled={streaming}
    ></textarea>
    <button type="submit" disabled={streaming || !input.trim()}>
      {streaming ? '응답 중…' : '보내기'}
    </button>
  </form>
</div>

<style>
  .chat {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .chathead {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.55rem 0.9rem;
    border-bottom: 1px solid var(--border);
    font-size: 0.8rem;
  }
  .chatlabel {
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tok {
    margin-left: auto;
    font-family: var(--font-mono);
    font-size: 0.7rem;
    color: var(--faint);
  }
  .stream {
    flex: 1;
    overflow-y: auto;
    padding: 0.9rem;
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
    min-height: 0;
  }
  .turn {
    display: flex;
  }
  .turn.user {
    justify-content: flex-end;
  }
  .bubble {
    max-width: 82%;
    padding: 0.5rem 0.75rem;
    border-radius: 12px;
    font-size: 0.85rem;
    line-height: 1.6;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .user .bubble {
    background: var(--brand-soft);
    color: var(--fg);
  }
  .assistant .bubble {
    background: var(--surface-hover);
    color: var(--muted);
  }
  /* 마크다운 렌더 버블 — pre-wrap 해제하고 전역 .markdown을 버블 크기에 맞춘다 */
  .bubble.md {
    white-space: normal;
    min-width: 60%;
  }
  .md :global(.markdown) {
    font-size: 0.85rem;
    line-height: 1.65;
  }
  .md :global(.markdown > :first-child) {
    margin-top: 0;
  }
  .md :global(.markdown > :last-child) {
    margin-bottom: 0;
  }
  .md :global(.markdown p) {
    margin: 0 0 0.5rem;
  }
  .md :global(.markdown ul),
  .md :global(.markdown ol) {
    margin: 0.3rem 0;
    padding-left: 1.2rem;
  }
  .md :global(.markdown table) {
    margin: 0.5rem 0;
  }
  .composer {
    display: flex;
    gap: 0.5rem;
    padding: 0.65rem 0.9rem;
    border-top: 1px solid var(--border);
    align-items: flex-end;
  }
  .composer textarea {
    flex: 1;
    resize: none;
    background: var(--inset);
    border: 1px solid var(--border);
    border-radius: 10px;
    color: var(--fg);
    font: inherit;
    font-size: 0.85rem;
    padding: 0.5rem 0.65rem;
  }
  .composer textarea:focus {
    outline: none;
    border-color: var(--brand);
  }
  .mini {
    padding: 0.2rem 0.45rem;
    font-size: 0.72rem;
  }
</style>
