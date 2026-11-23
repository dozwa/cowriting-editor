import { useState } from 'react'
import type { LiveView, PartyView, ScopeProgress, ScopeView } from './api'
import { Sprite } from './experts'
import { plural, when } from './text'

type LiveProps = {
  view: LiveView
  party: PartyView
  status: string
  isReady: boolean
  onChange(isOn: boolean, personas: string[]): void
  onJump(line: number): void
  onOpen(turn: string): void
}

/** Live review: experts leave short hints on what the author just wrote. */
export function LivePanel({ view, party, status, isReady, onChange, onJump, onOpen }: LiveProps) {
  const members = party.experts.filter(e => e.isMember)
  const seconds = Math.round(view.pauseMs / 1000)
  return (
    <div className="live">
      <section>
        <label className="switch">
          <input type="checkbox" checked={view.isOn} disabled={!isReady && !view.isOn} onChange={e => onChange(e.target.checked, view.personas)} /> Live review
        </label>
        <p className="muted">
          While it is on, the paragraphs you change are sent to the AI once you pause for {seconds} seconds: at most one review a minute and 20 an hour. Each review is logged. The experts give hints; they do not write for you.
        </p>
        {status !== '' && <p className="state" role="status">{status}</p>}
      </section>
      <section>
        <h3>Hints</h3>
        {view.hints.length === 0 ? (
          <p className="muted">{view.isOn ? 'No hints yet. Keep writing.' : 'Turn live review on to get hints while you write.'}</p>
        ) : (
          <ul className="hints">
            {view.hints.map((hint, i) => {
              const expert = party.experts.find(e => e.id === hint.persona)
              return (
                <li key={`${hint.turn}-${i}`}>
                  {expert !== undefined && <Sprite expert={expert} shared={party.sharedPalette} size={24} />}
                  <div>
                    <span className="top">
                      <strong>{hint.who}</strong>
                      {hint.line > 0 && <button className="link" onClick={() => onJump(hint.line)}>line {hint.line}</button>}
                      <span className="muted">{when(hint.ts)}</span>
                    </span>
                    <p>{hint.text}</p>
                    <button className="link" onClick={() => onOpen(hint.turn)}>Classify or reply</button>
                  </div>
                </li>
              )
            })}
          </ul>
        )}
      </section>
      <section>
        <h3>Who watches</h3>
        <ul className="watchers">
          {members.map(expert => (
            <li key={expert.id}>
              <label>
                <input
                  type="checkbox"
                  checked={view.personas.includes(expert.id)}
                  onChange={e => onChange(view.isOn, e.target.checked ? [...view.personas, expert.id] : view.personas.filter(id => id !== expert.id))}
                />{' '}
                {expert.name} <span className="muted">{expert.role}</span>
              </label>
            </li>
          ))}
        </ul>
      </section>
    </div>
  )
}

const percent = (share: number) => `${Math.round(share * 100)} %`

function Meter({ label, done, target }: { label: string; done: number; target?: number }) {
  if (target === undefined) return null
  return (
    <li>
      <span>{label}</span>
      <progress aria-label={label} max={target} value={Math.min(done, target)} />
      <span className="muted">{done} of {target}</span>
    </li>
  )
}

const STATUS = { covered: '✓ covered', partial: '◐ partly', missing: '○ missing' } as const

/** The line a check's "file:line" points at. */
const lineOf = (where: string | undefined) => {
  const found = /:(\d+)\s*$/.exec(where ?? '')
  return found === null ? 0 : Number(found[1])
}

function Progress({ progress, onJump }: { progress: ScopeProgress; onJump(line: number): void }) {
  return (
    <>
      <section>
        <p className="overall">
          <strong>{percent(progress.overall)}</strong> of your plan
          {progress.daysLeft !== undefined && <span className="muted"> · {progress.daysLeft >= 0 ? `${plural(progress.daysLeft, 'day')} left` : `${plural(-progress.daysLeft, 'day')} past the deadline`}</span>}
          {progress.wordsPerDay !== undefined && progress.wordsPerDay > 0 && <span className="muted"> · {progress.wordsPerDay} words a day to get there</span>}
        </p>
        {progress.goal !== '' && <p className="muted">{progress.goal}</p>}
        <ul className="meters">
          <Meter label="Words" {...progress.words} />
          <Meter label="Figures" {...progress.figures} />
          <Meter label="Tables" {...progress.tables} />
          <Meter label="References" {...progress.references} />
        </ul>
        {progress.markers > 0 && <p className="muted">{plural(progress.markers, 'open marker')} (TODO, ??, UNVERIFIED…) in the text</p>}
      </section>
      {progress.sections.length > 0 && (
        <section>
          <h3>Sections</h3>
          <ul className="sections">
            {progress.sections.map(section => (
              <li key={section.name}>
                <span>{section.name}</span>
                <span className="muted">
                  {section.isFound ? `${section.words}${section.target !== undefined ? ` of ${section.target}` : ''} words` : 'no heading of that name yet'}
                  {section.stage !== undefined ? ` · ${section.stage}` : ''}
                  {section.markers > 0 ? ` · ${plural(section.markers, 'marker')}` : ''}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
      {progress.points.length > 0 && (
        <section>
          <h3>Points</h3>
          <ul className="points">
            {progress.points.map(point => {
              const line = lineOf(point.where)
              return (
                <li key={point.text}>
                  <span className={point.isTicked ? 'point-status covered' : `point-status ${point.status ?? 'open'}`}>{point.isTicked ? '✓ ticked by you' : point.status === undefined ? '· not checked' : STATUS[point.status]}</span>
                  <span>
                    {point.text}
                    {point.section !== undefined && <span className="muted"> ({point.section})</span>}
                  </span>
                  {(point.note !== undefined || line > 0) && (
                    <span className="muted">
                      {point.note}
                      {line > 0 && <> <button className="link" onClick={() => onJump(line)}>line {line}</button></>}
                    </span>
                  )}
                </li>
              )
            })}
          </ul>
          {progress.checkedAt !== undefined && <p className="muted">Last checked {when(progress.checkedAt)}. A check is the AI's reading, not a verdict.</p>}
        </section>
      )}
    </>
  )
}

type ScopeProps = {
  view: ScopeView
  canCheck: boolean
  isChecking: boolean
  note: string
  onSave(text: string): Promise<boolean>
  onCheck(): void
  onJump(line: number): void
}

/** The author's plan and how far the text has come. */
export function ScopePanel({ view, canCheck, isChecking, note, onSave, onCheck, onJump }: ScopeProps) {
  const [isEditing, setEditing] = useState(false)
  const [text, setText] = useState(view.text)
  const hasPoints = (view.progress?.points.length ?? 0) > 0
  const edit = () => {
    setText(view.text === '' ? view.template : view.text)
    setEditing(true)
  }

  if (isEditing) {
    return (
      <form
        className="scope editing"
        onSubmit={async e => {
          e.preventDefault()
          if (await onSave(text)) setEditing(false)
        }}
      >
        <p className="muted">Your plan, in plain text: a goal, targets, sections and the points the text must make. It is yours; the AI cannot change it.</p>
        <textarea aria-label="Your plan" rows={22} value={text} onChange={e => setText(e.target.value)} spellCheck={false} />
        <div className="row">
          <button className="primary" type="submit">Save the plan</button>
          <button type="button" onClick={() => setEditing(false)}>Cancel</button>
        </div>
      </form>
    )
  }
  return (
    <div className="scope">
      {view.progress === null ? (
        <section>
          <p>No plan yet. Say what the text should reach (words, sections, the points it must make) and see how far you are while you write.</p>
          <button className="primary" onClick={edit}>{view.text === '' ? 'Make a plan' : 'Edit the plan'}</button>
        </section>
      ) : (
        <>
          <Progress progress={view.progress} onJump={onJump} />
          <section>
            <div className="row">
              <button onClick={edit}>Edit the plan</button>
              <button onClick={onCheck} disabled={!canCheck || !hasPoints || isChecking} title={hasPoints ? undefined : 'Your plan has no points yet'}>{isChecking ? 'Checking…' : 'Check the points'}</button>
            </div>
            <p className="muted">The check sends your plan's points and the document to the AI and asks which points the text makes. It is logged.</p>
            {note !== '' && <p className="state" role="status">{note}</p>}
          </section>
        </>
      )}
    </div>
  )
}
