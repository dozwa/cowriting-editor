import { type ReactNode, useEffect, useRef, useState } from 'react'
import type { Api, ModelChoice, ProjectOverrides, ProjectView, Settings, SettingsView } from './api'
import { Dialog, FeedbackDialog, UsageChoice } from './dialogs'
import { LANGUAGES, type LanguageId, languageFor, m } from './i18n'
import { ACTIONS, type Action, LIMITS, type Preferences, STANDARD, keysOf, showKeys, shortcutsOf, whyNot } from './preferences'

type Kind = 'anthropic' | 'local' | 'other' | 'claudeCode'

/** What each choice sets. Its label and what it sends are in the catalogue, under the same name. */
const KINDS: Record<Kind, { backend: string; address: string; model: string }> = {
  anthropic: { backend: 'anthropic', address: '', model: 'claude-opus-5-5' },
  local: { backend: 'openai-compatible', address: 'http://localhost:11434/v1', model: '' },
  claudeCode: { backend: 'claude-code', address: '', model: 'sonnet' },
  other: { backend: 'openai-compatible', address: '', model: '' },
}

const isLocalAddress = (url: string) => /^https?:\/\/(localhost|127\.0\.0\.1|\[::1\])(:\d+)?(\/|$)/.test(url)

/** The shortest answer limit that still leaves a model room to think and answer. */
const FEWEST_TOKENS = 500

/** The AI's settings while they are edited, and the connection test. */
const kindOf = (settings: Settings): Kind => (settings.backend === 'claude-code' ? 'claudeCode' : settings.backend !== 'openai-compatible' ? 'anthropic' : isLocalAddress(settings.baseUrl) ? 'local' : 'other')
const AI_FIELDS = ['backend', 'model', 'baseUrl', 'maxTokens'] as const

function useAi(api: Api, settings: SettingsView) {
  const isScripted = settings.backend.startsWith('fake:')
  const [kind, setKind] = useState<Kind>(kindOf(settings))
  const [model, setModel] = useState(isScripted ? KINDS.anthropic.model : settings.model)
  const [baseUrl, setBaseUrl] = useState(settings.baseUrl)
  const [key, setKey] = useState('')
  const [limit, setLimit] = useState(settings.maxTokens > 0 ? String(settings.maxTokens) : '')
  const [choices, setChoices] = useState<ModelChoice[] | null>(null)
  const [isTesting, setTesting] = useState(false)
  const [error, setError] = useState('')
  const chosen = KINDS[kind]
  const tokens = Math.floor(Number(limit))
  const isLimitFine = limit.trim() === '' || (Number.isFinite(tokens) && tokens >= FEWEST_TOKENS)
  const next: Settings = { backend: chosen.backend, model: model.trim(), baseUrl: baseUrl.trim(), maxTokens: kind === 'claudeCode' || limit.trim() === '' || !isLimitFine ? 0 : tokens }
  const typedKey = key.trim() === '' ? null : key.trim()
  /** Whether what is set here differs from these settings, or a key was typed. */
  const differsFrom = (from: Settings) => settings.isFirstRun || typedKey !== null || AI_FIELDS.some(field => next[field] !== from[field])
  /** Shows other settings: the general ones, when a document gives up its own. */
  const reset = (to: Settings) => {
    setKind(kindOf(to))
    setModel(to.model)
    setBaseUrl(to.baseUrl)
    setLimit(to.maxTokens > 0 ? String(to.maxTokens) : '')
    setChoices(null)
    setError('')
  }

  const choose = (to: Kind) => {
    setKind(to)
    setBaseUrl(KINDS[to].address)
    setModel(KINDS[to].model)
    setChoices(null)
    setError('')
  }

  const test = async () => {
    setTesting(true)
    setError('')
    try {
      const found = await api.testConnection(next, typedKey)
      setChoices(found)
      // a model the provider does not offer is replaced by one it does
      if (found.length > 0 && !found.some(choice => choice.id === model.trim())) setModel(found[0]!.id)
    } catch (e) {
      setChoices(null)
      setError(String(e))
    } finally {
      setTesting(false)
    }
  }
  const canSave = model.trim() !== '' && isLimitFine
  return { isScripted, kind, choose, model, setModel, baseUrl, setBaseUrl, key, setKey, limit, setLimit, isLimitFine, choices, setChoices, isTesting, test, error, next, typedKey, differsFrom, reset, canSave, chosen, hasStoredKey: settings.hasKey && settings.backend === chosen.backend }
}

function AiPane({ ai }: { ai: ReturnType<typeof useAi> }) {
  const { kind, choices, model } = ai
  const status = choices?.find(choice => choice.id === model.trim())?.status
  return (
    <>
      {ai.isScripted && <p className="hint">{m.settings.scripted}</p>}
      <div className="choice" role="radiogroup" aria-label={m.settings.kindsLabel}>
        {(Object.keys(KINDS) as Kind[]).map(k => (
          <label key={k}>
            <input type="radio" name="kind" checked={kind === k} onChange={() => ai.choose(k)} /> {m.settings.kinds[k].label}
          </label>
        ))}
      </div>
      {kind !== 'local' && kind !== 'claudeCode' && (
        <label>
          {m.settings.apiKey}
          <input type="password" autoComplete="off" value={ai.key} onChange={e => ai.setKey(e.target.value)} placeholder={ai.hasStoredKey ? m.settings.keyStored : ''} />
          <small>{m.settings.keyKept}</small>
        </label>
      )}
      {kind === 'claudeCode' && <p className="muted">{m.settings.claudeCodeNeeds}</p>}
      {kind !== 'claudeCode' && (
        <label>
          {kind === 'anthropic' ? m.settings.addressOptional : m.settings.address}
          <input value={ai.baseUrl} onChange={e => (ai.setBaseUrl(e.target.value), ai.setChoices(null))} placeholder={kind === 'anthropic' ? 'https://api.anthropic.com' : kind === 'other' ? 'https://api.openai.com/v1' : ''} />
          {kind === 'local' && (
            <small>
              <button className="link" onClick={() => ai.setBaseUrl('http://localhost:11434/v1')}>Ollama</button> · <button className="link" onClick={() => ai.setBaseUrl('http://localhost:1234/v1')}>LM Studio</button>
            </small>
          )}
          {kind === 'anthropic' && <small>{m.settings.addressGateway}</small>}
        </label>
      )}
      <label>
        {m.settings.model}
        {choices !== null && choices.length > 0 ? (
          <select value={model} onChange={e => ai.setModel(e.target.value)}>
            {choices.map(choice => (
              <option key={choice.id} value={choice.id}>{choice.name === choice.id || choice.name === '' ? choice.id : `${choice.name} (${choice.id})`}{choice.status === 'untested' ? '' : choice.status === 'tested' ? ' ✓' : ' ✗'}</option>
            ))}
          </select>
        ) : (
          <input value={model} onChange={e => ai.setModel(e.target.value)} placeholder={kind === 'local' ? m.settings.modelPlaceholder : ''} />
        )}
        <small>{status !== undefined ? `${m.settings.modelStatus[status]} ` : ''}{m.settings.disclosureNamesModel}</small>
      </label>
      <div className="row">
        <button onClick={() => void ai.test()} disabled={ai.isTesting}>{ai.isTesting ? m.settings.testing : m.settings.test}</button>
        {choices !== null && <span className="ok" role="status">{kind === 'claudeCode' ? m.settings.claudeCodeInstalled : m.settings.connected(choices.length)}</span>}
      </div>
      {kind === 'claudeCode' ? (
        <p className="muted">{m.settings.claudeCodeLength}</p>
      ) : (
        <label>
          {m.settings.longestAnswer}
          <input inputMode="numeric" value={ai.limit} onChange={e => ai.setLimit(e.target.value)} placeholder={m.settings.noLimit} aria-invalid={!ai.isLimitFine} />
          <small>
            {ai.isLimitFine ? m.settings.limitHint : m.settings.limitTooLow(FEWEST_TOKENS)}
          </small>
        </label>
      )}
      <p className="muted">{m.settings.kinds[kind].sent}</p>
      {ai.error !== '' && <p className="error" role="alert">{ai.error}</p>}
    </>
  )
}

/** A number the author types freely; it is held to its limits when they leave the field. */
function NumberField({ label, value, limits, unit, onChange, children }: { label: string; value: number; limits: readonly [number, number]; unit: string; onChange(value: number): void; children?: ReactNode }) {
  const [typed, setTyped] = useState(String(value))
  useEffect(() => setTyped(String(value)), [value])
  const commit = () => {
    const number = Math.round(Number(typed))
    const held = Number.isFinite(number) && typed.trim() !== '' ? Math.min(Math.max(number, limits[0]), limits[1]) : value
    setTyped(String(held))
    onChange(held)
  }
  return (
    <label>
      {label}
      <span className="with-unit">
        <input inputMode="numeric" value={typed} onChange={e => setTyped(e.target.value)} onBlur={commit} onKeyDown={e => e.key === 'Enter' && commit()} />
        <span className="muted">{unit}</span>
      </span>
      <small>{children} {m.settings.range(limits[0], limits[1])}</small>
    </label>
  )
}

function FolderField({ label, folder, unset, onPick, onChange }: { label: string; folder: string; unset: string; onPick(): Promise<string | null>; onChange(folder: string): void }) {
  return (
    <div className="field">
      <strong>{label}</strong>
      <span className={folder === '' ? 'folder muted' : 'folder'}>{folder === '' ? unset : folder}</span>
      <span className="row">
        <button onClick={async () => { const picked = await onPick(); if (picked !== null) onChange(picked) }} aria-label={m.settings.chooseFor(label)}>{m.settings.choose}</button>
        {folder !== '' && <button className="link" onClick={() => onChange('')} aria-label={m.settings.clearFor(label)}>{m.settings.clear}</button>}
      </span>
    </div>
  )
}

/** The pages in their order. Their names and what they are about are in the catalogue, under the same name. */
const SECTIONS = ['ai', 'live', 'look', 'files', 'language', 'shortcuts', 'about'] as const
type Section = (typeof SECTIONS)[number]

/** The pauses offered for saving by itself, in seconds; 0 for never. */
const AUTOSAVE = [0, 5, 30, 60, 300]

type Props = {
  api: Api
  settings: SettingsView
  /** The open document's project, with what it sets for itself; null when no document is open. */
  project: ProjectView | null
  documentName: string
  /** The open document's text, for a bug report whose author wants it in the file they attach. */
  getText?: () => string
  /** How the window would look with what is being set; null when the dialog closes without saving. */
  onPreview(preferences: Preferences | null): void
  onClose(saved: SettingsView | null): void
  /** The author wants the tour of the editor: the settings close without saving, and it starts. */
  onTour(): void
}

const LANGUAGES_OF_A_PROJECT = ['writingLanguage', 'aiLanguage', 'disclosureLanguage'] as const
const PACE_OF_A_PROJECT = ['livePauseSeconds', 'liveGapSeconds', 'livePerHour'] as const

export function SettingsDialog({ api, settings, project, documentName, getText, onPreview, onClose, onTour }: Props) {
  const [isLogDeleted, setLogDeleted] = useState(false)
  const ai = useAi(api, settings)
  const [section, setSection] = useState<Section>('ai')
  // what the open project sets for itself is shown as it holds there; the ticks say which pages are the project's own
  const overrides = project?.overrides ?? {}
  const start: Preferences = { ...settings.preferences, ...Object.fromEntries([...LANGUAGES_OF_A_PROJECT, ...PACE_OF_A_PROJECT].filter(key => overrides[key] != null).map(key => [key, overrides[key]])) }
  const startOwn = { languages: LANGUAGES_OF_A_PROJECT.some(key => overrides[key] != null), pace: PACE_OF_A_PROJECT.some(key => overrides[key] != null), ai: project !== null && settings.isAiOwn }
  const [own, setOwn] = useState(startOwn)
  const [draft, setDraft] = useState<Preferences>(start)
  const [error, setError] = useState('')
  /** The shortcut that waits for its keys, and what was wrong with the last keys pressed. */
  const [waiting, setWaiting] = useState<Action | null>(null)
  const [complaint, setComplaint] = useState('')
  const [feedback, setFeedback] = useState<'bug' | 'idea' | null>(null)
  const set = <K extends keyof Preferences>(key: K, value: Preferences[K]) => setDraft(now => ({ ...now, [key]: value }))

  useEffect(() => onPreview(draft), [draft, onPreview])
  const close = (saved: SettingsView | null) => {
    onPreview(null)
    onClose(saved)
  }

  const isChanged = JSON.stringify(draft) !== JSON.stringify(start) || own.languages !== startOwn.languages || own.pace !== startOwn.pace
  // the AI page is the document's own or the general one; it is compared with whichever it shows
  const isAiOwn = project !== null && own.ai
  const isAiChanged = own.ai !== startOwn.ai || ai.differsFrom(isAiOwn ? settings : settings.general)
  const save = async () => {
    try {
      if (!isAiOwn && ai.differsFrom(settings.general)) await api.setSettings(ai.next, ai.typedKey)
      // a document's own AI is kept with the document; a key typed for it still goes to the key store
      if (isAiOwn && ai.typedKey !== null && ai.next.backend !== 'claude-code') await api.setApiKey(ai.next.backend, ai.typedKey)
      if (isChanged || (project !== null && isAiChanged)) {
        // a page that is the project's own is saved with the project, and the general settings keep what they had there
        const general = { ...draft }
        const mine: ProjectOverrides = isAiOwn ? { ai: ai.next } : {}
        const keep = <K extends keyof ProjectOverrides & keyof Preferences>(key: K) => {
          mine[key] = draft[key]
          general[key] = settings.preferences[key]
        }
        if (project !== null && own.languages) LANGUAGES_OF_A_PROJECT.forEach(keep)
        if (project !== null && own.pace) PACE_OF_A_PROJECT.forEach(keep)
        if (project !== null) await api.setProject(mine)
        if (isChanged) await api.setPreferences(general)
      }
      close(await api.getSettings())
    } catch (e) {
      setError(String(e))
    }
  }

  const keys = shortcutsOf(draft.shortcuts)
  const setKeys = (action: Action, to: string | null) => {
    const changed = { ...draft.shortcuts }
    if (to === null) delete changed[action]
    else changed[action] = to
    set('shortcuts', changed)
  }
  // the keys for the shortcut that waits are caught wherever the focus is: a clicked button does not get it in every web view
  const caught = useRef<(event: KeyboardEvent) => void>(() => {})
  caught.current = event => {
    if (waiting === null) return
    event.preventDefault()
    event.stopPropagation()
    if (event.key === 'Escape') return setWaiting(null)
    const pressed = keysOf(event)
    if (pressed === null) return
    const why = whyNot(pressed, waiting, keys)
    setComplaint(why)
    if (why !== '') return
    setKeys(waiting, pressed === ACTIONS.flatMap(group => group.actions).find(action => action.id === waiting)?.keys ? null : pressed)
    setWaiting(null)
  }
  useEffect(() => {
    if (waiting === null) return
    const onKey = (event: KeyboardEvent) => caught.current(event)
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  }, [waiting])

  if (settings.isFirstRun) {
    return (
      <Dialog title={m.settings.firstRunTitle} onClose={() => close(null)}>
        <p>{m.settings.firstRunIntro}</p>
        <AiPane ai={ai} />
        {error !== '' && <p className="error" role="alert">{error}</p>}
        <div className="row end">
          <button onClick={() => close(null)}>{m.settings.later}</button>
          <button className="primary" onClick={() => void save()} disabled={!ai.canSave}>{m.settings.save}</button>
        </div>
      </Dialog>
    )
  }

  const perHour = Math.min(draft.livePerHour, Math.floor(3600 / draft.liveGapSeconds))
  const now = m.settings.sections[section]

  return (
    <>
    {/* Escape while a shortcut waits for its keys ends the waiting, not the dialog */}
    <Dialog title={m.settings.title} onClose={() => close(null)} mayClose={() => (waiting === null ? true : (setWaiting(null), false))}>
      <div className="settings">
        <nav role="tablist" aria-orientation="vertical" aria-label={m.settings.title}>
          {SECTIONS.map(id => (
            <button key={id} role="tab" aria-selected={section === id} onClick={() => (setSection(id), setWaiting(null), setComplaint(''))}>{m.settings.sections[id].name}</button>
          ))}
        </nav>
        <div className="pane" role="tabpanel" aria-label={now.name}>
          {now.about !== '' && <p className="about">{now.about}</p>}
          {section === 'ai' && project !== null && !ai.isScripted && (
            <>
              <label className="check own">
                <input type="checkbox" checked={own.ai} onChange={e => (setOwn({ ...own, ai: e.target.checked }), !e.target.checked && ai.reset(settings.general))} /> {m.settings.onlyThisDocument(documentName)}
              </label>
              {own.ai && <p className="hint">{m.settings.ownAiHint}</p>}
            </>
          )}
          {section === 'ai' && <AiPane ai={ai} />}

          {section === 'live' && (
            <>
              {project !== null && (
                <label className="check own">
                  <input type="checkbox" checked={own.pace} onChange={e => setOwn({ ...own, pace: e.target.checked })} /> {m.settings.onlyThisDocument(documentName)}
                  <small>{m.settings.ownPaceHint}</small>
                </label>
              )}
              <NumberField label={m.settings.livePause} value={draft.livePauseSeconds} limits={LIMITS.livePauseSeconds} unit={m.settings.seconds} onChange={v => set('livePauseSeconds', v)}>
                {m.settings.livePauseHint}
              </NumberField>
              <NumberField label={m.settings.liveGap} value={draft.liveGapSeconds} limits={LIMITS.liveGapSeconds} unit={m.settings.seconds} onChange={v => set('liveGapSeconds', v)}>
                {m.settings.liveGapHint}
              </NumberField>
              <NumberField label={m.settings.livePerHour} value={draft.livePerHour} limits={LIMITS.livePerHour} unit={m.settings.atMost} onChange={v => set('livePerHour', v)}>
                {m.settings.livePerHourHint}
              </NumberField>
              <p className="muted" role="status">{m.settings.liveSum(perHour)}</p>
            </>
          )}

          {section === 'look' && (
            <>
              <div className="choice across" role="radiogroup" aria-label={m.settings.themeLabel}>
                {(['system', 'light', 'dark'] as const).map(value => (
                  <label key={value}>
                    <input type="radio" name="theme" checked={draft.theme === value} onChange={() => set('theme', value)} /> {m.settings.themes[value]}
                  </label>
                ))}
              </div>
              <label>
                {m.settings.textFont}
                <select value={draft.textFont} onChange={e => set('textFont', e.target.value as Preferences['textFont'])}>
                  <option value="literata">{m.settings.textFonts.literata}</option>
                  <option value="sans">{m.settings.textFonts.sans}</option>
                  <option value="system-serif">{m.settings.textFonts.systemSerif}</option>
                  <option value="mono">{m.settings.textFonts.mono}</option>
                  <option value="other">{m.settings.textFonts.other}</option>
                </select>
              </label>
              {draft.textFont === 'other' && (
                <label>
                  {m.settings.fontName}
                  <input value={draft.textFontName} onChange={e => set('textFontName', e.target.value)} placeholder="Georgia" />
                  <small>{m.settings.fontNameHint}</small>
                </label>
              )}
              <div className="row">
                <NumberField label={m.settings.textSize} value={draft.textSize} limits={LIMITS.textSize} unit={m.settings.pixels} onChange={v => set('textSize', v)}>{m.settings.textSizeStandard(STANDARD.textSize)}</NumberField>
                <label>
                  {m.settings.lineWidth}
                  <select value={draft.lineWidth} onChange={e => set('lineWidth', e.target.value as Preferences['lineWidth'])}>
                    <option value="narrow">{m.settings.lineWidths.narrow}</option>
                    <option value="standard">{m.settings.lineWidths.standard}</option>
                    <option value="wide">{m.settings.lineWidths.wide}</option>
                  </select>
                  <small>{m.settings.lineWidthHint}</small>
                </label>
              </div>
              <label>
                {m.settings.uiSize}
                <select value={draft.uiSize} onChange={e => set('uiSize', e.target.value as Preferences['uiSize'])}>
                  {(['small', 'standard', 'large', 'larger'] as const).map(size => (
                    <option key={size} value={size}>{m.settings.uiSizes[size]}</option>
                  ))}
                </select>
                <small>{m.settings.uiSizeHint}</small>
              </label>
              <label>
                {m.settings.uiFont}
                <select value={draft.uiFont} onChange={e => set('uiFont', e.target.value as Preferences['uiFont'])}>
                  <option value="hanken">{m.settings.uiFonts.hanken}</option>
                  <option value="system">{m.settings.uiFonts.system}</option>
                </select>
              </label>
              <label className="check">
                <input type="checkbox" checked={draft.calm} onChange={e => set('calm', e.target.checked)} /> {m.settings.calm}
              </label>
              <p className="muted">{m.settings.calmHint}</p>
              <label className="check">
                <input type="checkbox" checked={draft.spellcheck} onChange={e => set('spellcheck', e.target.checked)} /> {m.settings.spellcheck}
              </label>
              <p className="muted">{m.settings.lookNote}</p>
            </>
          )}

          {section === 'files' && (
            <>
              <FolderField label={m.settings.documentsFolder} folder={draft.documentsFolder} unset={m.settings.documentsFolderUnset} onPick={api.pickFolder} onChange={v => set('documentsFolder', v)} />
              <FolderField label={m.settings.exportFolder} folder={draft.exportFolder} unset={m.settings.exportFolderUnset} onPick={api.pickFolder} onChange={v => set('exportFolder', v)} />
              <p className="muted">{m.settings.foldersNote}</p>
              <label>
                {m.settings.saving}
                <select value={draft.autosaveSeconds} onChange={e => set('autosaveSeconds', Number(e.target.value))}>
                  {AUTOSAVE.map(seconds => (
                    <option key={seconds} value={seconds}>{m.settings.autosave(seconds)}</option>
                  ))}
                  {!AUTOSAVE.includes(draft.autosaveSeconds) && <option value={draft.autosaveSeconds}>{m.settings.autosave(draft.autosaveSeconds)}</option>}
                </select>
                <small>{m.settings.autosaveHint}</small>
              </label>
              <label className="check">
                <input type="checkbox" checked={draft.reopenLast} onChange={e => set('reopenLast', e.target.checked)} /> {m.settings.reopenLast}
              </label>
            </>
          )}

          {section === 'language' && (
            <>
              <label>
                {m.settings.interfaceLanguage}
                <select value={draft.interfaceLanguage} onChange={e => set('interfaceLanguage', e.target.value as Preferences['interfaceLanguage'])}>
                  <option value="system">{m.settings.asSystem(LANGUAGES[languageFor('system')].name)}</option>
                  {(Object.keys(LANGUAGES) as LanguageId[]).map(id => (
                    <option key={id} value={id}>{LANGUAGES[id].name}</option>
                  ))}
                </select>
                <small>{m.settings.interfaceLanguageHint}</small>
              </label>
              {project !== null && (
                <label className="check own">
                  <input type="checkbox" checked={own.languages} onChange={e => setOwn({ ...own, languages: e.target.checked })} /> {m.settings.onlyThisDocument(documentName)}
                  <small>{m.settings.ownLanguagesHint}</small>
                </label>
              )}
              <label>
                {m.settings.writingLanguage}
                <select value={draft.writingLanguage} onChange={e => set('writingLanguage', e.target.value as Preferences['writingLanguage'])}>
                  <option value="document">{m.settings.asDocument}</option>
                  <option value="en">{m.settings.english}</option>
                  <option value="de">{m.settings.german}</option>
                </select>
                <small>{m.settings.writingLanguageHint}</small>
              </label>
              <label>
                {m.settings.aiLanguage}
                <select value={draft.aiLanguage} onChange={e => set('aiLanguage', e.target.value as Preferences['aiLanguage'])}>
                  <option value="auto">{m.settings.aiLanguageAuto}</option>
                  <option value="en">{m.settings.alwaysEnglish}</option>
                  <option value="de">{m.settings.alwaysGerman}</option>
                </select>
                <small>{m.settings.aiLanguageHint}</small>
              </label>
              <label>
                {m.settings.disclosureLanguage}
                <select value={draft.disclosureLanguage} onChange={e => set('disclosureLanguage', e.target.value as Preferences['disclosureLanguage'])}>
                  <option value="document">{m.settings.asWriting}</option>
                  <option value="en">{m.settings.english}</option>
                  <option value="de">{m.settings.german}</option>
                </select>
                <small>{m.settings.disclosureLanguageHint}</small>
              </label>
            </>
          )}

          {section === 'shortcuts' && (
            <>
              {complaint !== '' && <p className="error" role="alert">{complaint}</p>}
              {ACTIONS.map(group => (
                <table key={group.group} className="keys">
                  <caption>{group.group}</caption>
                  <tbody>
                    {group.actions.map(action => {
                      const isWaiting = waiting === action.id
                      return (
                        <tr key={action.id}>
                          <th scope="row">{action.name}</th>
                          <td>{isWaiting ? <em>{m.settings.pressKeys}</em> : keys[action.id] === '' ? <span className="muted">{m.settings.noKeys}</span> : <kbd>{showKeys(keys[action.id])}</kbd>}</td>
                          <td>
                            <button
                              className="link"
                              aria-label={isWaiting ? m.settings.stopFor(action.name) : m.settings.changeFor(action.name)}
                              onClick={() => (setWaiting(isWaiting ? null : action.id), setComplaint(''))}
                            >
                              {isWaiting ? m.settings.stop : m.settings.change}
                            </button>
                            {!isWaiting && keys[action.id] !== '' && <button className="link" aria-label={m.settings.removeFor(action.name)} onClick={() => setKeys(action.id, '')}>{m.settings.remove}</button>}
                            {!isWaiting && keys[action.id] !== action.keys && action.keys !== '' && (
                              <button className="link" aria-label={m.settings.standardFor(action.name)} onClick={() => { const why = whyNot(action.keys, action.id, keys); setComplaint(why); if (why === '') setKeys(action.id, null) }}>{m.settings.standard(showKeys(action.keys))}</button>
                            )}
                          </td>
                        </tr>
                      )
                    })}
                  </tbody>
                </table>
              ))}
              <p className="muted">{m.settings.sharedKeysNote}</p>
            </>
          )}

          {section === 'about' && (
            <>
              <p className="version">CoWriting Editor <span className="muted">{m.settings.version(settings.version)}</span></p>
              <p>{m.settings.whatItIs}</p>
              <p className="hint">{m.settings.alphaNote}</p>
              <dl className="where">
                {settings.contact !== '' && (
                  <>
                    <dt>{m.settings.contact}</dt>
                    <dd><span className="folder">{settings.contact}</span></dd>
                  </>
                )}
                <dt>{m.settings.repository}</dt>
                <dd>
                  <button className="link folder" onClick={() => void api.openGuide('repository').catch(e => setError(String(e)))}>{settings.repository.replace('https://', '')}</button>
                </dd>
                <dt>{m.settings.whereSettings}</dt>
                <dd>
                  <span className="folder">{settings.settingsFolder}</span> <button className="link" onClick={() => void api.showSettingsFolder().catch(e => setError(String(e)))}>{m.settings.showFolder}</button>
                </dd>
                <dt>{m.settings.whereKey}</dt>
                <dd>{m.settings.whereKeyIs}</dd>
                <dt>{m.settings.whereLog}</dt>
                <dd>{m.settings.whereLogIs}</dd>
                <dt>{m.settings.whereDiagnostics}</dt>
                <dd>{m.settings.whereDiagnosticsIs}</dd>
                <dt>{m.settings.sent}</dt>
                <dd>{m.settings.sentWhat}{settings.usage.isAvailable && m.settings.sentUsage}</dd>
              </dl>
              <h3>{m.settings.tourHeading}</h3>
              <p className="muted">{m.settings.tourAbout}</p>
              <div className="row">
                <button onClick={() => (onPreview(null), onTour())}>{m.settings.tourAgain}</button>
              </div>
              {settings.usage.isAvailable && (
                <>
                  <h3>{m.settings.usageHeading}</h3>
                  <p className="muted">{m.settings.usageAbout}</p>
                  <UsageChoice api={api} ping={draft.usagePing} figures={draft.usageFigures} onChange={(ping, figures) => (set('usagePing', ping), set('usageFigures', figures))} />
                  {settings.usage.installation !== null && <p className="muted">{m.settings.usageInstallation(settings.usage.installation)}</p>}
                </>
              )}
              <h3>{m.settings.diagnosticsHeading}</h3>
              <p className="muted">{m.settings.diagnosticsAbout}</p>
              <label className="check">
                <input type="checkbox" checked={draft.diagnostics} onChange={e => set('diagnostics', e.target.checked)} /> {m.settings.diagnostics}
              </label>
              <div className="row">
                <button onClick={() => void api.clearDiagnostics().then(() => setLogDeleted(true), e => setError(String(e)))}>{m.settings.diagnosticsDelete}</button>
                {isLogDeleted && <span className="ok" role="status">{m.settings.diagnosticsDeleted}</span>}
              </div>
              <h3>{m.settings.feedbackHeading}</h3>
              <p className="muted">{m.settings.feedbackAbout}</p>
              <div className="row">
                <button onClick={() => setFeedback('bug')}>{m.settings.reportBug}</button>
                <button onClick={() => setFeedback('idea')}>{m.settings.feedback}</button>
                <button onClick={() => (void api.supportAnswered('coffee').catch(() => {}), void api.openGuide('coffee').catch(e => setError(String(e))))}>{m.settings.coffee}</button>
              </div>
              <h3>{m.settings.licences}</h3>
              <p className="muted">{m.settings.licencesText}</p>
            </>
          )}
        </div>
      </div>
      {error !== '' && <p className="error" role="alert">{error}</p>}
      <div className="row end">
        <button className="link" onClick={() => (setDraft({ ...STANDARD }), setOwn({ ...own, languages: false, pace: false }), setComplaint(''))} disabled={JSON.stringify(draft) === JSON.stringify(STANDARD) && !own.languages && !own.pace} title={m.settings.backToStandardTitle}>{m.settings.backToStandard}</button>
        <span className="spacer" />
        <button onClick={() => close(null)}>{m.settings.cancel}</button>
        <button className="primary" onClick={() => void save()} disabled={!ai.canSave || (!isAiChanged && !isChanged)}>{m.settings.save}</button>
      </div>
    </Dialog>
    {/* beside the settings, not inside them: a dialog in a dialog would be drawn at twice the chosen size */}
    {feedback !== null && <FeedbackDialog api={api} settings={settings} kind={feedback} getText={getText} onClose={() => setFeedback(null)} />}
    </>
  )
}
