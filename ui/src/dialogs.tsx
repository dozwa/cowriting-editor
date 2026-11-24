import { type ReactNode, useEffect, useRef, useState } from 'react'
import type { Api, SettingsView, ExportOptions, Exported, DocumentExport, ExportTools, FileFormat, PdfChoice } from './api'
import { language, m } from './i18n'

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
  return (
    <dialog ref={ref} open={typeof HTMLDialogElement.prototype.showModal !== 'function' ? true : undefined} onClose={onClose} onCancel={e => mayClose !== undefined && !mayClose() && e.preventDefault()} aria-label={title}>
      <h2 tabIndex={-1}>{title}</h2>
      {children}
    </dialog>
  )
}

export function DisclosureDialog({ api, language, open, onClose }: { api: Api; language: 'en' | 'de'; open: number; onClose(): void }) {
  const [options, setOptions] = useState<ExportOptions>({ language, format: 'md', withPrompts: false, withAnswers: false })
  const [exported, setExported] = useState<Exported | null>(null)
  const [error, setError] = useState('')
  const set = (change: Partial<ExportOptions>) => (setOptions({ ...options, ...change }), setExported(null))

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
          <pre className="preview">{exported.register}</pre>
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
export function FeedbackDialog({ api, settings, kind: startsAs = 'idea', onClose }: { api: Api; settings: SettingsView; kind?: FeedbackKind; onClose(): void }) {
  const [kind, setKind] = useState<FeedbackKind>(startsAs)
  const [subject, setSubject] = useState('')
  const [text, setText] = useState('')
  const [did, setDid] = useState('')
  const [expected, setExpected] = useState('')
  const [withDetails, setWithDetails] = useState(true)
  const [status, setStatus] = useState('')
  const [error, setError] = useState('')
  const preview = useRef<HTMLTextAreaElement>(null)
  const t = m.dialogs
  const isBug = kind === 'bug'

  const title = `${isBug ? 'Bug: ' : kind === 'idea' ? 'Idea: ' : 'Feedback: '}${subject.trim()}`
  // the parts of a bug report are named in English: they are read by whoever maintains the editor
  const said = isBug ? [`What I did:\n${did.trim() || '-'}`, `What happened:\n${text.trim()}`, `What I expected:\n${expected.trim() || '-'}`].join('\n\n') : text.trim()
  const report = `${said}${withDetails ? `\n\n---\n${technicalDetails(settings)}` : ''}`
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
      <p className="muted">{t.feedbackIntro(settings.feedbackByMail)}</p>
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
      <label>
        {t.feedbackPreview}
        <textarea ref={preview} className="outgoing" readOnly rows={7} value={`${title}\n\n${report}`} />
        <small>{t.feedbackNothingElse}</small>
      </label>
      {error !== '' && <p className="error" role="alert">{error}</p>}
      {status !== '' && <p className="ok" role="status">{status}</p>}
      {!settings.feedbackByMail && <p className="muted">{t.feedbackIssueNote}</p>}
      <div className="row end">
        <button onClick={onClose}>{t.close}</button>
        <span className="spacer" />
        <button onClick={() => void copy()} disabled={!canSend}>{t.feedbackCopy}</button>
        <button className={settings.feedbackByMail ? undefined : 'primary'} onClick={() => void send('issue')} disabled={!canSend} title={t.feedbackIssueNote}>{t.feedbackIssue}</button>
        {settings.feedbackByMail && <button className="primary" onClick={() => void send('mail')} disabled={!canSend}>{t.feedbackMail}</button>}
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
