import { useState } from 'react'
import type { Expert, LinkedFile, PartyView } from './api'
import { m } from './i18n'

/** An expert's figure: one square per letter of its sprite, in its own colours or the shared ones. */
export function Sprite({ expert, shared, size = 36 }: { expert: Pick<Expert, 'sprite' | 'palette' | 'name'>; shared: Record<string, string>; size?: number }) {
  const width = expert.sprite[0]?.length ?? 0
  const height = expert.sprite.length
  if (width === 0) return null
  return (
    <svg className="sprite" viewBox={`0 0 ${width} ${height}`} width={size} height={(size * height) / width} shapeRendering="crispEdges" role="img" aria-label={expert.name}>
      {expert.sprite.flatMap((row, y) =>
        [...row].map((letter, x) => {
          const colour = expert.palette[letter] ?? shared[letter]
          return letter === '.' || colour === undefined ? null : <rect key={`${x}-${y}`} x={x} y={y} width={1} height={1} fill={colour} />
        }),
      )}
    </svg>
  )
}

/** A briefing the author edits and saves. */
function Brief({ label, hint, value, onSave }: { label: string; hint: string; value: string; onSave(body: string): void }) {
  const [text, setText] = useState(value)
  return (
    <div className="brief">
      <textarea aria-label={label} placeholder={hint} rows={3} value={text} onChange={e => setText(e.target.value)} />
      <div className="row">
        <button onClick={() => onSave(text)} disabled={text.trim() === value.trim()}>{m.experts.save}</button>
        {text.trim() !== value.trim() && <span className="muted">{m.experts.notSaved}</span>}
      </div>
    </div>
  )
}

/** What an expert is told, whole, for the author to reword. */
function Prompt({ expert, onSave }: { expert: Expert; onSave(prompt: string): void }) {
  const [text, setText] = useState(expert.prompt)
  return (
    <div className="brief prompt">
      <textarea aria-label={m.experts.promptFor(expert.name)} rows={10} value={text} onChange={e => setText(e.target.value)} />
      <p className="muted">{m.experts.promptNote}</p>
      <div className="row">
        <button onClick={() => onSave(text)} disabled={text.trim() === expert.prompt.trim() || text.trim() === ''}>{m.experts.save}</button>
        {expert.isPromptEdited && <button className="link" onClick={() => onSave('')}>{m.experts.backToStandard}</button>}
        {text.trim() !== expert.prompt.trim() && <span className="muted">{m.experts.notSaved}</span>}
      </div>
    </div>
  )
}

/** The author's files the AI is given as background, and whom each is for. */
function Files({ files, experts, onLink, onFor, onUnlink }: { files: LinkedFile[]; experts: Expert[]; onLink(): void; onFor(path: string, personas: string[]): void; onUnlink(path: string): void }) {
  const [open, setOpen] = useState<string | null>(null)
  const nameOf = (id: string) => experts.find(e => e.id === id)?.name ?? id
  return (
    <section className="files">
      <h3>{m.experts.files}</h3>
      <p className="muted">{m.experts.filesHint}</p>
      {files.length === 0 ? (
        <p className="muted">{m.experts.noFiles}</p>
      ) : (
        <ul>
          {files.map(file => (
            <li key={file.path}>
              <div className="row">
                <strong title={file.path}>{file.name}</strong>
                {file.isMissing && <span className="flag">{m.experts.fileMissing}</span>}
                {file.isCut && <span className="flag">{m.experts.fileCut}</span>}
                {file.isConverted && !file.isMissing && <span className="muted">{m.experts.fileConverted}</span>}
                <span className="spacer" />
                <button className="link" aria-expanded={open === file.path} aria-label={m.experts.fileFor(file.name)} onClick={() => setOpen(open === file.path ? null : file.path)}>
                  {file.personas.length === 0 ? m.experts.forEveryone : m.experts.forThese(file.personas.map(nameOf).join(', '))}
                </button>
                <button className="link" aria-label={m.experts.unlinkFile(file.name)} onClick={() => onUnlink(file.path)}>{m.experts.unlink}</button>
              </div>
              {open === file.path && (
                <div className="for" role="group" aria-label={m.experts.fileFor(file.name)}>
                  {experts.map(expert => (
                    <label key={expert.id}>
                      <input type="checkbox" checked={file.personas.includes(expert.id)} onChange={e => onFor(file.path, e.target.checked ? [...file.personas, expert.id] : file.personas.filter(id => id !== expert.id))} /> {expert.name}
                    </label>
                  ))}
                  <p className="muted">{m.experts.forEveryoneHint}</p>
                </div>
              )}
            </li>
          ))}
        </ul>
      )}
      <button onClick={onLink}>{m.experts.linkFile}</button>
    </section>
  )
}

type Props = {
  party: PartyView
  files: LinkedFile[]
  onLink(): void
  onFileFor(path: string, personas: string[]): void
  onUnlink(path: string): void
  onPrompt(id: string, prompt: string): void
  canAsk: boolean
  onReview(expertId: string): void
  onBrief(key: string, body: string): void
  onMember(id: string, isMember: boolean): void
  onAdd(name: string, role: string, looksAt: string): Promise<boolean>
}

export function ExpertsPanel({ party, files, canAsk, onReview, onBrief, onMember, onAdd, onLink, onFileFor, onUnlink, onPrompt }: Props) {
  const [open, setOpen] = useState<string | null>(null)
  const [prompting, setPrompting] = useState<string | null>(null)
  const [isAdding, setAdding] = useState(false)
  const [draft, setDraft] = useState({ name: '', role: '', looksAt: '' })
  // the party first, in its order; the experts not in it after
  const experts = [...party.experts.filter(e => e.isMember), ...party.experts.filter(e => !e.isMember)]

  return (
    <div className="experts">
      <section>
        <h3>{m.experts.aboutDocument}</h3>
        <p className="muted">{m.experts.aboutDocumentHint}</p>
        <Brief key={party.everyone} label={m.experts.briefingForAll} hint={m.experts.briefingForAllExample} value={party.everyone} onSave={body => onBrief('everyone', body)} />
      </section>
      <Files files={files} experts={party.experts.filter(e => e.isMember || files.some(file => file.personas.includes(e.id)))} onLink={onLink} onFor={onFileFor} onUnlink={onUnlink} />
      <ul>
        {experts.map(expert => (
          <li key={expert.id} className={expert.isMember ? 'expert' : 'expert out'}>
            <Sprite expert={expert} shared={party.sharedPalette} />
            <div className="who">
              <strong>{expert.name}</strong>
              <span className="muted">{expert.role}</span>
              <div className="row">
                <button onClick={() => onReview(expert.id)} disabled={!canAsk} title={canAsk ? undefined : m.experts.cannotAskNow}>{m.experts.askForReview}</button>
                <button className="link" aria-expanded={open === expert.id} onClick={() => (setOpen(open === expert.id ? null : expert.id), setPrompting(null))}>
                  {expert.brief === '' ? m.experts.brief : m.experts.briefed}
                </button>
                <button className="link" aria-expanded={prompting === expert.id} onClick={() => (setPrompting(prompting === expert.id ? null : expert.id), setOpen(null))}>
                  {expert.isPromptEdited ? m.experts.promptEdited : m.experts.prompt}
                </button>
                <label className="member">
                  <input type="checkbox" checked={expert.isMember} onChange={e => onMember(expert.id, e.target.checked)} aria-label={m.experts.isInParty(expert.name)} /> {m.experts.inParty}
                </label>
              </div>
              {open === expert.id && (
                <Brief key={expert.brief} label={m.experts.briefingFor(expert.name)} hint={m.experts.briefingForHint(expert.name)} value={expert.brief} onSave={body => onBrief(expert.id, body)} />
              )}
              {prompting === expert.id && <Prompt key={expert.prompt} expert={expert} onSave={prompt => onPrompt(expert.id, prompt)} />}
            </div>
          </li>
        ))}
      </ul>
      <section>
        {!isAdding ? (
          <button onClick={() => setAdding(true)}>{m.experts.newExpert}</button>
        ) : (
          <form
            className="new-expert"
            onSubmit={async e => {
              e.preventDefault()
              if (await onAdd(draft.name, draft.role, draft.looksAt)) {
                setDraft({ name: '', role: '', looksAt: '' })
                setAdding(false)
              }
            }}
          >
            <h3>{m.experts.newExpertHeading}</h3>
            <input aria-label={m.experts.name} placeholder={m.experts.nameExample} value={draft.name} onChange={e => setDraft({ ...draft, name: e.target.value })} />
            <input aria-label={m.experts.field} placeholder={m.experts.fieldExample} value={draft.role} onChange={e => setDraft({ ...draft, role: e.target.value })} />
            <textarea aria-label={m.experts.looksAt} rows={4} placeholder={m.experts.looksAtExample} value={draft.looksAt} onChange={e => setDraft({ ...draft, looksAt: e.target.value })} />
            <p className="muted">{m.experts.sameRules}</p>
            <div className="row">
              <button className="primary" type="submit" disabled={draft.name.trim() === '' || draft.role.trim() === '' || draft.looksAt.trim() === ''}>{m.experts.add}</button>
              <button type="button" onClick={() => setAdding(false)}>{m.experts.cancel}</button>
            </div>
          </form>
        )}
      </section>
    </div>
  )
}
