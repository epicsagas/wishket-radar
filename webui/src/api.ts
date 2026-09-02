// 토큰 관리 + API 래퍼. 401이면 wk-unauthorized 이벤트 → TokenGate 표시.

const TOKEN_KEY = 'wk_token'

export function bootstrapToken() {
  const q = new URLSearchParams(location.search).get('token')
  if (q) {
    localStorage.setItem(TOKEN_KEY, q)
    history.replaceState(null, '', location.pathname)
  }
}

export function getToken(): string {
  return localStorage.getItem(TOKEN_KEY) ?? ''
}

export function setToken(t: string) {
  localStorage.setItem(TOKEN_KEY, t)
}

export class ApiError extends Error {
  status: number
  constructor(status: number, message: string) {
    super(message)
    this.status = status
  }
}

export async function api<T = unknown>(path: string, opts: RequestInit = {}): Promise<T> {
  const res = await fetch(path, {
    ...opts,
    headers: {
      ...(opts.body ? { 'Content-Type': 'application/json' } : {}),
      Authorization: `Bearer ${getToken()}`,
      ...opts.headers,
    },
  })
  if (res.status === 401) {
    window.dispatchEvent(new Event('wk-unauthorized'))
    throw new ApiError(401, '인증 필요')
  }
  if (!res.ok) {
    let msg = `${res.status}`
    try {
      const j = await res.json()
      if (j?.error) msg = `${res.status}: ${j.error}`
    } catch { /* 본문 없음 */ }
    throw new ApiError(res.status, msg)
  }
  return (await res.json()) as T
}
