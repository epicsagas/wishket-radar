<script lang="ts">
  import { api } from '../api'
  import { appState, type FileEntry } from '../store'
  import FileEditor from '../components/FileEditor.svelte'

  // 상세에서 "관련 문서"를 눌러 들어오면 해당 파일을 바로 연다
  const entry = new URLSearchParams(location.hash.split('?')[1] ?? '')
  const root = 'proposals'
  let wanted = entry.get('file') ?? ''
  let files = $state<FileEntry[]>([])
  let selected = $state('')
  let error = $state('')
  let loading = $state(true)

  // 공고 ID별로 묶는다. ID가 없는 파일은 "기타"로.
  const groups = $derived.by(() => {
    const byId = new Map<string, FileEntry[]>()
    for (const f of files) {
      const k = f.project_id ?? ''
      if (!byId.has(k)) byId.set(k, [])
      byId.get(k)!.push(f)
    }
    // ID 있는 그룹 먼저(최근 수정순), 기타는 마지막
    return [...byId.entries()]
      .map(([id, items]) => ({
        id,
        items,
        title: id ? ($appState?.applications.find((a) => a.id === id)?.title ?? null) : null,
        mtime: Math.max(...items.map((i) => i.mtime_epoch)),
      }))
      .sort((a, b) => (!a.id ? 1 : !b.id ? -1 : 0) || b.mtime - a.mtime)
  })

  $effect(() => {
    selected = ''
    files = []
    error = ''
    loading = true
    api<{ files: FileEntry[] }>(`/api/files/${root}`)
      .then((res) => {
        files = res.files
        const pick = wanted && res.files.some((f) => f.name === wanted) ? wanted : res.files[0]?.name
        if (pick) selected = pick
        wanted = '' // 1회만 적용
      })
      .catch((e) => (error = String(e)))
      .finally(() => (loading = false))
  })

  // 제안서 AI 초안 (v0.7) — 공고별로 생성, 완료 후 편집기로 바로 연다
  let aiBusyId = $state<string | null>(null)
  let aiMsg = $state('')

  async function generateDraft(id: string) {
    aiBusyId = id
    aiMsg = ''
    try {
      const r = await api<{ path: string }>('/api/ai/proposal', {
        method: 'POST',
        body: JSON.stringify({ id }),
      })
      const res = await api<{ files: FileEntry[] }>(`/api/files/${root}`)
      files = res.files
      selected = r.path
      aiMsg = '초안이 생성되었습니다 — 편집 후 저장하세요.'
    } catch (e) {
      aiMsg = String(e)
    } finally {
      aiBusyId = null
    }
  }
</script>

<div class="page-head">
  <h1>제안서</h1>
  <span class="sub">
    {files.length}건{groups.filter((g) => g.id).length ? ` · 공고 ${groups.filter((g) => g.id).length}건` : ''} · 편집 시 이전 본문은 .bak 보관
  </span>
</div>

{#if error}<div class="banner err">{error}</div>{/if}
{#if aiMsg}<div class="banner info">{aiMsg}</div>{/if}

<div class="split">
  <div class="panel list">
    {#if loading}
      <div class="empty">불러오는 중…</div>
    {:else if files.length === 0}
      <div class="empty">
        <strong>제안서가 없습니다</strong>
        채팅에서 "이 공고 지원서 써줘"로 만들면 여기에 쌓입니다.
      </div>
    {:else}
      {#each groups as g (g.id || '__misc')}
        <div class="group">
          <div class="ghead">
            {#if g.id}
              <a href="#/pipeline/{g.id}" class="gid">{g.id}</a>
              <span class="gtitle">{g.title ?? ''}</span>
              <button
                class="ghost mini"
                style="margin-left: auto"
                onclick={() => generateDraft(g.id)}
                disabled={aiBusyId != null}
                title="AI로 제안서 초안 생성"
              >
                {aiBusyId === g.id ? '생성 중…' : 'AI 초안'}
              </button>
            {:else}
              <span class="gid misc">기타</span>
            {/if}
            <span class="gcount">{g.items.length}</span>
          </div>
          <ul class="filelist">
            {#each g.items as f (f.name)}
              <li class:sel={selected === f.name}>
                <button class="filebtn" onclick={() => (selected = f.name)}>{f.name.split('/').pop()}</button>
              </li>
            {/each}
          </ul>
        </div>
      {/each}
    {/if}
  </div>
  <div>
    {#if selected}
      <FileEditor root="proposals" name={selected} />
    {:else if !loading && files.length > 0}
      <div class="panel"><div class="empty">파일을 선택하세요.</div></div>
    {/if}
  </div>
</div>

<style>
  .group { border-bottom: 1px solid var(--border); padding: 0.5rem 0.35rem 0.6rem; }
  .group:last-child { border-bottom: none; }
  .ghead {
    display: flex; align-items: baseline; gap: 0.45rem;
    padding: 0.2rem 0.6rem 0.4rem; font-size: 0.78rem;
  }
  .gid { font-family: var(--font-mono); font-weight: 600; color: var(--brand-200); }
  .gid.misc { color: var(--faint); font-family: inherit; }
  .gtitle {
    color: var(--muted); overflow: hidden; text-overflow: ellipsis;
    white-space: nowrap; flex: 1; font-size: 0.75rem;
  }
  .gcount { color: var(--faint); font-size: 0.7rem; }
  /* 파일이 많아도 우측 편집기와 높이를 맞춘다 */
  .list { max-height: calc(100vh - 11rem); overflow: auto; }
</style>
