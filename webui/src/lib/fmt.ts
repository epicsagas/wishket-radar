// 표시용 포맷 헬퍼 — 페이지 간 D-day/등급 표기를 한 곳에서 결정한다.

export function dday(today: string, deadline: string): number | null {
  const t = Date.parse(today)
  const d = Date.parse(deadline)
  if (Number.isNaN(t) || Number.isNaN(d)) return null
  return Math.round((d - t) / 86400000)
}

export function ddayLabel(d: number | null): string {
  if (d === null) return '—'
  if (d === 0) return 'D-DAY'
  return d > 0 ? `D-${d}` : `D+${-d}`
}

export function ddayTone(d: number | null): string {
  if (d === null) return 'muted'
  if (d < 0) return 'muted'
  if (d <= 3) return 'bad'
  if (d <= 7) return 'warn'
  return 'info'
}

export function gradeTone(grade: string): string {
  const g = grade.trim().toUpperCase()
  if (g.startsWith('A')) return 'good'
  if (g.startsWith('B')) return 'warn'
  if (g.startsWith('C')) return 'bad'
  // matches.md의 매칭 점수(숫자)도 등급 칸에 올 수 있다
  const n = parseInt(g, 10)
  if (!Number.isNaN(n)) return n >= 30 ? 'good' : n >= 20 ? 'warn' : 'muted'
  return 'muted'
}

export function statusTone(status: string): string {
  switch (status) {
    case '완료':
    case '체결':
    case '진행 중':
      return 'good'
    case '미팅':
    case '상담':
      return 'info'
    case '미체결':
    case '탈락':
      return 'bad'
    case '철회':
    case '관심':
      return 'muted'
    default:
      return 'warn' // 지원
  }
}
