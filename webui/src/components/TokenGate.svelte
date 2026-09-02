<script lang="ts">
  import { untrack } from 'svelte'

  let { initial = '', onToken }: { initial?: string; onToken: (t: string) => void } = $props()
  // initial은 최초 렌더 시드값 — 이후 입력은 로컬 상태
  let value = $state(untrack(() => initial))
</script>

<div class="overlay dot-bg">
  <form
    onsubmit={(e) => {
      e.preventDefault()
      if (value.trim()) onToken(value.trim())
    }}
  >
    <div class="row"><span class="dot"></span><strong>wishket radar</strong></div>
    <p class="dim" style="margin: 0; font-size: 0.83rem">
      <span class="mono">~/.wishket-radar/dashboard-token</span> 값을 입력하세요.
    </p>
    <input type="text" bind:value placeholder="48자 hex 토큰" />
    <button type="submit">접속</button>
  </form>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.82);
    display: grid;
    place-items: center;
    z-index: 10;
  }
  form {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 1.5rem;
    width: min(420px, 90vw);
    display: flex;
    flex-direction: column;
    gap: 0.8rem;
    box-shadow: 0 20px 60px rgb(0 0 0 / 0.6);
  }
  .dot {
    width: 9px; height: 9px; border-radius: 50%;
    background: var(--brand); box-shadow: 0 0 12px var(--brand);
  }
</style>
