import { useState } from 'react'
import type { LiveView, PartyView, ScopeProgress, ScopeView } from './api'
import { Sprite } from './experts'
import { language, m } from './i18n'
import { when } from './text'

type LiveProps = {
  view: LiveView
  party: PartyView
  status: string
  isReady: boolean
  onChange(isOn: boolean, personas: string[]): void
  onJump(line: number): void
  onOpen(turn: string): void
}

/** “minute”, “90 seconds”, “5 minutes”: a time between reviews, in the words that follow “every”. */
export const span = (ms: number) => m.plan.span(ms)

/** Live review: experts leave short hints on what the author just wrote. */
export function LivePanel({ view, party, status, isReady, onChange, onJump, onOpen }: LiveProps) {
  const members = party.experts.filter(e => e.isMember)
  const seconds = Math.round(view.pauseMs / 1000)
  return (
    <div className="live">
      <section>
        <label className="switch">
          <input type="checkbox" checked={view.isOn} disabled={!isReady && !view.isOn} onChange={e => onChange(e.target.checked, view.personas)} /> {m.plan.liveReview}
        </label>
        <p className="muted">
          {m.plan.liveExplained(seconds, span(view.gapMs), view.perHour)}
        </p>
        {status !== '' && <p className="state" role="status">{status}</p>}
      </section>
      <section>
        <h3>{m.plan.hints}</h3>
        {view.hints.length === 0 ? (
          <p className="muted">{view.isOn ? m.plan.noHintsYet : m.plan.turnOnForHints}</p>
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
                      {hint.line > 0 && <button className="link" onClick={() => onJump(hint.line)}>{m.plan.line(hint.line)}</button>}
                      <span className="muted">{when(hint.ts)}</span>
                    </span>
                    <p>{hint.text}</p>
                    <button className="link" onClick={() => onOpen(hint.turn)}>{m.plan.classifyOrReply}</button>
                  </div>
                </li>
              )
            })}
          </ul>
        )}
      </section>
      <section>
        <h3>{m.plan.whoWatches}</h3>
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
      <span className="muted">{m.plan.doneOf(done, target)}</span>
    </li>
  )
}

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
          {m.plan.ofYourPlan(<strong>{percent(progress.overall)}</strong>)}
          {progress.daysLeft !== undefined && <span className="muted"> · {progress.daysLeft >= 0 ? m.plan.daysLeft(progress.daysLeft) : m.plan.daysPast(-progress.daysLeft)}</span>}
          {progress.wordsPerDay !== undefined && progress.wordsPerDay > 0 && <span className="muted"> · {m.plan.wordsPerDay(progress.wordsPerDay)}</span>}
        </p>
        {progress.goal !== '' && <p className="muted">{progress.goal}</p>}
        <ul className="meters">
          <Meter label={m.plan.words} {...progress.words} />
          <Meter label={m.plan.figures} {...progress.figures} />
          <Meter label={m.plan.tables} {...progress.tables} />
          <Meter label={m.plan.references} {...progress.references} />
        </ul>
        {progress.markers > 0 && <p className="muted">{m.plan.openMarkers(progress.markers)}</p>}
      </section>
      {progress.sections.length > 0 && (
        <section>
          <h3>{m.plan.sections}</h3>
          <ul className="sections">
            {progress.sections.map(section => (
              <li key={section.name}>
                <span>{section.name}</span>
                <span className="muted">
                  {section.isFound ? m.plan.sectionWords(section.words, section.target) : m.plan.noHeadingYet}
                  {section.stage !== undefined ? ` · ${section.stage}` : ''}
                  {section.markers > 0 ? ` · ${m.plan.markers(section.markers)}` : ''}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
      {progress.points.length > 0 && (
        <section>
          <h3>{m.plan.points}</h3>
          <ul className="points">
            {progress.points.map(point => {
              const line = lineOf(point.where)
              return (
                <li key={point.text}>
                  <span className={point.isTicked ? 'point-status covered' : `point-status ${point.status ?? 'open'}`}>{point.isTicked ? m.plan.tickedByYou : point.status === undefined ? m.plan.notChecked : m.plan[point.status]}</span>
                  <span>
                    {point.text}
                    {point.section !== undefined && <span className="muted"> ({point.section})</span>}
                  </span>
                  {(point.note !== undefined || line > 0) && (
                    <span className="muted">
                      {point.note}
                      {line > 0 && <> <button className="link" onClick={() => onJump(line)}>{m.plan.line(line)}</button></>}
                    </span>
                  )}
                </li>
              )
            })}
          </ul>
          {progress.checkedAt !== undefined && <p className="muted">{m.plan.lastChecked(when(progress.checkedAt))}</p>}
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
    setText(view.text !== '' ? view.text : language === 'de' ? view.templateDe : view.template)
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
        <p className="muted">{m.plan.planExplained}</p>
        <dl className="how">
          {m.plan.howTo.map(([part, how]) => (
            <div key={part}>
              <dt>{part}</dt>
              <dd>{how}</dd>
            </div>
          ))}
        </dl>
        <textarea aria-label={m.plan.yourPlan} rows={18} value={text} onChange={e => setText(e.target.value)} spellCheck={false} />
        <div className="row">
          <button className="primary" type="submit">{m.plan.savePlan}</button>
          <button type="button" onClick={() => setEditing(false)}>{m.plan.cancel}</button>
        </div>
      </form>
    )
  }
  return (
    <div className="scope">
      {view.progress === null ? (
        <section>
          <p>{m.plan.noPlanYet}</p>
          <button className="primary" onClick={edit}>{view.text === '' ? m.plan.makePlan : m.plan.editPlan}</button>
        </section>
      ) : (
        <>
          <Progress progress={view.progress} onJump={onJump} />
          <section>
            <div className="row">
              <button onClick={edit}>{m.plan.editPlan}</button>
              <button onClick={onCheck} disabled={!canCheck || !hasPoints || isChecking} title={hasPoints ? undefined : m.plan.noPointsYet}>{isChecking ? m.plan.checking : m.plan.checkPoints}</button>
            </div>
            <p className="muted">{m.plan.checkExplained}</p>
            {note !== '' && <p className="state" role="status">{note}</p>}
          </section>
        </>
      )}
    </div>
  )
}
