// /api/state 3초 폴링. 파일이 정규 소스라 서버가 항상 최신.

import { writable } from 'svelte/store'
import { api } from './api'

export interface AppState {
  version: string
  last_scan: string | null
  seen_count: number
  inbox_count: number
  details: Record<string, CachedDetail>
  analyses: Record<string, Analysis>
  today: string
  profile: { name: string | null; headline: string | null; skills: { name: string; weight: number }[] } | null
  profile_external: string | null
  applications: Application[]
  applications_parse_error: string | null
  stats: {
    by_status: Record<string, number>
    funnel: Record<string, number>
    open: number
    won: number
    lost: number
    win_rate: number | null
    samples: number
  }
  reports: FileEntry[]
}

export interface Application {
  id: string
  title: string
  url: string | null
  grade: string | null
  quote_manwon: number | null
  applied_at: string | null
  deadline: string | null
  status: string
  status_at: string | null
  next_action: string | null
  note: string | null
}

export interface Analysis {
  grade: string | null
  title: string | null
  fit: string | null
  caution: string | null
  proposal: string | null
  report: string | null
}

export interface CachedDetail {
  description: string | null
  conditions: [string, string][]
  role: string | null
  level: string | null
  location: string | null
  matched: string[]
  skills: string[]
  budget: string | null
  duration: string | null
  detail_fetched_at: string | null
}

export interface InboxItem {
  id: string
  title: string
  url: string | null
  score: number | null
  budget: string | null
  duration: string | null
  deadline: string | null
  skills: string[]
  first_seen: string
  analysis: Analysis | null
  expired?: boolean
  title_missing?: boolean
}

export interface FileEntry {
  name: string
  size: number
  mtime_epoch: number
  project_id?: string
}

export const appState = writable<AppState | null>(null)
export const stateError = writable<string | null>(null)

let timer: ReturnType<typeof setInterval> | null = null

async function poll() {
  try {
    appState.set(await api<AppState>('/api/state'))
    stateError.set(null)
  } catch (e) {
    if ((e as { status?: number }).status !== 401) stateError.set(String(e))
  }
}

/// 변경 직후 폴링을 기다리지 않고 즉시 재조회
export async function refresh() {
  await poll()
}

export function startPolling() {
  if (timer) return
  void poll()
  timer = setInterval(poll, 3000)
}

export function stopPolling() {
  if (timer) clearInterval(timer)
  timer = null
}

// 위시켓 실제 수주 퍼널 (서버 apps.rs STATUSES와 동일 순서)
export const STATUSES = [
  '관심', '지원', '상담', '미팅', '체결', '진행 중', '완료', '미체결', '탈락', '철회',
] as const

/// 칸반에 세로로 세울 진행 단계 (종결 상태는 별도 묶음)
export const ACTIVE_STAGES = ['관심', '지원', '상담', '미팅', '체결', '진행 중'] as const
export const CLOSED_STAGES = ['완료', '미체결', '탈락', '철회'] as const

/// 퍼널 전환율 표시 순서
export const FUNNEL_STAGES = ['지원', '상담', '미팅', '체결', '완료'] as const

/// 단계별 한 줄 설명 (위시켓 공식 흐름)
export const STAGE_HINT: Record<string, string> = {
  '관심': '지원 전 검토',
  '지원': '금액·기간 제안 제출',
  '상담': '위시켓 매니저가 지원자 선발',
  '미팅': '클라이언트·파트너·매니저 삼자 미팅',
  '체결': '계약 확정',
  '진행 중': '대금 선예치 후 개발 착수',
  '완료': '승인 후 대금 지급 종료',
  '미체결': '조건 불일치로 취소',
  '탈락': '선발되지 않음',
  '철회': '본인 포기',
}
