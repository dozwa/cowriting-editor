// The window with the app behind it replaced by a stand-in that keeps a log the way the real
// one does. What is checked is what the author sees and what the window asks the app to do.
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { expect, test } from 'vitest'
import type { AiTextRow, Api, AskEvent, Expert, LiveHint, LiveOutcome, ScopeProgress, LogView, Settings, SettingsView, ThreadMessage, TurnRow, Use, BibFile, SourcesView } from './api'
import { App } from './App'
import type { Editor, EditorHandlers } from './editor'
import { linkLines } from './text'

/** A text field in place of the real editor, which needs a browser's layout. */
const plainEditor = (parent: HTMLElement, handlers: EditorHandlers): Editor & { jumps: number[] } => {
  const field = document.createElement('textarea')
  field.setAttribute('aria-label', 'Document')
  field.addEventListener('input', () => handlers.onChange())
  // as the real editor does: told once the pasted text is in the document, with the line it starts on
  field.addEventListener('paste', event => {
    const pasted = event.clipboardData?.getData('text') ?? ''
    const line = field.value.slice(0, field.selectionStart).split('\n').length
    setTimeout(() => handlers.onPaste(pasted, line))
  })
  parent.append(field)
  const jumps: number[] = []
  lastEditor = { getText: () => field.value, setText: text => void (field.value = text), insert: text => (field.setRangeText(text, field.selectionStart, field.selectionEnd, 'end'), handlers.onChange()), jumpToLine: line => void jumps.push(line), focus: () => {}, destroy: () => field.remove(), jumps }
  return lastEditor
}
let lastEditor: Editor & { jumps: number[] }

function standIn(options: { isReady?: boolean; answer?: string; refuse?: boolean; settings?: Partial<SettingsView>; withoutTools?: boolean; review?: LiveOutcome; hasPandoc?: boolean } = {}) {
  const turns: TurnRow[] = []
  const calls: { saved: string[]; asked: { text: string; replyTo: string | null; persona: string | null; documentText: string }[]; annotated: [string, Use, string][]; exported: unknown[]; settings: [Settings, string | null][]; tested: Settings[]; party: unknown[][]; pasted: [string, number][]; comments: [string, string][]; live: unknown[][]; reviewed: string[]; scope: string[]; checked: string[]; searched: string[]; exports: [string, string, string][] } = { saved: [], asked: [], annotated: [], exported: [], settings: [], tested: [], party: [], pasted: [], comments: [], live: [], reviewed: [], scope: [], checked: [], searched: [], exports: [] }
  const watch = { isOn: false, personas: ['reviewer-2'], hints: [] as LiveHint[] }
  let plan = ''
  let bibFiles: BibFile[] = []
  let refsChecked: SourcesView['checked'] = null
  let isChecked = false
  // the stand-in's plan: one point per line that starts with '- '
  const progress = (text: string): ScopeProgress | null => {
    const points = plan.split('\n').filter(line => line.startsWith('- ')).map(line => line.slice(2))
    if (points.length === 0) return null
    const words = text.split(/\s+/).filter(Boolean).length
    return { goal: 'Show it.', words: { done: words, target: 100 }, figures: { done: 0 }, tables: { done: 0 }, references: { done: 0 }, daysLeft: 12, wordsPerDay: 8, points: points.map((text, i) => (isChecked ? { text, isTicked: false, status: i === 0 ? 'covered' : 'missing', where: i === 0 ? 'thesis.md:3' : undefined, note: i === 0 ? 'named' : undefined } : { text, isTicked: false })), checkedAt: isChecked ? '2026-11-23T10:10:00.000Z' : undefined, sections: [{ name: 'Methods', target: 50, stage: 'draft', words: 7, markers: 1, isFound: true }], markers: 1, content: 0.2, finish: 0.35, overall: isChecked ? 0.4 : 0.2 }
  }
  const aiText: AiTextRow[] = []
  // the stand-in's detection: text the answer said, word for word
  const detect = (text: string, source: 'paste' | 'typed', line: number, documentText: string): AiTextRow[] => {
    const turn = turns.find(t => t.answer !== '' && text.includes(t.answer))
    if (turn === undefined || aiText.some(row => row.turn === turn.id)) return []
    const row: AiTextRow = { id: `a${aiText.length + 1}`, ts: '2026-11-23T10:05:00.000Z', source, match: 'exact', kind: 'text', words: turn.answer.split(/\s+/).length, excerpt: turn.answer, line, turn: turn.id, asked: turn.asked, note: '', stillInText: documentText.includes(turn.answer) }
    aiText.unshift(row)
    return [row]
  }
  const figure = { palette: { H: '#6b4226' }, sprite: ['.HH.', 'HSSH'] }
  const experts: Expert[] = [
    { id: 'reviewer-2', name: 'Reviewer #2', short: 'Reviewer #2', role: 'Weaknesses, limitations, tough questions', category: 'feedback', isCustom: false, isMember: true, brief: '', ...figure },
    { id: 'clinician', name: 'The Clinician', short: 'Clinician', role: 'Clinical relevance', category: 'content', isCustom: false, isMember: false, brief: '', ...figure },
  ]
  let everyone = ''
  const view = (text = ''): LogView => ({
    stats: { turns: turns.length, today: turns.length, unannotated: turns.filter(t => t.isOpen).length, aiText: 0, annotations: turns.filter(t => t.use !== '').length, rejected: 0, guardEvents: 0 },
    turns: [...turns].reverse(),
    guard: options.refuse && turns.length > 0 ? [{ ts: '2026-11-23T10:00:00Z', tool: 'write_note', input: 'write_note ../../thesis.md', files: [], refused: true, turn: 't1' }] : [],
    aiText: aiText.map(row => ({ ...row, stillInText: text.includes(row.excerpt) })),
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
    saveDocument: async text => {
      calls.saved.push(text)
      return detect(text, 'typed', 0, text)
    },
    reportPaste: async (pasted, line, documentText) => {
      calls.pasted.push([pasted, line])
      return detect(pasted, 'paste', line, documentText)
    },
    liveView: async () => ({ ...watch, hints: [...watch.hints], pauseMs: 30 }),
    setLive: async (isOn, personas, documentText) => {
      calls.live.push([isOn, personas, documentText])
      Object.assign(watch, { isOn, personas })
    },
    liveReview: async (documentText): Promise<LiveOutcome> => {
      calls.reviewed.push(documentText)
      if (options.review !== undefined) return options.review
      turns.push({ id: `t${turns.length + 1}`, ts: '2026-11-23T10:06:00.000Z', category: 'feedback', asked: 'Live review (reviewer-2) of thesis.md l.4–4', answer: 'reviewer-2 l.4: Why should a reader believe this?', excerpt: 'reviewer-2 l.4: Why should a reader believe this?', model: 'claude-opus-5-5', provider: 'Anthropic', toolsUsed: [], use: '', note: '', isOpen: true, isAiText: false, replyTo: null, persona: 'reviewer-2', who: 'Reviewer #2' })
      watch.hints.unshift({ turn: `t${turns.length}`, persona: 'reviewer-2', who: 'Reviewer #2', line: 4, text: 'Why should a reader believe this?', ts: '2026-11-23T10:06:00.000Z' })
      return { kind: 'reviewed', hints: 1, ts: '2026-11-23T10:06:00.000Z' }
    },
    exportSetup: async () => [{ hasPandoc: options.hasPandoc ?? true, pdfEngine: 'xelatex', hasZotero: false }, '/work/thesis.pdf', '/work/thesis.docx'],
    pickExport: async (_format, suggested) => suggested,
    exportDocument: async (format, target, documentText) => {
      calls.exports.push([format, target, documentText])
      return { path: target, cited: 2, fromZotero: 0, missing: ['ghost1999'], warnings: [] }
    },
    sources: async () => ({ files: bibFiles, entries: bibFiles.reduce((n, f) => n + f.entries, 0), cited: 2, missing: bibFiles.length > 0 ? ['ghost1999'] : [], checked: refsChecked }),
    pickBibliography: async () => '/work/refs.bib',
    addBibliography: async path => {
      bibFiles.push({ path, name: 'refs.bib', format: 'BibTeX', entries: 3, isFromDocument: false })
    },
    removeBibliography: async path => {
      bibFiles = bibFiles.filter(f => f.path !== path)
    },
    searchSources: async query => {
      calls.searched.push(query)
      return { from: 'bibliography', hits: [{ citekey: 'smith2020', title: 'Walking breaks and attention', authors: 'Smith, Lee', year: '2020', doi: '10.1000/walk.2020', file: 'refs.bib' }] }
    },
    checkReferences: async () => {
      refsChecked = { ts: '2026-11-23T10:00:00.000Z', rows: [
        { key: 'smith2020', derived: false, file: 'refs.bib', title: 'Walking breaks and attention', year: '2020', doi: '10.1000/walk.2020', status: 'ok' },
        { key: 'lee2021', derived: false, file: 'refs.bib', title: 'Sitting time, measured', year: '2021', doi: '10.1000/sit.2021', status: 'mismatch', found: { doi: '10.1000/sit.2021', title: 'Standing desks in schools', year: '2021' } },
        { key: 'nodoi', derived: false, file: 'refs.bib', title: 'An entry with nothing else', year: '', doi: '', status: 'no_doi', suggestion: '10.1000/entry' },
      ] }
      return refsChecked
    },
    scope: async documentText => ({ text: plan, template: '# Scope\n\n## Points\n', progress: progress(documentText) }),
    setScope: async text => {
      calls.scope.push(text)
      plan = text
    },
    checkPoints: async documentText => {
      calls.checked.push(documentText)
      isChecked = true
      return { covered: 1, partial: 0, missing: 1 }
    },
    commentAiText: async (id, note) => {
      calls.comments.push([id, note])
      aiText.find(row => row.id === id)!.note = note
    },
    ask: async (input, onEvent: (event: AskEvent) => void) => {
      calls.asked.push(input)
      const id = `t${turns.length + 1}`
      const answer = options.answer ?? 'On line 3: why subjects and not participants?'
      onEvent({ kind: 'started', id })
      onEvent({ kind: 'tool', name: 'read_document' })
      onEvent({ kind: 'text', text: answer })
      turns.push({ id, ts: '2026-11-23T10:00:00Z', category: 'general', asked: input.text, answer, excerpt: answer, model: 'claude-opus-5-5', provider: 'Anthropic', toolsUsed: ['read_document'], use: '', note: '', isOpen: true, isAiText: false, replyTo: input.replyTo, persona: input.persona ?? '', who: experts.find(e => e.id === input.persona)?.name ?? '' })
      return { id, answer, isAiText: false, refused: 0, cutOff: null, withoutTools: options.withoutTools ?? false }
    },
    logView: async text => view(text),
    thread: async id => {
      const out: ThreadMessage[] = []
      for (const turn of turns) {
        if (turn.replyTo !== null) out.push({ id: turn.id, role: 'author', who: 'You', persona: '', ts: turn.ts, text: turn.asked })
        out.push({ id: turn.id, role: 'ai', who: turn.who === '' ? 'Assistant' : turn.who, persona: turn.persona, ts: turn.ts, text: turn.answer })
        if (turn.id === id) break
      }
      return out
    },
    annotate: async (reference, use, note) => {
      calls.annotated.push([reference, use, note])
      const turn = turns.find(t => t.id === reference)!
      Object.assign(turn, { use, note, isOpen: false })
    },
    party: async () => ({ experts: experts.map(e => ({ ...e })), everyone, sharedPalette: { S: '#f2c9a0' } }),
    setBrief: async (key, body) => {
      calls.party.push(['brief', key, body])
      if (key === 'everyone') everyone = body
      else experts.find(e => e.id === key)!.brief = body
    },
    setMember: async (id, isMember) => {
      calls.party.push(['member', id, isMember])
      experts.find(e => e.id === id)!.isMember = isMember
    },
    addExpert: async (name, role, looksAt) => {
      calls.party.push(['add', name, role, looksAt])
      if (name === 'Reviewer #2') throw 'There is already an expert called “Reviewer #2”, or the name has no letters to make an id from.'
      experts.push({ id: 'health-economist', name, short: name, role, category: 'content', isCustom: true, isMember: true, brief: '', ...figure })
      return 'health-economist'
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
  expect(calls.asked).toEqual([{ text: 'Any feedback?', replyTo: null, persona: null, documentText: '# Thesis\n\nWe recruited 48 subjects.\nMore.' }])
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

test('the author briefs an expert, asks it for a review, and replies to it', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await user.click(screen.getByRole('tab', { name: 'Experts' }))

  // the party first, the others after; each with its figure
  const names = screen.getAllByRole('listitem').map(item => within(item).getByRole('img').getAttribute('aria-label'))
  expect(names).toEqual(['Reviewer #2', 'The Clinician'])

  await user.type(screen.getByLabelText('Briefing for every expert'), 'A pilot study.')
  await user.click(within(screen.getByLabelText('Briefing for every expert').parentElement!).getByRole('button', { name: 'Save' }))
  const reviewer = within(screen.getAllByRole('listitem')[0]!)
  await user.click(reviewer.getByRole('button', { name: 'Brief' }))
  await user.type(screen.getByLabelText('Briefing for Reviewer #2'), 'Go easy on the sampling.')
  await user.click(within(screen.getByLabelText('Briefing for Reviewer #2').parentElement!).getByRole('button', { name: 'Save' }))
  await within(screen.getAllByRole('listitem')[0]!).findByRole('button', { name: 'Briefed' })
  await user.click(screen.getByLabelText('The Clinician is in the party'))
  await waitFor(() => expect(calls.party).toEqual([['brief', 'everyone', 'A pilot study.'], ['brief', 'reviewer-2', 'Go easy on the sampling.'], ['member', 'clinician', true]]))

  // a review is a logged question to that expert, shown under the expert's name
  await user.click(within(screen.getAllByRole('listitem')[0]!).getByRole('button', { name: 'Ask for a review' }))
  await screen.findByText(/why subjects and not participants/)
  expect(calls.asked[0]).toMatchObject({ text: 'Please review my document.', persona: 'reviewer-2', replyTo: null })
  expect(screen.getByRole('heading', { name: /Reviewer #2/ })).toBeTruthy()

  // the reply goes to the same expert, not to the assistant
  await user.type(screen.getByLabelText('Reply'), 'It is the usual term.')
  await user.click(screen.getByRole('button', { name: 'Reply' }))
  await waitFor(() => expect(calls.asked[1]).toMatchObject({ text: 'It is the usual term.', persona: 'reviewer-2', replyTo: 't1' }))

  // a new question can go to any member of the party, or to the assistant
  await user.click(screen.getByRole('button', { name: 'New question' }))
  const who = screen.getByLabelText('Who to ask') as HTMLSelectElement
  expect([...who.options].map(o => o.text)).toEqual(['Assistant', 'Reviewer #2', 'The Clinician'])
  await user.selectOptions(who, 'clinician')
  await user.type(screen.getByLabelText('Ask the AI'), 'Does this matter to patients?')
  await user.click(screen.getByRole('button', { name: 'Ask' }))
  await waitFor(() => expect(calls.asked[2]).toMatchObject({ persona: 'clinician', replyTo: null }))

  await user.click(screen.getByRole('tab', { name: /Log/ }))
  expect(screen.getAllByText('Reviewer #2').length).toBeGreaterThan(0)
})

test('the author adds an expert of their own, and a name that is taken is refused in plain words', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await user.click(screen.getByRole('tab', { name: 'Experts' }))
  await user.click(screen.getByRole('button', { name: 'New expert…' }))
  expect((screen.getByRole('button', { name: 'Add' }) as HTMLButtonElement).disabled).toBe(true)

  await user.type(screen.getByLabelText('Name'), 'Reviewer #2')
  await user.type(screen.getByLabelText('Field'), 'x')
  await user.type(screen.getByLabelText('What they look at'), 'y')
  await user.click(screen.getByRole('button', { name: 'Add' }))
  expect((await screen.findByRole('alert')).textContent).toContain('There is already an expert called')
  expect(screen.getAllByRole('listitem').length).toBe(2)

  await user.clear(screen.getByLabelText('Name'))
  await user.type(screen.getByLabelText('Name'), 'The Health Economist')
  await user.click(screen.getByRole('button', { name: 'Add' }))
  await waitFor(() => expect(screen.getAllByRole('listitem').length).toBe(3))
  expect(calls.party.at(-1)).toEqual(['add', 'The Health Economist', 'x', 'y'])
  expect(screen.queryByRole('alert')).toBeNull()
  expect(screen.queryByLabelText('Name')).toBeNull() // the form closes once the expert exists
})

test('AI text pasted into the document is logged, said once, and can be commented; the text is left alone', async () => {
  const user = userEvent.setup()
  const answer = 'Participants were randomly assigned to one of the two conditions by a colleague.'
  const { api, calls } = standIn({ answer })
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  const field = (await screen.findByLabelText('Document')) as HTMLTextAreaElement

  // pasting before anything was asked, or pasting one's own words, says nothing
  await user.click(field)
  await user.paste('My own sentence about the offices.')
  await waitFor(() => expect(calls.pasted.length).toBe(1))
  expect(screen.queryByRole('status')).toBeNull()

  await user.type(screen.getByLabelText('Ask the AI'), 'Write it for me.')
  await user.click(screen.getByRole('button', { name: 'Ask' }))
  await screen.findAllByText(answer)

  await user.click(field)
  await user.paste(`\n${answer}`)
  const notice = await screen.findByRole('status')
  expect(notice.textContent).toContain('Logged: 13 words you pasted match text from an AI answer. Nothing was changed')
  expect(field.value.endsWith(answer)).toBe(true) // nothing blocked, nothing marked
  expect(calls.pasted[1]).toEqual([`\n${answer}`, 4])
  expect(calls.saved).toEqual([]) // reporting a paste does not save the document

  await user.click(within(notice).getByRole('button', { name: 'Show in the log' }))
  expect(screen.queryByRole('status')).toBeNull()
  const row = within(screen.getByRole('heading', { name: 'AI text that entered your document' }).parentElement!).getAllByRole('listitem').at(-1)!
  expect(row.textContent).toContain('pasted · 13 words · word for word')
  expect(row.textContent).toContain('Still in your text')
  expect(row.textContent).toContain('from the answer to “Write it for me.”')
  await user.click(within(row).getByRole('button', { name: 'line 4' }))
  expect(lastEditor.jumps).toEqual([4])

  await user.type(within(row).getByLabelText(/^Comment on/), 'A placeholder.')
  await user.click(within(row).getByRole('button', { name: 'Save' }))
  await waitFor(() => expect(calls.comments).toEqual([['a1', 'A placeholder.']]))

  // the author rewrites the passage: the entry stays and says so; saving finds nothing new
  await user.clear(field)
  await user.type(field, 'A colleague drew lots.')
  await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Save' }))
  await waitFor(() => expect(screen.getByText(/No longer in your text/)).toBeTruthy())
  expect(screen.queryByRole('status')).toBeNull()
  expect(screen.getByDisplayValue('A placeholder.')).toBeTruthy() // the comment is kept
})

test('with live review on, a pause in writing sends the text for hints; off, nothing is sent', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  const field = (await screen.findByLabelText('Document')) as HTMLTextAreaElement

  // off: the author writes and pauses, and nothing is sent
  await user.type(field, 'First.')
  await new Promise(resolve => setTimeout(resolve, 80))
  expect(calls.reviewed).toEqual([])

  await user.click(screen.getByRole('tab', { name: /^Live/ }))
  expect(screen.getByText(/the paragraphs you change are sent to the AI once you pause/)).toBeTruthy()
  expect(screen.getByText('Turn live review on to get hints while you write.')).toBeTruthy()
  // who watches is the author's choice, from the party
  expect(screen.queryByLabelText(/The Clinician/)).toBeNull()
  await user.click(screen.getByLabelText('Live review'))
  await waitFor(() => expect(calls.live).toEqual([[true, ['reviewer-2'], '# Thesis\n\nWe recruited 48 subjects.\nFirst.']]))

  // on: nothing while the author is writing, one review once they pause
  await user.click(screen.getByRole('tab', { name: 'Ask' }))
  await user.type(field, ' Breaks help everyone.')
  expect(calls.reviewed).toEqual([])
  await waitFor(() => expect(calls.reviewed).toEqual(['# Thesis\n\nWe recruited 48 subjects.\nFirst. Breaks help everyone.']))
  expect(calls.saved).toEqual([]) // a review does not save the document

  // the hint is announced on the tab, shown with who gave it, and leads to its line and its logged turn
  const tab = await screen.findByRole('tab', { name: /^Live\s*1/ })
  await user.click(tab)
  expect(screen.getByRole('tab', { name: /^Live/ }).textContent).toBe('Live')
  expect(screen.getByText('Why should a reader believe this?')).toBeTruthy()
  expect(screen.getByRole('status').textContent).toMatch(/^1 hint at /)
  await user.click(screen.getByRole('button', { name: 'line 4' }))
  expect(lastEditor.jumps).toEqual([4])
  await user.click(screen.getByRole('button', { name: 'Classify or reply' }))
  expect(await screen.findByLabelText('Reply')).toBeTruthy()
  expect(screen.getByRole('tab', { name: /^Log/ }).textContent).toContain('1')

  // turned off again: writing on sends nothing more
  await user.click(screen.getByRole('tab', { name: /^Live/ }))
  await user.click(screen.getByLabelText('Live review'))
  await waitFor(() => expect(calls.live.at(-1)![0]).toBe(false))
  await user.type(field, ' More.')
  await new Promise(resolve => setTimeout(resolve, 80))
  expect(calls.reviewed.length).toBe(1)
})

test('a review that comes too soon is asked for again later, and a failed one is said plainly', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn({ review: { kind: 'wait', ms: 40 } })
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await user.click(screen.getByRole('tab', { name: /^Live/ }))
  await user.click(screen.getByLabelText('Live review'))
  await user.type(screen.getByLabelText('Document'), 'More words here.')
  await waitFor(() => expect(calls.reviewed.length).toBe(1))
  expect((await screen.findByRole('status')).textContent).toBe('waiting: at most one review a minute')
  await waitFor(() => expect(calls.reviewed.length).toBeGreaterThan(1))
})

test('the author makes a plan, sees how far the text is, and has the points checked', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await user.click(screen.getByRole('tab', { name: 'Scope' }))

  // no plan: an invitation, and the template to start from
  await user.click(await screen.findByRole('button', { name: 'Make a plan' }))
  const field = screen.getByLabelText('Your plan') as HTMLTextAreaElement
  expect(field.value).toBe('# Scope\n\n## Points\n')
  await user.type(field, '- 48 people in two offices\n- Breaks help')
  await user.click(screen.getByRole('button', { name: 'Save the plan' }))
  await waitFor(() => expect(calls.scope).toEqual(['# Scope\n\n## Points\n- 48 people in two offices\n- Breaks help']))

  // progress against the plan, before any check
  expect(await screen.findByText('20 %')).toBeTruthy()
  expect(screen.getByText(/12 days left/)).toBeTruthy()
  expect((screen.getByLabelText('Words') as HTMLProgressElement).max).toBe(100)
  expect(screen.getByText(/7 of 50 words · draft · 1 marker/)).toBeTruthy()
  expect(screen.getAllByText('· not checked').length).toBe(2)
  expect(calls.checked).toEqual([])

  // the check is the author's click; it says what it sends, and its result shows per point
  expect(screen.getByText(/sends your plan's points and the document to the AI/)).toBeTruthy()
  await user.click(screen.getByRole('button', { name: 'Check the points' }))
  expect((await screen.findByRole('status')).textContent).toBe('1 covered, 0 partly, 1 missing')
  expect(calls.checked).toEqual(['# Thesis\n\nWe recruited 48 subjects.\n'])
  expect(screen.getByText('✓ covered')).toBeTruthy()
  expect(screen.getByText('○ missing')).toBeTruthy()
  expect(screen.getByText('40 %')).toBeTruthy()
  await user.click(screen.getByRole('button', { name: 'line 3' }))
  expect(lastEditor.jumps).toEqual([3])

  // the progress follows the text as the author writes
  await user.type(screen.getByLabelText('Document'), 'Six more words in the text.')
  await waitFor(() => expect(screen.getByText(/12 of 100/)).toBeTruthy())
})

test('the author adds a bibliography, finds a reference and has the references checked', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await user.click(screen.getByRole('tab', { name: 'Sources' }))

  // nothing yet: how to get a bibliography, and nothing to check
  expect(await screen.findByText(/No bibliography yet/)).toBeTruthy()
  expect((screen.getByRole('button', { name: 'Check 0 references' }) as HTMLButtonElement).disabled).toBe(true)
  await user.click(screen.getByRole('button', { name: 'Add a bibliography file' }))
  expect(await screen.findByText('BibTeX, 3 references')).toBeTruthy()
  // a key the document cites and the file does not hold is named
  expect(screen.getByText('@ghost1999')).toBeTruthy()

  await user.type(screen.getByLabelText('Search your references'), 'walking{Enter}')
  expect(await screen.findByText(/Zotero is not running/)).toBeTruthy()
  expect(calls.searched).toEqual(['walking'])
  expect(screen.getByText('@smith2020')).toBeTruthy()
  // the key goes into the text where the cursor is, by the author's click
  await user.click(screen.getByRole('button', { name: 'Cite' }))
  expect((screen.getByLabelText('Document') as HTMLTextAreaElement).value).toContain('[@smith2020]')

  // the check: what needs a look comes first, and a suggested DOI is not called verified
  await user.click(screen.getByRole('button', { name: 'Check 3 references' }))
  expect((await screen.findByRole('status')).textContent).toMatch(/1 fine, 1 to look at, 1 without a DOI/)
  const verdicts = Array.from(document.querySelectorAll('.refs .verdict')).map(v => v.textContent)
  expect(verdicts).toEqual(['the DOI belongs to something else', 'no DOI', 'DOI and title agree'])
  expect(screen.getByText(/Crossref has: Standing desks in schools/)).toBeTruthy()
  expect(screen.getByText(/Check it yourself before you use it/)).toBeTruthy()

  await user.click(screen.getByRole('button', { name: 'Take off the list' }))
  expect(await screen.findByText(/No bibliography yet/)).toBeTruthy()
})

test('the author exports the document as it is in the editor, and is told which keys were not found', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await user.type(screen.getByLabelText('Document'), ' Not saved yet.')
  await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Export…' }))
  expect(await screen.findByText(/Zotero with Better BibTeX is not running/)).toBeTruthy()
  await user.selectOptions(screen.getByLabelText('Format'), 'docx')
  await user.click(screen.getByRole('button', { name: 'Export' }))
  const status = await screen.findByRole('status')
  expect(status.textContent).toContain('/work/thesis.docx')
  expect(status.textContent).toContain('Not found anywhere, and left as they are in the text: @ghost1999.')
  expect(calls.exports.length).toBe(1)
  const [format, target, exportedText] = calls.exports[0]!
  expect([format, target]).toEqual(['docx', '/work/thesis.docx'])
  expect(exportedText).toContain('Not saved yet.')
  expect(calls.saved).toEqual([])
})

test('without pandoc the export says what to install and does not try', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn({ hasPandoc: false })
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Export…' }))
  expect(await screen.findByText(/pandoc is not installed on this computer/)).toBeTruthy()
  expect((screen.getByRole('button', { name: 'Export' }) as HTMLButtonElement).disabled).toBe(true)
  expect(calls.exports).toEqual([])
})
