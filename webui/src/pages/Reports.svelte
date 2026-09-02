<script lang="ts" module>
  import { writable } from 'svelte/store'
  // Dashboard의 최근 리포트 링크가 진입 파일을 지정할 수 있게 export
  export const page = writable<string | null>(null)
</script>

<script lang="ts">
  import { api } from '../api'
  import type { FileEntry } from '../store'
  import FileEditor from '../components/FileEditor.svelte'

  let files = $state<FileEntry[]>([])
  let selected = $state('')
  let error = $state('')

  $effect(() => {
    if ($page) selected = $page
  })

  api<{ files: FileEntry[] }>('/api/files/reports')
    .then((r) => {
      files = r.files
      if (!selected && r.files.length) selected = r.files[0].name
    })
    .catch((e) => (error = String(e)))
</script>

<div class="page-head">
  <h1>리포트</h1>
  <span class="sub">{files.length}건 · 조회 전용</span>
</div>

{#if error}<div class="banner err">{error}</div>{/if}

<div class="split">
  <div class="panel list">
    <ul class="filelist">
      {#each files as f (f.name)}
        <li class:sel={selected === f.name}>
          <button class="filebtn" onclick={() => (selected = f.name)}>{f.name}</button>
        </li>
      {/each}
      {#if files.length === 0}<li><div class="empty">없음</div></li>{/if}
    </ul>
  </div>
  <div>
    <FileEditor root="reports" name={selected} editable={false} />
  </div>
</div>

<style>
  .list { max-height: calc(100vh - 11rem); overflow: auto; }
</style>
