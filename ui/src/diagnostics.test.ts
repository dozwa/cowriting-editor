import { afterEach, expect, test, vi } from 'vitest'
import type { Api } from './api'
import { flushDiagnostics, seeError, seeView, type Seen, stopDiagnostics, withDiagnostics } from './diagnostics'

afterEach(() => {
  stopDiagnostics()
  vi.useRealTimers()
})

/** A stand-in for the app with three commands; `recorded` is what reached the technical log. */
function standIn() {
  const recorded: Seen[][] = []
  let time = 0
  const api = {
    recordSeen: async (seen: Seen[]) => void recorded.push(seen),
    recentDiagnostics: async () => '',
    saveDocument: async (_text: string) => ((time += 12), []),
    scope: async (_text: string) => ((time += 3), null),
    ask: async (_input: { text: string }) => {
      time += 800
      throw 'The provider answered 500 for "Is my second paragraph about Marta too long?"'
    },
  } as unknown as Api
  return { api: withDiagnostics(api, () => time), recorded }
}

test('a command is recorded by its name, its time and how it ended, and never by what it was given', async () => {
  vi.useFakeTimers()
  const { api, recorded } = standIn()
  expect(await api.saveDocument('The whole text of my thesis.')).toEqual([])
  for (let i = 0; i < 4; i++) await api.scope('The whole text of my thesis.')
  expect(recorded).toEqual([])

  // handed over together after a while, the same command as one entry
  vi.advanceTimersByTime(5000)
  expect(recorded).toEqual([[{ kind: 'call', name: 'saveDocument', ok: true, ms: 12, count: 1, message: '' }, { kind: 'call', name: 'scope', ok: true, ms: 3, count: 4, message: '' }]])
  expect(JSON.stringify(recorded)).not.toContain('thesis')
})

test('a failure is handed over at once with its message, and still fails for whoever called', async () => {
  const { api, recorded } = standIn()
  await expect(api.ask({ text: 'Is it too long?', replyTo: null, persona: null, documentText: 'My text.' }, () => {})).rejects.toMatch(/answered 500/)
  expect(recorded).toHaveLength(1)
  expect(recorded[0]!).toMatchObject([{ kind: 'call', name: 'ask', ok: false, ms: 800, count: 1 }])
  // the message goes to the app as it was; taking out what is private is the app's (its own tests say how)
  expect(recorded[0]![0]!.message).toMatch(/^The provider answered 500/)
  expect(JSON.stringify(recorded)).not.toContain('My text.')
})

test('views and the window’s own errors are recorded, the recording itself is not', async () => {
  const { api, recorded } = standIn()
  seeView('tab:log')
  seeView('dialog:export')
  seeView('tab:log')
  await api.recentDiagnostics(10)
  expect(recorded).toEqual([])
  seeError('TypeError: undefined is not an object')
  expect(recorded[0]!.map(seen => [seen.kind, seen.name || seen.message])).toEqual([['view', 'tab:log'], ['view', 'dialog:export'], ['view', 'tab:log'], ['error', 'TypeError: undefined is not an object']])

  // nothing waits afterwards
  flushDiagnostics()
  expect(recorded).toHaveLength(1)
})

test('without an app to hand it to, nothing is kept', () => {
  seeView('tab:ask')
  seeError('anything')
  const { recorded } = standIn()
  flushDiagnostics()
  expect(recorded).toEqual([])
})
