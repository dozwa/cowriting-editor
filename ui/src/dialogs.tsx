import { type ReactNode, useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import type { Api, SettingsView, ExportOptions, Exported, DocumentExport, ExportTools, FileFormat, PdfChoice } from './api'
import { language, m } from './i18n'
import type { Preferences } from './preferences'

/** A chosen picture lies outside the document's folder: copy it beside the document, or name it where it is. */
export function PictureDialog({ name, linkedAs, onCopy, onLink, onClose }: { name: string; linkedAs: string; onCopy(): void; onLink(): void; onClose(): void }) {
  const t = m.dialogs
  return (
    <Dialog title={t.pictureTitle} onClose={onClose}>
      <p>{t.pictureOutside(name)}</p>
      <p className="muted">{t.pictureChoice(linkedAs)}</p>
      <div className="actions">
        <button onClick={onClose}>{t.pictureCancel}</button>
        <button onClick={onLink}>{t.pictureLink}</button>
        <button className="primary" onClick={onCopy}>{t.pictureCopy}</button>
      </div>
    </Dialog>
  )
}

/** `mayClose` is asked when Escape is pressed; saying no keeps the dialog open. */
export function Dialog({ title, onClose, mayClose, children }: { title: string; onClose(): void; mayClose?: () => boolean; children: ReactNode }) {
  const ref = useRef<HTMLDialogElement>(null)
  useEffect(() => {
    // jsdom has no showModal; the dialog is then simply shown
    if (ref.current !== null && !ref.current.open) ref.current.showModal?.()
    // opening puts the focus on the first control; in a dialog longer than the window that may be
    // at its end, so there the title takes the focus and the dialog is read from the top
    const dialog = ref.current
    if (dialog !== null && dialog.scrollHeight > dialog.clientHeight) {
      dialog.querySelector<HTMLElement>('h2')?.focus({ preventScroll: true })
      dialog.scrollTop = 0
    }
  }, [])
  // Escape closes, unless the dialog says no. The key is handled here as well as by the web view's own
  // dialog, so that it does not depend on the web view; both ways end here, and one press counts once.
  const pressed = useRef(0)
  const escape = (e: { preventDefault(): void; timeStamp: number }) => {
    e.preventDefault()
    if (e.timeStamp - pressed.current < 100) return
    pressed.current = e.timeStamp
    if (mayClose === undefined || mayClose()) onClose()
  }
  // drawn at the window's own level: a dialog opened from the side panel is not styled or sized as a part of it
  return createPortal(
    <dialog ref={ref} open={typeof HTMLDialogElement.prototype.showModal !== 'function' ? true : undefined} onClose={onClose} onCancel={escape} onKeyDown={e => e.key === 'Escape' && !e.defaultPrevented && escape(e)} aria-label={title}>
      <h2 tabIndex={-1}>{title}</h2>
      {children}
    </dialog>,
    document.body,
  )
}

export function DisclosureDialog({ api, language, open, onClose }: { api: Api; language: 'en' | 'de'; open: number; onClose(): void }) {
  const [options, setOptions] = useState<ExportOptions>({ language, format: 'md', withPrompts: false, withAnswers: false })
  const [exported, setExported] = useState<Exported | null>(null)
  const [error, setError] = useState('')
  const [isCopied, setCopied] = useState(false)
  const set = (change: Partial<ExportOptions>) => (setOptions({ ...options, ...change }), setExported(null), setCopied(false))
  const preview = useRef<HTMLPreElement>(null)
  const copy = async () => {
    if (exported === null) return
    try {
      await navigator.clipboard.writeText(exported.register)
      setCopied(true)
    } catch {
      // a web view that keeps the clipboard to itself: the text is selected, and the author copies it
      if (preview.current !== null) window.getSelection()?.selectAllChildren(preview.current)
    }
  }

  const write = async () => {
    try {
      setExported(await api.exportDisclosure(options))
      setError('')
    } catch (e) {
      setError(String(e))
    }
  }

  const t = m.dialogs
  return (
    <Dialog title={t.disclosureTitle} onClose={onClose}>
      <p>{t.disclosureIntro}</p>
      {open > 0 && <p className="hint">{t.notClassified(open)}</p>}
      <div className="row">
        <label>
          {t.language}
          <select value={options.language} onChange={e => set({ language: e.target.value as 'en' | 'de' })}>
            <option value="en">English</option>
            <option value="de">Deutsch</option>
          </select>
        </label>
        <label>
          {t.format}
          <select value={options.format} onChange={e => set({ format: e.target.value as 'md' | 'tex' })}>
            <option value="md">Markdown</option>
            <option value="tex">LaTeX</option>
          </select>
        </label>
      </div>
      <label className="check">
        <input type="checkbox" checked={options.withPrompts} onChange={e => set({ withPrompts: e.target.checked })} /> {t.withPrompts}
      </label>
      <label className="check">
        <input type="checkbox" checked={options.withAnswers} disabled={!options.withPrompts} onChange={e => set({ withAnswers: e.target.checked })} /> {t.withAnswers}
      </label>
      {error !== '' && <p className="error" role="alert">{error}</p>}
      {exported !== null && (
        <>
          <p className="muted">{t.writtenTo}</p>
          <ul className="paths">
            {exported.paths.map(path => (
              <li key={path}><code>{path}</code></li>
            ))}
          </ul>
          <div className="row written">
            <button onClick={() => void api.showWritten(exported.paths[0]!, 'folder').catch(e => setError(String(e)))}>{t.showInFolder}</button>
            <button onClick={() => void copy()}>{t.copyText}</button>
            {isCopied && <span className="muted" role="status">{t.copied}</span>}
          </div>
          <p className="muted">{t.hiddenFolder}</p>
          <pre className="preview" ref={preview}>{exported.register}</pre>
        </>
      )}
      <div className="row end">
        <button onClick={onClose}>{t.close}</button>
        <button className="primary" onClick={() => void write()}>{exported === null ? t.writeDisclosure : t.writeAgain}</button>
      </div>
    </Dialog>
  )
}

/** The typesetting programs by the names authors know; read when drawn, for the language shown now. */
const engineNames = (): Record<string, string> => ({ typst: m.dialogs.typstEngine, xelatex: 'LaTeX (XeLaTeX)', lualatex: 'LaTeX (LuaLaTeX)', pdflatex: 'LaTeX (pdfLaTeX)', tectonic: 'LaTeX (Tectonic)' })

/** What the author chose last time, kept in this window's storage; a guess from the system's language the first time. */
function rememberedPdf(): PdfChoice {
  const fallback: PdfChoice = { engine: null, paper: /^en-(US|CA)\b/.test(navigator.language) ? 'letter' : 'a4' }
  try {
    const kept = JSON.parse(localStorage.getItem('cowriting.pdf') ?? 'null') as Partial<PdfChoice> | null
    return { engine: typeof kept?.engine === 'string' ? kept.engine : null, paper: kept?.paper === 'letter' || kept?.paper === 'a4' ? kept.paper : fallback.paper }
  } catch {
    return fallback
  }
}

/** The document as a PDF or a Word file, made by pandoc, with the references behind its citekeys. */
export function ExportDialog({ api, getText, onClose }: { api: Api; getText(): string; onClose(): void }) {
  const [setup, setSetup] = useState<{ tools: ExportTools; targets: Record<FileFormat, string> } | null>(null)
  const [format, setFormat] = useState<FileFormat>('pdf')
  const [pdf, setPdf] = useState<PdfChoice>(rememberedPdf)
  const [made, setMade] = useState<DocumentExport | null>(null)
  const [isWorking, setWorking] = useState(false)
  const [error, setError] = useState('')

  useEffect(() => {
    api.exportSetup().then(([tools, pdf, docx]) => setSetup({ tools, targets: { pdf, docx } }), e => setError(String(e)))
  }, [api])

  const write = async () => {
    if (setup === null) return
    try {
      const target = await api.pickExport(format, setup.targets[format])
      if (target === null) return
      setWorking(true)
      setMade(null)
      setMade(await api.exportDocument(format, target, getText(), { ...pdf, engine }))
      setError('')
    } catch (e) {
      setError(String(e))
    } finally {
      setWorking(false)
    }
  }

  const tools = setup?.tools
  const engines = tools?.pdfEngines ?? []
  // a remembered engine that is gone falls back to the standard one
  const engine = pdf.engine !== null && engines.includes(pdf.engine) ? pdf.engine : (engines[0] ?? null)
  const choose = (change: Partial<PdfChoice>) => {
    const next = { ...pdf, ...change }
    setPdf(next)
    setMade(null)
    try {
      localStorage.setItem('cowriting.pdf', JSON.stringify(next))
    } catch {
      // not remembered, then
    }
  }
  const t = m.dialogs
  const cannot = tools === undefined ? '' : !tools.hasPandoc ? t.noPandoc : format === 'pdf' && engines.length === 0 ? t.noPdfEngine : ''
  return (
    <Dialog title={t.exportTitle} onClose={onClose}>
      <p>{t.exportIntro(<code>[@smith2020]</code>)}</p>
      <label>
        {t.format}
        <select value={format} onChange={e => (setFormat(e.target.value as FileFormat), setMade(null))}>
          <option value="pdf">PDF</option>
          <option value="docx">Word (.docx)</option>
        </select>
      </label>
      {format === 'pdf' && engines.length > 0 && (
        <div className="row">
          <label>
            {t.paper}
            <select value={pdf.paper} onChange={e => choose({ paper: e.target.value as PdfChoice['paper'] })}>
              <option value="a4">A4</option>
              <option value="letter">US Letter</option>
            </select>
          </label>
          {engines.length > 1 && (
            <label>
              {t.typesetWith}
              <select value={engine ?? ''} onChange={e => choose({ engine: e.target.value })}>
                {engines.map(name => <option key={name} value={name}>{engineNames()[name] ?? name}</option>)}
              </select>
            </label>
          )}
        </div>
      )}
      {format === 'docx' && <p className="muted">{t.wordNote}</p>}
      {tools !== undefined && (
        <p className="muted">{tools.hasZotero ? t.fromZotero : t.zoteroNotRunning}</p>
      )}
      {cannot !== '' && <p className="hint">{cannot}</p>}
      {error !== '' && <p className="error" role="alert">{error}</p>}
      {made !== null && (
        <div role="status">
          <p>{t.exportedTo(<code>{made.path}</code>)}</p>
          <div className="row written">
            <button onClick={() => void api.showWritten(made.path, 'open').catch(e => setError(String(e)))}>{t.openFile}</button>
            <button onClick={() => void api.showWritten(made.path, 'folder').catch(e => setError(String(e)))}>{t.showInFolder}</button>
          </div>
          <p className="muted">
            {made.cited === 0 ? t.citedNothing : t.citedSummary(made.cited, made.fromZotero)}
            {made.missing.length > 0 && ` ${t.notFound(made.missing.map(key => `@${key}`).join(', '))}`}
          </p>
        </div>
      )}
      <div className="actions">
        <button onClick={onClose}>{t.close}</button>
        <button className="primary" disabled={setup === null || cannot !== '' || isWorking} onClick={() => void write()}>{isWorking ? t.exporting : t.exportButton}</button>
      </div>
    </Dialog>
  )
}

/** The longest report the app hands over; it says no to more. */
const LONGEST_TITLE = 150
const LONGEST_TEXT = 4000

export type FeedbackKind = 'bug' | 'idea' | 'other'

/**
 * What the editor adds about itself when the author wants that. In English whatever the window
 * shows, since it is read by whoever maintains the editor. The provider's address is left out:
 * it can be a private one.
 */
function technicalDetails(settings: SettingsView): string {
  const isLocal = /^https?:\/\/(localhost|127\.0\.0\.1|\[::1\])/.test(settings.baseUrl)
  const ai = settings.backend.startsWith('fake:') ? 'scripted' : settings.backend === 'openai-compatible' ? `${isLocal ? 'a model on this computer' : 'an OpenAI-compatible provider'}, model ${settings.model}` : `${settings.backend}, model ${settings.model}`
  return `CoWriting Editor ${settings.version}, ${settings.system}\nShown in: ${language}\nAI: ${ai}`
}

/**
 * Feedback or a bug report to whoever maintains the editor. A bug is asked for in three parts, so
 * that it can be found again. The author sees every character that goes out, and sends it themselves.
 */
/** How far back the technical log goes in a report. */
const LOG_MINUTES = 10

/**
 * The technical log's newest lines, as many as fit into `room` characters; the day is left out of
 * each time. A report is handed over as an address, so the whole log often does not fit.
 */
function fitted(log: string, room: number): string {
  const lines = log.split('\n').filter(line => line !== '').map(line => line.replace(/^\d{4}-\d\d-\d\dT(\d\d:\d\d:\d\d)\.\d+Z/, '$1'))
  if (lines.length === 0) return '(nothing was recorded)'
  const note = (cut: number) => `(${cut} earlier lines left out)`
  const kept: string[] = []
  let used = 0
  for (const line of [...lines].reverse()) {
    const isLast = kept.length === lines.length - 1
    if (used + line.length + 1 + (isLast ? 0 : note(lines.length).length + 1) > room) break
    kept.unshift(line)
    used += line.length + 1
  }
  return kept.length === lines.length ? kept.join('\n') : [note(lines.length - kept.length), ...kept].join('\n')
}

/**
 * `getText` gives the open document's text; it is asked only when the author ticked that the text
 * goes into the file they save.
 */
export function FeedbackDialog({ api, settings, kind: startsAs = 'idea', getText, onClose }: { api: Api; settings: SettingsView; kind?: FeedbackKind; getText?: () => string; onClose(): void }) {
  const [kind, setKind] = useState<FeedbackKind>(startsAs)
  const [subject, setSubject] = useState('')
  const [text, setText] = useState('')
  const [did, setDid] = useState('')
  const [expected, setExpected] = useState('')
  const [withDetails, setWithDetails] = useState(true)
  // the technical log and the document's text go only when the author says so here
  const [log, setLog] = useState<string | null>(null)
  const [withText, setWithText] = useState(false)
  const [status, setStatus] = useState('')
  const [error, setError] = useState('')
  const preview = useRef<HTMLTextAreaElement>(null)
  const t = m.dialogs
  const isBug = kind === 'bug'

  const title = `${isBug ? 'Bug: ' : kind === 'idea' ? 'Idea: ' : 'Feedback: '}${subject.trim()}`
  // the parts of a bug report are named in English: they are read by whoever maintains the editor
  const said = isBug ? [`What I did:\n${did.trim() || '-'}`, `What happened:\n${text.trim()}`, `What I expected:\n${expected.trim() || '-'}`].join('\n\n') : text.trim()
  const own = `${said}${withDetails ? `\n\n---\n${technicalDetails(settings)}` : ''}`
  const logHeading = `\n\n--- Technical log, last ${LOG_MINUTES} minutes ---\n`
  const report = log === null ? own : `${own}${logHeading}${fitted(log, LONGEST_TEXT - own.length - logHeading.length)}`
  const keepsLog = settings.preferences.diagnostics
  const left = LONGEST_TEXT - report.length
  const canSend = subject.trim() !== '' && text.trim() !== '' && left >= 0

  const send = async (way: 'issue' | 'mail') => {
    setStatus('')
    try {
      await api.sendFeedback(way, title, report)
      setError('')
      setStatus(t.feedbackOpened(way))
    } catch (e) {
      setError(String(e))
    }
  }
  const addLog = async (isWanted: boolean) => {
    setLog(null)
    if (!isWanted) return
    try {
      setLog(await api.recentDiagnostics(LOG_MINUTES))
    } catch (e) {
      setError(String(e))
    }
  }
  const saveFile = async () => {
    setStatus('')
    try {
      const target = await api.pickReportFile()
      if (target === null) return
      const text = withText && getText !== undefined ? getText() : null
      await api.writeReportFile(target, LOG_MINUTES, text)
      setError('')
      setStatus(t.reportFileSaved(text !== null))
    } catch (e) {
      setError(String(e))
    }
  }
  const copy = async () => {
    const whole = `${title}\n\n${report}`
    try {
      await navigator.clipboard.writeText(whole)
      setStatus(t.feedbackCopied)
    } catch {
      // a web view that keeps the clipboard to itself: the report is selected, and the author copies it
      preview.current?.focus()
      preview.current?.select()
      setStatus(t.feedbackSelected)
    }
  }

  return (
    <Dialog title={startsAs === 'bug' ? t.bugTitle : t.feedbackTitle} onClose={onClose}>
      <p>{t.feedbackIntro(settings.feedbackByMail)}</p>
      <label>
        {t.feedbackKind}
        <select value={kind} onChange={e => setKind(e.target.value as FeedbackKind)}>
          <option value="bug">{t.feedbackKinds.bug}</option>
          <option value="idea">{t.feedbackKinds.idea}</option>
          <option value="other">{t.feedbackKinds.other}</option>
        </select>
      </label>
      <label>
        {t.feedbackSubject}
        <input value={subject} maxLength={LONGEST_TITLE - 10} onChange={e => setSubject(e.target.value)} />
      </label>
      {isBug && (
        <label>
          {t.bugDid}
          <textarea rows={3} value={did} onChange={e => setDid(e.target.value)} />
          <small>{t.bugDidHint}</small>
        </label>
      )}
      <label>
        {isBug ? t.bugHappened : t.feedbackText}
        <textarea rows={isBug ? 3 : 5} value={text} onChange={e => setText(e.target.value)} aria-invalid={left < 0} />
        {(left < 400 || !isBug) && <small className={left < 0 ? 'over' : undefined}>{t.feedbackLeft(left)}</small>}
      </label>
      {isBug && (
        <label>
          {t.bugExpected}
          <textarea rows={2} value={expected} onChange={e => setExpected(e.target.value)} />
        </label>
      )}
      <label className="check">
        <input type="checkbox" checked={withDetails} onChange={e => setWithDetails(e.target.checked)} /> {t.feedbackDetails}
      </label>
      {isBug && (
        <>
          <label className="check">
            <input type="checkbox" checked={log !== null} disabled={!keepsLog} onChange={e => void addLog(e.target.checked)} /> {t.feedbackLog(LOG_MINUTES)}
          </label>
          <p className="muted">{keepsLog ? t.feedbackLogHint : t.feedbackLogOff}</p>
        </>
      )}
      <label>
        {t.feedbackPreview}
        <textarea ref={preview} className="outgoing" readOnly rows={7} value={`${title}\n\n${report}`} />
        <small>{t.feedbackNothingElse}</small>
      </label>
      <button className="link" onClick={() => void copy()} disabled={!canSend}>{t.feedbackCopy}</button>
      {isBug && keepsLog && (
        <>
          <p className="muted">{t.reportFileAbout(LOG_MINUTES)}</p>
          {getText !== undefined && (
            <label className="check">
              <input type="checkbox" checked={withText} onChange={e => setWithText(e.target.checked)} /> {t.reportFileWithText}
            </label>
          )}
          {withText && <p className="hint">{t.reportFileTextNote}</p>}
          <div className="row">
            <button onClick={() => void saveFile()}>{t.reportFileSave}</button>
          </div>
        </>
      )}
      {error !== '' && <p className="error" role="alert">{error}</p>}
      {status !== '' && <p className="ok" role="status">{status}</p>}
      {!settings.feedbackByMail && <p className="muted">{t.feedbackIssueNote}</p>}
      <div className="row end">
        <button onClick={onClose}>{t.close}</button>
        <button className={settings.feedbackByMail ? undefined : 'primary'} onClick={() => void send('issue')} disabled={!canSend} title={t.feedbackIssueNote}>{t.feedbackIssue}</button>
        {settings.feedbackByMail && <button className="primary" onClick={() => void send('mail')} disabled={!canSend}>{t.feedbackMail}</button>}
      </div>
    </Dialog>
  )
}

/**
 * The two consents to usage figures, each its own tick, with what would be sent and the privacy
 * statement to read on the spot. Used where the editor asks once, and in the settings.
 */
export function UsageChoice({ api, ping, figures, onChange }: { api: Api; ping: boolean; figures: boolean; onChange(ping: boolean, figures: boolean): void }) {
  const t = m.dialogs
  const [preview, setPreview] = useState<[string, string] | null>(null)
  const [statement, setStatement] = useState<string | null>(null)
  const [error, setError] = useState('')
  const show = <T,>(shown: T | null, load: () => Promise<T>, set: (value: T | null) => void) => (shown !== null ? set(null) : void load().then(set, e => setError(String(e))))
  return (
    <>
      <label className="check">
        <input type="checkbox" checked={ping} onChange={e => onChange(e.target.checked, figures)} /> {t.usagePing}
      </label>
      <p className="muted">{t.usagePingWhat}</p>
      <label className="check">
        <input type="checkbox" checked={figures} onChange={e => onChange(ping, e.target.checked)} /> {t.usageFigures}
      </label>
      <p className="muted">{t.usageFiguresWhat}</p>
      <div className="row">
        <button className="link" aria-expanded={preview !== null} onClick={() => show(preview, api.usagePreview, setPreview)}>{t.usageShowSent}</button>
        <button className="link" aria-expanded={statement !== null} onClick={() => show(statement, () => api.privacyStatement(language), setStatement)}>{t.usageShowStatement}</button>
      </div>
      {preview !== null && (
        <>
          <label>
            {t.usageSentAtStart}
            <textarea className="outgoing" readOnly rows={6} value={preview[0]} />
          </label>
          <label>
            {t.usageSentAtClosing}
            <textarea className="outgoing" readOnly rows={10} value={preview[1]} />
          </label>
        </>
      )}
      {statement !== null && (
        <label>
          {t.usageStatement}
          <textarea className="outgoing" readOnly rows={14} value={statement} />
        </label>
      )}
      {error !== '' && <p className="error" role="alert">{error}</p>}
    </>
  )
}

/** Asked once: may the editor tell its maker that it is used? Nothing is ticked, and closing the dialog is a no. */
export function UsageDialog({ api, onAnswered }: { api: Api; onAnswered(saved: Preferences | null): void }) {
  const t = m.dialogs
  const [ping, setPing] = useState(false)
  const [figures, setFigures] = useState(false)
  const [error, setError] = useState('')
  const answer = (withPing: boolean, withFigures: boolean) => void api.usageAnswered(withPing, withFigures).then(onAnswered, e => (setError(String(e)), onAnswered(null)))
  return (
    <Dialog title={t.usageTitle} onClose={() => answer(false, false)}>
      <p>{t.usageIntro}</p>
      <UsageChoice api={api} ping={ping} figures={figures} onChange={(p, f) => (setPing(p), setFigures(f))} />
      <p className="muted">{t.usageChange}</p>
      {error !== '' && <p className="error" role="alert">{error}</p>}
      <div className="row end">
        <button onClick={() => answer(false, false)}>{t.usageNo}</button>
        <button className="primary" disabled={!ping && !figures} onClick={() => answer(ping, figures)}>{t.usageYes}</button>
      </div>
    </Dialog>
  )
}

/** Every tenth start the editor asks for a coffee, a word of feedback or a star. Whatever is chosen, it is quiet again for a while. */
export function SupportDialog({ api, withStar, onFeedback, onClose }: { api: Api; withStar: boolean; onFeedback(): void; onClose(): void }) {
  const t = m.dialogs
  const answer = (what: 'coffee' | 'feedback' | 'star' | 'later') => {
    void api.supportAnswered(what).catch(() => {})
    if (what === 'coffee') void api.openGuide('coffee').catch(() => {})
    if (what === 'star') void api.openGuide('repository').catch(() => {})
    if (what === 'feedback') onFeedback()
    else onClose()
  }
  return (
    <Dialog title={t.supportTitle} onClose={() => answer('later')}>
      <p>{t.supportIntro}</p>
      <div className="support">
        <button className="primary" onClick={() => answer('coffee')}>{t.supportCoffee}</button>
        <button onClick={() => answer('feedback')}>{t.supportFeedback}</button>
        {withStar && <button onClick={() => answer('star')}>{t.supportStar}</button>}
      </div>
      <p className="muted">{t.supportNote}</p>
      <div className="row end">
        <button onClick={() => answer('later')}>{t.supportLater}</button>
      </div>
    </Dialog>
  )
}
