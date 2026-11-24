// What the window tells the editor's technical log: which commands were called, how long they took
// and whether they failed; which tab or dialog was shown; errors of the window itself. Never what a
// command was given or gave back: a call is its name, its time and, when it failed, its message
// (the app takes out of that whatever is private before it writes it).
import type { Api } from './api'

export type Seen = { kind: 'call' | 'view' | 'error'; name: string; ok: boolean; ms: number; count: number; message: string }

/** Calls that are not recorded: the recording itself. */
const UNSEEN = new Set(['recordSeen', 'recentDiagnostics'])
/** What waits is handed over this long after the first entry; a failure at once. */
const WAIT_MS = 5000
const MOST_WAITING = 50

let hand: ((seen: Seen[]) => Promise<void>) | null = null
let waiting = new Map<string, Seen>()
let timer: ReturnType<typeof setTimeout> | undefined

/** Hands over what waits; a log that cannot be reached is no reason to stop anything. */
export function flushDiagnostics(): void {
  clearTimeout(timer)
  timer = undefined
  if (hand === null || waiting.size === 0) return
  const seen = [...waiting.values()]
  waiting = new Map()
  void hand(seen).catch(() => {})
}

function add(key: string, entry: Seen, isUrgent = false): void {
  if (hand === null) return
  const before = waiting.get(key)
  // the same thing again counts up: a command called at every keystroke is one line, not hundreds
  if (before !== undefined) waiting.set(key, { ...before, count: before.count + 1, ms: Math.max(before.ms, entry.ms), message: entry.message || before.message })
  else if (waiting.size < MOST_WAITING) waiting.set(key, entry)
  if (isUrgent) flushDiagnostics()
  else timer ??= setTimeout(flushDiagnostics, WAIT_MS)
}

/** A tab or a dialog was shown: its name, as the editor calls it. */
export function seeView(name: string): void {
  add(`view ${name} ${waiting.size}`, { kind: 'view', name, ok: true, ms: 0, count: 1, message: '' })
}

/** Something went wrong in the window itself. */
export function seeError(message: string): void {
  add(`error ${message}`, { kind: 'error', name: '', ok: false, ms: 0, count: 1, message: message.slice(0, 600) }, true)
}

/**
 * The same commands, each one timed and its outcome noted. From here on views and the window's
 * own errors are noted too.
 */
export function withDiagnostics(api: Api, now: () => number = () => performance.now()): Api {
  hand = seen => api.recordSeen(seen)
  const timed: Record<string, unknown> = {}
  for (const [name, command] of Object.entries(api) as [string, (...args: unknown[]) => Promise<unknown>][]) {
    timed[name] = UNSEEN.has(name)
      ? command
      : async (...args: unknown[]) => {
          const from = now()
          const note = (ok: boolean, message: string) => add(`call ${name} ${ok}`, { kind: 'call', name, ok, ms: Math.round(now() - from), count: 1, message }, !ok)
          try {
            const result = await command(...args)
            note(true, '')
            return result
          } catch (e) {
            note(false, String(e))
            throw e
          }
        }
  }
  return timed as Api
}

/** Errors nobody caught, and the last entries when the window goes. */
export function watchWindow(): void {
  window.addEventListener('error', event => seeError(`${event.message} (${(event.filename ?? '').split('/').pop() ?? ''}:${event.lineno})`))
  window.addEventListener('unhandledrejection', event => seeError(`unhandled: ${event.reason instanceof Error ? `${event.reason.name}: ${event.reason.message}` : String(event.reason)}`))
  window.addEventListener('pagehide', flushDiagnostics)
}

/** For tests: nothing waits and nothing is handed over. */
export function stopDiagnostics(): void {
  clearTimeout(timer)
  timer = undefined
  hand = null
  waiting = new Map()
}
