// The window with the app behind it replaced by a stand-in that keeps a log the way the real
// one does. What is checked is what the author sees and what the window asks the app to do.
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { expect, test } from 'vitest'
import type { Api, AskEvent, LogView, Settings, SettingsView, ThreadMessage, TurnRow, Use } from './api'
import { App } from './App'
import type { Editor } from './editor'
import { linkLines } from './text'

/** A text field in place of the real editor, which needs a browser's layout. */
const plainEditor = (parent: HTMLElement, handlers: { onChange(): void; onSave(): void }): Editor & { jumps: number[] } => {
  const field = document.createElement('textarea')
  field.setAttribute('aria-label', 'Document')
  field.addEventListener('input', () => handlers.onChange())
  parent.append(field)
  const jumps: number[] = []
  lastEditor = { getText: () => field.value, setText: text => void (field.value = text), jumpToLine: line => void jumps.push(line), focus: () => {}, destroy: () => field.remove(), jumps }
  return lastEditor
}
let lastEditor: Editor & { jumps: number[] }

function standIn(options: { isReady?: boolean; answer?: string; refuse?: boolean; settings?: Partial<SettingsView>; withoutTools?: boolean } = {}) {
  const turns: TurnRow[] = []
  const calls: { saved: string[]; asked: { text: string; replyTo: string | null; documentText: string }[]; annotated: [string, Use, string][]; exported: unknown[]; settings: [Settings, string | null][]; tested: Settings[] } = { saved: [], asked: [], annotated: [], exported: [], settings: [], tested: [] }
  const view = (): LogView => ({
    stats: { turns: turns.length, today: turns.length, unannotated: turns.filter(t => t.isOpen).length, aiText: 0, annotations: turns.filter(t => t.use !== '').length, rejected: 0, guardEvents: 0 },
    turns: [...turns].reverse(),
    guard: options.refuse && turns.length > 0 ? [{ ts: '2026-11-23T10:00:00Z', tool: 'write_note', input: 'write_note ../../thesis.md', files: [], refused: true, turn: 't1' }] : [],
    words: 7,
  })
  let settings: SettingsView = { backend: 'anthropic', model: 'claude-opus-5-5', baseUrl: '', hasKey: options.isReady ?? true, isReady: options.isReady ?? true, modelStatus: 'tested', isFirstRun: false, openAtStart: '/work/thesis.md', ...options.settings }
  const api: Api = {
    getSettings: async () => settings,
    setSettings: async (next, apiKey) => {
      calls.settings.push([next, apiKey])
      settings = { ...settings, ...next, isReady: true, isFirstRun: false, modelStatus: 'untested' }
    },
    testConnection: async next => {
      calls.tested.push(next)
      if (next.baseUrl === 'http://localhost:1234/v1') throw 'localhost:1234 could not be reached. Is the local model server running?'
      return [{ id: 'gemma3:4b', name: 'gemma3:4b', status: 'untested' }, { id: 'qwen3.5:9b', name: 'qwen3.5:9b', status: 'failed' }]
    },
    pickDocument: async () => null,
    pickNewDocument: async () => null,
    confirmLeave: async () => true,
    openDocument: async path => ({ path, name: 'thesis.md', text: '# Thesis\n\nWe recruited 48 subjects.\n', language: 'en' }),
    newDocument: async path => ({ path, name: 'new.md', text: '', language: 'en' }),
    saveDocument: async text => void calls.saved.push(text),
    ask: async (input, onEvent: (event: AskEvent) => void) => {
      calls.asked.push(input)
      const id = `t${turns.length + 1}`
      const answer = options.answer ?? 'On line 3: why subjects and not participants?'
      onEvent({ kind: 'started', id })
      onEvent({ kind: 'tool', name: 'read_document' })
      onEvent({ kind: 'text', text: answer })
      turns.push({ id, ts: '2026-11-23T10:00:00Z', category: 'general', asked: input.text, answer, excerpt: answer, model: 'claude-opus-5-5', provider: 'Anthropic', toolsUsed: ['read_document'], use: '', note: '', isOpen: true, isAiText: false, replyTo: input.replyTo })
      return { id, answer, isAiText: false, refused: 0, cutOff: null, withoutTools: options.withoutTools ?? false }
    },
    logView: async () => view(),
    thread: async id => {
      const out: ThreadMessage[] = []
      for (const turn of turns) {
        if (turn.replyTo !== null) out.push({ id: turn.id, role: 'author', who: 'You', persona: '', ts: turn.ts, text: turn.asked })
        out.push({ id: turn.id, role: 'ai', who: 'Assistant', persona: '', ts: turn.ts, text: turn.answer })
        if (turn.id === id) break
      }
      return out
    },
    annotate: async (reference, use, note) => {
      calls.annotated.push([reference, use, note])
      const turn = turns.find(t => t.id === reference)!
      Object.assign(turn, { use, note, isOpen: false })
    },
    exportDisclosure: async o => (calls.exported.push(o), { paths: ['/work/.cowriting/thesis.md/disclosure/ai-register.en.md'], register: '# AI register (disclosure of AI use)' }),
  }
  return { api, calls }
}

test('the author asks, reads the answer, classifies it, and the log shows the choice', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)

  // the document named at start is opened, and nothing waits yet
  expect(await screen.findByText('thesis.md')).toBeTruthy()
  expect(((await screen.findByLabelText('Document')) as HTMLTextAreaElement).value).toContain('48 subjects')
  expect(screen.getByText('saved')).toBeTruthy()

  // the author types on: the question is asked about the text as it is now, saved or not
  await user.type(screen.getByLabelText('Document'), 'More.')
  expect(screen.getByText('not saved')).toBeTruthy()
  await user.type(screen.getByLabelText('Ask the AI'), 'Any feedback?')
  await user.click(screen.getByRole('button', { name: 'Ask' }))
  await screen.findByText(/why subjects and not participants/)
  expect(calls.asked).toEqual([{ text: 'Any feedback?', replyTo: null, documentText: '# Thesis\n\nWe recruited 48 subjects.\nMore.' }])
  expect(calls.saved).toEqual([]) // asking does not save the document

  // a line the answer names leads there in the editor
  await user.click(screen.getByRole('button', { name: 'line 3' }))
  expect(lastEditor.jumps).toEqual([3])

  // one input waits, and the tab says so
  expect(within(screen.getByRole('tab', { name: /Log/ })).getByText('1')).toBeTruthy()
  await user.type(screen.getByLabelText('Comment on this input'), 'my field says subjects')
  await user.click(screen.getByRole('button', { name: '✗ not used' }))
  await waitFor(() => expect(calls.annotated).toEqual([['t1', 'rejected', 'my field says subjects']]))
  await screen.findByText(/decided against it/)
  expect(screen.queryByRole('button', { name: '✓ used' })).toBeNull()

  // a reply continues the thread
  await user.type(screen.getByLabelText('Reply'), 'It is the usual term.')
  await user.click(screen.getByRole('button', { name: 'Reply' }))
  await waitFor(() => expect(calls.asked[1]).toMatchObject({ text: 'It is the usual term.', replyTo: 't1' }))

  // the log lists both, what waits first; the disclosure is written on request
  await user.click(screen.getByRole('tab', { name: /Log/ }))
  const rows = screen.getAllByRole('button').filter(b => b.className === 'turn')
  expect(rows.map(r => r.textContent)).toEqual([expect.stringContaining('• to classify'), expect.stringContaining('✗ not used')])
  await user.click(screen.getByRole('button', { name: 'Disclosure…' }))
  expect(screen.getByText(/One AI input is not classified yet/)).toBeTruthy()
  await user.click(screen.getByRole('button', { name: 'Write the disclosure' }))
  await screen.findByText('/work/.cowriting/thesis.md/disclosure/ai-register.en.md')
  expect(calls.exported).toEqual([{ language: 'en', format: 'md', withPrompts: false, withAnswers: false }])
})

test('saving sends the text as typed, and a refused write is shown in the log', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn({ refuse: true })
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  expect((screen.getByRole('button', { name: 'Save' }) as HTMLButtonElement).disabled).toBe(true)
  await user.type(screen.getByLabelText('Document'), 'Mine.')
  await user.click(screen.getByRole('button', { name: 'Save' }))
  await waitFor(() => expect(calls.saved).toEqual(['# Thesis\n\nWe recruited 48 subjects.\nMine.']))
  await screen.findByText('saved')

  await user.type(screen.getByLabelText('Ask the AI'), 'rewrite it')
  await user.click(screen.getByRole('button', { name: 'Ask' }))
  await screen.findByText(/why subjects/)
  await user.click(screen.getByRole('tab', { name: /Log/ }))
  expect(screen.getByText('Writes the AI tried and the editor refused')).toBeTruthy()
  expect(screen.getByText(/write_note \.\.\/\.\.\/thesis\.md/)).toBeTruthy()
})

test('until the AI is set up the panel says so and asks nothing', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn({ isReady: false })
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText(/set it up first/)
  expect(screen.queryByLabelText('Ask the AI')).toBeNull()
  await user.click(screen.getByRole('button', { name: 'Open settings' }))
  expect(screen.getByLabelText(/^API key/)).toBeTruthy()
  expect(calls.asked).toEqual([])
})

test('on the first start the author sets up a local model, and is told it is untested and has no tools', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn({ isReady: false, withoutTools: true, settings: { isFirstRun: true, hasKey: false } })
  render(<App api={api} makeEditor={plainEditor} />)
  const dialog = within(await screen.findByRole('dialog', { name: 'Set up the AI' }))

  await user.click(dialog.getByRole('radio', { name: /A model on this computer/ }))
  expect(dialog.queryByLabelText(/^API key/)).toBeNull() // a local model needs no key
  expect(dialog.getByText('Nothing leaves this computer.')).toBeTruthy()

  // a server that is not running is said in plain words; nothing is saved
  await user.click(dialog.getByRole('button', { name: 'LM Studio' }))
  await user.click(dialog.getByRole('button', { name: 'Test the connection' }))
  expect((await dialog.findByRole('alert')).textContent).toContain('Is the local model server running?')

  await user.click(dialog.getByRole('button', { name: 'Ollama' }))
  await user.click(dialog.getByRole('button', { name: 'Test the connection' }))
  await dialog.findByText('Connected: 2 models')
  expect((dialog.getByLabelText(/Model/) as HTMLSelectElement).value).toBe('gemma3:4b')
  expect(dialog.getByText(/not tested for the assist-only rules/)).toBeTruthy()
  await user.click(dialog.getByRole('button', { name: 'Save' }))
  await waitFor(() => expect(calls.settings).toEqual([[{ backend: 'openai-compatible', model: 'gemma3:4b', baseUrl: 'http://localhost:11434/v1' }, null]]))

  // the window now asks, and says what kind of model answers
  expect(await screen.findByText('untested model')).toBeTruthy()
  expect(screen.getByRole('note').textContent).toMatch(/^Untested model: it may not keep to the assist-only rules/)
  await user.type(screen.getByLabelText('Ask the AI'), 'Any feedback?')
  await user.click(screen.getByRole('button', { name: 'Ask' }))
  await screen.findByText(/why subjects/)
  expect(screen.getByRole('note').textContent).toContain('This model cannot use tools: your document is sent along with each question')
})

test('a tested model carries no notice, and choosing Claude sends the key to the app only', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  expect(screen.queryByRole('note')).toBeNull()
  expect(screen.queryByText('untested model')).toBeNull()

  await user.click(screen.getByRole('button', { name: 'Settings…' }))
  const dialog = within(screen.getByRole('dialog', { name: 'Settings' }))
  expect((dialog.getByRole('radio', { name: /Claude/ }) as HTMLInputElement).checked).toBe(true)
  await user.type(dialog.getByLabelText(/^API key/), 'sk-ant-secret')
  await user.click(dialog.getByRole('button', { name: 'Save' }))
  await waitFor(() => expect(calls.settings).toEqual([[{ backend: 'anthropic', model: 'claude-opus-5-5', baseUrl: '' }, 'sk-ant-secret']]))
  await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
  expect(document.body.textContent).not.toContain('sk-ant-secret')
})

test('HTML in an answer is shown as text, and a web address is not a link', async () => {
  const user = userEvent.setup()
  const { api } = standIn({ answer: 'See <img src=x onerror="window.pwned=1"> and [this page](https://example.org/x).' })
  const { container } = render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await user.type(screen.getByLabelText('Ask the AI'), 'hi')
  await user.click(screen.getByRole('button', { name: 'Ask' }))
  await screen.findByText('this page')
  expect(container.querySelector('.answer img')).toBeNull()
  expect(container.querySelector('.answer a')).toBeNull()
  expect((window as unknown as { pwned?: number }).pwned).toBeUndefined()
})

test('mentions of a line become jumps, except inside code', () => {
  expect(linkLines('See line 12 and Zeile 3, lines 4–6; not `line 9`.')).toBe('See [line 12](#line-12) and [Zeile 3](#line-3), [lines 4–6](#line-4); not `line 9`.')
  // as models write it: capitalised, inside bold
  expect(linkLines('- **Line 14:** unclear')).toBe('- **[Line 14](#line-14):** unclear')
  expect(linkLines('```\nline 1\n```\nl. 7')).toBe('```\nline 1\n```\n[l. 7](#line-7)')
})
