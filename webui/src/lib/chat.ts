// SSE 채팅 클라이언트 — 서버(ai.rs)가 공급자 원문 프레임을 중계하므로
// 두 포맷(anthropic / openai·compatible) 모두에서 텍스트 delta만 뽑는다.
// usage는 서버가 영속하므로 클라이언트는 텍스트만 처리한다.

export interface ChatTurn {
  role: 'user' | 'assistant'
  content: string
}

export interface StreamResult {
  text: string
  conversationId: number | null
  error: string | null
}

/// 프레임 하나(파싱된 JSON)에서 표시할 텍스트 delta. 해당 없으면 null.
export function deltaOf(frame: unknown): string | null {
  const j = frame as Record<string, any>
  if (j?.type === 'content_block_delta' && typeof j.delta?.text === 'string') {
    return j.delta.text // anthropic
  }
  if (typeof j?.choices?.[0]?.delta?.content === 'string') {
    return j.choices[0].delta.content // openai·compatible
  }
  return null
}

/// 프레임에서 오류 메시지. 해당 없으면 null.
export function errorOf(frame: unknown): string | null {
  const j = frame as Record<string, any>
  if (j?.type === 'error') return j.error?.message ?? '공급자 오류'
  if (j?.error?.message) return j.error.message
  return null
}

export async function streamChat(
  body: { message: string; conversation_id?: number; project_id?: string },
  onDelta: (text: string) => void,
  signal?: AbortSignal,
): Promise<StreamResult> {
  const res = await fetch('/api/ai/chat', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${localStorage.getItem('wk_token') ?? ''}`,
    },
    body: JSON.stringify(body),
    signal,
  })
  const conversationId = Number(res.headers.get('x-conversation-id')) || null
  if (!res.ok) {
    if (res.status === 401) window.dispatchEvent(new Event('wk-unauthorized'))
    let msg = `${res.status}`
    try {
      const j = await res.json()
      if (j?.error) msg = `${res.status}: ${j.error}`
    } catch {
      /* 본문 없음 */
    }
    return { text: '', conversationId, error: msg }
  }
  if (!res.body) return { text: '', conversationId, error: '응답 본문이 없습니다' }

  const reader = res.body.getReader()
  const decoder = new TextDecoder()
  let buf = ''
  let text = ''
  let error: string | null = null

  const handleLine = (line: string) => {
    // 서버 relay가 data:{...}(무공간)도 받듯 클라도 겸용으로
    if (!line.startsWith('data:')) return
    const payload = line.slice(5).trim()
    if (payload === '[DONE]') return
    try {
      const frame = JSON.parse(payload)
      const err = errorOf(frame)
      if (err) error = err
      const d = deltaOf(frame)
      if (d) {
        text += d
        onDelta(d)
      }
    } catch {
      /* 불완전/비JSON 프레임 무시 */
    }
  }

  // 스트림이 끝났다는 건 서버의 usage·답변 영속이 끝났다는 뜻(tx drop 순서 보장)
  try {
    for (;;) {
      const { done, value } = await reader.read()
      if (done) break
      buf += decoder.decode(value, { stream: true })
      let nl: number
      while ((nl = buf.indexOf('\n')) >= 0) {
        handleLine(buf.slice(0, nl))
        buf = buf.slice(nl + 1)
      }
      // 개행 없는 비정상 스트림의 메모리 소진 방지
      if (buf.length > 1_000_000) {
        error = error ?? '응답 프레임이 비정상적으로 큽니다 — 스트림 중단'
        void reader.cancel().catch(() => {})
        break
      }
    }
    handleLine(buf + '\n') // 개행 없이 끝난 꼬리
  } catch (e) {
    error = error ?? String(e)
  }
  return { text, conversationId, error }
}
