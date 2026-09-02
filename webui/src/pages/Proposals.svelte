<script lang="ts">
  import { api } from '../api'
  import { appState, type FileEntry } from '../store'
  import FileEditor from '../components/FileEditor.svelte'

  const ROOTS = [
    { key: 'proposals', label: '제안서' },
    { key: 'portfolios', label: '포트폴리오' },
  ] as const

  // 상세에서 "관련 문서"를 눌러 들어오면 해당 파일을 바로 연다
  const entry = new URLSearchParams(location.hash.split('?')[1] ?? '')
  let root = $state<'proposals' | 'portfolios'>(
    entry.get('root') === 'portfolios' ? 'portfolios' : 'proposals',
  )
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
    const r = root
    // 탭을 바꾸면 선택을 반드시 비운다 — 안 그러면 이전 탭 파일을 조회해 404가 난다
    selected = ''
    files = []
    error = ''
    loading = true
    api<{ files: FileEntry[] }>(`/api/files/${r}`)
      .then((res) => {
        files = res.files
        const pick = wanted && res.files.some((f) => f.name === wanted) ? wanted : res.files[0]?.name
        if (pick) selected = pick
        wanted = '' // 1회만 적용
      })
      .catch((e) => (error = String(e)))
      .finally(() => (loading = false))
  })
</script>

<div class="page-head">
  <h1>{ROOTS.find((r) => r.key === root)?.label}</h1>
  <span class="sub">
    {files.length}건{groups.filter((g) => g.id).length ? ` · 공고 ${groups.filter((g) => g.id).length}건` : ''} · 편집 시 이전 본문은 .bak 보관
  </span>
</div>

{#if error}<div class="banner err">{error}</div>{/if}

<div class="toolbar">
  {#each ROOTS as r}
    <button class:ghost={root !== r.key} onclick={() => (root = r.key)}>{r.label}</button>
  {/each}
</div>

<div class="split">
  <div class="panel list">
    {#if loading}
      <div class="empty">불러오는 중…</div>
    {:else if files.length === 0}
      <div class="empty">
        <strong>{root === 'proposals' ? '제안서가 없습니다' : '포트폴리오가 없습니다'}</strong>
        {root === 'proposals'
          ? '채팅에서 "이 공고 지원서 써줘"로 만들면 여기에 쌓입니다.'
          : '채팅에서 "포트폴리오 써줘"로 만들면 여기에 쌓입니다.'}
      </div>
    {:else}
      {#each groups as g (g.id || '__misc')}
        <div class="group">
          <div class="ghead">
            {#if g.id}
              <a href="#/pipeline/{g.id}" class="gid">{g.id}</a>
              <span class="gtitle">{g.title ?? ''}</span>
            {:else}
              <span class="gid misc">기타</span>
            {/if}
            <span class="gcount">{g.items.length}</span>
          </div>
          <ul class="filelist">
            {#each g.items as f (f.name)}
              <li class:sel={selected === f.name}>
                <button class="filebtn" onclick={() => (selected = f.name)}>{f.name}</button>
              </li>
            {/each}
          </ul>
        </div>
      {/each}
    {/if}
  </div>
  <div>
    {#if selected}
      <FileEditor {root} name={selected} />
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
