<script lang="ts">
  import { api } from '../api'

  let {
    root,
    name,
    editable = true,
  }: { root: string; name: string; editable?: boolean } = $props()

  interface FileContent {
    name: string
    kind: string
    content: string | null
    html: string | null
    url: string | null
  }

  let data = $state<FileContent | null>(null)
  let text = $state('')
  let error = $state('')
  let saving = $state(false)
  let saved = $state(false)
  let editing = $state(false)

  $effect(() => {
    // name 바뀌면 로드하고, 편집 모드는 항상 닫는다(다른 파일로 이동)
    const n = name
    data = null
    error = ''
    editing = false
    if (!n) return
    api<FileContent>(`/api/files/${root}/${encodeURIComponent(n)}`)
      .then((d) => {
        data = d
        text = d.content ?? ''
      })
      .catch((e) => (error = String(e)))
  })

  async function save() {
    if (!name) return
    saving = true
    error = ''
    try {
      await api(`/api/files/${root}/${encodeURIComponent(name)}`, {
        method: 'PUT',
        body: JSON.stringify({ content: text }),
      })
      // 저장 후 렌더 뷰를 갱신하려면 다시 읽어야 한다(서버가 html을 만든다)
      data = await api<FileContent>(`/api/files/${root}/${encodeURIComponent(name)}`)
      text = data.content ?? ''
      editing = false
      saved = true
      setTimeout(() => (saved = false), 2000)
    } catch (e) {
      error = String(e)
    } finally {
      saving = false
    }
  }

  function cancel() {
    text = data?.content ?? ''
    editing = false
  }
</script>

{#if error}
  <div class="banner err">{error}</div>
{/if}

{#if data === null}
  <div class="panel"><div class="empty">{name ? '불러오는 중…' : '파일을 선택하세요.'}</div></div>
{:else if data.kind === 'pdf'}
  <div class="panel" style="padding: 1rem 1.15rem">
    <a href={data.url} target="_blank" rel="noopener noreferrer">{data.name} ↗</a>
    <span class="dim"> (PDF는 새 탭에서 열립니다)</span>
  </div>
{:else}
  <div class="filepane">
    <div class="bar">
      <span class="mono dim">{data.name.split('/').pop()}</span>
      <div class="row">
        {#if saved}<span class="dim">저장됨</span>{/if}
        {#if editable}
          {#if editing}
            <button onclick={save} disabled={saving}>{saving ? '저장 중…' : '저장'}</button>
            <button class="ghost" onclick={cancel} disabled={saving}>취소</button>
          {:else}
            <button onclick={() => (editing = true)}>편집</button>
          {/if}
        {/if}
      </div>
    </div>

    {#if editing}
      <textarea bind:value={text} spellcheck="false"></textarea>
    {:else if data.html}
      <div class="view markdown">{@html data.html}</div>
    {:else}
      <!-- txt/yaml 등 렌더 대상이 아닌 파일은 원문 그대로 -->
      <pre class="view plain">{data.content}</pre>
    {/if}
  </div>
{/if}

<style>
  /* 헤더/툴바를 뺀 나머지 화면 높이를 전부 쓴다 */
  .filepane {
    display: flex;
    flex-direction: column;
    height: calc(100vh - 11rem);
    min-height: 22rem;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    overflow: hidden;
  }
  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.6rem;
    padding: 0.5rem 0.8rem;
    border-bottom: 1px solid var(--border);
    background: #101010;
    flex: none;
  }
  .view {
    flex: 1;
    overflow: auto;
    padding: 1.1rem 1.3rem;
    margin: 0;
  }
  .plain {
    white-space: pre-wrap;
    word-break: break-word;
    font-family: var(--font-mono);
    font-size: 0.8rem;
    line-height: 1.7;
    color: var(--muted);
  }
  .filepane textarea {
    flex: 1;
    min-height: 0;
    border: none;
    border-radius: 0;
    background: var(--bg);
    resize: none;
  }
  .filepane textarea:focus { box-shadow: none; }
</style>
