import { useCallback, useEffect, useRef, useState } from 'react'
import type { AiTextRow, Api, AskEvent, LiveView, LogView, ScopeView, SourcesView, Opened, PartyView, SettingsView, ThreadMessage, Use } from './api'
import { ExpertsPanel } from './experts'
import { LivePanel, ScopePanel } from './plan'
import { SourcesPanel } from './sources'
import { DisclosureDialog, SettingsDialog, ExportDialog } from './dialogs'
import { createEditor, type Editor } from './editor'
import { AskPanel, LogPanel } from './panels'
import { MODEL_NOTICE, plural, when } from './text'

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

const EMPTY_LOG: LogView = { stats: { turns: 0, today: 0, unannotated: 0, aiText: 0, annotations: 0, rejected: 0, guardEvents: 0 }, turns: [], guard: [], aiText: [], words: 0 }

const TOOL_STEPS: Record<string, string> = {
  read_document: 'reading your document',
  search_document: 'searching your document',
  list_notes: 'looking at its notes',
  search_bibliography: 'searching your references',
  read_note: 'reading a note',
  write_note: 'writing a note',
}

/** What the author is told when text from an AI answer entered the document. */
function foundNotice(rows: AiTextRow[]): string {
  const words = rows.reduce((sum, row) => sum + row.words, 0)
  const how = rows.every(row => row.source === 'paste') ? 'you pasted' : 'in your document'
  const what = rows.every(row => row.kind === 'correction') ? 'a language correction from an AI answer' : 'text from an AI answer'
  return `Logged: ${words} words ${how} match ${what}. Nothing was changed; the passage will be listed in your disclosure.`
}

export function App({ api, makeEditor = createEditor }: Props) {
  const host = useRef<HTMLDivElement>(null)
  const editor = useRef<Editor | null>(null)
  const [doc, setDoc] = useState<Opened | null>(null)
  const [isDirty, setDirty] = useState(false)
  const [settings, setSettings] = useState<SettingsView | null>(null)
  const [tab, setTab] = useState<'ask' | 'experts' | 'live' | 'scope' | 'sources' | 'log'>('ask')
  const [log, setLog] = useState<LogView>(EMPTY_LOG)
  const [thread, setThread] = useState<ThreadMessage[]>([])
  const [live, setLive] = useState<Live | null>(null)
  const [error, setError] = useState('')
  const [dialog, setDialog] = useState<'settings' | 'disclosure' | 'export' | null>(null)
  const [isWithoutTools, setWithoutTools] = useState(false)
  const [party, setParty] = useState<PartyView | null>(null)
  const [watching, setWatching] = useState<LiveView | null>(null)
  const [watchStatus, setWatchStatus] = useState('')
  /** Hints that arrived while the Live tab was not open. */
  const [newHints, setNewHints] = useState(0)
  const [scope, setScope] = useState<ScopeView | null>(null)
  const [sources, setSources] = useState<SourcesView | null>(null)
  const [isChecking, setChecking] = useState(false)
  const [checkNote, setCheckNote] = useState('')
  /** What the editor just logged about AI text in the document, said once. */
  const [found, setFound] = useState('')

  const fail = (e: unknown) => setError(typeof e === 'string' ? e : e instanceof Error ? e.message : String(e))

  const refreshLog = useCallback(async () => {
    try {
      const text = editor.current?.getText() ?? ''
      setLog(await api.logView(text))
      setScope(await api.scope(text))
      setSources(await api.sources(text))
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
      setWatchStatus('reviewing…')
      const outcome = await api.liveReview(editor.current.getText())
      if (outcome.kind === 'wait') {
        setWatchStatus('waiting: at most one review a minute')
        window.clearTimeout(pausing.current)
        pausing.current = window.setTimeout(() => void reviewRef.current(), outcome.ms)
        return
      }
      if (outcome.kind === 'reviewed') {
        setWatchStatus(outcome.hints > 0 ? `${plural(outcome.hints, 'hint')} at ${when(outcome.ts)}` : `looked at ${when(outcome.ts)}: nothing to add, keep writing`)
        setWatching(await api.liveView())
        if (tabRef.current !== 'live') setNewHints(n => n + outcome.hints)
        await refreshRef.current()
      } else if (outcome.kind === 'paused') setWatchStatus('paused: 20 reviews in the last hour')
      else if (outcome.kind === 'failed') setWatchStatus('the last live review failed; it will try again after your next change')
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
      setCheckNote(`${found.covered} covered, ${found.partial} partly, ${found.missing} missing`)
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

  const show = useCallback(
    async (opened: Opened) => {
      setDoc(opened)
      setThread([])
      setLive(null)
      setError('')
      editor.current?.setText(opened.text)
      setDirty(false)
      setLog(await api.logView(opened.text))
      setParty(await api.party())
      setWatching(await api.liveView())
      setScope(await api.scope(opened.text))
      setSources(await api.sources(opened.text))
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
        if (watchingRef.current?.isOn === true) {
          setWatchStatus('you are writing…')
          window.clearTimeout(pausing.current)
          pausing.current = window.setTimeout(() => void reviewRef.current(), watchingRef.current.pauseMs)
        }
      },
      onSave: () => void saveRef.current(),
      onPaste: (text, line) => void pastedRef.current(text, line),
    })
    editor.current.setText(doc.text)
    setDirty(false)
    editor.current.focus()
  }, [doc, makeEditor])

  useEffect(() => {
    api
      .getSettings()
      .then(async s => {
        setSettings(s)
        if (s.isFirstRun) setDialog('settings')
        if (s.openAtStart !== null) await show(await api.openDocument(s.openAtStart))
      })
      .catch(fail)
  }, [api, show])

  const leaveIsFine = async () => !isDirty || (await api.confirmLeave())

  const open = async () => {
    if (!(await leaveIsFine())) return
    try {
      const path = await api.pickDocument()
      if (path !== null) await show(await api.openDocument(path))
    } catch (e) {
      fail(e)
    }
  }

  const create = async () => {
    if (!(await leaveIsFine())) return
    try {
      const path = await api.pickNewDocument()
      if (path !== null) await show(await api.newDocument(path))
    } catch (e) {
      fail(e)
    }
  }

  const ask = async (text: string, replyTo: string | null, persona: string | null) => {
    if (editor.current === null || live !== null) return
    setError('')
    setTab('ask')
    if (replyTo === null) setThread([])
    setLive({ asked: text, who: persona === null ? 'Assistant' : (party?.experts.find(e => e.id === persona)?.name ?? persona), text: '', steps: [] })
    const onEvent = (event: AskEvent) =>
      setLive(now => {
        if (now === null) return now
        if (event.kind === 'text') return { ...now, text: now.text + event.text }
        if (event.kind === 'tool') return { ...now, steps: [...now.steps, TOOL_STEPS[event.name] ?? `asking for “${event.name}”`] }
        if (event.kind === 'refused') return { ...now, steps: [...now.steps.slice(0, -1), `tried to write where it may not (${event.input}): refused and logged`] }
        return now
      })
    try {
      const result = await api.ask({ text, replyTo, persona, documentText: editor.current.getText() }, onEvent)
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

  const annotate = async (id: string, use: Use, note: string) => {
    try {
      await api.annotate(id, use, note)
      await refreshLog()
    } catch (e) {
      fail(e)
    }
  }

  const settingsDialog = dialog === 'settings' && settings !== null && (
    <SettingsDialog api={api} settings={settings} onClose={saved => (setDialog(null), saved !== null && (setSettings(saved), setWithoutTools(false)))} />
  )
  const isScripted = settings?.backend.startsWith('fake:') ?? false
  const notice = [
    settings === null || isScripted ? '' : MODEL_NOTICE[settings.modelStatus],
    isWithoutTools ? 'This model cannot use tools: your document is sent along with each question, and it cannot write notes.' : '',
  ]
    .filter(line => line !== '')
    .join(' ')

  if (doc === null) {
    return (
      <main className="welcome">
        <h1>CoWriting Editor</h1>
        <p>You write. The AI gives feedback, asks questions and explains: it cannot change your text. Everything it says is logged, and the log becomes your disclosure of AI use.</p>
        <div className="row">
          <button className="primary" onClick={open}>Open a markdown file…</button>
          <button onClick={create}>New document…</button>
        </div>
        {settings !== null && !settings.isReady && (
          <p className="muted">
            The AI is not set up yet. <button className="link" onClick={() => setDialog('settings')}>Set it up</button>
          </p>
        )}
        {error !== '' && <p className="error" role="alert">{error}</p>}
        {settingsDialog}
      </main>
    )
  }

  const open_ = log.stats.unannotated
  return (
    <div className="app">
      <header className="bar">
        <button onClick={open}>Open…</button>
        <button onClick={create}>New…</button>
        <button onClick={() => void save()} disabled={!isDirty}>Save</button>
        <span className="name" title={doc.path}>
          {doc.name}
          <span className={isDirty ? 'state dirty' : 'state'}>{isDirty ? 'not saved' : 'saved'}</span>
        </span>
        <span className="spacer" />
        <span className="muted">{plural(log.words, 'word')}</span>
        <button onClick={() => setDialog('export')}>Export…</button>
        <button onClick={() => setDialog('disclosure')} disabled={log.stats.turns === 0} title={log.stats.turns === 0 ? 'Nothing to disclose yet: the AI has not been asked' : undefined}>Disclosure…</button>
        <button onClick={() => setDialog('settings')}>Settings…</button>
      </header>
      {found !== '' && (
        <p className="found" role="status">
          {found}{' '}
          <button className="link" onClick={() => (setTab('log'), setFound(''))}>Show in the log</button>{' '}
          <button className="link" onClick={() => setFound('')}>Dismiss</button>
        </p>
      )}
      {error !== '' && (
        <p className="error" role="alert">
          {error} <button className="link" onClick={() => setError('')}>Dismiss</button>
        </p>
      )}
      <div className="work">
        <section className="page" ref={host} />
        <aside className="panel">
          <nav className="tabs" role="tablist">
            <button role="tab" aria-selected={tab === 'ask'} onClick={() => setTab('ask')}>Ask</button>
            <button role="tab" aria-selected={tab === 'experts'} onClick={() => setTab('experts')}>Experts</button>
            <button role="tab" aria-selected={tab === 'live'} onClick={() => (setTab('live'), setNewHints(0))}>
              Live{newHints > 0 && <span className="badge" title={`${plural(newHints, 'new hint')}`}>{newHints}</span>}
            </button>
            <button role="tab" aria-selected={tab === 'scope'} onClick={() => setTab('scope')}>Scope</button>
            <button role="tab" aria-selected={tab === 'sources'} onClick={() => (setTab('sources'), void refreshLog())}>Sources</button>
            <button role="tab" aria-selected={tab === 'log'} onClick={() => setTab('log')}>
              Log{open_ > 0 && <span className="badge" title={`${plural(open_, 'answer')} to classify`}>{open_}</span>}
            </button>
          </nav>
          {tab === 'ask' ? (
            <AskPanel thread={thread} live={live} turns={log.turns} isReady={settings?.isReady ?? true} notice={notice} party={party} onAsk={ask} onAnnotate={annotate} onJump={line => editor.current?.jumpToLine(line)} onNew={() => setThread([])} onSettings={() => setDialog('settings')} />
          ) : tab === 'experts' && party !== null ? (
            <ExpertsPanel
              party={party}
              canAsk={(settings?.isReady ?? true) && live === null}
              onReview={id => void ask('Please review my document.', null, id)}
              onBrief={(key, body) => void changeParty(() => api.setBrief(key, body))}
              onMember={(id, isMember) => void changeParty(() => api.setMember(id, isMember))}
              onAdd={(name, role, looksAt) => changeParty(() => api.addExpert(name, role, looksAt))}
            />
          ) : tab === 'live' && watching !== null && party !== null ? (
            <LivePanel view={watching} party={party} status={watchStatus} isReady={settings?.isReady ?? true} onChange={(isOn, personas) => void changeWatching(isOn, personas)} onJump={line => editor.current?.jumpToLine(line)} onOpen={openTurn} />
          ) : tab === 'scope' && scope !== null ? (
            <ScopePanel view={scope} canCheck={settings?.isReady ?? true} isChecking={isChecking} note={checkNote} onSave={saveScope} onCheck={() => void checkPoints()} onJump={line => editor.current?.jumpToLine(line)} />
          ) : tab === 'sources' && sources !== null ? (
            <SourcesPanel
              view={sources}
              onAdd={async () => {
                const path = await api.pickBibliography()
                if (path !== null) await withSources(text => api.addBibliography(path, text))
              }}
              onRemove={async path => void (await withSources(() => api.removeBibliography(path)))}
              onSearch={query => withSources(text => api.searchSources(query, text))}
              onCheck={async () => void (await withSources(text => api.checkReferences(text)))}
              onCite={key => editor.current?.insert(`[@${key}]`)}
            />
          ) : (
            <LogPanel log={log} onOpen={openTurn} onJump={line => editor.current?.jumpToLine(line)} onComment={async (id, note) => {
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
          {settings === null ? '' : isScripted ? 'Scripted answers (no model)' : `${settings.model} · ${settings.baseUrl !== '' ? settings.baseUrl : settings.backend === 'anthropic' ? 'Anthropic' : 'OpenAI'}`}
          {settings !== null && !isScripted && settings.modelStatus !== 'tested' && <span className="flag" title={MODEL_NOTICE[settings.modelStatus]}>{settings.modelStatus === 'failed' ? 'failed the rule test' : 'untested model'}</span>}
        </span>
        <span className="spacer" />
        <span>{plural(log.stats.turns, 'AI input')} logged{open_ > 0 ? ` · ${open_} to classify` : ''}{log.guard.length > 0 ? ` · ${plural(log.guard.length, 'refused write')}` : ''}</span>
      </footer>
      {settingsDialog}
      {dialog === 'export' && <ExportDialog api={api} getText={() => editor.current?.getText() ?? ''} onClose={() => setDialog(null)} />}
      {dialog === 'disclosure' && <DisclosureDialog api={api} language={doc.language === 'de' ? 'de' : 'en'} open={open_} onClose={() => setDialog(null)} />}
    </div>
  )
}
