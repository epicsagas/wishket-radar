// 해시 라우터 — 패키지 없음. SPA 폴백 불필요.

import { writable } from 'svelte/store'

export const route = writable<string>(currentRoute())

function currentRoute(): string {
  const h = location.hash.replace(/^#/, '')
  return h.startsWith('/') ? h : '/inbox'
}

/// 경로를 세그먼트로. 쿼리는 떼어낸다.
/// "/pipeline/123" → ["pipeline", "123"]
/// "/proposals?file=a.md" → ["proposals"]
export function segments(r: string): string[] {
  return r.split('?')[0].split('/').filter(Boolean)
}

export function go(path: string) {
  location.hash = path
}

window.addEventListener('hashchange', () => route.set(currentRoute()))
