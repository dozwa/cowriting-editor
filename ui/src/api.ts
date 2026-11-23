// Everything the window asks of the app. The window itself has no file or network access:
// the document, the log and the API key are only touched on the Rust side.
import { Channel, invoke } from '@tauri-apps/api/core'
import { confirm, open, save } from '@tauri-apps/plugin-dialog'

export type Opened = { path: string; name: string; text: string; language: string }

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
}

export type GuardRow = { ts: string; tool: string; input: string; files: string[]; refused: boolean; turn: string | null }

export type LogView = {
  stats: { turns: number; today: number; unannotated: number; aiText: number; annotations: number; rejected: number; guardEvents: number }
  turns: TurnRow[]
  guard: GuardRow[]
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
  saveDocument: (text: string) => invoke<void>('save_document', { text }),
  ask: (input: { text: string; replyTo: string | null; documentText: string }, onEvent: (event: AskEvent) => void) => {
    const channel = new Channel<AskEvent>()
    channel.onmessage = onEvent
    return invoke<AskResult>('ask', { input, onEvent: channel })
  },
  logView: (documentText: string) => invoke<LogView>('log_view', { documentText }),
  thread: (id: string) => invoke<ThreadMessage[]>('thread', { id }),
  annotate: (reference: string, use: Use, note: string) => invoke<void>('annotate', { reference, use, note }),
  exportDisclosure: (options: ExportOptions) => invoke<Exported>('export_disclosure', { options }),
}

export type Api = typeof api
