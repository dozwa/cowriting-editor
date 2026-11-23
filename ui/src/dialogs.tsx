import { type ReactNode, useEffect, useRef, useState } from 'react'
import type { Api, ExportOptions, Exported, ModelChoice, Settings, SettingsView, DocumentExport, ExportTools, FileFormat, PdfChoice } from './api'

function Dialog({ title, onClose, children }: { title: string; onClose(): void; children: ReactNode }) {
  const ref = useRef<HTMLDialogElement>(null)
  useEffect(() => {
    // jsdom has no showModal; the dialog is then simply shown
    if (ref.current !== null && !ref.current.open) ref.current.showModal?.()
  }, [])
  return (
    <dialog ref={ref} open={typeof HTMLDialogElement.prototype.showModal !== 'function' ? true : undefined} onClose={onClose} aria-label={title}>
      <h2>{title}</h2>
      {children}
    </dialog>
  )
}

type Kind = 'anthropic' | 'local' | 'other'

const KINDS: Record<Kind, { label: string; backend: string; address: string; model: string; sent: string }> = {
  anthropic: { label: 'Claude, with an Anthropic API key', backend: 'anthropic', address: '', model: 'claude-opus-5-5', sent: 'Your questions and your document are sent to Anthropic when you ask.' },
  local: { label: 'A model on this computer (Ollama, LM Studio)', backend: 'openai-compatible', address: 'http://localhost:11434/v1', model: '', sent: 'Nothing leaves this computer.' },
  other: { label: 'Another provider (OpenAI-compatible API)', backend: 'openai-compatible', address: '', model: '', sent: 'Your questions and your document are sent to the provider at this address when you ask.' },
}

const isLocalAddress = (url: string) => /^https?:\/\/(localhost|127\.0\.0\.1|\[::1\])(:\d+)?(\/|$)/.test(url)

const STATUS_WORDS = { tested: 'kept to the assist-only rules when tested', failed: 'did not keep to the assist-only rules when tested', untested: 'not tested for the assist-only rules' }

export function SettingsDialog({ api, settings, onClose }: { api: Api; settings: SettingsView; onClose(saved: SettingsView | null): void }) {
  const isScripted = settings.backend.startsWith('fake:')
  const [kind, setKind] = useState<Kind>(settings.backend !== 'openai-compatible' ? 'anthropic' : isLocalAddress(settings.baseUrl) ? 'local' : 'other')
  const [model, setModel] = useState(isScripted ? KINDS.anthropic.model : settings.model)
  const [baseUrl, setBaseUrl] = useState(settings.baseUrl)
  const [key, setKey] = useState('')
  const [choices, setChoices] = useState<ModelChoice[] | null>(null)
  const [isTesting, setTesting] = useState(false)
  const [error, setError] = useState('')
  const chosen = KINDS[kind]
  const next: Settings = { backend: chosen.backend, model: model.trim(), baseUrl: baseUrl.trim() }
  const hasStoredKey = settings.hasKey && settings.backend === chosen.backend

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
      const found = await api.testConnection(next, key.trim() === '' ? null : key.trim())
      setChoices(found)
      // a model the provider does not offer is replaced by one it does
      if (found.length > 0 && !found.some(m => m.id === model.trim())) setModel(found[0]!.id)
    } catch (e) {
      setChoices(null)
      setError(String(e))
    } finally {
      setTesting(false)
    }
  }

  const save = async () => {
    try {
      await api.setSettings(next, key.trim() === '' ? null : key.trim())
      onClose(await api.getSettings())
    } catch (e) {
      setError(String(e))
    }
  }

  const status = choices?.find(m => m.id === model.trim())?.status
  return (
    <Dialog title={settings.isFirstRun ? 'Set up the AI' : 'Settings'} onClose={() => onClose(null)}>
      {settings.isFirstRun && <p>Choose where the AI's answers come from. You can change this later, and you can write without it.</p>}
      {isScripted && <p className="hint">This run answers from a script, not a model. The settings below apply to normal runs.</p>}
      <div className="choice" role="radiogroup" aria-label="Where the AI's answers come from">
        {(Object.keys(KINDS) as Kind[]).map(k => (
          <label key={k}>
            <input type="radio" name="kind" checked={kind === k} onChange={() => choose(k)} /> {KINDS[k].label}
          </label>
        ))}
      </div>
      {kind !== 'local' && (
        <label>
          API key
          <input type="password" autoComplete="off" value={key} onChange={e => setKey(e.target.value)} placeholder={hasStoredKey ? 'A key is stored. Type a new one to replace it.' : ''} />
          <small>Kept in your system’s key store, not in a file.</small>
        </label>
      )}
      <label>
        {kind === 'anthropic' ? 'Address of the API (optional)' : 'Address of the API'}
        <input value={baseUrl} onChange={e => (setBaseUrl(e.target.value), setChoices(null))} placeholder={kind === 'anthropic' ? 'https://api.anthropic.com' : kind === 'other' ? 'https://api.openai.com/v1' : ''} />
        {kind === 'local' && (
          <small>
            <button className="link" onClick={() => setBaseUrl('http://localhost:11434/v1')}>Ollama</button> · <button className="link" onClick={() => setBaseUrl('http://localhost:1234/v1')}>LM Studio</button>
          </small>
        )}
        {kind === 'anthropic' && <small>Only for a gateway that speaks the Claude API, such as one your institution runs.</small>}
      </label>
      <label>
        Model
        {choices !== null && choices.length > 0 ? (
          <select value={model} onChange={e => setModel(e.target.value)}>
            {choices.map(m => (
              <option key={m.id} value={m.id}>{m.name === m.id || m.name === '' ? m.id : `${m.name} (${m.id})`}{m.status === 'untested' ? '' : m.status === 'tested' ? ' ✓' : ' ✗'}</option>
            ))}
          </select>
        ) : (
          <input value={model} onChange={e => setModel(e.target.value)} placeholder={kind === 'local' ? 'Test the connection to list the models' : ''} />
        )}
        <small>{status !== undefined ? `This model: ${STATUS_WORDS[status]}. ` : ''}The disclosure names the model that answered.</small>
      </label>
      <div className="row">
        <button onClick={() => void test()} disabled={isTesting}>{isTesting ? 'Testing…' : 'Test the connection'}</button>
        {choices !== null && <span className="ok" role="status">Connected: {choices.length === 1 ? '1 model' : `${choices.length} models`}</span>}
      </div>
      <p className="muted">{chosen.sent}</p>
      {error !== '' && <p className="error" role="alert">{error}</p>}
      <div className="row end">
        <button onClick={() => onClose(null)}>{settings.isFirstRun ? 'Later' : 'Cancel'}</button>
        <button className="primary" onClick={() => void save()} disabled={model.trim() === '' || (kind !== 'anthropic' && baseUrl.trim() === '' && kind === 'local')}>Save</button>
      </div>
    </Dialog>
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

  return (
    <Dialog title="Disclosure of AI use" onClose={onClose}>
      <p>A register of what the AI was used for, built from the log. Check it and adapt it to your institution’s rules before you hand it in.</p>
      {open > 0 && <p className="hint">{open === 1 ? 'One AI input is' : `${open} AI inputs are`} not classified yet and will be listed as “not annotated”.</p>}
      <div className="row">
        <label>
          Language
          <select value={options.language} onChange={e => set({ language: e.target.value as 'en' | 'de' })}>
            <option value="en">English</option>
            <option value="de">Deutsch</option>
          </select>
        </label>
        <label>
          Format
          <select value={options.format} onChange={e => set({ format: e.target.value as 'md' | 'tex' })}>
            <option value="md">Markdown</option>
            <option value="tex">LaTeX</option>
          </select>
        </label>
      </div>
      <label className="check">
        <input type="checkbox" checked={options.withPrompts} onChange={e => set({ withPrompts: e.target.checked })} /> Also write the appendix with every prompt
      </label>
      <label className="check">
        <input type="checkbox" checked={options.withAnswers} disabled={!options.withPrompts} onChange={e => set({ withAnswers: e.target.checked })} /> Include the AI’s answers in the appendix
      </label>
      {error !== '' && <p className="error" role="alert">{error}</p>}
      {exported !== null && (
        <>
          <p className="muted">Written to:</p>
          <ul className="paths">
            {exported.paths.map(path => (
              <li key={path}><code>{path}</code></li>
            ))}
          </ul>
          <pre className="preview">{exported.register}</pre>
        </>
      )}
      <div className="row end">
        <button onClick={onClose}>Close</button>
        <button className="primary" onClick={() => void write()}>{exported === null ? 'Write the disclosure' : 'Write again'}</button>
      </div>
    </Dialog>
  )
}

const ENGINE_NAMES: Record<string, string> = { typst: 'Typst (comes with the editor)', xelatex: 'LaTeX (XeLaTeX)', lualatex: 'LaTeX (LuaLaTeX)', pdflatex: 'LaTeX (pdfLaTeX)', tectonic: 'LaTeX (Tectonic)' }

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
  const cannot = tools === undefined ? '' : !tools.hasPandoc ? 'pandoc was not found: it comes with the editor’s installer, so this copy is incomplete. Installing pandoc from pandoc.org also works.' : format === 'pdf' && engines.length === 0 ? 'A PDF needs a program that typesets it (Typst, or a LaTeX such as TeX Live, MacTeX or MiKTeX), and none was found. Word export works without.' : ''
  return (
    <Dialog title="Export the document" onClose={onClose}>
      <p>Makes a PDF or a Word file of the document as it is in the editor now. Citations like <code>[@smith2020]</code> become references. Your document is not changed.</p>
      <label>
        Format
        <select value={format} onChange={e => (setFormat(e.target.value as FileFormat), setMade(null))}>
          <option value="pdf">PDF</option>
          <option value="docx">Word (.docx)</option>
        </select>
      </label>
      {format === 'pdf' && engines.length > 0 && (
        <div className="row">
          <label>
            Paper
            <select value={pdf.paper} onChange={e => choose({ paper: e.target.value as PdfChoice['paper'] })}>
              <option value="a4">A4</option>
              <option value="letter">US Letter</option>
            </select>
          </label>
          {engines.length > 1 && (
            <label>
              Typeset with
              <select value={engine ?? ''} onChange={e => choose({ engine: e.target.value })}>
                {engines.map(name => <option key={name} value={name}>{ENGINE_NAMES[name] ?? name}</option>)}
              </select>
            </label>
          )}
        </div>
      )}
      {format === 'docx' && <p className="muted">Page size, fonts and styles of a Word file are set in Word.</p>}
      {tools !== undefined && (
        <p className="muted">
          {tools.hasZotero ? 'References come from your Zotero library by citekey (Better BibTeX), then from your bibliography files.' : 'Zotero with Better BibTeX is not running: references come from your bibliography files only.'}
        </p>
      )}
      {cannot !== '' && <p className="hint">{cannot}</p>}
      {error !== '' && <p className="error" role="alert">{error}</p>}
      {made !== null && (
        <div role="status">
          <p>Written to <code>{made.path}</code>.</p>
          <p className="muted">
            {made.cited === 0 ? 'The document cites nothing.' : `${made.cited} cited, ${made.fromZotero} of them from Zotero.`}
            {made.missing.length > 0 && ` Not found anywhere, and left as they are in the text: ${made.missing.map(key => `@${key}`).join(', ')}.`}
          </p>
        </div>
      )}
      <div className="actions">
        <button onClick={onClose}>Close</button>
        <button className="primary" disabled={setup === null || cannot !== '' || isWorking} onClick={() => void write()}>{isWorking ? 'Exporting…' : 'Export'}</button>
      </div>
    </Dialog>
  )
}
