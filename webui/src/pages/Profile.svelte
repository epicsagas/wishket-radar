<script lang="ts">
  import { api } from '../api'
  import { appState, refresh, type FileEntry } from '../store'
  import FileEditor from '../components/FileEditor.svelte'

  interface Skill { name: string; keywords: string[]; weight: number }
  interface ProfileData {
    name: string | null
    headline: string | null
    skills: Skill[]
    roles: string[]
    notes: string | null
  }

  let profile = $state<ProfileData | null>(null)
  let raw = $state<string | null>(null)
  let hasComments = $state(false)
  let tab = $state<'profile' | 'portfolio' | 'ai'>('profile')
  let mode = $state<'form' | 'yaml'>('form')

  // AI 설정 (BYOK) — 키는 마스킹으로만 표시되고, 빈 칸으로 저장하면 기존 키 유지
  interface AiConf {
    configured: boolean
    provider?: string
    base_url?: string | null
    model?: string
    temperature?: number | null
    api_key_masked?: string
  }
  let ai = $state<AiConf | null>(null)
  let aiKey = $state('')
  let aiSaving = $state(false)
  let aiSaved = $state(false)
  let aiError = $state('')

  function loadAi() {
    api<AiConf>('/api/settings/ai')
      .then((r) => (ai = r))
      .catch((e) => (aiError = String(e)))
  }
  loadAi()

  async function saveAi() {
    if (!ai) return
    aiSaving = true
    aiError = ''
    try {
      ai = await api<AiConf>('/api/settings/ai', {
        method: 'PUT',
        body: JSON.stringify({
          provider: ai.provider ?? 'anthropic',
          base_url: ai.base_url || null,
          model: ai.model ?? '',
          temperature: ai.temperature ?? null,
          api_key: aiKey,
        }),
      })
      aiKey = ''
      aiSaved = true
      setTimeout(() => (aiSaved = false), 2500)
    } catch (e) {
      aiError = String(e)
    } finally {
      aiSaving = false
    }
  }

  // 포트폴리오 — 프로필과 같은 층위의 "나를 설명하는 자산"
  let folio = $state<FileEntry[]>([])
  let folioSel = $state('')
  let folioLoading = $state(true)
  api<{ files: FileEntry[] }>('/api/files/portfolios')
    .then((r) => {
      folio = r.files
      if (r.files.length) folioSel = r.files[0].name
    })
    .catch(() => { /* 없으면 빈 목록 */ })
    .finally(() => (folioLoading = false))
  let yamlText = $state('')
  let editingYaml = $state(false)
  let error = $state('')
  let saveError = $state('')
  let saving = $state(false)
  let saved = $state(false)
  let dirty = $state(false)

  // 키워드는 쉼표 구분 문자열로 편집한다 (배열 직접 편집은 비개발자에게 어렵다)
  let keywordText = $state<string[]>([])

  function load() {
    api<{ content: string | null; profile: ProfileData | null; has_comments: boolean }>('/api/profile')
      .then((r) => {
        raw = r.content
        yamlText = r.content ?? ''
        hasComments = r.has_comments
        profile = r.profile
        keywordText = r.profile?.skills.map((s) => s.keywords.join(', ')) ?? []
        dirty = false
      })
      .catch((e) => (error = String(e)))
  }
  load()

  // 프리셋 (v0.7) — 용도별 매칭 프리셋. 전환하면 스캔·AI 평가·대화가 전부 활성 프리셋을 쓴다
  let presets = $state<string[]>([])
  let activePreset = $state<string | null>(null)
  let presetError = $state('')

  function loadPresets() {
    api<{ names: string[]; active: string | null }>('/api/profile/presets')
      .then((r) => {
        presets = r.names
        activePreset = r.active
      })
      .catch((e) => (presetError = String(e)))
  }
  loadPresets()

  async function activatePreset(name: string) {
    if (name === activePreset) return
    if (dirty && !confirm('저장되지 않은 변경이 사라집니다. 전환할까요?')) return
    presetError = ''
    try {
      await api(`/api/profile/presets/${encodeURIComponent(name)}/activate`, { method: 'POST' })
      activePreset = name
      load()
      await refresh()
    } catch (e) {
      presetError = String(e)
    }
  }

  async function newPreset() {
    const name = prompt('새 프리셋 이름 — 현재 프로필 내용이 복제됩니다')
    if (!name?.trim()) return
    presetError = ''
    try {
      await api('/api/profile/presets', {
        method: 'POST',
        body: JSON.stringify({ name: name.trim() }),
      })
      loadPresets()
    } catch (e) {
      presetError = String(e)
    }
  }

  async function removePreset(name: string) {
    if (!confirm(`프리셋 "${name}"을 삭제할까요?`)) return
    presetError = ''
    try {
      await api(`/api/profile/presets/${encodeURIComponent(name)}`, { method: 'DELETE' })
      loadPresets()
    } catch (e) {
      presetError = String(e)
    }
  }

  // 가중치 보정 (v0.7) — 공고 히스토리 기반 조정안. 적용은 양식에 반영만,
  // 저장 전까지 프로필은 불변.
  interface WeightSuggestion {
    name: string
    from: number | null
    to: number
    reason: string
  }
  let wBusy = $state(false)
  let wError = $state('')
  let wSugs = $state<WeightSuggestion[]>([])

  async function suggestWeights() {
    wBusy = true
    wError = ''
    wSugs = []
    try {
      const r = await api<{ suggestions: WeightSuggestion[] }>('/api/ai/weights', {
        method: 'POST',
      })
      wSugs = r.suggestions
    } catch (e) {
      wError = String(e)
    } finally {
      wBusy = false
    }
  }

  function applyWeights() {
    if (!profile) return
    for (const s of wSugs) {
      const sk = profile.skills.find((k) => k.name === s.name)
      if (sk) sk.weight = s.to
    }
    touch()
    wSugs = []
  }

  function touch() { dirty = true; saved = false }

  function addSkill() {
    profile!.skills = [...profile!.skills, { name: '', keywords: [], weight: 1 }]
    keywordText = [...keywordText, '']
    touch()
  }
  function removeSkill(i: number) {
    profile!.skills = profile!.skills.filter((_, j) => j !== i)
    keywordText = keywordText.filter((_, j) => j !== i)
    touch()
  }
  function moveSkill(i: number, dir: -1 | 1) {
    const j = i + dir
    if (j < 0 || j >= profile!.skills.length) return
    const s = [...profile!.skills]
    ;[s[i], s[j]] = [s[j], s[i]]
    const k = [...keywordText]
    ;[k[i], k[j]] = [k[j], k[i]]
    profile!.skills = s
    keywordText = k
    touch()
  }

  function addRole() { profile!.roles = [...profile!.roles, '']; touch() }
  function removeRole(i: number) {
    profile!.roles = profile!.roles.filter((_, j) => j !== i)
    touch()
  }

  async function saveForm() {
    if (!profile) return
    saving = true
    saveError = ''
    try {
      const payload: ProfileData = {
        ...profile,
        skills: profile.skills.map((s, i) => ({
          ...s,
          name: s.name.trim(),
          keywords: keywordText[i]
            .split(',')
            .map((k) => k.trim())
            .filter(Boolean),
        })),
        roles: profile.roles.map((r) => r.trim()).filter(Boolean),
      }
      await api('/api/profile/structured', { method: 'PUT', body: JSON.stringify(payload) })
      saved = true
      load()
      await refresh()
      setTimeout(() => (saved = false), 2500)
    } catch (e) {
      saveError = String(e)
    } finally {
      saving = false
    }
  }

  async function saveYaml() {
    saving = true
    saveError = ''
    try {
      await api('/api/profile', { method: 'PUT', body: JSON.stringify({ content: yamlText }) })
      saved = true
      editingYaml = false
      load()
      await refresh()
      setTimeout(() => (saved = false), 2500)
    } catch (e) {
      saveError = String(e)
    } finally {
      saving = false
    }
  }

  const totalWeight = $derived(profile?.skills.reduce((a, s) => a + (s.weight || 0), 0) ?? 0)
</script>

<div class="page-head">
  <h1>내 정보</h1>
  <span class="sub">
    {tab === 'profile'
      ? 'profile.yaml · 저장 즉시 다음 스캔부터 반영'
      : tab === 'portfolio'
        ? `포트폴리오 ${folio.length}건 · 제안서 첨부에 재사용`
        : 'BYOK — 키는 이 서버에만 저장되고 응답으로 다시 보내지 않습니다'}
  </span>
</div>

<div class="toolbar">
  <button class:ghost={tab !== 'profile'} onclick={() => (tab = 'profile')}>매칭 프로필</button>
  <button class:ghost={tab !== 'portfolio'} onclick={() => (tab = 'portfolio')}>포트폴리오</button>
  <button class:ghost={tab !== 'ai'} onclick={() => (tab = 'ai')}>AI 설정</button>
</div>

{#if tab === 'ai'}
  <div class="panel sec">
    <h2>AI 설정 <span class="dim">BYOK · 인박스 상세에서 "AI 평가"에 사용</span></h2>
    {#if aiError}<div class="banner err">{aiError}</div>{/if}
    {#if !ai}
      <div class="empty">불러오는 중…</div>
    {:else}
      <div class="field">
        <label for="ai-provider">공급자</label>
        <select id="ai-provider" bind:value={ai.provider} onchange={() => (ai = { ...ai!, provider: ai!.provider ?? 'anthropic' })}>
          <option value="anthropic">Anthropic</option>
          <option value="openai">OpenAI</option>
          <option value="compatible">호환 엔드포인트 (OpenAI 형식)</option>
        </select>
      </div>
      {#if ai.provider === 'compatible'}
        <div class="field">
          <label for="ai-base">Base URL</label>
          <input id="ai-base" type="text" bind:value={ai.base_url} placeholder="예: https://openrouter.ai/api/v1" />
        </div>
      {/if}
      <div class="field">
        <label for="ai-key">API 키</label>
        <input
          id="ai-key" type="password" bind:value={aiKey}
          placeholder={ai.configured ? `저장됨 (${ai.api_key_masked}) — 바꾸려면 입력` : '키를 입력하세요'}
          autocomplete="off"
        />
      </div>
      <div class="field">
        <label for="ai-model">모델</label>
        <input id="ai-model" type="text" bind:value={ai.model} placeholder="예: claude-sonnet-5 / gpt-5" />
      </div>
      <div class="field">
        <label for="ai-temp">온도</label>
        <input id="ai-temp" type="number" step="0.1" min="0" max="2" bind:value={ai.temperature} placeholder="기본값" />
      </div>
      <div class="savebar">
        <button onclick={saveAi} disabled={aiSaving}>
          {aiSaving ? '저장 중…' : aiSaved ? '저장됨' : '저장'}
        </button>
      </div>
    {/if}
  </div>
{:else if tab === 'portfolio'}
  {#if folioLoading}
    <div class="panel"><div class="empty">불러오는 중…</div></div>
  {:else if folio.length === 0}
    <div class="panel">
      <div class="empty">
        <strong>포트폴리오가 없습니다</strong>
        채팅에서 "이 프로젝트로 포트폴리오 써줘"라고 하면 위시켓 등록 폼 양식으로 만들어 드립니다.
      </div>
    </div>
  {:else}
    <div class="split">
      <div class="panel list">
        <ul class="filelist">
          {#each folio as f (f.name)}
            <li class:sel={folioSel === f.name}>
              <button class="filebtn" onclick={() => (folioSel = f.name)}>{f.name}</button>
            </li>
          {/each}
        </ul>
      </div>
      <div><FileEditor root="portfolios" name={folioSel} /></div>
    </div>
  {/if}
{:else}
{#if $appState?.profile_external}
  <div class="banner info">
    매칭(scout/scan)은 다른 경로의 프로필을 사용 중: <span class="mono">{$appState.profile_external}</span>
  </div>
{/if}

{#if error}
  <div class="banner err">{error}</div>
{:else if raw === null}
  <div class="panel"><div class="empty"><strong>profile.yaml 없음</strong>온보딩으로 생성하세요.</div></div>
{:else}
  <div class="panel sec">
    <h2>
      프로필 프리셋
      <span class="dim">용도별 매칭 프리셋 — 전환하면 스캔·AI 평가·대화가 전부 활성 프리셋으로</span>
    </h2>
    {#if presetError}<div class="banner err">{presetError}</div>{/if}
    <div class="presetrow">
      <select
        aria-label="프리셋 전환"
        value={activePreset ?? ''}
        onchange={(e) => activatePreset((e.currentTarget as HTMLSelectElement).value)}
        disabled={!presets.length}
      >
        {#if !presets.length}<option value="">프리셋 없음</option>{/if}
        {#each presets as p (p)}
          <option value={p}>{p}{p === activePreset ? ' · 활성' : ''}</option>
        {/each}
      </select>
      <button class="ghost" onclick={newPreset}>＋ 새 프리셋</button>
      {#if activePreset && presets.length > 1}
        <button class="ghost del" onclick={() => removePreset(activePreset!)}>삭제</button>
      {/if}
    </div>
  </div>

  <div class="toolbar">
    <button class:ghost={mode !== 'form'} onclick={() => (mode = 'form')}>양식</button>
    <button class:ghost={mode !== 'yaml'} onclick={() => (mode = 'yaml')}>YAML</button>
    {#if saved}<span class="dim" style="align-self: center">저장됨</span>{/if}
  </div>

  {#if saveError}<div class="banner err">{saveError}</div>{/if}

  {#if mode === 'form'}
    {#if !profile}
      <div class="banner err">
        YAML을 구조로 읽지 못했습니다. YAML 탭에서 문법을 고쳐 주세요.
      </div>
    {:else}
      {#if hasComments}
        <div class="banner info">
          원문에 주석(#)이 있습니다. 양식으로 저장하면 주석은 사라집니다. 보존하려면 YAML 탭에서 편집하세요.
        </div>
      {/if}

      <div class="panel sec">
        <h2>기본 정보</h2>
        <div class="field">
          <label for="p-name">이름</label>
          <input id="p-name" type="text" bind:value={profile.name} oninput={touch} placeholder="예: epiccounty" />
        </div>
        <div class="field">
          <label for="p-head">한 줄 소개</label>
          <input id="p-head" type="text" bind:value={profile.headline} oninput={touch} placeholder="예: 19년 시니어 백엔드 · 결제·핀테크" />
        </div>
      </div>

      <div class="panel sec">
        <h2>
          기술 <span class="dim">{profile.skills.length}개 · 총 가중치 {totalWeight}</span>
          <button
            class="ghost mini"
            style="margin-left: auto"
            onclick={suggestWeights}
            disabled={wBusy}
            title="관심·스킵 히스토리 기반 가중치 조정안"
          >
            {wBusy ? '분석 중…' : 'AI 보정'}
          </button>
        </h2>
        <p class="dim hint">
          가중치가 높을수록 매칭 점수에 크게 반영됩니다. 키워드는 쉼표로 구분하며, 공고 본문에 이 단어가 있으면 점수가 올라갑니다.
        </p>
        {#if wError}<div class="banner err">{wError}</div>{/if}
        {#if wSugs.length}
          <div class="banner info">
            조정안 {wSugs.length}건 — "적용"하면 양식에 반영됩니다(저장 전까지 프로필은 불변).
            <button onclick={applyWeights}>적용</button>
            <button class="ghost" onclick={() => (wSugs = [])}>무시</button>
          </div>
          <table class="wsug">
            <thead><tr><th>스킬</th><th>현재</th><th>제안</th><th>근거</th></tr></thead>
            <tbody>
              {#each wSugs as s (s.name)}
                <tr>
                  <td>{s.name}</td>
                  <td class="mono">{s.from ?? '-'}</td>
                  <td class="mono">{s.to}</td>
                  <td class="reason">{s.reason}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
        {#each profile.skills as sk, i (i)}
          <div class="skill">
            <div class="srow">
              <input
                class="sname" type="text" bind:value={sk.name} oninput={touch}
                placeholder="기술 이름 (예: Rust)" aria-label="기술 이름"
              />
              <label class="wlabel">
                가중치
                <select bind:value={sk.weight} onchange={touch} aria-label="가중치">
                  {#each [1, 2, 3, 4, 5] as w}<option value={w}>{w}</option>{/each}
                </select>
              </label>
              <div class="sbtns">
                <button class="ghost mini" onclick={() => moveSkill(i, -1)} disabled={i === 0} aria-label="위로">↑</button>
                <button class="ghost mini" onclick={() => moveSkill(i, 1)} disabled={i === profile.skills.length - 1} aria-label="아래로">↓</button>
                <button class="ghost mini del" onclick={() => removeSkill(i)} aria-label="삭제">삭제</button>
              </div>
            </div>
            <input
              class="skw" type="text" bind:value={keywordText[i]} oninput={touch}
              placeholder="키워드 (쉼표 구분) — 예: rust, 러스트, cargo, axum" aria-label="키워드"
            />
          </div>
        {/each}
        <button class="ghost" onclick={addSkill}>+ 기술 추가</button>
      </div>

      <div class="panel sec">
        <h2>수행 가능한 역할</h2>
        <div class="roles">
          {#each profile.roles as _, i (i)}
            <div class="rrow">
              <input type="text" bind:value={profile.roles[i]} oninput={touch} placeholder="예: 백엔드 개발자" aria-label="역할" />
              <button class="ghost mini del" onclick={() => removeRole(i)} aria-label="삭제">삭제</button>
            </div>
          {/each}
        </div>
        <button class="ghost" onclick={addRole}>+ 역할 추가</button>
      </div>

      <div class="panel sec">
        <h2>메모</h2>
        <textarea bind:value={profile.notes} oninput={touch} placeholder="근무 조건, 선호 도메인 등" style="min-height: 8rem"></textarea>
      </div>

      <div class="savebar">
        <button onclick={saveForm} disabled={saving || !dirty}>
          {saving ? '저장 중…' : dirty ? '저장' : '변경 없음'}
        </button>
        {#if dirty}<button class="ghost" onclick={load} disabled={saving}>되돌리기</button>{/if}
      </div>
    {/if}
  {:else}
    <div class="filepane">
      <div class="bar">
        <span class="mono dim">profile.yaml</span>
        <div class="row">
          {#if editingYaml}
            <button onclick={saveYaml} disabled={saving}>{saving ? '검증 중…' : '검증 후 저장'}</button>
            <button class="ghost" onclick={() => { yamlText = raw ?? ''; editingYaml = false }} disabled={saving}>취소</button>
          {:else}
            <button onclick={() => (editingYaml = true)}>편집</button>
          {/if}
        </div>
      </div>
      {#if editingYaml}
        <textarea bind:value={yamlText} spellcheck="false"></textarea>
      {:else}
        <pre class="view plain">{raw}</pre>
      {/if}
    </div>
  {/if}
{/if}
{/if}

<style>
  .list { max-height: calc(100vh - 13rem); overflow: auto; }
  .sec { padding: 1rem 1.15rem 1.15rem; margin-bottom: 0.9rem; }
  .sec h2 { display: flex; align-items: baseline; gap: 0.5rem; }
  .sec h2 .dim { text-transform: none; letter-spacing: 0; font-size: 0.72rem; font-weight: 400; }
  .hint { font-size: 0.76rem; margin: -0.3rem 0 0.9rem; }

  .field { display: grid; grid-template-columns: 6rem 1fr; gap: 0.7rem; align-items: center; margin-bottom: 0.6rem; }
  .field label { color: var(--faint); font-size: 0.8rem; }
  .field input { width: 100%; }

  .skill {
    border: 1px solid var(--border); border-radius: 10px;
    padding: 0.6rem 0.7rem; margin-bottom: 0.5rem; background: var(--inset);
  }
  .srow { display: flex; gap: 0.5rem; align-items: center; margin-bottom: 0.45rem; flex-wrap: wrap; }
  .sname { flex: 1; min-width: 9rem; font-weight: 600; }
  .skw { width: 100%; font-family: var(--font-mono); font-size: 0.76rem; }
  .wlabel { display: flex; align-items: center; gap: 0.35rem; color: var(--faint); font-size: 0.75rem; }
  .sbtns { display: flex; gap: 0.25rem; }
  .mini { padding: 0.2rem 0.45rem; font-size: 0.72rem; }
  .del:hover { color: var(--bad); border-color: var(--bad); }

  .roles { display: grid; gap: 0.4rem; margin-bottom: 0.6rem; }
  .rrow { display: flex; gap: 0.4rem; }
  .rrow input { flex: 1; }

  .savebar {
    position: sticky; bottom: 0; display: flex; gap: 0.5rem;
    padding: 0.8rem 0; background: linear-gradient(transparent, var(--bg) 40%);
  }

  .filepane {
    display: flex; flex-direction: column;
    height: calc(100vh - 15rem); min-height: 22rem;
    border: 1px solid var(--border); border-radius: var(--radius);
    background: var(--surface); overflow: hidden;
  }
  .bar {
    display: flex; align-items: center; justify-content: space-between; gap: 0.6rem;
    padding: 0.5rem 0.8rem; border-bottom: 1px solid var(--border);
    background: var(--inset); flex: none;
  }
  .view { flex: 1; overflow: auto; padding: 1.1rem 1.3rem; margin: 0; }
  .plain {
    white-space: pre-wrap; word-break: break-word;
    font-family: var(--font-mono); font-size: 0.8rem; line-height: 1.7; color: var(--muted);
  }
  .filepane textarea { flex: 1; min-height: 0; border: none; border-radius: 0; background: var(--bg); resize: none; }
  .filepane textarea:focus { box-shadow: none; }
  .presetrow { display: flex; gap: 0.45rem; align-items: center; }
  .presetrow select { min-width: 12rem; }
  .wsug { width: 100%; border-collapse: collapse; margin: 0.6rem 0; font-size: 0.8rem; }
  .wsug th, .wsug td { text-align: left; padding: 0.35rem 0.5rem; border-bottom: 1px solid var(--border); vertical-align: top; }
  .wsug th { color: var(--faint); font-weight: 500; font-size: 0.72rem; }
  .wsug .reason { color: var(--muted); }
</style>
