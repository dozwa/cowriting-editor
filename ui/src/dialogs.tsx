import { type ReactNode, useEffect, useRef, useState } from 'react'
import type { Api, ExportOptions, Exported, Settings, SettingsView } from './api'

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

export function SettingsDialog({ api, settings, onClose }: { api: Api; settings: SettingsView; onClose(saved: SettingsView | null): void }) {
  const [model, setModel] = useState(settings.model)
  const [baseUrl, setBaseUrl] = useState(settings.baseUrl)
  const [key, setKey] = useState('')
  const [error, setError] = useState('')
  const isScripted = settings.backend.startsWith('fake:')

  const save = async () => {
    const next: Settings = { backend: isScripted ? 'anthropic' : settings.backend, model: model.trim(), baseUrl: baseUrl.trim() }
    try {
      await api.setSettings(next, key.trim() === '' ? null : key.trim())
      onClose({ ...settings, ...next, backend: settings.backend, hasKey: settings.hasKey || key.trim() !== '' })
    } catch (e) {
      setError(String(e))
    }
  }

  return (
    <Dialog title="Settings" onClose={() => onClose(null)}>
      {isScripted && <p className="hint">This run answers from a script, not a model. The settings below apply to normal runs.</p>}
      <label>
        Anthropic API key
        <input type="password" autoComplete="off" value={key} onChange={e => setKey(e.target.value)} placeholder={settings.hasKey ? 'A key is stored. Type a new one to replace it.' : 'sk-ant-…'} />
        <small>Kept in your system’s key store, not in a file. Your questions and your document are sent to the provider below when you ask.</small>
      </label>
      <label>
        Model
        <input value={model} onChange={e => setModel(e.target.value)} />
        <small>For example claude-opus-5-5, claude-sonnet-5-5 or claude-haiku-5-5. The disclosure names the model that answered.</small>
      </label>
      <label>
        Address of the API (optional)
        <input value={baseUrl} onChange={e => setBaseUrl(e.target.value)} placeholder="https://api.anthropic.com" />
        <small>Only for a gateway that speaks the Claude API, such as one your institution runs.</small>
      </label>
      {error !== '' && <p className="error" role="alert">{error}</p>}
      <div className="row end">
        <button onClick={() => onClose(null)}>Cancel</button>
        <button className="primary" onClick={() => void save()} disabled={model.trim() === ''}>Save</button>
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
