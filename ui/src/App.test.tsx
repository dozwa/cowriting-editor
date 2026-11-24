// The window with the app behind it replaced by a stand-in that keeps a log the way the real
// one does. What is checked is what the author sees and what the window asks the app to do.
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { expect, test, vi } from 'vitest'
import type { ExternalUse, ProjectView, AiTextRow, Api, AskEvent, Expert, LiveHint, LiveOutcome, ScopeProgress, LogView, Settings, SettingsView, ThreadMessage, TurnRow, Use, BibFile, Orphan, PdfChoice, SourcesView } from './api'
import { App } from './App'
import type { Editor, EditorHandlers, Format, View } from './editor'
import { type Preferences, STANDARD } from './preferences'
import { linkLines } from './text'

/** What the stand-in editor was told, for the tests to read. */
type Seen = { jumps: number[]; formats: Format[]; view: View | null }

/** A text field in place of the real editor, which needs a browser's layout. */
const plainEditor = (parent: HTMLElement, handlers: EditorHandlers): Editor & Seen => {
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
  const formats: Format[] = []
  lastEditor = { getText: () => field.value, setText: text => void (field.value = text), insert: text => (field.setRangeText(text, field.selectionStart, field.selectionEnd, 'end'), handlers.onChange()), jumpToLine: line => void jumps.push(line), format: kind => void formats.push(kind), setView: view => void (lastEditor.view = view), focus: () => {}, destroy: () => field.remove(), jumps, formats, view: null }
  return lastEditor
}
let lastEditor: Editor & Seen

function standIn(options: { isReady?: boolean; holdAnswer?: boolean; answer?: string; refuse?: boolean; settings?: Partial<SettingsView>; withoutTools?: boolean; review?: LiveOutcome; hasPandoc?: boolean; pdfEngines?: string[]; orphans?: Orphan[]; stylePicked?: string } = {}) {
  const turns: TurnRow[] = []
  let release = () => {}
  const calls: { saved: string[]; asked: { text: string; replyTo: string | null; persona: string | null; documentText: string }[]; annotated: [string, Use, string][]; exported: unknown[]; settings: [Settings, string | null][]; tested: Settings[]; party: unknown[][]; pasted: [string, number][]; comments: [string, string][]; live: unknown[][]; reviewed: string[]; scope: string[]; checked: string[]; searched: string[]; exports: [string, string, string][]; pdf: PdfChoice[]; adopted: string[]; moved: [string, string][]; preferences: Preferences[]; picked: string[]; feedback: [string, string, string][]; usage: [boolean, boolean][]; reportFiles: [string, number, string | null][]; project: unknown[][]; support: string[]; pages: string[] } = { feedback: [], usage: [], reportFiles: [], project: [], support: [], pages: [], saved: [], asked: [], annotated: [], exported: [], settings: [], tested: [], party: [], pasted: [], comments: [], live: [], reviewed: [], scope: [], checked: [], searched: [], exports: [], pdf: [], adopted: [], moved: [], preferences: [], picked: [] }
  const watch = { isOn: false, personas: ['reviewer-2'], hints: [] as LiveHint[] }
  const pace = () => ({ pauseMs: settings.preferences.livePauseSeconds === STANDARD.livePauseSeconds ? 30 : settings.preferences.livePauseSeconds * 1000, gapMs: settings.preferences.liveGapSeconds * 1000, perHour: settings.preferences.livePerHour })
  let plan = ''
  let bibFiles: BibFile[] = []
  let refsChecked: SourcesView['checked'] = null
  let style: SourcesView['style'] = null
  let isChecked = false
  // the stand-in's plan: one point per line that starts with '- '
  const progress = (text: string): ScopeProgress | null => {
    const points = plan.split('\n').filter(line => line.startsWith('- ')).map(line => line.slice(2))
    if (points.length === 0) return null
    const words = text.split(/\s+/).filter(Boolean).length
    return { goal: 'Show it.', words: { done: words, target: 100 }, figures: { done: 0 }, tables: { done: 0 }, references: { done: 0 }, daysLeft: 12, wordsPerDay: 8, points: points.map((text, i) => (isChecked ? { text, isTicked: false, status: i === 0 ? 'covered' : 'missing', where: i === 0 ? 'thesis.md:3' : undefined, note: i === 0 ? 'named' : undefined } : { text, isTicked: false })), checkedAt: isChecked ? '2026-11-23T10:10:00.000Z' : undefined, sections: [{ name: 'Methods', target: 50, stage: 'draft', words: 7, markers: 1, isFound: true }], markers: 1, content: 0.2, finish: 0.35, overall: isChecked ? 0.4 : 0.2 }
  }
  const aiText: AiTextRow[] = []
  let external: ExternalUse[] = []
  // the stand-in's detection: text the answer said, word for word
  const detect = (text: string, source: 'paste' | 'typed', line: number, documentText: string): AiTextRow[] => {
    const turn = turns.find(t => t.answer !== '' && text.includes(t.answer))
    if (turn === undefined || aiText.some(row => row.turn === turn.id)) return []
    const row: AiTextRow = { id: `a${aiText.length + 1}`, ts: '2026-11-23T10:05:00.000Z', source, match: 'exact', kind: 'text', words: turn.answer.split(/\s+/).length, excerpt: turn.answer, line, turn: turn.id, asked: turn.asked, note: '', stillInText: documentText.includes(turn.answer) }
    aiText.unshift(row)
    return [row]
  }
  const figure = { palette: { H: '#6b4226' }, sprite: ['.HH.', 'HSSH'], prompt: 'You are an expert. Your perspective: the weak points.', isPromptEdited: false }
  let project: ProjectView = { files: [], overrides: {}, folder: '/home/me/.config/cowriting-editor/projects/thesis.md-1' }
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
    external: external.map(row => ({ ...row })),
    words: 7,
  })
  let settings: SettingsView = { backend: 'anthropic', model: 'claude-opus-5-5', baseUrl: '', maxTokens: 0, general: { backend: 'anthropic', model: 'claude-opus-5-5', baseUrl: '', maxTokens: 0 }, isAiOwn: false, preferences: { ...STANDARD }, version: '0.1.0', system: 'macos aarch64', feedbackByMail: true, contact: 'me@example.org', repository: 'https://github.com/dozwa/cowriting-editor', asking: { isDue: false, withStar: true }, showTour: false, usage: { isAvailable: false, isAsked: false, installation: null }, settingsFolder: '/home/me/.config/cowriting-editor', hasKey: options.isReady ?? true, isReady: options.isReady ?? true, modelStatus: 'tested', isFirstRun: false, openAtStart: '/work/thesis.md', ...options.settings }
  const api: Api = {
    sendFeedback: async (way, title, text) => void calls.feedback.push([way, title, text]),
    usageAnswered: async (ping, figures) => {
      calls.usage.push([ping, figures])
      settings = { ...settings, preferences: { ...settings.preferences, usagePing: ping, usageFigures: figures }, usage: { isAvailable: true, isAsked: true, installation: ping || figures ? 'a1b2c3' : null } }
      return settings.preferences
    },
    usagePreview: async () => ['{ "kind": "start", "installation": "(a random number, made when you agree)" }', '{ "kind": "session", "aiRequests": 0 }'],
    privacyStatement: async language => (language === 'de' ? 'Datenschutzerklärung: Nutzungszahlen' : 'Privacy statement: usage figures'),
    recordSeen: async () => {},
    recentDiagnostics: async () => ['2026-11-24T10:01:00.000Z start CoWriting Editor 0.1.0, macos aarch64', '2026-11-24T10:02:10.500Z call exportDocument failed 4012ms: pandoc ended with <path>', '2026-11-24T10:02:11.000Z document at-failed-export: 41 bytes, 41 characters, 4 lines'].join('\n'),
    clearDiagnostics: async () => void calls.support.push('log deleted'),
    pickReportFile: async () => '/work/cowriting-report.txt',
    writeReportFile: async (target, minutes, documentText) => void calls.reportFiles.push([target, minutes, documentText]),
    stateExternal: async stated => {
      calls.project.push(['state', stated])
      external = stated.id === '' ? [...external, { ...stated, id: `x${external.length + 1}` }] : external.map(row => (row.id === stated.id ? { ...stated, id: `${stated.id}b` } : row))
    },
    withdrawExternal: async id => {
      calls.project.push(['withdraw', id])
      external = external.filter(row => row.id !== id)
    },
    project: async () => project,
    pickLinkedFile: async () => '/work/wording.md',
    linkFile: async (path, personas) => {
      calls.project.push(['link', path, personas])
      if (path.endsWith('.pdf')) throw `${path} cannot be read as text.`
      const file = { path, name: path.split('/').pop()!, personas, isMissing: false, chars: 120, isCut: false, isConverted: false }
      project = { ...project, files: [...project.files.filter(f => f.path !== path), file] }
      return project
    },
    unlinkFile: async path => {
      calls.project.push(['unlink', path])
      project = { ...project, files: project.files.filter(f => f.path !== path) }
      return project
    },
    setProject: async overrides => {
      calls.project.push(['set', overrides])
      project = { ...project, overrides }
      // as the app does: the document's own AI is what holds from now on
      settings = { ...settings, ...(overrides.ai ?? settings.general), isAiOwn: overrides.ai !== undefined }
      return project
    },
    setPrompt: async (id, prompt) => {
      calls.project.push(['prompt', id, prompt])
      const expert = experts.find(e => e.id === id)!
      Object.assign(expert, prompt === '' ? { prompt: figure.prompt, isPromptEdited: false } : { prompt, isPromptEdited: true })
    },
    supportAnswered: async what => void calls.support.push(what),
    tourSeen: async () => void calls.support.push('tour'),
    getSettings: async () => settings,
    setApiKey: async (backend, apiKey) => void calls.project.push(['key', backend, apiKey]),
    setSettings: async (next, apiKey) => {
      calls.settings.push([next, apiKey])
      settings = { ...settings, ...next, general: next, isReady: true, isFirstRun: false, modelStatus: 'untested' }
    },
    testConnection: async next => {
      calls.tested.push(next)
      if (next.baseUrl === 'http://localhost:1234/v1') throw 'localhost:1234 could not be reached. Is the local model server running?'
      return [{ id: 'gemma3:4b', name: 'gemma3:4b', status: 'untested' }, { id: 'qwen3.5:9b', name: 'qwen3.5:9b', status: 'failed' }]
    },
    setPreferences: async preferences => {
      calls.preferences.push(preferences)
      // as the app does: held to the limits, and returned as kept
      const kept = { ...preferences, textSize: Math.min(Math.max(preferences.textSize, 13), 28) }
      settings = { ...settings, preferences: kept }
      return kept
    },
    showSettingsFolder: async () => void calls.picked.push('shown'),
    showWritten: async (path, how) => void calls.picked.push(`${how}:${path}`),
    pickFolder: async () => '/work/exports',
    pickDocument: async folder => (calls.picked.push(`open in ${folder}`), null),
    pickNewDocument: async folder => (calls.picked.push(`new in ${folder}`), null),
    confirmLeave: async () => true,
    openDocument: async path => ({ path, name: 'thesis.md', text: '# Thesis\n\nWe recruited 48 subjects.\n', language: 'en', orphans: options.orphans ?? [] }),
    adoptLog: async name => {
      calls.adopted.push(name)
      return { path: '/work/thesis.md', name: 'thesis.md', text: '# Thesis\n\nWe recruited 48 subjects.\n', language: 'en', orphans: [] }
    },
    pickMove: async () => '/work/drafts/chapter-1.md',
    openGuide: async which => void calls.pages.push(which),
    moveDocument: async (target, text) => {
      calls.moved.push([target, text])
      return { path: target, name: 'chapter-1.md', text, language: 'en', orphans: [] }
    },
    newDocument: async path => ({ path, name: 'new.md', text: '', language: 'en', orphans: [] }),
    saveDocument: async text => {
      calls.saved.push(text)
      return detect(text, 'typed', 0, text)
    },
    reportPaste: async (pasted, line, documentText) => {
      calls.pasted.push([pasted, line])
      return detect(pasted, 'paste', line, documentText)
    },
    liveView: async () => ({ ...watch, hints: [...watch.hints], ...pace() }),
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
    exportSetup: async () => [{ hasPandoc: options.hasPandoc ?? true, pdfEngines: options.pdfEngines ?? ['typst'], isOwn: true, hasZotero: false }, '/work/thesis.pdf', '/work/thesis.docx'],
    pickExport: async (_format, suggested) => suggested,
    exportDocument: async (format, target, documentText, pdf) => {
      calls.exports.push([format, target, documentText])
      calls.pdf.push(pdf)
      return { path: target, cited: 2, fromZotero: 0, missing: ['ghost1999'], warnings: [] }
    },
    sources: async () => ({ files: bibFiles, entries: bibFiles.reduce((n, f) => n + f.entries, 0), cited: 2, missing: bibFiles.length > 0 ? ['ghost1999'] : [], checked: refsChecked, style }),
    pickBibliography: async () => '/work/refs.bib',
    pickStyle: async () => options.stylePicked ?? '/work/apa.csl',
    setStyle: async path => {
      if (!path.endsWith('.csl')) throw 'A reference style is a .csl file, as zotero.org/styles has them.'
      style = { path, name: 'apa.csl', title: 'American Psychological Association 7th edition', isFromDocument: false }
    },
    removeStyle: async () => {
      style = null
    },
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
    scope: async documentText => ({ text: plan, template: '## Goal\n\n## Points\n', templateDe: '## Ziel\n\n## Kernaussagen\n', progress: progress(documentText) }),
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
    stopAnswer: async () => {
      calls.picked.push('stopped')
      release()
    },
    ask: async (input, onEvent: (event: AskEvent) => void) => {
      calls.asked.push(input)
      const id = `t${turns.length + 1}`
      let answer = options.answer ?? 'On line 3: why subjects and not participants?'
      onEvent({ kind: 'started', id })
      onEvent({ kind: 'tool', name: 'read_document' })
      if (options.holdAnswer === true) {
        // an answer that comes in slowly, until the author stops it
        answer = 'Why subjects'
        onEvent({ kind: 'text', text: answer })
        await new Promise<void>(stopped => (release = stopped))
        answer += '\n\n[The answer is incomplete: The author stopped it.]'
        turns.push({ id, ts: '2026-11-23T10:00:00Z', category: 'general', asked: input.text, answer, excerpt: answer, model: 'claude-opus-5-5', provider: 'Anthropic', toolsUsed: [], use: '', note: '', isOpen: true, isAiText: false, replyTo: input.replyTo, persona: input.persona ?? '', who: '' })
        return { id, answer, isAiText: false, refused: 0, cutOff: 'The author stopped it.', withoutTools: false }
      }
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
  // it lies in a hidden folder: it can be shown there, and its text copied
  const copied: string[] = []
  Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText: async (text: string) => void copied.push(text) } })
  await user.click(screen.getByRole('button', { name: 'Show in its folder' }))
  expect(calls.picked).toEqual(['folder:/work/.cowriting/thesis.md/disclosure/ai-register.en.md'])
  await user.click(screen.getByRole('button', { name: 'Copy the text' }))
  await screen.findByText('Copied.')
  expect(copied).toEqual(['# AI register (disclosure of AI use)'])
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
  await waitFor(() => expect(calls.settings).toEqual([[{ backend: 'openai-compatible', model: 'gemma3:4b', baseUrl: 'http://localhost:11434/v1', maxTokens: 0 }, null]]))

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
  expect((dialog.getByRole('radio', { name: /Anthropic API key/ }) as HTMLInputElement).checked).toBe(true)
  await user.type(dialog.getByLabelText(/^API key/), 'sk-ant-secret')
  await user.click(dialog.getByRole('button', { name: 'Save' }))
  await waitFor(() => expect(calls.settings).toEqual([[{ backend: 'anthropic', model: 'claude-opus-5-5', baseUrl: '', maxTokens: 0 }, 'sk-ant-secret']]))
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
  const tab = await screen.findByRole('tab', { name: /^Live review\s*1/ })
  await user.click(tab)
  expect(screen.getByRole('tab', { name: /^Live/ }).textContent).toBe('Live review')
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
  expect((await screen.findByRole('status')).textContent).toBe('waiting: at most one review every minute')
  await waitFor(() => expect(calls.reviewed.length).toBeGreaterThan(1))
})

test('the author makes a plan, sees how far the text is, and has the points checked', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await user.click(screen.getByRole('tab', { name: 'Plan' }))

  // no plan: an invitation, and the template to start from
  await user.click(await screen.findByRole('button', { name: 'Make a plan' }))
  const field = screen.getByLabelText('Your plan') as HTMLTextAreaElement
  expect(field.value).toBe('## Goal\n\n## Points\n')
  // how each part is written stands beside it, not inside it
  expect(screen.getByText(/One line for each thing the text must say/)).toBeTruthy()
  await user.type(field, '- 48 people in two offices\n- Breaks help')
  await user.click(screen.getByRole('button', { name: 'Save the plan' }))
  await waitFor(() => expect(calls.scope).toEqual(['## Goal\n\n## Points\n- 48 people in two offices\n- Breaks help']))

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
  // the file just written can be opened and found from here
  await user.click(within(status).getByRole('button', { name: 'Open' }))
  await user.click(within(status).getByRole('button', { name: 'Show in its folder' }))
  expect(calls.picked).toEqual(['open:/work/thesis.docx', 'folder:/work/thesis.docx'])
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
  expect(await screen.findByText(/pandoc was not found/)).toBeTruthy()
  expect((screen.getByRole('button', { name: 'Export' }) as HTMLButtonElement).disabled).toBe(true)
  expect(calls.exports).toEqual([])
})

/** An empty storage for this window, as a fresh install has it. */
function windowStorage() {
  const kept = new Map<string, string>()
  vi.stubGlobal('localStorage', { getItem: (key: string) => kept.get(key) ?? null, setItem: (key: string, value: string) => void kept.set(key, value) })
}

test('a PDF is set with Typst on the chosen paper, and an installed LaTeX can be chosen and is remembered', async () => {
  const user = userEvent.setup()
  windowStorage()
  const { api, calls } = standIn({ pdfEngines: ['typst', 'xelatex'] })
  const { unmount } = render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Export…' }))

  // the standard: Typst
  expect(((await screen.findByLabelText('Typeset with')) as HTMLSelectElement).value).toBe('typst')
  await user.selectOptions(screen.getByLabelText('Paper'), 'letter')
  await user.click(screen.getByRole('button', { name: 'Export' }))
  await screen.findByRole('status')
  expect(calls.pdf).toEqual([{ engine: 'typst', paper: 'letter' }])

  // the author's own LaTeX
  await user.selectOptions(screen.getByLabelText('Typeset with'), 'xelatex')
  await user.click(screen.getByRole('button', { name: 'Export' }))
  await waitFor(() => expect(calls.pdf[1]).toEqual({ engine: 'xelatex', paper: 'letter' }))
  // a Word file has neither choice
  await user.selectOptions(screen.getByLabelText('Format'), 'docx')
  expect(screen.queryByLabelText('Paper')).toBeNull()
  unmount()

  // next time the choice is still there
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Export…' }))
  expect(((await screen.findByLabelText('Typeset with')) as HTMLSelectElement).value).toBe('xelatex')
  expect((screen.getByLabelText('Paper') as HTMLSelectElement).value).toBe('letter')
  vi.unstubAllGlobals()
})

test('with Typst alone there is nothing to choose but the paper', async () => {
  const user = userEvent.setup()
  windowStorage()
  // a system set to German: A4
  vi.stubGlobal('navigator', { ...navigator, language: 'de-DE' })
  // the editor's own words stay English here; what follows the system is the paper
  const { api } = standIn({ settings: { preferences: { ...STANDARD, interfaceLanguage: 'en' } } })
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Export…' }))
  expect(((await screen.findByLabelText('Paper')) as HTMLSelectElement).value).toBe('a4')
  expect(screen.queryByLabelText('Typeset with')).toBeNull()
  vi.unstubAllGlobals()
})

test('Claude Code is chosen without a key or an address, and the dialog says whose sign-in is used', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await user.click(screen.getByRole('button', { name: 'Settings…' }))
  const dialog = within(screen.getByRole('dialog', { name: 'Settings' }))
  await user.click(dialog.getByRole('radio', { name: /Claude Code on this computer/ }))
  expect(dialog.queryByLabelText(/^API key/)).toBeNull()
  expect(dialog.queryByLabelText(/Address of the API/)).toBeNull()
  expect(dialog.getByText(/The editor does not see or store that sign-in/)).toBeTruthy()
  expect((dialog.getByLabelText(/^Model/) as HTMLInputElement).value).toBe('sonnet')
  await user.click(dialog.getByRole('button', { name: 'Save' }))
  await waitFor(() => expect(calls.settings).toEqual([[{ backend: 'claude-code', model: 'sonnet', baseUrl: '', maxTokens: 0 }, null]]))
})

test('a renamed document is offered its old log, and takes it only when the author says so', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn({ orphans: [{ name: 'draft.md', isSameText: true, questions: 4 }] })
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  const offer = await screen.findByText(/This folder holds an AI log for/)
  expect(offer.textContent).toContain('draft.md (4 questions)')
  expect(offer.textContent).toContain('Its last saved text is exactly this document.')
  expect(calls.adopted).toEqual([])
  await user.click(screen.getByRole('button', { name: 'Yes, continue its log' }))
  await waitFor(() => expect(calls.adopted).toEqual(['draft.md']))
  await waitFor(() => expect(screen.queryByText(/This folder holds an AI log for/)).toBeNull())
})

test('saying no to the old log leaves it alone, and moving sends the text as it is in the editor', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn({ orphans: [{ name: 'draft.md', isSameText: false, questions: 1 }] })
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  expect((await screen.findByText(/This folder holds an AI log for/)).textContent).toContain('Its last saved text differs from this document.')
  await user.click(screen.getByRole('button', { name: 'No, this is a new document' }))
  expect(screen.queryByText(/This folder holds an AI log for/)).toBeNull()
  expect(calls.adopted).toEqual([])

  await user.type(screen.getByLabelText('Document'), ' Not saved yet.')
  await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Move…' }))
  await screen.findByText('chapter-1.md')
  expect(calls.moved.length).toBe(1)
  expect(calls.moved[0]![0]).toBe('/work/drafts/chapter-1.md')
  expect(calls.moved[0]![1]).toContain('Not saved yet.')
  // the moved document is saved where it is now
  expect(within(screen.getByRole('banner')).getByText('saved')).toBeTruthy()
})

test('an action button asks with what was typed, or its own question, and names the action', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  const actions = within(screen.getByRole('group', { name: 'Ready-made requests' }))
  // explaining needs something to explain
  expect((actions.getByRole('button', { name: 'Explain' }) as HTMLButtonElement).disabled).toBe(true)

  await user.click(actions.getByRole('button', { name: 'Language' }))
  await waitFor(() => expect(calls.asked.length).toBe(1))
  expect(calls.asked[0]).toMatchObject({ text: 'Check the language of the whole document.', command: 'language', persona: null, replyTo: null })
  // in the thread the next message is a reply: no action buttons there
  await waitFor(() => expect(screen.queryByRole('group', { name: 'Ready-made requests' })).toBeNull())

  await user.click(screen.getByRole('button', { name: 'New question' }))
  await user.type(screen.getByLabelText('Ask the AI'), 'effect size')
  await user.click(within(screen.getByRole('group', { name: 'Ready-made requests' })).getByRole('button', { name: 'Explain' }))
  await waitFor(() => expect(calls.asked.length).toBe(2))
  expect(calls.asked[1]).toMatchObject({ text: 'effect size', command: 'explain' })
})

test('the status bar says Claude Code when Claude Code answers', async () => {
  const { api } = standIn({ settings: { backend: 'claude-code', model: 'haiku', baseUrl: '' } })
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  expect(await screen.findByText('haiku · Claude Code')).toBeTruthy()
})

/** A working storage for one test: this test window has none of its own. */
function withStorage() {
  const kept = new Map<string, string>()
  vi.stubGlobal('localStorage', { getItem: (key: string) => kept.get(key) ?? null, setItem: (key: string, value: string) => void kept.set(key, value), removeItem: (key: string) => void kept.delete(key) })
}

test('the side panel is as wide as the author made it, also the next time', async () => {
  withStorage()
  const user = userEvent.setup()
  const first = render(<App api={standIn().api} makeEditor={plainEditor} />)
  const divider = await screen.findByRole('separator', { name: 'Width of the side panel' })
  divider.focus()
  await user.keyboard('{ArrowLeft}{ArrowLeft}')
  const width = Number(divider.getAttribute('aria-valuenow'))
  expect(width).toBe(448)
  // never so wide that no room is left for the text
  await user.keyboard('{ArrowLeft}'.repeat(40))
  expect(Number(divider.getAttribute('aria-valuenow'))).toBe(window.innerWidth - 320)
  await user.keyboard('{Home}{ArrowLeft}')
  first.unmount()

  render(<App api={standIn().api} makeEditor={plainEditor} />)
  expect((await screen.findByRole('separator', { name: 'Width of the side panel' })).getAttribute('aria-valuenow')).toBe('424')
  vi.unstubAllGlobals()
})

test('markdown buttons, line numbers and the source view are the author\'s choice, and stay chosen', async () => {
  withStorage()
  const user = userEvent.setup()
  const first = render(<App api={standIn().api} makeEditor={plainEditor} />)
  // the buttons are there from the start, for someone new to markdown; the rest is off
  const bar = await screen.findByRole('toolbar', { name: 'Markdown' })
  await waitFor(() => expect(lastEditor.view).toEqual({ lineNumbers: false, source: false, spellcheck: true, language: 'en' }))
  await user.click(within(bar).getByRole('button', { name: 'Bold' }))
  await user.click(within(bar).getByRole('button', { name: 'Table' }))
  expect(lastEditor.formats).toEqual(['bold', 'table'])
  expect(within(bar).getByRole('button', { name: 'Bold' }).title).toContain('**bold**')

  await user.click(screen.getByRole('button', { name: 'Line numbers' }))
  await user.click(screen.getByRole('button', { name: 'Source' }))
  await waitFor(() => expect(lastEditor.view).toEqual({ lineNumbers: true, source: true, spellcheck: true, language: 'en' }))
  await user.click(screen.getByRole('button', { name: 'Markdown buttons' }))
  expect(screen.queryByRole('toolbar', { name: 'Markdown' })).toBeNull()
  first.unmount()

  render(<App api={standIn().api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await waitFor(() => expect(lastEditor.view).toEqual({ lineNumbers: true, source: true, spellcheck: true, language: 'en' }))
  expect(screen.queryByRole('toolbar', { name: 'Markdown' })).toBeNull()
  expect(screen.getByRole('button', { name: 'Source' }).getAttribute('aria-pressed')).toBe('true')
  vi.unstubAllGlobals()
})

test('the author can read how markdown works, and go on to the full guide in the browser', async () => {
  const user = userEvent.setup()
  const { api } = standIn()
  const opened: string[] = []
  api.openGuide = async which => void opened.push(which)
  render(<App api={api} makeEditor={plainEditor} />)
  await user.click(await screen.findByRole('button', { name: 'How markdown works' }))
  const help = screen.getByRole('dialog', { name: 'How markdown works' })
  for (const typed of ['## Heading', '**bold** and *italic*', '[@smith2020]', 'bibliography: refs.bib']) expect(help.textContent).toContain(typed)
  // the window follows no address itself: it asks the app for one of the two guides
  expect(help.querySelector('a')).toBeNull()
  await user.click(within(help).getByRole('button', { name: 'Markdown in 60 seconds' }))
  expect(opened).toEqual(['markdown'])
  await user.click(within(help).getByRole('button', { name: 'Close' }))
  expect(screen.queryByRole('dialog', { name: 'How markdown works' })).toBeNull()
})

test('the settings change how the editor looks at once, keep it on Save, and put it back on Cancel', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  const root = document.documentElement
  const openSection = async (name: string) => {
    if (screen.queryByRole('dialog', { name: 'Settings' }) === null) await user.click(screen.getByRole('button', { name: 'Settings…' }))
    const dialog = within(screen.getByRole('dialog', { name: 'Settings' }))
    await user.click(dialog.getByRole('tab', { name }))
    return dialog
  }

  // seen while it is chosen, gone when the author cancels
  let dialog = await openSection('Text and look')
  await user.click(dialog.getByRole('radio', { name: 'Dark' }))
  expect(root.dataset.theme).toBe('dark')
  await user.clear(dialog.getByLabelText(/^Size of your text/))
  await user.type(dialog.getByLabelText(/^Size of your text/), '400{Enter}')
  expect(root.style.getPropertyValue('--text-size')).toBe('28px') // held to what can be read
  await user.click(dialog.getByRole('button', { name: 'Cancel' }))
  expect(root.dataset.theme).toBeUndefined()
  expect(root.style.getPropertyValue('--text-size')).toBe('18px')
  expect(calls.preferences).toEqual([])

  // look, files, live review and language in one go; the AI's settings are not touched
  dialog = await openSection('Text and look')
  await user.click(dialog.getByRole('radio', { name: 'Dark' }))
  await user.selectOptions(dialog.getByLabelText(/^Length of a line/), 'wide')
  await user.click(dialog.getByRole('checkbox', { name: /spelling mistakes/ }))
  dialog = await openSection('Files')
  await user.click(dialog.getByRole('button', { name: 'Choose: Folder for documents' }))
  expect(await dialog.findByText('/work/exports')).toBeTruthy()
  await user.selectOptions(dialog.getByLabelText(/^Saving/), '30')
  dialog = await openSection('Live review')
  await user.clear(dialog.getByLabelText(/^Pause before a review/))
  await user.type(dialog.getByLabelText(/^Pause before a review/), '45')
  await user.clear(dialog.getByLabelText(/^Time between two reviews/))
  await user.type(dialog.getByLabelText(/^Time between two reviews/), '600{Enter}')
  expect(dialog.getByRole('status').textContent).toContain('at most 6 reviews are sent in an hour')
  dialog = await openSection('Language')
  await user.selectOptions(dialog.getByLabelText(/^Language of the disclosure/), 'de')
  await user.click(dialog.getByRole('button', { name: 'Save' }))
  await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
  expect(calls.settings).toEqual([])
  expect(calls.preferences).toEqual([{ ...STANDARD, theme: 'dark', lineWidth: 'wide', spellcheck: false, documentsFolder: '/work/exports', autosaveSeconds: 30, livePauseSeconds: 45, liveGapSeconds: 600, disclosureLanguage: 'de' }])

  // and the window is as set: dark, wide lines, no spelling marks, "Open" starts in the folder, live review says its pace
  expect(root.dataset.theme).toBe('dark')
  expect(root.style.getPropertyValue('--measure')).toBe('920px')
  await waitFor(() => expect(lastEditor.view?.spellcheck).toBe(false))
  await user.click(screen.getByRole('button', { name: 'Open…' }))
  await waitFor(() => expect(calls.picked).toEqual(['open in /work/exports']))
  await user.click(screen.getByRole('tab', { name: /Live review/ }))
  expect(await screen.findByText(/once you pause for 45 seconds: at most one review every 10 minutes and 20 an hour/)).toBeTruthy()
  await user.type(screen.getByLabelText('Document'), ' x')
  await user.click(screen.getByRole('tab', { name: 'Ask' }))
  await user.type(screen.getByLabelText('Ask the AI'), 'Any feedback?')
  await user.click(screen.getByRole('button', { name: 'Ask' }))
  await screen.findByText(/why subjects/)
  await user.click(screen.getByRole('button', { name: 'Disclosure…' }))
  expect((within(screen.getByRole('dialog')).getByLabelText(/Language/) as HTMLSelectElement).value).toBe('de')
  await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: /Close|Cancel/ }))

  // everything but the AI goes back to how the editor came
  dialog = await openSection('About')
  expect(dialog.getByText('version 0.1.0')).toBeTruthy()
  expect(dialog.getByText('/home/me/.config/cowriting-editor')).toBeTruthy()
  await user.click(dialog.getByRole('button', { name: 'Show the folder' }))
  expect(calls.picked).toContain('shown')
  await user.click(dialog.getByRole('button', { name: 'Everything but the AI back to standard' }))
  expect(root.dataset.theme).toBeUndefined()
  await user.click(dialog.getByRole('button', { name: 'Save' }))
  await waitFor(() => expect(calls.preferences[1]).toEqual(STANDARD))
  root.removeAttribute('style')
})

test('an answer\'s length can be limited, but not to less than a model needs to answer at all', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await user.click(screen.getByRole('button', { name: 'Settings…' }))
  const dialog = within(screen.getByRole('dialog', { name: 'Settings' }))
  const save = dialog.getByRole('button', { name: 'Save' }) as HTMLButtonElement
  expect(save.disabled).toBe(true) // nothing changed yet
  await user.type(dialog.getByLabelText(/^Longest answer/), '100')
  expect(dialog.getByText(/At least 500, or empty for no limit/)).toBeTruthy()
  expect(save.disabled).toBe(true)
  await user.clear(dialog.getByLabelText(/^Longest answer/))
  await user.type(dialog.getByLabelText(/^Longest answer/), '2000')
  await user.click(save)
  await waitFor(() => expect(calls.settings).toEqual([[{ backend: 'anthropic', model: 'claude-opus-5-5', baseUrl: '', maxTokens: 2000 }, null]]))
  expect(calls.preferences).toEqual([])

  // Claude Code decides the length itself, and the dialog says so instead of offering a field
  await user.click(screen.getByRole('button', { name: 'Settings…' }))
  const again = within(screen.getByRole('dialog', { name: 'Settings' }))
  expect((again.getByLabelText(/^Longest answer/) as HTMLInputElement).value).toBe('2000')
  await user.click(again.getByRole('radio', { name: /Claude Code/ }))
  expect(again.queryByLabelText(/^Longest answer/)).toBeNull()
  expect(again.getByText(/How long an answer may be is Claude Code’s own decision/)).toBeTruthy()
})

test('with saving by itself on, the text is saved once the author pauses', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn({ settings: { preferences: { ...STANDARD, autosaveSeconds: 0.2 } } })
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await user.type(screen.getByLabelText('Document'), 'Mine.')
  expect(calls.saved).toEqual([]) // not while typing
  await waitFor(() => expect(calls.saved).toEqual(['# Thesis\n\nWe recruited 48 subjects.\nMine.']))
  await screen.findByText('saved')
})

test('shortcuts do what their buttons do, and the author can give them other keys', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  const text = screen.getByLabelText('Document')
  await user.type(text, 'Mine.')
  await user.keyboard('{Control>}b{/Control}{Control>}s{/Control}')
  expect(lastEditor.formats).toEqual(['bold'])
  await waitFor(() => expect(calls.saved.length).toBe(1))
  expect(within(screen.getByRole('toolbar', { name: 'Markdown' })).getByRole('button', { name: 'Bold' }).title).toContain('(Ctrl+B)')
  // a markdown shortcut writes into the text only: in the question field it does nothing
  await user.click(screen.getByLabelText('Ask the AI'))
  await user.keyboard('{Control>}b{/Control}')
  expect(lastEditor.formats).toEqual(['bold'])

  // the tabs of the side panel have keys too, from the text as from anywhere else
  await user.click(text)
  await user.keyboard('{Control>}6{/Control}')
  expect(screen.getByRole('tab', { name: /Log/ }).getAttribute('aria-selected')).toBe('true')
  await user.keyboard('{Control>}2{/Control}')
  expect(screen.getByRole('tab', { name: 'Experts' }).getAttribute('aria-selected')).toBe('true')
  expect(lastEditor.formats).toEqual(['bold'])

  await user.keyboard('{Control>},{/Control}')
  const dialog = within(await screen.findByRole('dialog', { name: 'Settings' }))
  await user.click(dialog.getByRole('tab', { name: 'Shortcuts' }))
  expect(within(dialog.getByRole('row', { name: /^Show Plan/ })).getByText('Ctrl+4')).toBeTruthy()
  const row = () => within(dialog.getByRole('row', { name: /^Bold/ }))
  expect(row().getByText('Ctrl+B')).toBeTruthy()
  await user.click(dialog.getByRole('button', { name: 'Change the shortcut for Bold' }))
  expect(row().getByText('Press the keys…')).toBeTruthy()
  // keys another shortcut has, keys without Ctrl, and keys every program shares are refused with the reason
  await user.keyboard('{Control>}i{/Control}')
  expect(dialog.getByRole('alert').textContent).toContain('Ctrl+I is the shortcut for “Italic”')
  await user.keyboard('x')
  expect(dialog.getByRole('alert').textContent).toContain('A shortcut needs the Ctrl key')
  await user.keyboard('{Control>}c{/Control}')
  expect(dialog.getByRole('alert').textContent).toContain('Ctrl+C stays what it is')
  await user.keyboard('{Control>}{Shift>}B{/Shift}{/Control}')
  expect(dialog.queryByRole('alert')).toBeNull()
  expect(row().getByText('Ctrl+Shift+B')).toBeTruthy()
  await user.click(dialog.getByRole('button', { name: 'Remove the shortcut for Italic' }))
  await user.click(dialog.getByRole('button', { name: 'Save' }))
  await waitFor(() => expect(calls.preferences.at(-1)?.shortcuts).toEqual({ bold: 'Mod-Shift-b', italic: '' }))

  // the old keys are free, the new ones work, and the button's tooltip names them
  await user.click(text)
  await user.keyboard('{Control>}b{/Control}{Control>}i{/Control}')
  expect(lastEditor.formats).toEqual(['bold'])
  await user.keyboard('{Control>}{Shift>}B{/Shift}{/Control}')
  expect(lastEditor.formats).toEqual(['bold', 'bold'])
  expect(within(screen.getByRole('toolbar', { name: 'Markdown' })).getByRole('button', { name: 'Bold' }).title).toContain('(Ctrl+Shift+B)')
})

test('the author chooses a reference style file for the export, and takes it off again', async () => {
  const user = userEvent.setup()
  const { api } = standIn()
  const opened: string[] = []
  api.openGuide = async which => void opened.push(which)
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await user.click(screen.getByRole('tab', { name: 'Sources' }))
  // without a choice the export's style is named, so nobody wonders where "Chicago" came from
  expect(await screen.findByText(/None chosen: citations and the reference list are exported in pandoc's standard style/)).toBeTruthy()
  await user.click(screen.getByRole('button', { name: 'Choose a style file' }))
  expect(await screen.findByText('American Psychological Association 7th edition')).toBeTruthy()
  expect(screen.getByText('apa.csl')).toBeTruthy()
  expect(screen.getByRole('button', { name: 'Choose another style file' })).toBeTruthy()
  // where to get styles: the app opens that one page, the window follows no address
  await user.click(screen.getByRole('button', { name: 'Reference styles to download' }))
  expect(opened).toEqual(['styles'])
  await user.click(screen.getByRole('button', { name: 'Take off: back to the standard style' }))
  expect(await screen.findByText(/None chosen/)).toBeTruthy()
})

test('a file that is no style is refused in plain words', async () => {
  const user = userEvent.setup()
  render(<App api={standIn({ stylePicked: '/work/refs.bib' }).api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await user.click(screen.getByRole('tab', { name: 'Sources' }))
  await user.click(await screen.findByRole('button', { name: 'Choose a style file' }))
  expect((await screen.findByRole('alert')).textContent).toContain('A reference style is a .csl file')
  expect(screen.getByText(/None chosen/)).toBeTruthy()
})

test('the editor speaks German as soon as it is chosen, and the three languages are saved apart', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await user.click(screen.getByRole('button', { name: 'Settings…' }))
  let dialog = within(screen.getByRole('dialog', { name: 'Settings' }))
  await user.click(dialog.getByRole('tab', { name: 'Language' }))

  // tried at once, the dialog included; English text and German answers stay what they are
  await user.selectOptions(dialog.getByLabelText(/^Language of the editor/), 'de')
  dialog = within(screen.getByRole('dialog', { name: 'Einstellungen' }))
  expect(document.documentElement.lang).toBe('de')
  await user.selectOptions(dialog.getByLabelText(/^Sprache, in der Sie schreiben/), 'en')
  await user.selectOptions(dialog.getByLabelText(/^Sprache, in der die KI antwortet/), 'de')
  await user.click(dialog.getByRole('button', { name: 'Speichern' }))
  expect(calls.preferences.at(-1)).toMatchObject({ interfaceLanguage: 'de', writingLanguage: 'en', aiLanguage: 'de' })

  // the window itself: buttons, tabs, and the ready-made requests
  const banner = within(screen.getByRole('banner'))
  banner.getByRole('button', { name: 'Exportieren…' })
  expect(banner.queryByRole('button', { name: 'Export…' })).toBeNull()
  screen.getByRole('tab', { name: /Quellen/ })

  // the experts that come with the editor have German names and fields; the app's names are untouched
  await user.click(screen.getByRole('tab', { name: 'Experten' }))
  screen.getByText('Die Klinikerin')
  screen.getByText('Schwächen, Limitationen, unbequeme Fragen')
  expect(screen.queryByText('The Clinician')).toBeNull()

  // and back, without starting again
  await user.click(banner.getByRole('button', { name: 'Einstellungen…' }))
  dialog = within(screen.getByRole('dialog', { name: 'Einstellungen' }))
  await user.click(dialog.getByRole('tab', { name: 'Sprache' }))
  await user.selectOptions(dialog.getByLabelText(/^Sprache des Editors/), 'en')
  await user.click(within(screen.getByRole('dialog', { name: 'Settings' })).getByRole('button', { name: 'Save' }))
  within(screen.getByRole('banner')).getByRole('button', { name: 'Export…' })
})

test('buttons and panels are drawn as large as the author sets them, and the text keeps its own size', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  const root = document.documentElement
  expect(root.style.getPropertyValue('--ui-zoom')).toBe('')
  await user.click(screen.getByRole('button', { name: 'Settings…' }))
  const dialog = within(screen.getByRole('dialog', { name: 'Settings' }))
  await user.click(dialog.getByRole('tab', { name: 'Text and look' }))
  await user.selectOptions(dialog.getByLabelText(/Size of buttons and panels/), 'Large')
  expect(root.style.getPropertyValue('--ui-zoom')).toBe('1.15')
  expect(root.style.getPropertyValue('--text-size')).toBe('18px')
  await user.click(dialog.getByRole('button', { name: 'Save' }))
  await waitFor(() => expect(calls.preferences.at(-1)).toMatchObject({ uiSize: 'large', textSize: 18 }))
  expect(root.style.getPropertyValue('--ui-zoom')).toBe('1.15')
})

test('a bug is reported from a button everybody sees, in three parts, and goes out as the mail that was shown', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  // the footer says which version this is, and that it is an alpha
  expect(within(screen.getByRole('contentinfo')).getByText('alpha 0.1.0')).toBeTruthy()

  await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Report a bug' }))
  const dialog = within(screen.getByRole('dialog', { name: 'Report a bug' }))
  const mail = dialog.getByRole('button', { name: 'Open as mail' }) as HTMLButtonElement
  expect(mail.disabled).toBe(true)
  await user.type(dialog.getByLabelText('In one line'), 'Export stops')
  await user.type(dialog.getByLabelText(/What you did/), 'Chose PDF.')
  await user.type(dialog.getByLabelText(/What happened/), 'Nothing arrives.')
  await user.type(dialog.getByLabelText(/What you expected/), 'A PDF.')
  const outgoing = () => (dialog.getByLabelText(/What goes out/) as HTMLTextAreaElement).value
  expect(outgoing()).toBe('Bug: Export stops\n\nWhat I did:\nChose PDF.\n\nWhat happened:\nNothing arrives.\n\nWhat I expected:\nA PDF.\n\n---\nCoWriting Editor 0.1.0, macos aarch64\nShown in: en\nAI: anthropic, model claude-opus-5-5')
  expect(outgoing()).not.toContain('48 subjects')
  expect(calls.feedback).toEqual([])

  // the technical log goes only when asked for, and is shown line by line before it does
  const withLog = dialog.getByRole('checkbox', { name: /technical log of the last 10 minutes/ }) as HTMLInputElement
  expect(withLog.checked).toBe(false)
  await user.click(withLog)
  await waitFor(() => expect(outgoing()).toContain('--- Technical log, last 10 minutes ---\n10:01:00 start CoWriting Editor 0.1.0, macos aarch64\n10:02:10 call exportDocument failed 4012ms: pandoc ended with <path>\n10:02:11 document at-failed-export'))
  expect(outgoing()).not.toContain('48 subjects')
  await user.click(withLog)
  expect(outgoing()).not.toContain('Technical log')

  // a file to attach holds the log; the document's text goes into it only after the author agreed
  await user.click(dialog.getByRole('button', { name: 'Save a file to attach…' }))
  await waitFor(() => expect(calls.reportFiles).toEqual([['/work/cowriting-report.txt', 10, null]]))
  expect(dialog.getByRole('status').textContent).toContain('without any text of yours')
  await user.click(dialog.getByRole('checkbox', { name: /Put the text of my document into this file too/ }))
  expect(dialog.getByText(/An issue on GitHub is public/)).toBeTruthy()
  await user.click(dialog.getByRole('button', { name: 'Save a file to attach…' }))
  await waitFor(() => expect(calls.reportFiles.at(-1)).toEqual(['/work/cowriting-report.txt', 10, expect.stringContaining('We recruited 48 subjects.')]))
  expect(dialog.getByRole('status').textContent).toContain('with the text of your document')
  expect(outgoing()).not.toContain('48 subjects')

  // without the details only the author's words go, and what is handed over is what was shown
  await user.click(dialog.getByRole('checkbox', { name: /editor’s version/ }))
  await user.click(mail)
  expect(calls.feedback).toEqual([['mail', 'Bug: Export stops', 'What I did:\nChose PDF.\n\nWhat happened:\nNothing arrives.\n\nWhat I expected:\nA PDF.']])
  expect(dialog.getByRole('status').textContent).toContain('sent only when you send it there')
  await user.click(dialog.getByRole('button', { name: 'Close' }))

  // About names the contact and the project's page, and has the same forms
  await user.click(screen.getByRole('button', { name: 'Settings…' }))
  const settings = within(screen.getByRole('dialog', { name: 'Settings' }))
  await user.click(settings.getByRole('tab', { name: 'About' }))
  settings.getByText('me@example.org')
  await user.click(settings.getByRole('button', { name: 'github.com/dozwa/cowriting-editor' }))
  expect(calls.pages).toEqual(['repository'])
  await user.click(settings.getByRole('button', { name: 'Give feedback…' }))
  const feedback = within(screen.getByRole('dialog', { name: 'Feedback' }))
  await user.type(feedback.getByLabelText('In one line'), 'Footnotes')
  await user.type(feedback.getByLabelText(/What you want to say/), 'I miss them in the export.')
  await user.click(feedback.getByRole('checkbox'))
  await user.click(feedback.getByRole('button', { name: 'Open as GitHub issue' }))
  expect(calls.feedback.at(-1)).toEqual(['issue', 'Idea: Footnotes', 'I miss them in the export.'])
  // closing the form leaves the settings open
  await user.click(feedback.getByRole('button', { name: 'Close' }))
  expect(screen.getByRole('dialog', { name: 'Settings' })).toBeTruthy()
})

test('at a start when the editor asks for support it asks once, and what the author chooses is told to the app', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn({ settings: { asking: { isDue: true, withStar: false } } })
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  const dialog = within(await screen.findByRole('dialog', { name: 'Does the editor help you?' }))
  // a star that was given is not asked for again
  expect(dialog.queryByRole('button', { name: 'Star it on GitHub' })).toBeNull()
  dialog.getByRole('button', { name: 'Give feedback' })
  await user.click(dialog.getByRole('button', { name: 'Buy me a coffee' }))
  expect(calls.support).toEqual(['coffee'])
  expect(calls.pages).toEqual(['coffee'])
  expect(screen.queryByRole('dialog', { name: 'Does the editor help you?' })).toBeNull()
  // nothing asks when it is not such a start
  const quiet = standIn()
  render(<App api={quiet.api} makeEditor={plainEditor} />)
  await screen.findAllByText('thesis.md')
  expect(screen.queryByRole('dialog', { name: 'Does the editor help you?' })).toBeNull()
})

test('the author links a file as background, says whom it is for, and rewords an expert', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await user.click(screen.getByRole('tab', { name: 'Experts' }))
  screen.getByText('None linked.')
  await user.click(screen.getByRole('button', { name: 'Link a file…' }))
  await screen.findByText('wording.md')
  expect(calls.project).toEqual([['link', '/work/wording.md', []]])

  // for everyone until an expert is ticked
  await user.click(screen.getByRole('button', { name: 'Who gets wording.md' }))
  await user.click(within(screen.getByRole('group', { name: 'Who gets wording.md' })).getByRole('checkbox', { name: 'Reviewer #2' }))
  await waitFor(() => expect(calls.project.at(-1)).toEqual(['link', '/work/wording.md', ['reviewer-2']]))
  expect(screen.getByRole('button', { name: 'Who gets wording.md' }).textContent).toBe('For Reviewer #2')

  // the whole of what an expert is told can be reworded, and put back
  const reviewer = () => within(screen.getByText('Weaknesses, limitations, tough questions').closest('li')!)
  await user.click(reviewer().getByRole('button', { name: 'Prompt' }))
  const prompt = reviewer().getByLabelText('What Reviewer #2 is told') as HTMLTextAreaElement
  expect(prompt.value).toContain('the weak points')
  await user.clear(prompt)
  await user.type(prompt, 'You are kind.')
  await user.click(reviewer().getByRole('button', { name: 'Save' }))
  await waitFor(() => expect(calls.project.at(-1)).toEqual(['prompt', 'reviewer-2', 'You are kind.']))
  await user.click(await within(screen.getByText('Weaknesses, limitations, tough questions').closest('li')!).findByRole('button', { name: 'Back to the standard' }))
  await waitFor(() => expect(calls.project.at(-1)).toEqual(['prompt', 'reviewer-2', '']))

  await user.click(screen.getByRole('button', { name: 'Unlink wording.md' }))
  await screen.findByText('None linked.')
})

test('languages can be set for one document alone, and distraction-free hides the explanations until it is turned off', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await user.click(screen.getByRole('button', { name: 'Settings…' }))
  let dialog = within(screen.getByRole('dialog', { name: 'Settings' }))
  await user.click(dialog.getByRole('tab', { name: 'Language' }))
  await user.click(dialog.getByRole('checkbox', { name: /Only for thesis.md/ }))
  await user.selectOptions(dialog.getByLabelText(/^Language the AI answers in/), 'de')
  await user.click(dialog.getByRole('button', { name: 'Save' }))
  await waitFor(() => expect(calls.project.at(-1)).toEqual(['set', { writingLanguage: 'document', aiLanguage: 'de', disclosureLanguage: 'document' }]))
  // the general settings keep what they had
  expect(calls.preferences.at(-1)).toMatchObject({ aiLanguage: 'auto' })

  // opened again, the dialog shows what holds for this document, and unticking makes it general
  await user.click(screen.getByRole('button', { name: 'Settings…' }))
  dialog = within(screen.getByRole('dialog', { name: 'Settings' }))
  await user.click(dialog.getByRole('tab', { name: 'Language' }))
  expect((dialog.getByRole('checkbox', { name: /Only for thesis.md/ }) as HTMLInputElement).checked).toBe(true)
  expect((dialog.getByLabelText(/^Language the AI answers in/) as HTMLSelectElement).value).toBe('de')
  await user.click(dialog.getByRole('button', { name: 'Cancel' }))

  // distraction-free: from the status bar, kept with the settings
  expect(document.documentElement.dataset.calm).toBeUndefined()
  await user.click(screen.getByRole('button', { name: 'Calm' }))
  await waitFor(() => expect(document.documentElement.dataset.calm).toBe(''))
  expect(calls.preferences.at(-1)).toMatchObject({ calm: true })
  await user.click(screen.getByRole('button', { name: 'Calm' }))
  await waitFor(() => expect(document.documentElement.dataset.calm).toBeUndefined())
})

test('AI use outside the editor is stated in a form with two things needed, corrected, and taken back', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  // nothing logged yet: no disclosure, but the way to state outside use is there
  expect((screen.getByRole('button', { name: 'Disclosure…' }) as HTMLButtonElement).disabled).toBe(true)
  await user.click(screen.getByRole('tab', { name: /Log/ }))
  await user.click(screen.getByRole('button', { name: 'State AI use elsewhere…' }))
  let dialog = within(screen.getByRole('dialog', { name: 'AI use outside the editor' }))
  const save = dialog.getByRole('button', { name: 'Add to the log' }) as HTMLButtonElement
  expect(save.disabled).toBe(true)
  await user.type(dialog.getByLabelText('Tool'), 'ChatGPT')
  expect(save.disabled).toBe(true)
  await user.type(dialog.getByLabelText('What it was used for'), 'R code for the analysis')
  await user.type(dialog.getByLabelText(/What it worked on/), 'the trial data')
  await user.click(save)
  await waitFor(() => expect(calls.project.at(-1)).toEqual(['state', { id: '', tool: 'ChatGPT', model: '', date: '', purpose: 'R code for the analysis', material: 'the trial data', result: '', checked: '' }]))
  await screen.findByText('R code for the analysis')
  expect(screen.queryByRole('dialog', { name: 'AI use outside the editor' })).toBeNull()
  // a statement alone is something to disclose
  expect((screen.getByRole('button', { name: 'Disclosure…' }) as HTMLButtonElement).disabled).toBe(false)

  await user.click(screen.getByRole('button', { name: 'Correct the statement about ChatGPT' }))
  dialog = within(screen.getByRole('dialog', { name: 'Correct the statement' }))
  await user.type(dialog.getByLabelText(/Model or version/), 'GPT-x')
  await user.click(dialog.getByRole('button', { name: 'Save the correction' }))
  await waitFor(() => expect(calls.project.at(-1)).toMatchObject(['state', { id: 'x1', model: 'GPT-x', purpose: 'R code for the analysis' }]))
  await screen.findByText('GPT-x')

  await user.click(screen.getByRole('button', { name: 'Take back the statement about ChatGPT' }))
  await waitFor(() => expect(screen.queryByText('R code for the analysis')).toBeNull())
  expect(calls.project.at(-1)).toEqual(['withdraw', 'x1b'])
})

test('a new author is shown around: the tour goes through the window part by part, marks what it speaks about, and is told to the app once it is through', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn({ settings: { showTour: true } })
  const { container } = render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  const tour = within(await screen.findByRole('dialog', { name: 'You write, the AI reads' }))
  tour.getByText('Step 1 of 12')
  expect(tour.queryByRole('button', { name: 'Back' })).toBeNull()
  const marked = () => [...container.querySelectorAll('.toured')].map(part => part.textContent)

  // the AI is set up here, so that step offers nothing to do; Settings is marked
  await user.click(tour.getByRole('button', { name: 'Next' }))
  screen.getByRole('dialog', { name: 'Choose the AI' })
  expect(tour.queryByRole('button', { name: 'Set up the AI…' })).toBeNull()
  expect(marked()).toEqual(['Settings…'])
  await user.click(tour.getByRole('button', { name: 'Next' }))
  expect(marked()).toEqual(['Open…', 'New…'])

  // the steps about the side panel show the tab they speak about
  await user.click(tour.getByRole('button', { name: 'Next' }))
  await user.click(tour.getByRole('button', { name: 'Next' }))
  screen.getByRole('dialog', { name: 'Ask about your text' })
  expect(screen.getByRole('tab', { name: 'Ask' }).getAttribute('aria-selected')).toBe('true')
  for (const name of ['Experts', 'Live review', 'Plan', 'Sources', 'Log']) {
    await user.click(tour.getByRole('button', { name: 'Next' }))
    expect(screen.getByRole('tab', { name: new RegExp(`^${name}`) }).getAttribute('aria-selected')).toBe('true')
    expect(container.querySelector('.panel')?.classList.contains('toured')).toBe(true)
  }
  // back is a step back, tab and all
  await user.click(tour.getByRole('button', { name: 'Back' }))
  expect(screen.getByRole('tab', { name: 'Sources' }).getAttribute('aria-selected')).toBe('true')
  await user.click(tour.getByRole('button', { name: 'Next' }))

  await user.click(tour.getByRole('button', { name: 'Next' }))
  expect(marked()).toEqual(['Export…', 'Disclosure…'])
  await user.click(tour.getByRole('button', { name: 'Next' }))
  tour.getByText('Step 12 of 12')
  expect(calls.support).toEqual([])
  await user.click(tour.getByRole('button', { name: 'Done' }))
  expect(calls.support).toEqual(['tour'])
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(marked()).toEqual([])
})

test('at the very first start the tour comes before the set-up, offers it, and skipping the tour leads to it', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn({ isReady: false, settings: { isFirstRun: true, hasKey: false, showTour: true, openAtStart: null } })
  render(<App api={api} makeEditor={plainEditor} />)
  const tour = within(await screen.findByRole('dialog', { name: 'You write, the AI reads' }))
  expect(screen.queryByRole('dialog', { name: 'Set up the AI' })).toBeNull()

  // no document is open: the tour says so where it speaks about parts that are not there
  await user.click(tour.getByRole('button', { name: 'Next' }))
  tour.getByRole('button', { name: 'Set up the AI…' })
  expect(tour.queryByRole('note')).toBeNull()
  await user.click(tour.getByRole('button', { name: 'Next' }))
  await user.click(tour.getByRole('button', { name: 'Next' }))
  expect(tour.getByRole('note').textContent).toBe('Open a document to see this part of the window.')

  await user.click(tour.getByRole('button', { name: 'Skip the tour' }))
  expect(calls.support).toEqual(['tour'])
  expect(screen.queryByRole('dialog', { name: 'Write' })).toBeNull()
  await screen.findByRole('dialog', { name: 'Set up the AI' })
})

test('the tour can be seen again: from the first page, and from About in the settings', async () => {
  const user = userEvent.setup()
  const { api } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  expect(screen.queryByRole('dialog')).toBeNull() // seen before: it does not start by itself
  await user.click(screen.getByRole('button', { name: 'Settings…' }))
  const settings = within(await screen.findByRole('dialog', { name: 'Settings' }))
  await user.click(settings.getByRole('tab', { name: 'About' }))
  await user.click(settings.getByRole('button', { name: 'Show the tour' }))
  expect(screen.queryByRole('dialog', { name: 'Settings' })).toBeNull()
  screen.getByRole('dialog', { name: 'You write, the AI reads' })
  // Escape closes it
  await user.keyboard('{Escape}')
  expect(screen.queryByRole('dialog')).toBeNull()

  const fresh = standIn({ settings: { openAtStart: null } })
  render(<App api={fresh.api} makeEditor={plainEditor} />)
  await user.click(await screen.findByRole('button', { name: 'Take the tour' }))
  screen.getByRole('dialog', { name: 'You write, the AI reads' })
})

test('usage figures are asked for once, nothing is ticked, and only what was ticked is agreed to', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn({ settings: { usage: { isAvailable: true, isAsked: false, installation: null } } })
  render(<App api={api} makeEditor={plainEditor} />)
  const dialog = within(await screen.findByRole('dialog', { name: 'May the editor say that it is used?' }))
  const ping = dialog.getByRole('checkbox', { name: /a sign of use at each start/ }) as HTMLInputElement
  const figures = dialog.getByRole('checkbox', { name: /figures of each session/ }) as HTMLInputElement
  expect([ping.checked, figures.checked]).toEqual([false, false])
  // agreeing to nothing is not an agreement
  expect((dialog.getByRole('button', { name: 'Agree to what is ticked' }) as HTMLButtonElement).disabled).toBe(true)

  // what would go out and the statement can be read before deciding
  await user.click(dialog.getByRole('button', { name: 'Show what would be sent' }))
  expect(((await dialog.findByLabelText('At start')) as HTMLTextAreaElement).value).toContain('"kind": "start"')
  await user.click(dialog.getByRole('button', { name: 'Read the privacy statement' }))
  expect(((await dialog.findByLabelText('Privacy statement')) as HTMLTextAreaElement).value).toBe('Privacy statement: usage figures')

  await user.click(ping)
  await user.click(dialog.getByRole('button', { name: 'Agree to what is ticked' }))
  await waitFor(() => expect(calls.usage).toEqual([[true, false]]))
  await waitFor(() => expect(screen.queryByRole('dialog', { name: 'May the editor say that it is used?' })).toBeNull())

  // the settings show what stands, and a tick taken away there is saved as a withdrawal
  await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Settings…' }))
  const settings = within(screen.getByRole('dialog', { name: 'Settings' }))
  await user.click(settings.getByRole('tab', { name: 'About' }))
  const standing = settings.getByRole('checkbox', { name: /a sign of use at each start/ }) as HTMLInputElement
  expect(standing.checked).toBe(true)
  expect(settings.getByText('This installation’s number: a1b2c3')).toBeTruthy()
  await user.click(standing)
  await user.click(settings.getByRole('button', { name: 'Save' }))
  await waitFor(() => expect(calls.preferences.at(-1)).toMatchObject({ usagePing: false, usageFigures: false }))
})

test('closing the question about usage figures is a no, and without a server nobody is asked or shown anything', async () => {
  const user = userEvent.setup()
  const asked = standIn({ settings: { usage: { isAvailable: true, isAsked: false, installation: null } } })
  const first = render(<App api={asked.api} makeEditor={plainEditor} />)
  const dialog = within(await screen.findByRole('dialog', { name: 'May the editor say that it is used?' }))
  await user.click(dialog.getByRole('checkbox', { name: /figures of each session/ }))
  await user.click(dialog.getByRole('button', { name: 'Send nothing' }))
  await waitFor(() => expect(asked.calls.usage).toEqual([[false, false]]))
  first.unmount()

  const quiet = standIn()
  render(<App api={quiet.api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  expect(screen.queryByRole('dialog')).toBeNull()
  await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Settings…' }))
  const settings = within(screen.getByRole('dialog', { name: 'Settings' }))
  await user.click(settings.getByRole('tab', { name: 'About' }))
  expect(settings.queryByText('Usage figures')).toBeNull()
  expect(quiet.calls.usage).toEqual([])
})

test('a document can have an AI of its own: it is kept with the document, and the general settings stay', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Settings…' }))
  let dialog = within(screen.getByRole('dialog', { name: 'Settings' }))
  const own = dialog.getByRole('checkbox', { name: 'Only for thesis.md' }) as HTMLInputElement
  expect(own.checked).toBe(false)
  await user.click(own)
  const model = dialog.getByLabelText(/^Model/) as HTMLInputElement
  await user.clear(model)
  await user.type(model, 'claude-haiku-5-5')
  await user.click(dialog.getByRole('button', { name: 'Save' }))
  await waitFor(() => expect(calls.project.at(-1)).toEqual(['set', { ai: { backend: 'anthropic', model: 'claude-haiku-5-5', baseUrl: '', maxTokens: 0 } }]))
  expect(calls.settings).toEqual([])
  // the window says which model this document is asked with
  await waitFor(() => expect(within(screen.getByRole('contentinfo')).getByText(/claude-haiku-5-5/)).toBeTruthy())

  // giving it up shows the general settings again, and saving takes the document's own away
  await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Settings…' }))
  dialog = within(screen.getByRole('dialog', { name: 'Settings' }))
  const again = dialog.getByRole('checkbox', { name: 'Only for thesis.md' }) as HTMLInputElement
  expect(again.checked).toBe(true)
  expect((dialog.getByLabelText(/^Model/) as HTMLInputElement).value).toBe('claude-haiku-5-5')
  await user.click(again)
  expect((dialog.getByLabelText(/^Model/) as HTMLInputElement).value).toBe('claude-opus-5-5')
  await user.click(dialog.getByRole('button', { name: 'Save' }))
  await waitFor(() => expect(calls.project.at(-1)).toEqual(['set', {}]))
  expect(calls.settings).toEqual([])
  await waitFor(() => expect(within(screen.getByRole('contentinfo')).getByText(/claude-opus-5-5/)).toBeTruthy())
})

test('a file is linked for one expert from that expert’s own card, and taken away there', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await user.click(screen.getByRole('tab', { name: 'Experts' }))
  const files = (name: string) => screen.getByRole('button', { name: `Background files of ${name}` })
  expect(files('Reviewer #2').textContent).toBe('Files')
  await user.click(files('Reviewer #2'))
  const reviewer = () => within(screen.getByRole('group', { name: 'Background files of Reviewer #2' }))
  reviewer().getByText('No file is linked for Reviewer #2 alone.')
  await user.click(reviewer().getByRole('button', { name: 'Link a file for Reviewer #2…' }))
  await waitFor(() => expect(calls.project.at(-1)).toEqual(['link', '/work/wording.md', ['reviewer-2']]))
  await waitFor(() => expect(files('Reviewer #2').textContent).toBe('Files (1)'))
  // the list of all files says the same
  expect(screen.getByRole('button', { name: 'Who gets wording.md' }).textContent).toBe('For Reviewer #2')

  // the same file for a second expert: both get it
  await user.click(files('The Clinician'))
  await user.click(screen.getByRole('button', { name: 'Link a file for The Clinician…' }))
  await waitFor(() => expect(calls.project.at(-1)).toEqual(['link', '/work/wording.md', ['reviewer-2', 'clinician']]))

  // taken from one it stays with the other; taken from the last it is unlinked, not handed to everyone
  await user.click(files('Reviewer #2'))
  await user.click(screen.getByRole('button', { name: 'Take wording.md away from Reviewer #2' }))
  await waitFor(() => expect(calls.project.at(-1)).toEqual(['link', '/work/wording.md', ['clinician']]))
  await user.click(files('The Clinician'))
  await user.click(screen.getByRole('button', { name: 'Take wording.md away from The Clinician' }))
  await waitFor(() => expect(calls.project.at(-1)).toEqual(['unlink', '/work/wording.md']))
  await screen.findByText('None linked.')
})

test('Escape closes a dialog', async () => {
  const user = userEvent.setup()
  const { api } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Export…' }))
  await user.click(await screen.findByLabelText('Format'))
  await user.keyboard('{Escape}')
  await waitFor(() => expect(screen.queryByLabelText('Format')).toBeNull())
})

test('an answer that is coming in can be stopped, and what was read stays', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn({ holdAnswer: true })
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  expect(screen.queryByRole('button', { name: 'Stop' })).toBeNull()
  await user.type(screen.getByLabelText('Ask the AI'), 'Any feedback?')
  await user.click(screen.getByRole('button', { name: 'Ask' }))
  await screen.findByText('Why subjects')
  await user.click(screen.getByRole('button', { name: 'Stop' }))
  expect(calls.picked).toEqual(['stopped'])
  // the part that came is the answer, marked as incomplete, and waits to be classified like any other
  await screen.findByText(/The answer is incomplete: The author stopped it\./)
  expect(screen.queryByRole('button', { name: 'Stop' })).toBeNull()
  expect(await screen.findByRole('button', { name: '✓ used' })).toBeTruthy()
})

test('a classification can be changed afterwards', async () => {
  const user = userEvent.setup()
  const { api, calls } = standIn()
  render(<App api={api} makeEditor={plainEditor} />)
  await screen.findByText('thesis.md')
  await user.type(screen.getByLabelText('Ask the AI'), 'Any feedback?')
  await user.click(screen.getByRole('button', { name: 'Ask' }))
  await user.click(await screen.findByRole('button', { name: '✗ not used' }))
  await screen.findByText(/decided against it/)

  // thought better of it: the choices come back, with the way out of leaving it
  await user.click(screen.getByRole('button', { name: 'Change “✗ not used”' }))
  await user.click(screen.getByRole('button', { name: 'Leave it as it is' }))
  expect(screen.queryByRole('button', { name: '✓ used' })).toBeNull()
  await user.click(screen.getByRole('button', { name: 'Change “✗ not used”' }))
  await user.type(screen.getByLabelText('Comment on this input'), 'took it after all')
  await user.click(screen.getByRole('button', { name: '✓ used' }))
  await waitFor(() => expect(calls.annotated).toEqual([['t1', 'rejected', ''], ['t1', 'adopted', 'took it after all']]))
  await screen.findByText(/as suggested/)
})
