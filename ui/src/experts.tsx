import { useState } from 'react'
import type { Expert, PartyView } from './api'

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
        <button onClick={() => onSave(text)} disabled={text.trim() === value.trim()}>Save</button>
        {text.trim() !== value.trim() && <span className="muted">not saved</span>}
      </div>
    </div>
  )
}

type Props = {
  party: PartyView
  canAsk: boolean
  onReview(expertId: string): void
  onBrief(key: string, body: string): void
  onMember(id: string, isMember: boolean): void
  onAdd(name: string, role: string, looksAt: string): Promise<boolean>
}

export function ExpertsPanel({ party, canAsk, onReview, onBrief, onMember, onAdd }: Props) {
  const [open, setOpen] = useState<string | null>(null)
  const [isAdding, setAdding] = useState(false)
  const [draft, setDraft] = useState({ name: '', role: '', looksAt: '' })
  // the party first, in its order; the experts not in it after
  const experts = [...party.experts.filter(e => e.isMember), ...party.experts.filter(e => !e.isMember)]

  return (
    <div className="experts">
      <section>
        <h3>About this document</h3>
        <p className="muted">What every expert should know: the field, the journal, the kind of text.</p>
        <Brief key={party.everyone} label="Briefing for every expert" hint="A field study for a psychology journal, British spelling…" value={party.everyone} onSave={body => onBrief('everyone', body)} />
      </section>
      <ul>
        {experts.map(expert => (
          <li key={expert.id} className={expert.isMember ? 'expert' : 'expert out'}>
            <Sprite expert={expert} shared={party.sharedPalette} />
            <div className="who">
              <strong>{expert.name}</strong>
              <span className="muted">{expert.role}</span>
              <div className="row">
                <button onClick={() => onReview(expert.id)} disabled={!canAsk} title={canAsk ? undefined : 'Set up the AI first, or wait for the answer in progress'}>Ask for a review</button>
                <button className="link" aria-expanded={open === expert.id} onClick={() => setOpen(open === expert.id ? null : expert.id)}>
                  {expert.brief === '' ? 'Brief' : 'Briefed'}
                </button>
                <label className="member">
                  <input type="checkbox" checked={expert.isMember} onChange={e => onMember(expert.id, e.target.checked)} aria-label={`${expert.name} is in the party`} /> in the party
                </label>
              </div>
              {open === expert.id && (
                <Brief key={expert.brief} label={`Briefing for ${expert.name}`} hint={`What ${expert.name} should look for in this document, or leave alone`} value={expert.brief} onSave={body => onBrief(expert.id, body)} />
              )}
            </div>
          </li>
        ))}
      </ul>
      <section>
        {!isAdding ? (
          <button onClick={() => setAdding(true)}>New expert…</button>
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
            <h3>New expert for this document</h3>
            <input aria-label="Name" placeholder="Name, for example The Health Economist" value={draft.name} onChange={e => setDraft({ ...draft, name: e.target.value })} />
            <input aria-label="Field" placeholder="Field, for example costs, QALYs, perspective of the analysis" value={draft.role} onChange={e => setDraft({ ...draft, role: e.target.value })} />
            <textarea aria-label="What they look at" rows={4} placeholder="What this expert looks at and asks: the questions of the field, what counts as evidence there, the mistakes outsiders make." value={draft.looksAt} onChange={e => setDraft({ ...draft, looksAt: e.target.value })} />
            <p className="muted">The expert works under the same rules as the others: hints and questions, never text for your manuscript.</p>
            <div className="row">
              <button className="primary" type="submit" disabled={draft.name.trim() === '' || draft.role.trim() === '' || draft.looksAt.trim() === ''}>Add</button>
              <button type="button" onClick={() => setAdding(false)}>Cancel</button>
            </div>
          </form>
        )}
      </section>
    </div>
  )
}
