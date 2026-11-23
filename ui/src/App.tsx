import { useCallback, useEffect, useRef, useState } from 'react'
import type { Api, AskEvent, LogView, Opened, SettingsView, ThreadMessage, Use } from './api'
import { DisclosureDialog, SettingsDialog } from './dialogs'
import { createEditor, type Editor } from './editor'
import { AskPanel, LogPanel } from './panels'
import { MODEL_NOTICE, plural } from './text'

type Props = {
  api: Api
  /** Tests give a plain text field instead of the real editor. */
  makeEditor?: typeof createEditor
}

export type Live = {
  /** What the author asked, shown until the turn is in the log. */
  asked: string
  text: string
  /** What the model is doing besides writing: "reading the document", a refused write. */
  steps: string[]
}

const EMPTY_LOG: LogView = { stats: { turns: 0, today: 0, unannotated: 0, aiText: 0, annotations: 0, rejected: 0, guardEvents: 0 }, turns: [], guard: [], words: 0 }

const TOOL_STEPS: Record<string, string> = {
  read_document: 'reading your document',
  search_document: 'searching your document',
  list_notes: 'looking at its notes',
  read_note: 'reading a note',
  write_note: 'writing a note',
}

export function App({ api, makeEditor = createEditor }: Props) {
  const host = useRef<HTMLDivElement>(null)
  const editor = useRef<Editor | null>(null)
  const [doc, setDoc] = useState<Opened | null>(null)
  const [isDirty, setDirty] = useState(false)
  const [settings, setSettings] = useState<SettingsView | null>(null)
  const [tab, setTab] = useState<'ask' | 'log'>('ask')
  const [log, setLog] = useState<LogView>(EMPTY_LOG)
  const [thread, setThread] = useState<ThreadMessage[]>([])
  const [live, setLive] = useState<Live | null>(null)
  const [error, setError] = useState('')
  const [dialog, setDialog] = useState<'settings' | 'disclosure' | null>(null)
  const [isWithoutTools, setWithoutTools] = useState(false)

  const fail = (e: unknown) => setError(typeof e === 'string' ? e : e instanceof Error ? e.message : String(e))

  const refreshLog = useCallback(async () => {
    try {
      setLog(await api.logView(editor.current?.getText() ?? ''))
    } catch (e) {
      fail(e)
    }
  }, [api])

  const save = useCallback(async () => {
    if (editor.current === null) return
    try {
      await api.saveDocument(editor.current.getText())
      setDirty(false)
      setError('')
      await refreshLog()
    } catch (e) {
      fail(e)
    }
  }, [api, refreshLog])

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
      },
      onSave: () => void saveRef.current(),
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

  const ask = async (text: string, replyTo: string | null) => {
    if (editor.current === null || live !== null) return
    setError('')
    setTab('ask')
    if (replyTo === null) setThread([])
    setLive({ asked: text, text: '', steps: [] })
    const onEvent = (event: AskEvent) =>
      setLive(now => {
        if (now === null) return now
        if (event.kind === 'text') return { ...now, text: now.text + event.text }
        if (event.kind === 'tool') return { ...now, steps: [...now.steps, TOOL_STEPS[event.name] ?? `asking for “${event.name}”`] }
        if (event.kind === 'refused') return { ...now, steps: [...now.steps.slice(0, -1), `tried to write where it may not (${event.input}): refused and logged`] }
        return now
      })
    try {
      const result = await api.ask({ text, replyTo, documentText: editor.current.getText() }, onEvent)
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
        <button onClick={() => setDialog('disclosure')} disabled={log.stats.turns === 0} title={log.stats.turns === 0 ? 'Nothing to disclose yet: the AI has not been asked' : undefined}>Disclosure…</button>
        <button onClick={() => setDialog('settings')}>Settings…</button>
      </header>
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
            <button role="tab" aria-selected={tab === 'log'} onClick={() => setTab('log')}>
              Log{open_ > 0 && <span className="badge" title={`${plural(open_, 'answer')} to classify`}>{open_}</span>}
            </button>
          </nav>
          {tab === 'ask' ? (
            <AskPanel thread={thread} live={live} turns={log.turns} isReady={settings?.isReady ?? true} notice={notice} onAsk={ask} onAnnotate={annotate} onJump={line => editor.current?.jumpToLine(line)} onNew={() => setThread([])} onSettings={() => setDialog('settings')} />
          ) : (
            <LogPanel log={log} onOpen={openTurn} />
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
      {dialog === 'disclosure' && <DisclosureDialog api={api} language={doc.language === 'de' ? 'de' : 'en'} open={open_} onClose={() => setDialog(null)} />}
    </div>
  )
}
