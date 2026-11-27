// Everything the window asks of the app. The window itself has no file or network access:
// the document, the log and the API key are only touched on the Rust side.
import { Channel, invoke } from '@tauri-apps/api/core'
import { confirm, open, save } from '@tauri-apps/plugin-dialog'
import { m } from './i18n'
import type { Seen } from './diagnostics'
import type { Preferences } from './preferences'

/** An AI log in the document's folder whose document is gone. */
export type Orphan = { name: string; isSameText: boolean; questions: number }
/** A stored file that could not be read and was set aside, or that a newer version of the editor wrote and this one leaves alone. */
export type StoredNotice = { what: 'setAside'; file: string; keptAs: string } | { what: 'newer'; file: string }
export type Opened = { path: string; name: string; text: string; language: string; orphans: Orphan[]; notices: StoredNotice[] }

export type BibFile = { path: string; name: string; format: string; entries: number; isFromDocument: boolean; error?: string }
export type RefRecord = { doi: string; title: string; year: string }
export type CheckedRef = { key: string; derived: boolean; file: string; title: string; year: string; doi: string; status: 'ok' | 'mismatch' | 'not_found' | 'no_doi' | 'error'; found?: RefRecord; suggestion?: string; detail?: string }
export type SourcesView = { files: BibFile[]; entries: number; cited: number; missing: string[]; checked: { ts: string; rows: CheckedRef[] } | null; style: Style | null }
/** The reference style of the export: a CSL file. `isFromDocument` when the front matter names it (`csl:`). */
export type Style = { path: string; name: string; title: string; isFromDocument: boolean; error?: string }
export type RefHit = { citekey: string | null; title: string; authors: string; year: string; doi: string; file?: string }
export type SourceHits = { from: 'zotero' | 'bibliography'; hits: RefHit[] }

/** A picture as the open document would name it: `path` goes between the brackets, `isInside` when it lies in the document's folder or below. */
export type Picture = { path: string; name: string; isInside: boolean }
/** The pictures a move would leave behind, by the way the text names them: in the document's folder, and elsewhere. */
export type PicturesToMove = { inside: string[]; outside: string[] }

export type FileFormat = 'pdf' | 'docx'
export type ExportTools = { hasPandoc: boolean; pdfEngines: string[]; isOwn: boolean; hasZotero: boolean; /** The typefaces on this computer a PDF can be set in. */ fonts: string[] }
/** How the text of a PDF is set; whatever is empty, null or false is left to the standard. */
export type PdfLayout = { font: string; fontSize: number | null; marginCm: number | null; lineSpacing: number | null; indent: boolean; ragged: boolean; numbered: boolean }
export type PdfChoice = { engine: string | null; paper: 'a4' | 'letter'; layout: PdfLayout }
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
  /** What the expert is told about its perspective and voice, and whether the author reworded it in this project. */
  prompt: string
  isPromptEdited: boolean
}

/** A file of the author's that the AI is given as background knowledge. `personas` empty: for everyone. */
export type LinkedFile = { path: string; name: string; personas: string[]; isMissing: boolean; chars: number; isCut: boolean; isConverted: boolean }
/** What a project sets differently from the general settings; what is absent is as there. */
export type ProjectOverrides = Partial<Pick<Preferences, 'writingLanguage' | 'aiLanguage' | 'disclosureLanguage' | 'livePauseSeconds' | 'liveGapSeconds' | 'livePerHour'>> & { /** The document's own provider and model. */ ai?: Settings }
export type ProjectView = { files: LinkedFile[]; overrides: ProjectOverrides; folder: string }

export type PartyView = { experts: Expert[]; everyone: string; sharedPalette: Record<string, string> }

export type GuardRow = { ts: string; tool: string; input: string; files: string[]; refused: boolean; turn: string | null }

/** A hint an expert left on a line the author just wrote. */
export type LiveHint = { turn: string; persona: string; who: string; line: number; text: string; ts: string }

export type LiveView = { isOn: boolean; personas: string[]; hints: LiveHint[]; pauseMs: number; gapMs: number; perHour: number }

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

export type ScopeView = { text: string; template: string; templateDe: string; progress: ScopeProgress | null }
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

/** AI use outside the editor, as the author states it. `id` is empty for a new statement. */
export type ExternalUse = { id: string; tool: string; model: string; date: string; purpose: string; material: string; result: string; checked: string }

export type LogView = {
  /** the document's characters, counted as the words are: a run of spaces is one, marks and addresses are left out */
  chars: { withSpaces: number; withoutSpaces: number }
  /** what the author stated about other AI tools, the first statement first */
  external: ExternalUse[]
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

/** `maxTokens` is the longest answer the author allows; 0 for no limit of theirs. */
export type Settings = { backend: string; model: string; baseUrl: string; maxTokens: number }
/** Whether a model kept to the assist-only rules in the behaviour suite. */
export type ModelStatus = 'tested' | 'failed' | 'untested'
export type SettingsView = Settings & { /** The general AI settings; the fields above are what holds for the open document, which can have its own. */ general: Settings; isAiOwn: boolean; hasKey: boolean; isReady: boolean; modelStatus: ModelStatus; isFirstRun: boolean; openAtStart: string | null; preferences: Preferences; version: string; settingsFolder: string; system: string; feedbackByMail: boolean; contact: string; repository: string; asking: { isDue: boolean; withStar: boolean }; showTour: boolean; usage: { isAvailable: boolean; isAsked: boolean; installation: string | null }; notices: StoredNotice[] }
export type ModelChoice = { id: string; name: string; status: ModelStatus }

export type ExportOptions = { language: 'en' | 'de'; format: 'md' | 'tex'; withPrompts: boolean; withAnswers: boolean }
export type Exported = { paths: string[]; register: string }

/** Read when a file dialog opens, so the name is in the language shown now. */
const markdown = () => [{ name: m.dialogs.fileMarkdown, extensions: ['md', 'markdown'] }]

/** The pages the editor can show in the browser; the addresses here are for reading, the app has its own. */
export const GUIDES = {
  markdown: {
    get name(): string {
      return m.formatbar.guides.markdown
    },
    address: 'commonmark.org/help',
  },
  pandoc: {
    get name(): string {
      return m.formatbar.guides.pandoc
    },
    address: 'pandoc.org/MANUAL.html',
  },
  styles: {
    get name(): string {
      return m.formatbar.guides.styles
    },
    address: 'zotero.org/styles',
  },
}
export type Guide = keyof typeof GUIDES
/** A page the app opens in the browser: a guide, the project's page, or the page for a coffee. */
export type Page = Guide | 'repository' | 'coffee'

export const api = {
  getSettings: () => invoke<SettingsView>('get_settings'),
  setSettings: (settings: Settings, apiKey: string | null) => invoke<void>('set_settings', { settings, apiKey }),
  /** Puts a key for one provider into the system's key store, without changing the general settings. */
  setApiKey: (backend: string, apiKey: string) => invoke<void>('set_api_key', { backend, apiKey }),
  /** Asks the provider for its models with settings that are not saved yet. */
  testConnection: (settings: Settings, apiKey: string | null) => invoke<ModelChoice[]>('test_connection', { settings, apiKey }),
  /** Saves the preferences; returns them as the app keeps them, within their limits. */
  setPreferences: (preferences: Preferences) => invoke<Preferences>('set_preferences', { preferences }),
  /** Shows the folder the settings are in, in the system's file manager. */
  showSettingsFolder: () => invoke<void>('show_settings_folder'),
  /** Asks the author for a folder; null when they cancel. */
  pickFolder: async (): Promise<string | null> => {
    const path = await open({ multiple: false, directory: true })
    return typeof path === 'string' ? path : null
  },
  /** Asks the author for a file, starting in `folder` if one is given; null when they cancel. */
  pickDocument: async (folder = ''): Promise<string | null> => {
    const path = await open({ multiple: false, directory: false, filters: markdown(), ...(folder === '' ? {} : { defaultPath: folder }) })
    return typeof path === 'string' ? path : null
  },
  pickNewDocument: (folder = '') => save({ filters: markdown(), defaultPath: folder === '' ? 'untitled.md' : `${folder}${folder.includes('\\') ? '\\' : '/'}untitled.md` }),
  /** Asks before unsaved changes are left behind. */
  confirmLeave: () => confirm(m.dialogs.leaveQuestion, { title: m.dialogs.leaveTitle, kind: 'warning' }),
  openDocument: (path: string) => invoke<Opened>('open_document', { path }),
  newDocument: (path: string) => invoke<Opened>('new_document', { path }),
  /** The author says this document is the renamed one that log was kept for. */
  /** Opens a report as a new issue in the browser or a new mail, filled in; the author sends it. Where it goes is fixed in the app. */
  sendFeedback: (way: 'issue' | 'mail', title: string, text: string) => invoke<void>('send_feedback', { way, title, text }),
  /** What the window saw (commands and how they ended, views, its own errors), for the editor's technical log. */
  recordSeen: (seen: Seen[]) => invoke<void>('record_seen', { seen }),
  /** The technical log's last minutes, as the author can add them to a report. */
  recentDiagnostics: (minutes: number) => invoke<string>('recent_diagnostics', { minutes }),
  clearDiagnostics: () => invoke<void>('clear_diagnostics'),
  /** Asks the author where a file to attach to a report goes; null when they cancel. */
  pickReportFile: () => save({ filters: [{ name: 'Text', extensions: ['txt'] }], defaultPath: 'cowriting-report.txt' }),
  /** Writes that file: the log's last minutes and, only if the author agreed, the text given here. */
  writeReportFile: (target: string, minutes: number, documentText: string | null) => invoke<void>('write_report_file', { target, minutes, documentText }),
  /** The author's answer about usage figures: a sign of use at start, and a session's figures at closing, each a yes or a no. Returns the preferences as kept. */
  usageAnswered: (ping: boolean, figures: boolean) => invoke<Preferences>('usage_answered', { ping, figures }),
  /** The two messages as they would be sent now: the sign of use, and this session's figures so far. */
  usagePreview: () => invoke<[string, string]>('usage_preview'),
  privacyStatement: (language: string) => invoke<string>('privacy_statement', { language }),
  /** Shows a guide in the author's browser. Which pages these are is fixed in the app. */
  openGuide: (which: Page) => invoke<void>('open_guide', { which }),
  /** States AI use outside the editor, or corrects a statement (its `id` given). The log only grows. */
  stateExternal: (stated: ExternalUse) => invoke<void>('state_external', { stated }),
  /** Takes a statement back: it stays in the log and leaves the disclosure. */
  withdrawExternal: (id: string) => invoke<void>('withdraw_external', { id }),
  /** What belongs to the open project besides text and log: linked files, and its own settings. */
  project: () => invoke<ProjectView>('project'),
  /** Asks the author for a picture, starting in the document's folder; null when they cancel. */
  pickPicture: async (folder = ''): Promise<string | null> => {
    const path = await open({ multiple: false, directory: false, filters: [{ name: m.dialogs.filePicture, extensions: ['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'pdf'] }], ...(folder === '' ? {} : { defaultPath: folder }) })
    return typeof path === 'string' ? path : null
  },
  /** How the open document would name that picture. */
  picture: (path: string) => invoke<Picture>('picture', { path }),
  /** Copies the picture into `images/` beside the document; the picture itself stays as it is. */
  copyPicture: (path: string) => invoke<Picture>('copy_picture', { path }),
  /** Asks the author for a file to link; null when they cancel. */
  pickLinkedFile: async (): Promise<string | null> => {
    const path = await open({ multiple: false, directory: false })
    return typeof path === 'string' ? path : null
  },
  /** Links a file as background knowledge for these experts (none: for everyone), or changes whom a linked file is for. */
  linkFile: (path: string, personas: string[]) => invoke<ProjectView>('link_file', { path, personas }),
  unlinkFile: (path: string) => invoke<ProjectView>('unlink_file', { path }),
  setProject: (overrides: ProjectOverrides) => invoke<ProjectView>('set_project', { overrides }),
  /** Rewords what an expert is told, in this project; empty brings back the standard for an expert that comes with the editor. */
  setPrompt: (id: string, prompt: string) => invoke<void>('set_prompt', { id, prompt }),
  /** The author answered the editor's asking for support: 'coffee', 'feedback', 'star' or 'later'. */
  supportAnswered: (what: 'coffee' | 'feedback' | 'star' | 'later') => invoke<void>('support_answered', { what }),
  /** The author went through the tour or closed it: it does not start by itself again. */
  tourSeen: () => invoke<void>('tour_seen'),
  adoptLog: (name: string) => invoke<Opened>('adopt_log', { name }),
  /** Asks the author where the document should go; null when they cancel. */
  pickMove: (current: string) => save({ filters: markdown(), defaultPath: current }),
  /** Moves or renames the open document, with its log. */
  picturesToMove: (target: string, text: string) => invoke<PicturesToMove>('pictures_to_move', { target, text }),
  moveDocument: (target: string, text: string, pictures?: 'copy' | 'leave') => invoke<Opened>('move_document', { target, text, pictures: pictures ?? null }),
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
  exportDocument: (format: FileFormat, target: string, documentText: string, pdf: PdfChoice) => invoke<DocumentExport>('export_document', { format, target, documentText, pdf }),
  sources: (documentText: string) => invoke<SourcesView>('sources', { documentText }),
  /** Asks the author for a bibliography file; null when they cancel. */
  pickBibliography: async (): Promise<string | null> => {
    const path = await open({ multiple: false, directory: false, filters: [{ name: m.dialogs.fileBibliography, extensions: ['bib', 'bibtex', 'json', 'ris', 'nbib'] }] })
    return typeof path === 'string' ? path : null
  },
  addBibliography: (path: string, documentText: string) => invoke<void>('add_bibliography', { path, documentText }),
  removeBibliography: (path: string) => invoke<void>('remove_bibliography', { path }),
  /** Asks the author for a reference style file; null when they cancel. */
  pickStyle: async (): Promise<string | null> => {
    const path = await open({ multiple: false, directory: false, filters: [{ name: m.dialogs.fileStyle, extensions: ['csl'] }] })
    return typeof path === 'string' ? path : null
  },
  setStyle: (path: string, documentText: string) => invoke<void>('set_style', { path, documentText }),
  removeStyle: () => invoke<void>('remove_style'),
  searchSources: (query: string, documentText: string) => invoke<SourceHits>('search_sources', { query, documentText }),
  checkReferences: (documentText: string) => invoke<SourcesView['checked']>('check_references', { documentText }),
  /** Ends the answer under way; `ask` then comes back with what was read so far. */
  stopAnswer: () => invoke<void>('stop_answer'),
  ask: (input: { text: string; replyTo: string | null; persona: string | null; command?: string | null; documentText: string }, onEvent: (event: AskEvent) => void) => {
    const channel = new Channel<AskEvent>()
    channel.onmessage = onEvent
    return invoke<AskResult>('ask', { input, onEvent: channel })
  },
  logView: (documentText: string) => invoke<LogView>('log_view', { documentText }),
  thread: (id: string) => invoke<ThreadMessage[]>('thread', { id }),
  annotate: (reference: string, use: Use, note: string) => invoke<void>('annotate', { reference, use, note }),
  /** The hints marked as done, by answer: each by its place among the answer's list items and table rows, from 0. */
  hintMarks: () => invoke<Record<string, number[]>>('hint_marks'),
  setHintMarks: (turn: string, done: number[]) => invoke<void>('set_hint_marks', { turn, done }),
  /** Shows a file the editor has just written: marked in its folder, or opened with the system's program. */
  showWritten: (path: string, how: 'folder' | 'open') => invoke<void>('show_written', { path, how }),
  exportDisclosure: (options: ExportOptions) => invoke<Exported>('export_disclosure', { options }),
  party: () => invoke<PartyView>('party'),
  setBrief: (key: string, body: string) => invoke<void>('set_brief', { key, body }),
  setMember: (id: string, isMember: boolean) => invoke<void>('set_member', { id, isMember }),
  /** Makes an expert of the author's own for this document; returns its id. */
  addExpert: (name: string, role: string, looksAt: string) => invoke<string>('add_expert', { name, role, looksAt }),
}

export type Api = typeof api
