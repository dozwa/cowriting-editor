import { useEffect, useRef, useState } from 'react'
import Markdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import type { AiTextRow, ExternalUse, LogView, PartyView, ThreadMessage, TurnRow, Use } from './api'
import { Dialog } from './dialogs'
import { Sprite } from './experts'
import type { Live } from './App'
import { m } from './i18n'
import { linkLines, USE_LABELS, when } from './text'

/** An AI answer as markdown; mentions of a line jump there in the editor. Raw HTML in an answer is shown as text, never run. */
function Answer({ text, onJump }: { text: string; onJump(line: number): void }) {
  return (
    <div className="answer">
      <Markdown
        remarkPlugins={[remarkGfm]}
        components={{
          a({ href, children }) {
            const line = /^#line-(\d+)$/.exec(href ?? '')
            if (line !== null) return <button className="link" onClick={() => onJump(Number(line[1]))}>{children}</button>
            // the window opens no web pages: an address is shown, not followed
            return <span className="address" title={href}>{children}</span>
          },
        }}
      >
        {linkLines(text)}
      </Markdown>
    </div>
  )
}

/** The four choices under an AI input, or the choice the author made. */
function Classify({ turn, onAnnotate }: { turn: TurnRow; onAnnotate(id: string, use: Use, note: string): void }) {
  const [note, setNote] = useState(turn.note)
  // a choice can be made again: the log keeps both, and the later one counts
  const [isChanging, setChanging] = useState(false)
  if (turn.use !== '' && !isChanging) {
    return (
      <p className="classified">
        {USE_LABELS[turn.use].button} <span className="muted">· {USE_LABELS[turn.use].meaning}{turn.note !== '' ? ` · ${m.panels.quoted(turn.note)}` : ''}</span>{' '}
        <button className="link" aria-label={m.panels.changeChoiceOf(USE_LABELS[turn.use].button)} onClick={() => setChanging(true)}>{m.panels.changeChoice}</button>
      </p>
    )
  }
  return (
    <div className="classify">
      <p className="muted">
        {m.panels.whatDidYouDo} {isChanging && <button className="link" onClick={() => setChanging(false)}>{m.panels.keepChoice}</button>}
      </p>
      <div className="row">
        {(Object.keys(USE_LABELS) as Use[]).map(use => (
          <button key={use} title={USE_LABELS[use].meaning} onClick={() => onAnnotate(turn.id, use, note)}>
            {USE_LABELS[use].button}
          </button>
        ))}
      </div>
      <input aria-label={m.panels.commentOnInput} placeholder={m.panels.commentPlaceholder} value={note} onChange={e => setNote(e.target.value)} />
    </div>
  )
}

type AskProps = {
  thread: ThreadMessage[]
  live: Live | null
  turns: TurnRow[]
  isReady: boolean
  /** Said above the question field: an untested model, a model without tools. */
  notice: string
  party: PartyView | null
  onAsk(text: string, replyTo: string | null, persona: string | null, command?: string): void
  onAnnotate(id: string, use: Use, note: string): void
  onJump(line: number): void
  onNew(): void
  /** Ends the answer that is coming in. */
  onStop(): void
  onSettings(): void
}

/**
 * The ready-made requests; what each tells the AI is in the core (`crates/core/src/actions.rs`).
 * The texts are read when they are used, so they follow the language. Explain has no question of its own.
 */
export const ACTIONS: { id: string; label: string; hint: string; request: string }[] = [
  { id: 'feedback', get label() { return m.panels.feedback }, get hint() { return m.panels.feedbackHint }, get request() { return m.panels.feedbackRequest } },
  { id: 'socratic', get label() { return m.panels.socratic }, get hint() { return m.panels.socraticHint }, get request() { return m.panels.socraticRequest } },
  { id: 'structure', get label() { return m.panels.structure }, get hint() { return m.panels.structureHint }, get request() { return m.panels.structureRequest } },
  { id: 'language', get label() { return m.panels.language }, get hint() { return m.panels.languageHint }, get request() { return m.panels.languageRequest } },
  { id: 'explain', get label() { return m.panels.explain }, get hint() { return m.panels.explainHint }, request: '' },
  { id: 'consistency', get label() { return m.panels.consistency }, get hint() { return m.panels.consistencyHint }, get request() { return m.panels.consistencyRequest } },
]

export function AskPanel({ thread, live, turns, isReady, notice, party, onAsk, onAnnotate, onJump, onNew, onStop, onSettings }: AskProps) {
  const [text, setText] = useState('')
  const end = useRef<HTMLDivElement>(null)
  const byId = new Map(turns.map(t => [t.id, t]))
  const last = thread[thread.length - 1]
  const isReply = thread.length > 0
  const [asked, setAsked] = useState('')
  const expertOf = (id: string) => party?.experts.find(e => e.id === id)
  // a reply goes to whoever spoke last in the thread
  const inThread = [...thread].reverse().find(message => message.role === 'ai')?.persona ?? ''
  const to = isReply ? inThread : asked
  const toName = to === '' ? '' : (expertOf(to)?.name ?? to)

  useEffect(() => {
    end.current?.scrollIntoView?.({ block: 'end' })
    // also once the log knows the new turn: its choices appear under the answer
  }, [thread, live?.text, turns.length])

  const send = () => {
    const question = text.trim()
    if (question === '' || live !== null) return
    onAsk(question, isReply && last !== undefined ? last.id : null, to === '' ? null : to)
    setText('')
  }

  /** An action button: what is typed narrows it; with nothing typed it asks its own question. */
  const start = (action: (typeof ACTIONS)[number]) => {
    const typed = text.trim()
    if (live !== null || (typed === '' && action.request === '')) return
    onAsk(typed === '' ? action.request : typed, null, null, action.id)
    setText('')
  }

  return (
    <div className="ask">
      <div className="messages">
        {thread.length === 0 && live === null && (
          <div className="hint">
            <p>{m.panels.askHint}</p>
            <p className="muted">{m.panels.askRules}</p>
          </div>
        )}
        {thread.map((message, i) => {
          const turn = byId.get(message.id)
          // one turn can be two messages (a reply and its answer): the choices go under the answer
          const isAnswer = message.role === 'ai'
          return (
            <article key={`${message.id}-${i}`} className={message.role}>
              <h3>
                {isAnswer && expertOf(message.persona) !== undefined && party !== null && <Sprite expert={expertOf(message.persona)!} shared={party.sharedPalette} size={20} />}
                {message.role === 'author' ? m.panels.you : message.persona === '' ? m.panels.assistant : (expertOf(message.persona)?.name ?? message.who)} <span className="muted">{when(message.ts)}</span>
                {isAnswer && turn?.isAiText && <span className="flag" title={m.panels.aiTextInAnswer}>{m.panels.aiText}</span>}
              </h3>
              {isAnswer ? <Answer text={message.text} onJump={onJump} /> : <p className="said">{message.text}</p>}
              {isAnswer && turn !== undefined && <Classify key={turn.id + turn.use} turn={turn} onAnnotate={onAnnotate} />}
            </article>
          )
        })}
        {live !== null && (
          <>
            <article className="author">
              <h3>{m.panels.you}</h3>
              <p className="said">{live.asked}</p>
            </article>
            <article className="ai" aria-busy="true">
              <h3>{live.who}</h3>
              {live.steps.map((step, i) => (
                <p key={i} className="step">{step}</p>
              ))}
              {live.text === '' ? <p className="muted">{m.panels.thinking}</p> : <Answer text={live.text} onJump={onJump} />}
            </article>
          </>
        )}
        <div ref={end} />
      </div>
      {!isReady ? (
        <p className="hint">
          {m.panels.setUpFirst(<button className="link" onClick={onSettings}>{m.panels.openSettings}</button>)}
        </p>
      ) : (
        <form
          className="composer"
          onSubmit={e => {
            e.preventDefault()
            send()
          }}
        >
          {notice !== '' && <p className="notice" role="note">{notice}</p>}
          <textarea
            aria-label={isReply ? m.panels.replyField : m.panels.askField}
            placeholder={isReply ? m.panels.replyPlaceholder(toName) : m.panels.askPlaceholder(toName)}
            rows={3}
            value={text}
            onChange={e => setText(e.target.value)}
            onKeyDown={e => {
              if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) send()
            }}
          />
          {!isReply && to === '' && (
            <div className="actions" role="group" aria-label={m.panels.readyMade}>
              {ACTIONS.map(action => (
                <button key={action.id} type="button" title={action.hint} disabled={live !== null || (action.request === '' && text.trim() === '')} onClick={() => start(action)}>
                  {action.label}
                </button>
              ))}
            </div>
          )}
          <div className="row">
            <button className="primary" type="submit" disabled={live !== null || text.trim() === ''}>
              {live !== null ? m.panels.answering : isReply ? m.panels.reply : m.panels.ask}
            </button>
            {live !== null && <button type="button" onClick={onStop}>{m.panels.stop}</button>}
            {isReply && live === null && <button type="button" onClick={onNew}>{m.panels.newQuestion}</button>}
            {!isReply && party !== null && (
              <select aria-label={m.panels.whoToAsk} value={asked} onChange={e => setAsked(e.target.value)}>
                <option value="">{m.panels.assistant}</option>
                {party.experts.filter(e => e.isMember).map(e => (
                  <option key={e.id} value={e.id}>{e.name}</option>
                ))}
              </select>
            )}
            <span className="spacer" />
            <span className="muted">{m.panels.whatIsSent}</span>
          </div>
        </form>
      )}
    </div>
  )
}

/** One passage of AI text in the document, with the author's comment on it. */
function AiTextItem({ row, onOpen, onJump, onComment }: { row: AiTextRow; onOpen(id: string): void; onJump(line: number): void; onComment(id: string, note: string): void }) {
  const [note, setNote] = useState(row.note)
  return (
    <li>
      <span className="top">
        <span className="flag">{row.kind === 'correction' ? m.panels.languageCorrection : m.panels.aiText}</span>
        <span>
          {row.source === 'paste' ? m.panels.pasted : m.panels.typed} · {m.panels.words(row.words)} · {row.match === 'exact' ? m.panels.wordForWord : m.panels.closeToAnswer}
        </span>
        <span className="muted">{when(row.ts)}</span>
      </span>
      <span className="quote">{m.panels.quoted(row.excerpt)}</span>
      <span className="muted">
        {row.stillInText ? m.panels.stillInText : m.panels.noLongerInText}
        {row.stillInText && row.line > 0 && (
          <>
            {' · '}
            <button className="link" onClick={() => onJump(row.line)}>{m.panels.line(row.line)}</button>
          </>
        )}
        {' · '}
        {m.panels.fromAnswerTo(<button className="link" onClick={() => onOpen(row.turn)}>{m.panels.quoted(row.asked)}</button>)}
      </span>
      <span className="row">
        <input aria-label={m.panels.commentOnPassage(row.excerpt.slice(0, 40))} placeholder={m.panels.commentForDisclosure} value={note} onChange={e => setNote(e.target.value)} />
        <button onClick={() => onComment(row.id, note)} disabled={note.trim() === row.note.trim()}>{m.panels.save}</button>
      </span>
    </li>
  )
}

const NOTHING_STATED: ExternalUse = { id: '', tool: '', model: '', date: '', purpose: '', material: '', result: '', checked: '' }

/** The form for AI use outside the editor: the tool and what it was for, and the rest only where it matters. */
function ExternalDialog({ start, onSave, onClose }: { start: ExternalUse; onSave(stated: ExternalUse): Promise<boolean>; onClose(): void }) {
  const [stated, setStated] = useState(start)
  const t = m.panels
  const field = (key: keyof ExternalUse, label: string, example = '', rows = 0) => (
    <label>
      {label}
      {rows > 0 ? <textarea rows={rows} value={stated[key]} placeholder={example} onChange={e => setStated({ ...stated, [key]: e.target.value })} /> : <input value={stated[key]} placeholder={example} onChange={e => setStated({ ...stated, [key]: e.target.value })} />}
    </label>
  )
  return (
    <Dialog title={start.id === '' ? t.externalTitle : t.externalEditTitle} onClose={onClose}>
      <p>{t.externalIntro}</p>
      <div className="row">
        {field('tool', t.externalTool, t.externalToolExample)}
        {field('model', t.externalModel)}
      </div>
      {field('purpose', t.externalPurpose, t.externalPurposeExample, 2)}
      <label>
        {t.externalDate}
        <input value={stated.date} placeholder="2026-11-24" onChange={e => setStated({ ...stated, date: e.target.value })} />
        <small>{t.externalDateHint}</small>
      </label>
      {field('material', t.externalMaterial, t.externalMaterialExample, 2)}
      {field('result', t.externalResult, '', 2)}
      {field('checked', t.externalChecked, '', 2)}
      <div className="row end">
        <button onClick={onClose}>{m.dialogs.close}</button>
        <button className="primary" disabled={stated.tool.trim() === '' || stated.purpose.trim() === ''} onClick={async () => (await onSave(stated)) && onClose()}>{start.id === '' ? t.externalSave : t.externalSaveEdit}</button>
      </div>
    </Dialog>
  )
}

/** What the author stated about other AI tools, and the way to state more. */
function External({ rows, onSave, onWithdraw }: { rows: ExternalUse[]; onSave(stated: ExternalUse): Promise<boolean>; onWithdraw(id: string): void }) {
  const [editing, setEditing] = useState<ExternalUse | null>(null)
  const t = m.panels
  return (
    <section className="external">
      <h3>{t.externalHeading}</h3>
      <p className="muted">{t.externalExplained}</p>
      {rows.length > 0 && (
        <ul>
          {rows.map(row => (
            <li key={row.id}>
              <div className="top">
                <strong>{row.tool}</strong>
                {row.model !== '' && <span className="muted">{row.model}</span>}
                {row.date !== '' && <span className="muted">{row.date}</span>}
              </div>
              <p>{row.purpose}</p>
              {row.material !== '' && <p className="muted">{t.externalMaterialLabel} {row.material}</p>}
              {row.result !== '' && <p className="muted">{t.externalResultLabel} {row.result}</p>}
              {row.checked !== '' && <p className="muted">{t.externalCheckedLabel} {row.checked}</p>}
              <div className="row">
                <button className="link" aria-label={t.externalEditFor(row.tool)} onClick={() => setEditing(row)}>{t.externalEdit}</button>
                <button className="link" aria-label={t.externalWithdrawFor(row.tool)} onClick={() => onWithdraw(row.id)}>{t.externalWithdraw}</button>
              </div>
            </li>
          ))}
        </ul>
      )}
      <button onClick={() => setEditing(NOTHING_STATED)}>{t.externalAdd}</button>
      {rows.length > 0 && <p className="muted">{t.externalKept}</p>}
      {editing !== null && <ExternalDialog start={editing} onSave={onSave} onClose={() => setEditing(null)} />}
    </section>
  )
}

type LogProps = { log: LogView; onOpen(id: string): void; onJump(line: number): void; onComment(id: string, note: string): void; onExternal(stated: ExternalUse): Promise<boolean>; onWithdraw(id: string): void }

export function LogPanel({ log, onOpen, onJump, onComment, onExternal, onWithdraw }: LogProps) {
  const { stats } = log
  const external = <External rows={log.external} onSave={onExternal} onWithdraw={onWithdraw} />
  if (log.turns.length === 0 && log.guard.length === 0) {
    return (
      <div className="log">
        <p className="hint">{m.panels.nothingLogged}</p>
        {external}
      </div>
    )
  }
  // what waits for the author comes first
  const turns = [...log.turns.filter(t => t.isOpen), ...log.turns.filter(t => !t.isOpen)]
  return (
    <div className="log">
      <p className="summary">
        {m.panels.summary(stats.turns, stats.unannotated, stats.aiText, log.aiText.length)}
      </p>
      <ul>
        {turns.map(turn => (
          <li key={turn.id}>
            <button className="turn" onClick={() => onOpen(turn.id)}>
              <span className="top">
                <span className={turn.isOpen ? 'use open' : 'use'}>{turn.use === '' ? (turn.isOpen ? m.panels.toClassify : m.panels.noChoiceNeeded) : USE_LABELS[turn.use].button}</span>
                {turn.isAiText && <span className="flag">{m.panels.aiText}</span>}
                {turn.who !== '' && <span className="by">{turn.who}</span>}
                <span className="muted">{when(turn.ts)}</span>
              </span>
              <span className="asked">{turn.replyTo !== null ? '↳ ' : ''}{turn.asked}</span>
              <span className="muted excerpt">{turn.excerpt}</span>
            </button>
          </li>
        ))}
      </ul>
      {log.aiText.length > 0 && (
        <>
          <h3>{m.panels.aiTextHeading}</h3>
          <p className="muted">{m.panels.aiTextExplained}</p>
          <ul className="ai-text">
            {log.aiText.map(row => (
              <AiTextItem key={`${row.id}-${row.note}`} row={row} onOpen={onOpen} onJump={onJump} onComment={onComment} />
            ))}
          </ul>
        </>
      )}
      {log.guard.length > 0 && (
        <>
          <h3>{m.panels.refusedHeading}</h3>
          <ul className="guard">
            {log.guard.map((row, i) => (
              <li key={i}>
                <span className="muted">{when(row.ts)}</span> {row.refused ? row.input : `${row.tool}: ${row.files.join(', ')}`}
              </li>
            ))}
          </ul>
          <p className="muted">{m.panels.nothingChanged}</p>
        </>
      )}
      {external}
    </div>
  )
}
