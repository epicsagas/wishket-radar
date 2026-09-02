<script lang="ts">
  import { onMount } from 'svelte'
  import { route, segments } from './router'
  import { startPolling, stopPolling, appState, stateError } from './store'
  import { getToken, setToken } from './api'
  import TokenGate from './components/TokenGate.svelte'
  import Dashboard from './pages/Dashboard.svelte'
  import Inbox from './pages/Inbox.svelte'
  import InboxDetail from './pages/InboxDetail.svelte'
  import ItemDetail from './pages/ItemDetail.svelte'
  import Pipeline from './pages/Pipeline.svelte'
  import Proposals from './pages/Proposals.svelte'
  import Profile from './pages/Profile.svelte'
  import Reports from './pages/Reports.svelte'

  let unauthorized = $state(false)

  const pages: Record<string, { comp: typeof Dashboard; label: string }> = {
    '/inbox': { comp: Inbox, label: '인박스' },
    '/dashboard': { comp: Dashboard, label: '대시보드' },
    '/pipeline': { comp: Pipeline, label: '지원 파이프라인' },
    '/proposals': { comp: Proposals, label: '제안서' },
    '/profile': { comp: Profile, label: '내 정보' },
    '/reports': { comp: Reports, label: '리포트' },
  }
  const order = Object.keys(pages)

  // /pipeline/{id} 는 항목 상세 — 각 공고가 고유 URL을 갖는다
  const seg = $derived(segments($route))
  const detailId = $derived(seg[0] === 'pipeline' && seg[1] ? seg[1] : null)
  const inboxId = $derived(seg[0] === 'inbox' && seg[1] ? seg[1] : null)
  const Page = $derived((pages[`/${seg[0] ?? ''}`] ?? pages['/inbox']).comp)

  // 네비 우측 카운트 — 데이터가 있는 곳만 표시
  const counts = $derived<Record<string, number | null>>({
    '/inbox': $appState?.inbox_count ?? null,
    '/pipeline': $appState?.applications.length ?? null,
    '/reports': $appState?.reports.length ?? null,
  })

  function onUnauthorized() {
    unauthorized = true
    stopPolling()
  }

  function onTokenSubmitted(t: string) {
    setToken(t)
    unauthorized = false
    startPolling()
  }

  onMount(() => {
    window.addEventListener('wk-unauthorized', onUnauthorized)
    if (getToken()) startPolling()
    else unauthorized = true
    return () => {
      window.removeEventListener('wk-unauthorized', onUnauthorized)
      stopPolling()
    }
  })
</script>

<div class="layout">
  <nav>
    <div class="brandmark"><span class="dot"></span> wishket radar</div>
    {#each order as href}
      <a class="navlink" class:active={`/${seg[0] ?? ''}` === href} href="#{href}">
        <span>{pages[href].label}</span>
        {#if counts[href]}<span class="count">{counts[href]}</span>{/if}
      </a>
    {/each}
    <footer>v{$appState?.version ?? '—'}</footer>
  </nav>

  <main>
    {#if $stateError}
      <div class="banner err">{$stateError}</div>
    {/if}
    {#if $appState?.applications_parse_error}
      <div class="banner err">applications.yaml 파싱 실패: {$appState.applications_parse_error}</div>
    {/if}
    {#if inboxId}
      <InboxDetail id={inboxId} />
    {:else if detailId}
      <ItemDetail id={detailId} />
    {:else}
      <Page />
    {/if}
  </main>
</div>

{#if unauthorized}
  <TokenGate initial={getToken()} onToken={onTokenSubmitted} />
{/if}
