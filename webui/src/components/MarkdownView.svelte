<script lang="ts">
  // 본인 state-dir 파일을 서버에서 html로 변환한 결과만 받는다(비신뢰 아님).
  let { html = '' }: { html?: string } = $props()
</script>

{#if html}
  <div class="markdown">{@html html}</div>
{:else}
  <p class="dim">(내용 없음)</p>
{/if}
