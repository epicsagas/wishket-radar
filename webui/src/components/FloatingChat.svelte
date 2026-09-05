<script lang="ts">
  // 전 페이지 우하단 플로팅 채팅. 미니 모달에서 질문하고, 확대 버튼으로
  // 대화 화면(#/chats/{id}) 전환 — 같은 ChatPanel 컴포넌트.
  // 마지막 대화는 localStorage에 기억 — 새로고침·재기동 후에도 이어가기.
  import { go } from '../router'
  import ChatPanel from './ChatPanel.svelte'

  const LAST_CONV_KEY = 'wk_last_conversation'

  let open = $state(false)
  let convId = $state<number | null>(Number(localStorage.getItem(LAST_CONV_KEY)) || null)

  function remember(id: number | null) {
    convId = id
    if (id != null) localStorage.setItem(LAST_CONV_KEY, String(id))
    else localStorage.removeItem(LAST_CONV_KEY)
  }

  function expand() {
    open = false
    go(convId != null ? `/chats/${convId}` : '/chats')
  }

  function close() {
    open = false
  }
</script>

<button
  class="fab"
  class:active={open}
  aria-label="AI 대화"
  title="AI 대화"
  onclick={() => (open = !open)}
>
  {#if open}
    <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true">
      <path d="M6 6l12 12M18 6L6 18" />
    </svg>
  {:else}
    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M21 11.5a8.4 8.4 0 0 1-9 8.4 9 9 0 0 1-3.3-.6L3 21l1.7-4.3a8.2 8.2 0 0 1-1.2-4.3A8.4 8.4 0 0 1 12.5 4 8.4 8.4 0 0 1 21 11.5z" />
    </svg>
  {/if}
</button>

{#if open}
  <div class="sheet">
    <div class="modal">
      <div class="mhead">
        <button class="ghost mini" onclick={expand} title="대화 화면으로 열기">
          ⤢ 확대
        </button>
        <button class="ghost mini" onclick={close}>닫기</button>
      </div>
      <div class="mbody">
        <ChatPanel
          conversationId={convId}
          onConversation={remember}
        />
      </div>
    </div>
  </div>
{/if}

<style>
  .fab {
    position: fixed;
    right: 1.4rem;
    bottom: 1.4rem;
    width: 3.1rem;
    height: 3.1rem;
    border-radius: 50%;
    background: var(--brand);
    color: var(--brand-fg);
    border: none;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    box-shadow: var(--shadow-lg);
    z-index: 60;
  }
  .fab:hover {
    filter: brightness(1.08);
  }
  .sheet {
    position: fixed;
    right: 1.4rem;
    bottom: 5rem;
    width: min(23rem, calc(100vw - 2rem));
    height: min(30rem, calc(100vh - 8rem));
    z-index: 59;
  }
  .modal {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lg);
    overflow: hidden;
  }
  .mhead {
    display: flex;
    justify-content: space-between;
    padding: 0.4rem 0.55rem;
    border-bottom: 1px solid var(--border);
  }
  .mini {
    padding: 0.2rem 0.5rem;
    font-size: 0.72rem;
  }
  .mbody {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  /* 모바일: 전체화면이 기본 */
  @media (max-width: 760px) {
    .sheet {
      inset: 0;
      width: auto;
      height: auto;
      border-radius: 0;
    }
    .modal {
      border-radius: 0;
      border: none;
    }
    /* 전체화면 모달과 FAB 겹침 방지 — 헤더의 닫기 버튼 사용 */
    .fab.active {
      display: none;
    }
    .fab {
      right: 1rem;
      bottom: 1rem;
    }
  }
</style>
