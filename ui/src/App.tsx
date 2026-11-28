import { type CSSProperties, useCallback, useEffect, useRef, useState } from 'react'
import type { ProjectView as Project, ProjectOverview, Counted, Picture, PicturesToMove, AiTextRow, Api, AskEvent, LiveView, LogView, ScopeView, SourcesView, Opened, PartyView, SettingsView, ThreadMessage, Use } from './api'
import { ExpertsPanel } from './experts'
import { LivePanel, ScopePanel, span } from './plan'
import { SourcesPanel } from './sources'
import { ProjectPanel } from './project'
import { seeView } from './diagnostics'
import { DisclosureDialog, ExportDialog, FeedbackDialog, MovePicturesDialog, PictureDialog, SupportDialog, UsageDialog } from './dialogs'
import { Tour, TOUR } from './tour'
import { createEditor, type Editor, type Format } from './editor'
import { FormatBar, MarkdownHelp } from './formatbar'
import { AskPanel, LogPanel } from './panels'
import { languageFor, m, setLanguage } from './i18n'
import { type Action, type Preferences, STANDARD, TABS, TAB_OF, type Tab, type TabAction, applyLook, keysOf, shortcutsOf } from './preferences'
import { SettingsDialog } from './settings'
import { MODEL_NOTICE, when } from './text'

/** The side panel is never narrower than this, and the text never narrower than `TEXT_MIN`. */
const PANEL_MIN = 300
const TEXT_MIN = 320
const PANEL_STANDARD = 400

/** How the author likes the text shown: the markdown buttons, line numbers, styled or as source. */
type Look = { buttons: boolean; lineNumbers: boolean; source: boolean }

function rememberedLook(): Look {
  const look: Look = { buttons: true, lineNumbers: false, source: false }
  try {
    const kept = JSON.parse(localStorage.getItem('cowriting.look') ?? 'null') as Partial<Look> | null
    for (const key of ['buttons', 'lineNumbers', 'source'] as const) if (typeof kept?.[key] === 'boolean') look[key] = kept[key]
  } catch {
    // the standard look, then
  }
  return look
}

/** The panel's width as the author left it, kept in this window's storage; null for the standard width. */
function rememberedPanel(): number | null {
  try {
    const kept = Number(localStorage.getItem('cowriting.panel'))
    return Number.isFinite(kept) && kept >= PANEL_MIN ? kept : null
  } catch {
    return null
  }
}

type Props = {
  api: Api
  /** Tests give a plain text field instead of the real editor. */
  makeEditor?: typeof createEditor
}

export type Live = {
  /** What the author asked, shown until the turn is in the log. */
  asked: string
  /** Who is answering: the assistant or an expert. */
  who: string
  text: string
  /** What the model is doing besides writing: "reading the document", a refused write. */
  steps: string[]
}

const EMPTY_LOG: LogView = { stats: { turns: 0, today: 0, unannotated: 0, aiText: 0, annotations: 0, rejected: 0, guardEvents: 0 }, turns: [], guard: [], aiText: [], external: [], words: 0, chars: { withSpaces: 0, withoutSpaces: 0 } }

/** What the author is told when text from an AI answer entered the document. */
function foundNotice(rows: AiTextRow[]): string {
  const words = rows.reduce((sum, row) => sum + row.words, 0)
  return m.app.found(words, rows.every(row => row.source === 'paste'), rows.every(row => row.kind === 'correction'))
}

/** The kinds of picture the export can use, by what a browser calls them, with their file ending. */
const PICTURE_KINDS: Record<string, string> = { 'image/png': 'png', 'image/jpeg': 'jpg', 'image/gif': 'gif', 'image/svg+xml': 'svg', 'image/webp': 'webp', 'application/pdf': 'pdf' }

export function App({ api, makeEditor = createEditor }: Props) {
  const host = useRef<HTMLDivElement>(null)
  const editor = useRef<Editor | null>(null)
  const [doc, setDoc] = useState<Opened | null>(null)
  const [isDirty, setDirty] = useState(false)
  const [settings, setSettings] = useState<SettingsView | null>(null)
  const [tab, setTab] = useState<Tab>('ask')
  const [log, setLog] = useState<LogView>(EMPTY_LOG)
  const [thread, setThread] = useState<ThreadMessage[]>([])
  const [live, setLive] = useState<Live | null>(null)
  const [error, setError] = useState('')
  const [dialog, setDialog] = useState<'settings' | 'disclosure' | 'export' | 'markdown' | 'feedback' | 'bug' | null>(null)
  /** The step of the tour that is shown; null when it is not running. */
  const [tourStep, setTourStep] = useState<number | null>(null)
  /** The editor asked for support at this start, and the author answered. */
  const [isAsked, setAsked] = useState(false)
  const [project, setProject] = useState<Project | null>(null)
  // the project of several files this document is one of; null for a document on its own
  const [files, setFiles] = useState<ProjectOverview | null>(null)
  const [isWithoutTools, setWithoutTools] = useState(false)
  const [party, setParty] = useState<PartyView | null>(null)
  const [watching, setWatching] = useState<LiveView | null>(null)
  const [watchStatus, setWatchStatus] = useState('')
  /** Hints that arrived while the Live review tab was not open. */
  const [newHints, setNewHints] = useState(0)
  const [scope, setScope] = useState<ScopeView | null>(null)
  const [sources, setSources] = useState<SourcesView | null>(null)
  const [isChecking, setChecking] = useState(false)
  const [checkNote, setCheckNote] = useState('')
  /** What the editor just logged about AI text in the document, said once. */
  const [found, setFound] = useState('')
  /** The author said the logs without a document are not this one's. */
  const [orphansSeen, setOrphansSeen] = useState(false)
  const [marks, setMarks] = useState<Record<string, number[]>>({})
  const [moving, setMoving] = useState<{ target: string; pictures: PicturesToMove } | null>(null)
  const [storedSeen, setStoredSeen] = useState(false)
  const [panelWidth, setPanelWidth] = useState(rememberedPanel)
  const [look, setLook] = useState(rememberedLook)
  /** The preferences as the settings dialog has them while it is open, to show at once. */
  const [preview, setPreview] = useState<Preferences | null>(null)
  const preferences = settings?.preferences ?? STANDARD
  const preferencesRef = useRef(preferences)
  preferencesRef.current = preferences
  useEffect(() => applyLook(preview ?? preferences), [preview, preferences])
  // the editor's own words, in the language set (or being tried in the settings); everything drawn below reads them
  setLanguage(languageFor((preview ?? preferences).interfaceLanguage))
  // the experts that come with the editor, named in the language shown; an author's own experts stay as written
  const shownParty: PartyView | null = party === null ? null : { ...party, experts: party.experts.map(expert => (expert.isCustom ? expert : { ...expert, ...m.experts.builtIn[expert.id] })) }
  /** The language the author writes in: what they said in the settings, else what the document was found to be. */
  // what this project sets for itself goes before the general settings
  const here: Preferences = { ...preferences, ...Object.fromEntries(Object.entries(project?.overrides ?? {}).filter(([key, value]) => key in STANDARD && value !== null && value !== undefined)) }
  const said = (preview ?? here).writingLanguage
  const writing: 'en' | 'de' = said !== 'document' ? said : doc?.language === 'de' ? 'de' : 'en'
  const turn = (key: keyof Look) => {
    const next = { ...look, [key]: !look[key] }
    setLook(next)
    try {
      localStorage.setItem('cowriting.look', JSON.stringify(next))
    } catch {
      // not remembered, then
    }
  }
  const work = useRef<HTMLDivElement>(null)
  const dragFrom = useRef(0)

  /** The side panel as wide as the author wants it, within what leaves room for the text; null is the standard width. */
  const resizePanel = (width: number | null) => {
    const next = width === null ? null : Math.round(Math.min(Math.max(width, PANEL_MIN), Math.max(PANEL_MIN, window.innerWidth - TEXT_MIN)))
    setPanelWidth(next)
    try {
      if (next === null) localStorage.removeItem('cowriting.panel')
      else localStorage.setItem('cowriting.panel', String(next))
    } catch {
      // not remembered, then
    }
  }
  const panelNow = () => panelWidth ?? (work.current?.querySelector('.panel')?.getBoundingClientRect().width || PANEL_STANDARD)

  const fail = (e: unknown) => setError(typeof e === 'string' ? e : e instanceof Error ? e.message : String(e))

  const refreshLog = useCallback(async () => {
    try {
      const text = editor.current?.getText() ?? ''
      setLog(await api.logView(text))
      setScope(await api.scope(text))
      setSources(await api.sources(text))
      // the other files are read from disk for this, so only while the overview is looked at
      if (tabRef.current === 'project') setFiles(await api.projectFiles(text))
    } catch (e) {
      fail(e)
    }
  }, [api])

  const save = useCallback(async () => {
    if (editor.current === null) return
    try {
      const rows = await api.saveDocument(editor.current.getText())
      setDirty(false)
      setError('')
      if (rows.length > 0) setFound(foundNotice(rows))
      await refreshLog()
    } catch (e) {
      fail(e)
    }
  }, [api, refreshLog])

  const pasted = useCallback(
    async (text: string, line: number) => {
      if (editor.current === null) return
      try {
        const rows = await api.reportPaste(text, line, editor.current.getText())
        if (rows.length === 0) return
        setFound(foundNotice(rows))
        await refreshLog()
      } catch (e) {
        fail(e)
      }
    },
    [api, refreshLog],
  )
  // live review: once the author has paused, the changed paragraphs are sent; Rust keeps the pace
  const pausing = useRef<number | undefined>(undefined)
  const tabRef = useRef(tab)
  tabRef.current = tab
  const review = useCallback(async () => {
    if (editor.current === null) return
    try {
      setWatchStatus(m.app.reviewing)
      const outcome = await api.liveReview(editor.current.getText())
      if (outcome.kind === 'wait') {
        const gap = watchingRef.current?.gapMs ?? 60_000
        setWatchStatus(m.app.waiting(span(gap), gap))
        window.clearTimeout(pausing.current)
        pausing.current = window.setTimeout(() => void reviewRef.current(), outcome.ms)
        return
      }
      if (outcome.kind === 'reviewed') {
        setWatchStatus(outcome.hints > 0 ? m.app.hintsAt(outcome.hints, when(outcome.ts)) : m.app.nothingToAdd(when(outcome.ts)))
        setWatching(await api.liveView())
        if (tabRef.current !== 'live') setNewHints(n => n + outcome.hints)
        await refreshRef.current()
      } else if (outcome.kind === 'paused') setWatchStatus(m.app.paused(watchingRef.current?.perHour ?? 20))
      else if (outcome.kind === 'failed') setWatchStatus(m.app.reviewFailed)
      else setWatchStatus('')
    } catch (e) {
      setWatchStatus('')
      fail(e)
    }
  }, [api])
  const reviewRef = useRef(review)
  reviewRef.current = review
  const watchingRef = useRef(watching)
  watchingRef.current = watching

  const changeWatching = async (isOn: boolean, personas: string[]) => {
    if (editor.current === null) return
    try {
      await api.setLive(isOn, personas, editor.current.getText())
      setWatching(await api.liveView())
      setWatchStatus('')
      if (!isOn) window.clearTimeout(pausing.current)
      setError('')
    } catch (e) {
      fail(e)
    }
  }

  /** Something the author does to their references; the tab shows the result. */
  const withSources = async <T,>(act: (text: string) => Promise<T>): Promise<T | null> => {
    const text = editor.current?.getText() ?? ''
    try {
      const result = await act(text)
      setSources(await api.sources(text))
      setError('')
      return result
    } catch (e) {
      fail(e)
      return null
    }
  }

  const saveScope = async (text: string): Promise<boolean> => {
    if (editor.current === null) return false
    try {
      await api.setScope(text, editor.current.getText())
      setScope(await api.scope(editor.current.getText()))
      setError('')
      return true
    } catch (e) {
      fail(e)
      return false
    }
  }

  const checkPoints = async () => {
    if (editor.current === null) return
    setChecking(true)
    setCheckNote('')
    try {
      const found = await api.checkPoints(editor.current.getText())
      setCheckNote(m.app.pointsFound(found.covered, found.partial, found.missing))
      await refreshLog()
      setError('')
    } catch (e) {
      fail(e)
    } finally {
      setChecking(false)
    }
  }

  const pastedRef = useRef(pasted)
  pastedRef.current = pasted

  // the handlers the editor was created with must reach the latest save
  const saveRef = useRef(save)
  saveRef.current = save
  const refreshRef = useRef(refreshLog)
  refreshRef.current = refreshLog
  const counting = useRef<number | undefined>(undefined)
  const selecting = useRef<number | undefined>(undefined)
  const [selected, setSelected] = useState<Counted | null>(null)
  const autosaving = useRef<number | undefined>(undefined)

  const show = useCallback(
    async (opened: Opened) => {
      setDoc(opened)
      setOrphansSeen(false)
      setSelected(null)
      if (opened.notices.length > 0) setStoredSeen(false)
      setThread([])
      setLive(null)
      setError('')
      editor.current?.setText(opened.text)
      setDirty(false)
      setLog(await api.logView(opened.text))
      setMarks(await api.hintMarks())
      setPicturesFolder(await api.picturesFolder())
      setParty(await api.party())
      setProject(await api.project())
      // a document can have an AI of its own: what holds now is asked for again
      setSettings(await api.getSettings())
      setWatching(await api.liveView())
      setScope(await api.scope(opened.text))
      setSources(await api.sources(opened.text))
      setFiles(await api.projectFiles(opened.text))
      setWatchStatus('')
      setNewHints(0)
      editor.current?.focus()
    },
    [api],
  )

  useEffect(() => {
    if (doc === null || host.current === null || editor.current !== null) return
    editor.current = makeEditor(host.current, {
      onChange: () => {
        setDirty(true)
        window.clearTimeout(counting.current)
        counting.current = window.setTimeout(() => void refreshRef.current(), 600)
        // saving by itself, if the author wants that: once they have paused
        window.clearTimeout(autosaving.current)
        const pause = preferencesRef.current.autosaveSeconds
        if (pause > 0) autosaving.current = window.setTimeout(() => void saveRef.current(), pause * 1000)
        if (watchingRef.current?.isOn === true) {
          setWatchStatus(m.app.writing)
          window.clearTimeout(pausing.current)
          pausing.current = window.setTimeout(() => void reviewRef.current(), watchingRef.current.pauseMs)
        }
      },
      onPaste: (text, line) => void pastedRef.current(text, line),
      onPicture: file => void pastePictureRef.current(file),
      // what is selected is counted once the selection rests; a bare cursor shows the document's count again
      onSelect: (from, to) => {
        window.clearTimeout(selecting.current)
        if (from === to) return setSelected(null)
        selecting.current = window.setTimeout(() => {
          const text = editor.current?.getText()
          if (text !== undefined) api.countSelection(text, from, to).then(setSelected, () => setSelected(null))
        }, 200)
      },
    })
    editor.current.setText(doc.text)
    setDirty(false)
    editor.current.focus()
  }, [doc, makeEditor])

  useEffect(() => {
    editor.current?.setView({ lineNumbers: look.lineNumbers, source: look.source, spellcheck: (preview ?? preferences).spellcheck, language: writing })
  }, [doc, look.lineNumbers, look.source, preview, preferences, writing])

  useEffect(() => {
    api
      .getSettings()
      .then(async s => {
        setSettings(s)
        // a new author is shown around first; setting up the AI is a step of that
        if (s.showTour) setTourStep(0)
        else if (s.isFirstRun) setDialog('settings')
        if (s.openAtStart !== null) await show(await api.openDocument(s.openAtStart))
      })
      .catch(fail)
  }, [api, show])

  const leaveIsFine = async () => !isDirty || (await api.confirmLeave())

  const open = async () => {
    if (!(await leaveIsFine())) return
    try {
      const path = await api.pickDocument(preferences.documentsFolder)
      if (path !== null) await show(await api.openDocument(path))
    } catch (e) {
      fail(e)
    }
  }

  /** Rename or move: the text as it is in the editor goes to the new place, the log with it. */
  const move = async () => {
    if (doc === null || editor.current === null) return
    try {
      const target = await api.pickMove(doc.path)
      if (target === null) return
      const text = editor.current.getText()
      // pictures the text names from its folder would stay behind: the author says what becomes of them
      const pictures = await api.picturesToMove(target, text)
      if (pictures.inside.length + pictures.outside.length > 0) setMoving({ target, pictures })
      else await show(await api.moveDocument(target, text))
    } catch (e) {
      fail(e)
    }
  }
  const moveWith = async (pictures: 'copy' | 'leave') => {
    const target = moving?.target
    setMoving(null)
    if (target === undefined || editor.current === null) return
    try {
      await show(await api.moveDocument(target, editor.current.getText(), pictures))
    } catch (e) {
      fail(e)
    }
  }

  const adopt = async (name: string) => {
    try {
      await show(await api.adoptLog(name))
    } catch (e) {
      fail(e)
    }
  }

  const create = async () => {
    if (!(await leaveIsFine())) return
    try {
      const path = await api.pickNewDocument(preferences.documentsFolder)
      if (path !== null) await show(await api.newDocument(path))
    } catch (e) {
      fail(e)
    }
  }

  const ask = async (text: string, replyTo: string | null, persona: string | null, command?: string) => {
    if (editor.current === null || live !== null) return
    setError('')
    setTab('ask')
    if (replyTo === null) setThread([])
    setLive({ asked: text, who: persona === null ? m.app.assistant : (shownParty?.experts.find(e => e.id === persona)?.name ?? persona), text: '', steps: [] })
    const onEvent = (event: AskEvent) =>
      setLive(now => {
        if (now === null) return now
        if (event.kind === 'text') return { ...now, text: now.text + event.text }
        if (event.kind === 'tool') return { ...now, steps: [...now.steps, (m.app.toolSteps as Record<string, string>)[event.name] ?? m.app.askingFor(event.name)] }
        if (event.kind === 'refused') return { ...now, steps: [...now.steps.slice(0, -1), m.app.refusedWrite(event.input)] }
        return now
      })
    try {
      const result = await api.ask({ text, replyTo, persona, ...(command === undefined ? {} : { command }), documentText: editor.current.getText() }, onEvent)
      setWithoutTools(result.withoutTools)
      setThread(await api.thread(result.id))
    } catch (e) {
      fail(e)
    } finally {
      setLive(null)
      await refreshLog()
    }
  }

  const openTurn = async (id: string) => {
    try {
      setThread(await api.thread(id))
      setTab('ask')
    } catch (e) {
      fail(e)
    }
  }

  /** A change to the party, then the party as it is now. */
  const changeParty = async (change: () => Promise<unknown>): Promise<boolean> => {
    try {
      await change()
      setParty(await api.party())
      setError('')
      return true
    } catch (e) {
      fail(e)
      return false
    }
  }

  /** Ticks hints of an answer off as done. Shown at once; if it cannot be kept, the tick is taken back. */
  const mark = async (id: string, done: number[]) => {
    const before = marks
    setMarks({ ...marks, [id]: done })
    try {
      await api.setHintMarks(id, done)
    } catch (e) {
      setMarks(before)
      fail(e)
    }
  }

  const annotate = async (id: string, use: Use, note: string) => {
    try {
      await api.annotate(id, use, note)
      await refreshLog()
    } catch (e) {
      fail(e)
    }
  }

  /** Distraction-free on or off: kept with the settings, like the look it is part of. */
  const setCalm = async (calm: boolean) => {
    try {
      await api.setPreferences({ ...preferences, calm })
      setSettings(await api.getSettings())
    } catch (e) {
      fail(e)
    }
  }
  const withProject = async (change: () => Promise<Project>) => {
    try {
      setProject(await change())
      setError('')
    } catch (e) {
      fail(e)
    }
  }

  /** Shows a tab of the side panel, with what opening it means. */
  const showTab = (to: Tab) => {
    setTab(to)
    if (to === 'live') setNewHints(0)
    if (to === 'sources') void refreshLog()
    if (to === 'project') void api.projectFiles(editor.current?.getText() ?? '').then(setFiles, fail)
  }

  // a project of several files: what is open is saved before another file takes its place
  const folderOf = (path: string) => path.replace(/[\\/][^\\/]*$/, '')
  const saveIfChanged = async () => {
    if (!isDirty || editor.current === null) return
    const rows = await api.saveDocument(editor.current.getText())
    setDirty(false)
    if (rows.length > 0) setFound(foundNotice(rows))
  }
  const openFile = async (path: string) => {
    try {
      await saveIfChanged()
      await show(await api.openDocument(path))
    } catch (e) {
      fail(e)
    }
  }
  /** A change to the project's list, then the overview as it is now. */
  const withFiles = async (change: () => Promise<unknown>): Promise<boolean> => {
    try {
      await change()
      setFiles(await api.projectFiles(editor.current?.getText() ?? ''))
      setError('')
      return true
    } catch (e) {
      fail(e)
      return false
    }
  }
  /** The open document joins or leaves a project: it is opened again, because its experts and settings are other ones then. */
  const reopened = async (change: () => Promise<unknown>) => {
    if (doc === null) return
    try {
      await saveIfChanged()
      await change()
      await show(await api.openDocument(doc.path))
      setError('')
    } catch (e) {
      fail(e)
    }
  }
  const makeProject = async () => {
    if (doc === null) return
    try {
      const path = await api.pickProject(doc.path)
      if (path !== null) await reopened(() => api.makeProject(path))
    } catch (e) {
      fail(e)
    }
  }
  const addFile = async (pick: (folder: string) => Promise<string | null>) => {
    if (files === null) return
    try {
      const path = await pick(folderOf(files.path))
      if (path !== null) await withFiles(() => api.addProjectFile(path))
    } catch (e) {
      fail(e)
    }
  }

  // the picture button: the author chooses the file, and the editor writes its path as seen from the document
  const [chosenPicture, setChosenPicture] = useState<{ file: string; seen: Picture } | null>(null)
  const [picturesFolder, setPicturesFolder] = useState('images')
  const pastePictureRef = useRef<(file: File) => Promise<void>>(async () => {})
  const dropFilesRef = useRef<(paths: string[], x: number, y: number) => Promise<void>>(async () => {})
  const insertPicture = async () => {
    if (doc === null) return
    try {
      const file = await api.pickPicture(doc.path.slice(0, Math.max(0, doc.path.length - doc.name.length - 1)))
      if (file === null) return
      const seen = await api.picture(file)
      // outside the document's folder the author says whether it is copied
      if (seen.isInside) editor.current?.format('image', seen.path)
      else setChosenPicture({ file, seen })
    } catch (e) {
      fail(e)
    }
  }
  const copyPicture = async (file: string, into: string) => {
    setChosenPicture(null)
    try {
      editor.current?.format('image', (await api.copyPicture(file, into)).path)
      setPicturesFolder(await api.picturesFolder())
    } catch (e) {
      fail(e)
    }
  }
  // a picture from the clipboard has no file: it is saved into the project's pictures folder and named in the text
  const pastePicture = async (file: File) => {
    const kind = PICTURE_KINDS[file.type]
    if (kind === undefined) return fail(m.app.notAPicture(file.type))
    try {
      const saved = await api.pastePicture(kind, Array.from(new Uint8Array(await file.arrayBuffer())))
      editor.current?.format('image', saved.path)
    } catch (e) {
      fail(e)
    }
  }
  pastePictureRef.current = pastePicture
  // files dropped on the app's window: each picture as the button would take it, where it was dropped
  const dropFiles = async (paths: string[], x: number, y: number) => {
    if (doc === null || dialogRef.current !== null) return
    editor.current?.cursorAt(x, y)
    for (const file of paths) {
      try {
        const seen = await api.picture(file)
        if (seen.isInside) editor.current?.format('image', seen.path)
        // one question at a time: a picture outside the folder is asked about, and the rest of the drop waits for another
        else return setChosenPicture({ file, seen })
      } catch (e) {
        return fail(e)
      }
    }
  }
  dropFilesRef.current = dropFiles
  useEffect(() => api.onFilesDropped((paths, x, y) => void dropFilesRef.current(paths, x, y)), [api])
  const format = (kind: Format) => (kind === 'image' ? void insertPicture() : editor.current?.format(kind))

  // the shortcuts, as the author set them; they rest while a dialog is open
  const act = useRef<(action: Action) => void>(() => {})
  act.current = action => {
    if (action === 'open') void open()
    else if (action === 'new') void create()
    else if (action === 'settings') setDialog('settings')
    else if (action === 'buttons' || action === 'lineNumbers' || action === 'source') turn(action)
    else if (action === 'calm') void setCalm(!preferences.calm)
    else if (doc === null) return
    else if (action === 'save') void save()
    else if (action === 'export') setDialog('export')
    else if (action === 'find') editor.current?.find()
    else if (action === 'ask') {
      setTab('ask')
      window.setTimeout(() => document.querySelector<HTMLElement>('.composer textarea')?.focus())
    } else if (action in TAB_OF) showTab(TAB_OF[action as TabAction])
    else format(action as Format)
  }
  const dialogRef = useRef(dialog)
  dialogRef.current = dialog
  // a tab the author hides while it is open gives way to the next one that is shown
  const hiddenTabs = preferences.hiddenTabs.join(' ')
  useEffect(() => {
    const hidden: Tab[] = preferencesRef.current.hiddenTabs
    if (hidden.includes(tabRef.current)) setTab([...TABS.slice(TABS.indexOf(tabRef.current) + 1), ...TABS].find(next => !hidden.includes(next)) ?? 'ask')
  }, [hiddenTabs])
  // which tab and which dialog were shown goes into the technical log: names of the editor's own
  useEffect(() => seeView(`tab:${tab}`), [tab])
  useEffect(() => {
    if (dialog !== null) seeView(`dialog:${dialog}`)
  }, [dialog])
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (dialogRef.current !== null) return
      const pressed = keysOf(event)
      if (pressed === null) return
      const keys = shortcutsOf(preferencesRef.current.shortcuts)
      const action = (Object.keys(keys) as Action[]).find(id => keys[id] === pressed)
      if (action === undefined) return
      // a markdown shortcut writes into the text, so it only works while the cursor is there
      const isOfTheWindow = ['save', 'open', 'new', 'find', 'export', 'settings', 'ask', 'buttons', 'lineNumbers', 'source', 'calm'].includes(action) || action in TAB_OF
      if (!isOfTheWindow && !(event.target instanceof Element && event.target.closest('.sheet') !== null)) return
      event.preventDefault()
      event.stopPropagation()
      act.current(action)
    }
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  }, [])

  const toStep = (step: number) => {
    setTourStep(step)
    const tab = TOUR[step]?.tab
    if (tab !== undefined) doc === null ? setTab(tab) : showTab(tab)
  }
  const endTour = () => {
    setTourStep(null)
    void api.tourSeen().catch(() => {})
    // who skipped the tour before setting up the AI is offered that now, as before there was a tour
    if (settings !== null && settings.isFirstRun && !settings.isReady) setDialog('settings')
  }
  const settingsDialog = dialog === 'settings' && settings !== null && (
    <SettingsDialog api={api} getText={doc === null ? undefined : () => editor.current?.getText() ?? ''} onTour={() => (setDialog(null), toStep(0))} settings={settings} project={doc === null ? null : project} documentName={doc?.name ?? ''} onPreview={setPreview} onClose={saved => (setDialog(null), saved !== null && (setSettings(saved), setWithoutTools(false), doc !== null && void api.project().then(setProject, () => {}), void api.liveView().then(setWatching, () => {})))} />
  )
  // asked once, and only where there is somewhere to send to and a privacy statement in force
  const isUsageDue = settings !== null && settings.usage.isAvailable && !settings.usage.isAsked && !settings.isFirstRun && dialog === null && tourStep === null
  const isSupportDue = !isUsageDue && settings !== null && settings.asking.isDue && !isAsked && !settings.isFirstRun && dialog === null && tourStep === null
  const otherDialogs = settings !== null && (
    <>
      {(dialog === 'feedback' || dialog === 'bug') && <FeedbackDialog api={api} settings={settings} kind={dialog === 'bug' ? 'bug' : 'idea'} getText={doc === null ? undefined : () => editor.current?.getText() ?? ''} onClose={() => setDialog(null)} />}
      {tourStep !== null && dialog === null && <Tour step={tourStep} hasDocument={doc !== null} isReady={settings.isReady} onStep={toStep} onSettings={() => setDialog('settings')} onEnd={endTour} />}
      {isUsageDue && <UsageDialog api={api} onAnswered={saved => (setSettings({ ...settings, preferences: saved ?? settings.preferences, usage: { ...settings.usage, isAsked: true } }), void api.getSettings().then(now => setSettings({ ...now, usage: { ...now.usage, isAsked: true } }), () => {}))} />}
      {isSupportDue && <SupportDialog api={api} withStar={settings.asking.withStar} onFeedback={() => (setAsked(true), setDialog('feedback'))} onClose={() => setAsked(true)} />}
    </>
  )
  /** Where negative feedback goes: one button everybody finds. */
  const bugButton = (
    <button className="bug" data-tour="around" onClick={() => setDialog('bug')} title={m.app.reportBugTitle}>
      <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round">
        <ellipse cx="8" cy="9.2" rx="3.4" ry="4.3" />
        <path d="M5.6 4.9a2.4 2.4 0 0 1 4.8 0M8 5v8.5M4.6 8H1.8M14.2 8h-2.8M4.9 11.2l-2.3 1.6M11.1 11.2l2.3 1.6M5 6.2 3 4.4M11 6.2l2-1.8" />
      </svg>
      {m.app.reportBug}
    </button>
  )
  const version = settings !== null && <span className="version" title={m.app.alphaTitle}>{shownVersion(settings.version, m.app.alpha)}</span>
  const isScripted = settings?.backend.startsWith('fake:') ?? false
  const notice = [
    settings === null || isScripted ? '' : MODEL_NOTICE[settings.modelStatus],
    isWithoutTools ? m.app.withoutTools : '',
  ]
    .filter(line => line !== '')
    .join(' ')

  if (doc === null) {
    return (
      <main className="welcome">
        <h1>{m.app.welcomeTitle}</h1>
        <p className="margin-note">{m.app.welcomeMargin}</p>
        <p className="promise">{m.app.welcomePromise}</p>
        <div className="row">
          <button className="primary" data-tour="open" onClick={open}>{m.app.openFile}</button>
          <button data-tour="open" onClick={create}>{m.app.newDocument}</button>
        </div>
        {settings !== null && !settings.isReady && (
          <p className="muted">{m.app.notSetUp(<button className="link" onClick={() => setDialog('settings')}>{m.app.setItUp}</button>)}</p>
        )}
        {error !== '' && <p className="error" role="alert">{error}</p>}
        <p className="row foot">{version}{bugButton}{tourStep === null && <button className="link" onClick={() => toStep(0)}>{m.app.takeTour}</button>}</p>
        {settingsDialog}
        {otherDialogs}
      </main>
    )
  }

  const open_ = log.stats.unannotated
  // a hidden tab is in the row while it is the open one (a shortcut, the tour or a link led there), and Live review while it is on
  const inRow = (which: Tab) => which === tab || !(preferences.hiddenTabs as Tab[]).includes(which) || (which === 'live' && watching?.isOn === true)
  const stored = [...(settings?.notices ?? []), ...doc.notices]
  const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path

  return (
    <div className="app">
      <header className="bar">
        <button data-tour="open" onClick={open}>{m.app.open}</button>
        <button data-tour="open" onClick={create}>{m.app.new}</button>
        <button onClick={() => void save()} disabled={!isDirty}>{m.app.save}</button>
        <button onClick={() => void move()} title={m.app.moveTitle}>{m.app.move}</button>
        <span className="name" title={doc.path}>
          {files !== null && <button className="link of-project" title={m.app.tabProjectTitle} onClick={() => showTab('project')}>{files.name}</button>}
          {doc.name}
          <span className={isDirty ? 'state dirty' : 'state'}>{isDirty ? m.app.notSaved : m.app.saved}</span>
        </span>
        <span className="spacer" />
        {selected === null ? (
          <span className="muted" title={m.app.charsDetail(log.chars.withSpaces, log.chars.withoutSpaces)}>{m.app.words(log.words)} · {m.app.chars(log.chars.withSpaces)}</span>
        ) : (
          <span className="muted counted-selection" role="status" title={m.app.selectedDetail(selected.chars.withoutSpaces, log.words, log.chars.withSpaces)}>{m.app.selectedCount(selected.words, selected.chars.withSpaces)}</span>
        )}
        <button data-tour="finish" onClick={() => setDialog('export')}>{m.app.export}</button>
        <button data-tour="finish" onClick={() => setDialog('disclosure')} disabled={log.stats.turns === 0 && log.external.length === 0} title={log.stats.turns === 0 && log.external.length === 0 ? m.app.nothingToDisclose : undefined}>{m.app.disclosure}</button>
        <button data-tour="settings around" onClick={() => setDialog('settings')}>{m.app.settings}</button>
        {bugButton}
      </header>
      {doc.orphans.length > 0 && !orphansSeen && (
        <div className="notice" role="note">
          {doc.orphans.slice(0, 3).map(orphan => (
            <p key={orphan.name}>
              {m.app.orphan(<strong>{orphan.name}</strong>, orphan.questions, orphan.isSameText, doc.name)}{' '}
              <button className="link" onClick={() => void adopt(orphan.name)}>{m.app.continueLog}</button>
            </p>
          ))}
          <button className="link" onClick={() => setOrphansSeen(true)}>{m.app.newDocumentHere}</button>
        </div>
      )}
      {stored.length > 0 && !storedSeen && (
        <div className="notice" role="note">
          {stored.map(notice => (
            <p key={notice.file}>{notice.what === 'setAside' ? m.app.storedSetAside(fileName(notice.file), fileName(notice.keptAs)) : m.app.storedNewer(fileName(notice.file))}</p>
          ))}
          <button className="link" onClick={() => setStoredSeen(true)}>{m.app.dismiss}</button>
        </div>
      )}
      {found !== '' && (
        <p className="found" role="status">
          {found}{' '}
          <button className="link" onClick={() => (setTab('log'), setFound(''))}>{m.app.showInLog}</button>{' '}
          <button className="link" onClick={() => setFound('')}>{m.app.dismiss}</button>
        </p>
      )}
      {error !== '' && (
        <p className="error" role="alert">
          {error} <button className="link" onClick={() => setError('')}>{m.app.dismiss}</button>
        </p>
      )}
      <div className="work" ref={work} style={panelWidth === null ? undefined : ({ '--panel': `${panelWidth}px` } as CSSProperties)}>
        <section className="page" data-tour="write">
          {look.buttons && <FormatBar keys={shortcutsOf(preferences.shortcuts)} onFormat={format} onHelp={() => setDialog('markdown')} />}
          <div className="sheet" ref={host} />
        </section>
        <aside className="panel" data-tour="panel">
          <div
            className="divider"
            role="separator"
            aria-orientation="vertical"
            aria-label={m.app.panelWidth}
            aria-valuenow={Math.round(panelNow())}
            aria-valuemin={PANEL_MIN}
            tabIndex={0}
            title={m.app.panelWidthTitle}
            onPointerDown={e => {
              e.currentTarget.setPointerCapture?.(e.pointerId)
              dragFrom.current = e.clientX
            }}
            onPointerMove={e => {
              // a click or double-click that trembles is not a drag
              if (e.currentTarget.hasPointerCapture?.(e.pointerId) && work.current !== null && Math.abs(e.clientX - dragFrom.current) > 3) resizePanel(work.current.getBoundingClientRect().right - e.clientX)
            }}
            onDoubleClick={() => resizePanel(null)}
            onKeyDown={e => {
              if (e.key === 'ArrowLeft') resizePanel(panelNow() + 24)
              else if (e.key === 'ArrowRight') resizePanel(panelNow() - 24)
              else if (e.key === 'Home') resizePanel(null)
              else return
              e.preventDefault()
            }}
          />
          <nav className="tabs" role="tablist">
            <button role="tab" title={m.app.tabAskTitle} aria-selected={tab === 'ask'} onClick={() => showTab('ask')}>{m.app.tabAsk}</button>
            {inRow('experts') && <button role="tab" title={m.app.tabExpertsTitle} aria-selected={tab === 'experts'} onClick={() => showTab('experts')}>{m.app.tabExperts}</button>}
            {inRow('live') && (
              <button role="tab" title={m.app.tabLiveTitle} aria-selected={tab === 'live'} onClick={() => showTab('live')}>
                {m.app.tabLive}{newHints > 0 && <span className="badge hints" title={m.app.newHints(newHints)}>{newHints}</span>}
              </button>
            )}
            {inRow('scope') && <button role="tab" title={m.app.tabPlanTitle} aria-selected={tab === 'scope'} onClick={() => showTab('scope')}>{m.app.tabPlan}</button>}
            {inRow('project') && <button role="tab" title={m.app.tabProjectTitle} aria-selected={tab === 'project'} onClick={() => showTab('project')}>{m.app.tabProject}</button>}
            {inRow('sources') && <button role="tab" title={m.app.tabSourcesTitle} aria-selected={tab === 'sources'} onClick={() => showTab('sources')}>{m.app.tabSources}</button>}
            {inRow('log') && (
              <button role="tab" title={m.app.tabLogTitle} aria-selected={tab === 'log'} onClick={() => showTab('log')}>
                {m.app.tabLog}{open_ > 0 && <span className="badge" title={m.app.toClassify(open_)}>{open_}</span>}
              </button>
            )}
          </nav>
          {tab === 'ask' ? (
            <AskPanel thread={thread} live={live} turns={log.turns} isReady={settings?.isReady ?? true} notice={notice} party={shownParty} onAsk={ask} onAnnotate={annotate} marks={marks} onMark={(id, done) => void mark(id, done)} onJump={line => editor.current?.jumpToLine(line)} onNew={() => setThread([])} onStop={() => void api.stopAnswer().catch(fail)} onSettings={() => setDialog('settings')} />
          ) : tab === 'experts' && shownParty !== null ? (
            <ExpertsPanel
              party={shownParty}
              files={project?.files ?? []}
              onLink={async () => {
                const path = await api.pickLinkedFile()
                if (path !== null) await withProject(() => api.linkFile(path, []))
              }}
              onLinkFor={async id => {
                const path = await api.pickLinkedFile()
                if (path === null) return
                // a file everyone gets already reaches this expert; one for others is now for this one too
                const linked = project?.files.find(file => file.path === path)
                const personas = linked === undefined ? [id] : linked.personas.length === 0 ? [] : [...new Set([...linked.personas, id])]
                await withProject(() => api.linkFile(path, personas))
              }}
              onFileFor={(path, personas) => void withProject(() => api.linkFile(path, personas))}
              onUnlink={path => void withProject(() => api.unlinkFile(path))}
              onPrompt={(id, prompt) => void changeParty(() => api.setPrompt(id, prompt))}
              canAsk={(settings?.isReady ?? true) && live === null}
              onReview={id => void ask(m.app.reviewRequest, null, id)}
              onBrief={(key, body) => void changeParty(() => api.setBrief(key, body))}
              onMember={(id, isMember) => void changeParty(() => api.setMember(id, isMember))}
              onAdd={(name, role, looksAt) => changeParty(() => api.addExpert(name, role, looksAt))}
            />
          ) : tab === 'live' && watching !== null && shownParty !== null ? (
            <LivePanel view={watching} party={shownParty} status={watchStatus} isReady={settings?.isReady ?? true} onChange={(isOn, personas) => void changeWatching(isOn, personas)} onJump={line => editor.current?.jumpToLine(line)} onOpen={openTurn} />
          ) : tab === 'scope' && scope !== null ? (
            <ScopePanel view={scope} canCheck={settings?.isReady ?? true} isChecking={isChecking} note={checkNote} onSave={saveScope} onCheck={() => void checkPoints()} onJump={line => editor.current?.jumpToLine(line)} />
          ) : tab === 'project' ? (
            <ProjectPanel
              view={files}
              onMake={() => void makeProject()}
              onOpen={path => void openFile(path)}
              onAdd={() => void addFile(api.pickProjectFile)}
              onAddNew={() => void addFile(api.pickNewDocument)}
              onRemove={path => void (files?.files.find(file => file.path === path)?.isOpen === true ? reopened(() => api.removeProjectFile(path)) : withFiles(() => api.removeProjectFile(path)))}
              onOrder={paths => void withFiles(() => api.orderProjectFiles(paths))}
              onRename={(path, name) => withFiles(() => api.renameProjectFile(path, name))}
              onMoveOpen={() => void move()}
              onGoal={(goal, words) => withFiles(() => api.setProjectGoal(goal, words))}
            />
          ) : tab === 'sources' && sources !== null ? (
            <SourcesPanel
              view={sources}
              onAdd={async () => {
                const path = await api.pickBibliography()
                if (path !== null) await withSources(text => api.addBibliography(path, text))
              }}
              onRemove={async path => void (await withSources(() => api.removeBibliography(path)))}
              onStyle={async () => {
                const path = await api.pickStyle()
                if (path !== null) await withSources(text => api.setStyle(path, text))
              }}
              onNoStyle={async () => void (await withSources(() => api.removeStyle()))}
              onStyles={() => void api.openGuide('styles').catch(fail)}
              onSearch={query => withSources(text => api.searchSources(query, text))}
              onCheck={async () => void (await withSources(text => api.checkReferences(text)))}
              onCite={key => editor.current?.insert(`[@${key}]`)}
            />
          ) : (
            <LogPanel log={log} onOpen={openTurn} onJump={line => editor.current?.jumpToLine(line)} chats={{ pick: api.pickChat, read: api.readChats }} onExternal={async stated => {
              try {
                await api.stateExternal(stated)
                await refreshLog()
                setError('')
                return true
              } catch (e) {
                fail(e)
                return false
              }
            }} onWithdraw={id => void api.withdrawExternal(id).then(refreshLog, fail)} onComment={async (id, note) => {
              try {
                await api.commentAiText(id, note)
                await refreshLog()
              } catch (e) {
                fail(e)
              }
            }} />
          )}
        </aside>
      </div>
      <footer className="status">
        <span>
          {settings === null ? '' : isScripted ? m.app.scripted : `${settings.model} · ${settings.baseUrl !== '' ? settings.baseUrl : settings.backend === 'anthropic' ? 'Anthropic' : settings.backend === 'claude-code' ? 'Claude Code' : 'OpenAI'}`}
          {settings !== null && !isScripted && settings.modelStatus !== 'tested' && <span className="flag" title={MODEL_NOTICE[settings.modelStatus]}>{settings.modelStatus === 'failed' ? m.app.failedModel : m.app.untestedModel}</span>}
        </span>
        <span className="spacer" />
        <span className="looks" role="group" aria-label={m.app.looks}>
          <button aria-pressed={look.buttons} onClick={() => turn('buttons')} title={m.app.markdownButtonsTitle}>{m.app.markdownButtons}</button>
          <button aria-pressed={look.lineNumbers} onClick={() => turn('lineNumbers')} title={m.app.lineNumbersTitle}>{m.app.lineNumbers}</button>
          <button aria-pressed={look.source} onClick={() => turn('source')} title={m.app.sourceTitle}>{m.app.source}</button>
          <button data-tour="around" aria-pressed={preferences.calm} onClick={() => void setCalm(!preferences.calm)} title={m.app.calmTitle}>{m.app.calm}</button>
        </span>
        <span>{m.app.logged(log.stats.turns, open_, log.guard.length)}</span>
        {version}
      </footer>
      {settingsDialog}
      {otherDialogs}
      {moving !== null && <MovePicturesDialog inside={moving.pictures.inside} outside={moving.pictures.outside} onCopy={() => void moveWith('copy')} onLeave={() => void moveWith('leave')} onClose={() => setMoving(null)} />}
      {chosenPicture !== null && <PictureDialog name={chosenPicture.seen.name} linkedAs={chosenPicture.seen.path} folder={picturesFolder} onCopy={into => void copyPicture(chosenPicture.file, into)} onLink={() => (editor.current?.format('image', chosenPicture.seen.path), setChosenPicture(null))} onClose={() => setChosenPicture(null)} />}
      {dialog === 'markdown' && <MarkdownHelp onOpen={which => void api.openGuide(which).catch(fail)} onClose={() => setDialog(null)} />}
      {dialog === 'export' && <ExportDialog api={api} getText={() => editor.current?.getText() ?? ''} onClose={() => setDialog(null)} />}
      {dialog === 'disclosure' && <DisclosureDialog api={api} language={here.disclosureLanguage !== 'document' ? here.disclosureLanguage : writing} open={open_} hasOutside={log.external.some(stated => stated.attachments.length > 0)} onClose={() => setDialog(null)} />}
    </div>
  )
}

/** The version in the footer. One that says what it is ("0.1.1-dev", "0.1.1-alpha") stands as it is; a bare number is called an alpha. */
export function shownVersion(version: string, alpha: (version: string) => string): string {
  return version.includes('-') ? version : alpha(version)
}
