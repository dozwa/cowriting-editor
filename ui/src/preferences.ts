// What the author set besides the AI: how the editor looks, where files go, how often live review
// asks, and the keyboard shortcuts. The app keeps them (settings.json) and holds them to their limits.
import type { Format } from './editor'
import { type LanguageId, m } from './i18n'

export type Preferences = {
  theme: 'system' | 'light' | 'dark'
  textFont: 'literata' | 'sans' | 'system-serif' | 'mono' | 'other'
  /** A typeface installed on this computer, by name, for 'other'. */
  textFontName: string
  textSize: number
  lineWidth: 'narrow' | 'standard' | 'wide'
  uiFont: 'hanken' | 'system'
  /** How large buttons, panels and dialogs are drawn; the text has its own size. */
  uiSize: 'small' | 'standard' | 'large' | 'larger'
  spellcheck: boolean
  /** Distraction-free: explanations and hints around the controls are hidden. */
  calm: boolean
  /** Tabs of the side panel the author took out of the row. */
  hiddenTabs: HideableTab[]
  /** Save by itself once the author has paused this long; 0 for never. */
  autosaveSeconds: number
  documentsFolder: string
  exportFolder: string
  reopenLast: boolean
  /** The editor's own words: as the computer is set, or one of the languages it has. */
  interfaceLanguage: 'system' | LanguageId
  /** What the author writes in: as found in the document, or said here. */
  writingLanguage: 'document' | 'en' | 'de'
  /** What the AI answers in: 'auto' follows the author and the manuscript. */
  aiLanguage: 'auto' | 'en' | 'de'
  disclosureLanguage: 'document' | 'en' | 'de'
  livePauseSeconds: number
  liveGapSeconds: number
  livePerHour: number
  /** Shortcuts the author changed: what is done → the keys; empty for none. */
  shortcuts: Record<string, string>
  /** Keep the editor's technical log (what was used, what failed; no text, no names). */
  diagnostics: boolean
  /** The author agreed: a sign of use goes to the editor's maker at each start. */
  usagePing: boolean
  /** The author agreed, separately: a session's figures go there when the editor closes. */
  usageFigures: boolean
}

export const STANDARD: Preferences = {
  theme: 'system',
  textFont: 'literata',
  textFontName: '',
  textSize: 18,
  lineWidth: 'standard',
  uiFont: 'hanken',
  uiSize: 'standard',
  spellcheck: true,
  calm: false,
  hiddenTabs: [],
  autosaveSeconds: 0,
  documentsFolder: '',
  exportFolder: '',
  reopenLast: false,
  interfaceLanguage: 'system',
  writingLanguage: 'document',
  aiLanguage: 'auto',
  disclosureLanguage: 'document',
  livePauseSeconds: 20,
  liveGapSeconds: 60,
  livePerHour: 20,
  shortcuts: {},
  diagnostics: true,
  usagePing: false,
  usageFigures: false,
}

export const LIMITS = {
  textSize: [13, 28],
  livePauseSeconds: [5, 600],
  liveGapSeconds: [15, 3600],
  livePerHour: [1, 120],
} as const

const TEXT_FONTS: Record<Exclude<Preferences['textFont'], 'other'>, string> = {
  literata: 'var(--serif)',
  sans: 'var(--ui)',
  'system-serif': "'Iowan Old Style', 'Palatino Linotype', Palatino, Georgia, serif",
  mono: 'var(--mono)',
}
/** The line's length at the standard text size; it grows and shrinks with the text. */
const LINE_WIDTHS = { narrow: 600, standard: 752, wide: 920 }
/** How much larger than standard everything around the text is drawn. */
const UI_SIZES = { small: 0.9, standard: 1, large: 1.15, larger: 1.3 }

/** Makes the window look as the preferences say. */
export function applyLook(preferences: Preferences, root: HTMLElement = document.documentElement) {
  if (preferences.calm) root.dataset.calm = ''
  else delete root.dataset.calm
  if (preferences.theme === 'system') delete root.dataset.theme
  else root.dataset.theme = preferences.theme
  const set = (name: string, value: string | null) => (value === null ? root.style.removeProperty(name) : root.style.setProperty(name, value))
  // the name was reduced to letters, digits, spaces and dashes by the app; quotes cannot be in it
  set('--text', preferences.textFont === 'other' ? `'${preferences.textFontName.replace(/['"\\;{}]/g, '')}', var(--serif)` : TEXT_FONTS[preferences.textFont])
  set('--text-size', `${preferences.textSize}px`)
  set('--measure', `${Math.round((LINE_WIDTHS[preferences.lineWidth] * preferences.textSize) / STANDARD.textSize)}px`)
  set('--ui-zoom', preferences.uiSize === 'standard' ? null : String(UI_SIZES[preferences.uiSize] ?? 1))
  set('--ui', preferences.uiFont === 'system' ? "system-ui, -apple-system, 'Segoe UI', sans-serif" : null)
}

/** What a shortcut can do. The markdown buttons carry their `Format`; the rest is the window's. */
export type Tab = 'ask' | 'experts' | 'live' | 'scope' | 'project' | 'sources' | 'log'
export type TabAction = 'tabAsk' | 'tabExperts' | 'tabLive' | 'tabPlan' | 'tabSources' | 'tabLog'
export type Action = 'save' | 'open' | 'new' | 'export' | 'settings' | 'ask' | 'buttons' | 'lineNumbers' | 'source' | 'calm' | 'find' | TabAction | Format

/** The tabs in the order of the row. */
export const TABS: Tab[] = ['ask', 'experts', 'live', 'scope', 'project', 'sources', 'log']
/** "Ask" cannot be hidden, so the panel is never empty. */
export type HideableTab = Exclude<Tab, 'ask'>
export const HIDEABLE_TABS = TABS.filter((tab): tab is HideableTab => tab !== 'ask')

/** The tab of the side panel each of its shortcuts shows. */
export const TAB_OF: Record<TabAction, Tab> = { tabAsk: 'ask', tabExperts: 'experts', tabLive: 'live', tabPlan: 'scope', tabSources: 'sources', tabLog: 'log' }

/** An action with its standard keys. Its name is read when it is shown, so it is in the language shown then. */
const action = (id: Action, keys: string) => ({ id, keys, get name() { return m.settings.actions[id] } })

export const ACTIONS: { group: string; actions: { id: Action; name: string; keys: string }[] }[] = [
  {
    get group() { return m.settings.groups.document },
    actions: [action('save', 'Mod-s'), action('open', 'Mod-o'), action('new', 'Mod-n'), action('find', 'Mod-f'), action('export', 'Mod-e'), action('ask', 'Mod-j'), action('settings', 'Mod-,')],
  },
  {
    get group() { return m.settings.groups.shown },
    actions: [action('source', 'Mod-/'), action('lineNumbers', 'Mod-Shift-l'), action('buttons', ''), action('calm', 'Mod-Shift-d')],
  },
  {
    get group() { return m.settings.groups.panel },
    actions: [action('tabAsk', 'Mod-1'), action('tabExperts', 'Mod-2'), action('tabLive', 'Mod-3'), action('tabPlan', 'Mod-4'), action('tabSources', 'Mod-5'), action('tabLog', 'Mod-6')],
  },
  {
    get group() { return m.settings.groups.markdown },
    actions: [
      action('bold', 'Mod-b'),
      action('italic', 'Mod-i'),
      action('title', 'Mod-Alt-1'),
      action('heading', 'Mod-Alt-2'),
      action('subheading', 'Mod-Alt-3'),
      action('bullets', 'Mod-Shift-8'),
      action('numbers', 'Mod-Shift-7'),
      action('quote', 'Mod-Shift-9'),
      action('link', 'Mod-k'),
      action('citation', 'Mod-Shift-k'),
      action('footnote', 'Mod-Alt-f'),
      action('image', ''),
      action('table', ''),
      action('comment', 'Mod-Shift-c'),
    ],
  },
]

const ALL = ACTIONS.flatMap(group => group.actions)

/** The keys each action has now: the standard, with what the author changed. */
export function shortcutsOf(changed: Record<string, string>): Record<Action, string> {
  return Object.fromEntries(ALL.map(action => [action.id, changed[action.id] ?? action.keys])) as Record<Action, string>
}

export const actionName = (id: Action) => ALL.find(action => action.id === id)?.name ?? id

const isMac = () => /Mac|iPhone|iPad/.test(navigator.platform)

/**
 * The keys an event stands for, as 'Mod-Shift-8': Mod is Command on a Mac and Ctrl elsewhere.
 * Digits are read from the key's place on the keyboard, and so are letters while Alt is held
 * (Alt changes what they type); everything else is what the key types. Null for a modifier alone.
 */
export function keysOf(event: KeyboardEvent): string | null {
  const code = event.code ?? ''
  const key = /^Digit\d$/.test(code) ? code.slice(5) : event.altKey && /^Key[A-Z]$/.test(code) ? code.slice(3).toLowerCase() : event.key === ' ' ? 'Space' : event.key.length === 1 ? event.key.toLowerCase() : event.key
  if (['Meta', 'Control', 'Alt', 'Shift', 'Dead', 'Unidentified', 'CapsLock', 'Process'].includes(key)) return null
  const mod = isMac() ? event.metaKey : event.ctrlKey
  return [mod ? 'Mod' : '', event.altKey ? 'Alt' : '', event.shiftKey ? 'Shift' : '', key].filter(part => part !== '').join('-')
}

/** Copy, paste, undo and the like stay what they are everywhere. */
const TAKEN = ['Mod-c', 'Mod-v', 'Mod-x', 'Mod-a', 'Mod-z', 'Mod-Shift-z', 'Mod-y', 'Mod-q', 'Mod-w', 'Mod-h', 'Mod-m']

/** Why these keys cannot be a shortcut; empty when they can. */
export function whyNot(keys: string, action: Action, now: Record<Action, string>): string {
  if (!keys.startsWith('Mod-') && !/^(Alt-|Shift-)*F\d{1,2}$/.test(keys)) return m.settings.needsModifier(isMac())
  if (TAKEN.includes(keys)) return m.settings.keysTaken(showKeys(keys))
  const other = ALL.find(a => a.id !== action && now[a.id] === keys)
  return other === undefined ? '' : m.settings.keysUsed(showKeys(keys), other.name)
}

/** Keys as the author reads them: ⌘⇧8 on a Mac, Ctrl+Shift+8 elsewhere, with Ctrl and Shift named as the language's keyboards print them. */
export function showKeys(keys: string): string {
  if (keys === '') return ''
  const parts = keys.split(/-(?!$)/)
  const key = parts.pop() ?? ''
  const shown = key.length === 1 ? key.toUpperCase() : key
  if (isMac()) return parts.map(part => ({ Mod: '⌘', Alt: '⌥', Shift: '⇧' })[part] ?? part).join('') + shown
  return [...parts.map(part => (part === 'Mod' ? m.settings.ctrlKey : part === 'Shift' ? m.settings.shiftKey : part)), shown].join('+')
}
