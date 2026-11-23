// Everything the window asks of the app. The window itself has no file or network access:
// the document, the log and the API key are only touched on the Rust side.
import { Channel, invoke } from '@tauri-apps/api/core'
import { confirm, open, save } from '@tauri-apps/plugin-dialog'

export type Opened = { path: string; name: string; text: string; language: string }

export type BibFile = { path: string; name: string; format: string; entries: number; isFromDocument: boolean; error?: string }
export type RefRecord = { doi: string; title: string; year: string }
export type CheckedRef = { key: string; derived: boolean; file: string; title: string; year: string; doi: string; status: 'ok' | 'mismatch' | 'not_found' | 'no_doi' | 'error'; found?: RefRecord; suggestion?: string; detail?: string }
export type SourcesView = { files: BibFile[]; entries: number; cited: number; missing: string[]; checked: { ts: string; rows: CheckedRef[] } | null }
export type RefHit = { citekey: string | null; title: string; authors: string; year: string; doi: string; file?: string }
export type SourceHits = { from: 'zotero' | 'bibliography'; hits: RefHit[] }

export type FileFormat = 'pdf' | 'docx'
export type ExportTools = { hasPandoc: boolean; pdfEngine: string | null; hasZotero: boolean }
export type DocumentExport = { path: string; cited: number; fromZotero: number; missing: string[]; warnings: string[] }

export type Use = 'adopted' | 'adapted' | 'rejected' | 'info'

export type TurnRow = {
  id: string
  ts: string
  category: string
  asked: string
  answer: string
  /** the answer's beginning as plain words */
  excerpt: string
  model: string
  provider: string
  toolsUsed: string[]
  /** '' while the author has not classified it */
  use: Use | ''
  note: string
  isOpen: boolean
  isAiText: boolean
  replyTo: string | null
  /** the expert who answered: id and name; '' for the assistant itself */
  persona: string
  who: string
}

export type Expert = {
  id: string
  name: string
  short: string
  role: string
  category: string
  isCustom: boolean
  palette: Record<string, string>
  sprite: string[]
  isMember: boolean
  /** what the author told this expert about the document */
  brief: string
}

export type PartyView = { experts: Expert[]; everyone: string; sharedPalette: Record<string, string> }

export type GuardRow = { ts: string; tool: string; input: string; files: string[]; refused: boolean; turn: string | null }

/** A hint an expert left on a line the author just wrote. */
export type LiveHint = { turn: string; persona: string; who: string; line: number; text: string; ts: string }

export type LiveView = { isOn: boolean; personas: string[]; hints: LiveHint[]; pauseMs: number }

/** What a call for a live review came to. */
export type LiveOutcome =
  | { kind: 'off' }
  | { kind: 'unchanged' }
  | { kind: 'wait'; ms: number }
  | { kind: 'paused' }
  | { kind: 'failed'; message: string }
  | { kind: 'reviewed'; hints: number; ts: string }

export type Count = { done: number; target?: number }
export type PointStatus = 'covered' | 'partial' | 'missing'

export type ScopeProgress = {
  goal: string
  words: Count
  figures: Count
  tables: Count
  references: Count
  deadline?: string
  daysLeft?: number
  wordsPerDay?: number
  points: { text: string; section?: string; isTicked: boolean; status?: PointStatus; where?: string; note?: string }[]
  checkedAt?: string
  sections: { name: string; target?: number; stage?: 'outline' | 'draft' | 'revised' | 'final'; words: number; markers: number; isFound: boolean }[]
  markers: number
  /** 0–1 */
  content: number
  finish: number | null
  overall: number
}

export type ScopeView = { text: string; template: string; progress: ScopeProgress | null }
export type CheckSummary = { covered: number; partial: number; missing: number }

/** A passage of the document that matches a logged answer. */
export type AiTextRow = {
  id: string
  ts: string
  source: 'paste' | 'typed'
  match: 'exact' | 'near'
  /** 'correction': from an answer about grammar and spelling */
  kind: 'text' | 'correction'
  words: number
  excerpt: string
  line: number
  /** the turn whose answer it matches, and what the author had asked */
  turn: string
  asked: string
  note: string
  stillInText: boolean
}

export type LogView = {
  stats: { turns: number; today: number; unannotated: number; aiText: number; annotations: number; rejected: number; guardEvents: number }
  turns: TurnRow[]
  guard: GuardRow[]
  /** AI text the editor saw enter the document, newest first. */
  aiText: AiTextRow[]
  words: number
}

export type ThreadMessage = { id: string; role: 'ai' | 'author'; who: string; persona: string; ts: string; text: string }

export type AskEvent =
  | { kind: 'started'; id: string }
  | { kind: 'text'; text: string }
  | { kind: 'tool'; name: string }
  | { kind: 'refused'; tool: string; input: string }

export type AskResult = { id: string; answer: string; isAiText: boolean; refused: number; cutOff: string | null; withoutTools: boolean }

export type Settings = { backend: string; model: string; baseUrl: string }
/** Whether a model kept to the assist-only rules in the behaviour suite. */
export type ModelStatus = 'tested' | 'failed' | 'untested'
export type SettingsView = Settings & { hasKey: boolean; isReady: boolean; modelStatus: ModelStatus; isFirstRun: boolean; openAtStart: string | null }
export type ModelChoice = { id: string; name: string; status: ModelStatus }

export type ExportOptions = { language: 'en' | 'de'; format: 'md' | 'tex'; withPrompts: boolean; withAnswers: boolean }
export type Exported = { paths: string[]; register: string }

const MARKDOWN = [{ name: 'Markdown', extensions: ['md', 'markdown'] }]

export const api = {
  getSettings: () => invoke<SettingsView>('get_settings'),
  setSettings: (settings: Settings, apiKey: string | null) => invoke<void>('set_settings', { settings, apiKey }),
  /** Asks the provider for its models with settings that are not saved yet. */
  testConnection: (settings: Settings, apiKey: string | null) => invoke<ModelChoice[]>('test_connection', { settings, apiKey }),
  /** Asks the author for a file; null when they cancel. */
  pickDocument: async (): Promise<string | null> => {
    const path = await open({ multiple: false, directory: false, filters: MARKDOWN })
    return typeof path === 'string' ? path : null
  },
  pickNewDocument: () => save({ filters: MARKDOWN, defaultPath: 'untitled.md' }),
  /** Asks before unsaved changes are left behind. */
  confirmLeave: () => confirm('This document has changes that are not saved. Leave it anyway?', { title: 'Unsaved changes', kind: 'warning' }),
  openDocument: (path: string) => invoke<Opened>('open_document', { path }),
  newDocument: (path: string) => invoke<Opened>('new_document', { path }),
  /** Saves; returns the AI text found in what changed since the last save, now logged. */
  saveDocument: (text: string) => invoke<AiTextRow[]>('save_document', { text }),
  /** Text that was just pasted or dropped: compared with the logged answers; returns what was found, now logged. */
  reportPaste: (pasted: string, line: number, documentText: string) => invoke<AiTextRow[]>('report_paste', { pasted, line, documentText }),
  commentAiText: (id: string, note: string) => invoke<void>('comment_ai_text', { id, note }),
  liveView: () => invoke<LiveView>('live'),
  /** Turns live review on or off and says who watches; only what changes from then on is reviewed. */
  setLive: (isOn: boolean, personas: string[], documentText: string) => invoke<void>('set_live', { isOn, personas, documentText }),
  /** Called when the author has paused: sends the changed paragraphs, if it is time. */
  liveReview: (documentText: string) => invoke<LiveOutcome>('live_review', { documentText }),
  scope: (documentText: string) => invoke<ScopeView>('scope', { documentText }),
  setScope: (text: string, documentText: string) => invoke<void>('set_scope', { text, documentText }),
  /** Sends the plan's points and the document to the model; logged. */
  checkPoints: (documentText: string) => invoke<CheckSummary>('check_points', { documentText }),
  /** What there is to export with, and where a PDF and a Word file would go. */
  exportSetup: () => invoke<[ExportTools, string, string]>('export_setup'),
  /** Asks the author where the export goes; null when they cancel. */
  pickExport: (format: FileFormat, suggested: string) => save({ filters: [{ name: format === 'pdf' ? 'PDF' : 'Word', extensions: [format] }], defaultPath: suggested }),
  exportDocument: (format: FileFormat, target: string, documentText: string) => invoke<DocumentExport>('export_document', { format, target, documentText }),
  sources: (documentText: string) => invoke<SourcesView>('sources', { documentText }),
  /** Asks the author for a bibliography file; null when they cancel. */
  pickBibliography: async (): Promise<string | null> => {
    const path = await open({ multiple: false, directory: false, filters: [{ name: 'Bibliography', extensions: ['bib', 'bibtex', 'json', 'ris', 'nbib'] }] })
    return typeof path === 'string' ? path : null
  },
  addBibliography: (path: string, documentText: string) => invoke<void>('add_bibliography', { path, documentText }),
  removeBibliography: (path: string) => invoke<void>('remove_bibliography', { path }),
  searchSources: (query: string, documentText: string) => invoke<SourceHits>('search_sources', { query, documentText }),
  checkReferences: (documentText: string) => invoke<SourcesView['checked']>('check_references', { documentText }),
  ask: (input: { text: string; replyTo: string | null; persona: string | null; documentText: string }, onEvent: (event: AskEvent) => void) => {
    const channel = new Channel<AskEvent>()
    channel.onmessage = onEvent
    return invoke<AskResult>('ask', { input, onEvent: channel })
  },
  logView: (documentText: string) => invoke<LogView>('log_view', { documentText }),
  thread: (id: string) => invoke<ThreadMessage[]>('thread', { id }),
  annotate: (reference: string, use: Use, note: string) => invoke<void>('annotate', { reference, use, note }),
  exportDisclosure: (options: ExportOptions) => invoke<Exported>('export_disclosure', { options }),
  party: () => invoke<PartyView>('party'),
  setBrief: (key: string, body: string) => invoke<void>('set_brief', { key, body }),
  setMember: (id: string, isMember: boolean) => invoke<void>('set_member', { id, isMember }),
  /** Makes an expert of the author's own for this document; returns its id. */
  addExpert: (name: string, role: string, looksAt: string) => invoke<string>('add_expert', { name, role, looksAt }),
}

export type Api = typeof api
