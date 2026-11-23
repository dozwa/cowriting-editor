import { useEffect, useRef, useState } from 'react'
import Markdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import type { AiTextRow, LogView, PartyView, ThreadMessage, TurnRow, Use } from './api'
import { Sprite } from './experts'
import type { Live } from './App'
import { linkLines, plural, USE_LABELS, when } from './text'

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
  const [note, setNote] = useState('')
  if (turn.use !== '') {
    return (
      <p className="classified">
        {USE_LABELS[turn.use].button} <span className="muted">· {USE_LABELS[turn.use].meaning}{turn.note !== '' ? ` · “${turn.note}”` : ''}</span>
      </p>
    )
  }
  return (
    <div className="classify">
      <p className="muted">What did you do with this? It goes into your disclosure.</p>
      <div className="row">
        {(Object.keys(USE_LABELS) as Use[]).map(use => (
          <button key={use} title={USE_LABELS[use].meaning} onClick={() => onAnnotate(turn.id, use, note)}>
            {USE_LABELS[use].button}
          </button>
        ))}
      </div>
      <input aria-label="Comment on this input" placeholder="Comment (optional)" value={note} onChange={e => setNote(e.target.value)} />
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
  onAsk(text: string, replyTo: string | null, persona: string | null): void
  onAnnotate(id: string, use: Use, note: string): void
  onJump(line: number): void
  onNew(): void
  onSettings(): void
}

export function AskPanel({ thread, live, turns, isReady, notice, party, onAsk, onAnnotate, onJump, onNew, onSettings }: AskProps) {
  const [text, setText] = useState('')
  const end = useRef<HTMLDivElement>(null)
  const byId = new Map(turns.map(t => [t.id, t]))
  const last = thread[thread.length - 1]
  const isReply = thread.length > 0
  const [asked, setAsked] = useState('')
  const expertOf = (id: string) => party?.experts.find(e => e.id === id)
  // a reply goes to whoever spoke last in the thread
  const inThread = [...thread].reverse().find(m => m.role === 'ai')?.persona ?? ''
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

  return (
    <div className="ask">
      <div className="messages">
        {thread.length === 0 && live === null && (
          <div className="hint">
            <p>Ask for feedback, a question to think about, or an explanation. The AI reads your document as it is now.</p>
            <p className="muted">It cannot change your text. If you ask it to write for you, it will say no, or mark what it wrote as AI text for your disclosure.</p>
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
                {message.role === 'author' ? 'You' : message.who} <span className="muted">{when(message.ts)}</span>
                {isAnswer && turn?.isAiText && <span className="flag" title="This answer contains text the AI wrote for the manuscript. It is listed in your disclosure.">AI text</span>}
              </h3>
              {isAnswer ? <Answer text={message.text} onJump={onJump} /> : <p className="said">{message.text}</p>}
              {isAnswer && turn !== undefined && <Classify key={turn.id + turn.use} turn={turn} onAnnotate={onAnnotate} />}
            </article>
          )
        })}
        {live !== null && (
          <>
            <article className="author">
              <h3>You</h3>
              <p className="said">{live.asked}</p>
            </article>
            <article className="ai" aria-busy="true">
              <h3>{live.who}</h3>
              {live.steps.map((step, i) => (
                <p key={i} className="step">{step}</p>
              ))}
              {live.text === '' ? <p className="muted">Thinking…</p> : <Answer text={live.text} onJump={onJump} />}
            </article>
          </>
        )}
        <div ref={end} />
      </div>
      {!isReady ? (
        <p className="hint">
          To ask the AI, set it up first: an API key, or a model on your own computer. <button className="link" onClick={onSettings}>Open settings</button>
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
            aria-label={isReply ? 'Reply' : 'Ask the AI'}
            placeholder={isReply ? (toName === '' ? 'Reply…' : `Reply to ${toName}…`) : toName === '' ? 'Ask about your text…' : `Ask ${toName}…`}
            rows={3}
            value={text}
            onChange={e => setText(e.target.value)}
            onKeyDown={e => {
              if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) send()
            }}
          />
          <div className="row">
            <button className="primary" type="submit" disabled={live !== null || text.trim() === ''}>
              {live !== null ? 'Answering…' : isReply ? 'Reply' : 'Ask'}
            </button>
            {isReply && live === null && <button type="button" onClick={onNew}>New question</button>}
            {!isReply && party !== null && (
              <select aria-label="Who to ask" value={asked} onChange={e => setAsked(e.target.value)}>
                <option value="">Assistant</option>
                {party.experts.filter(e => e.isMember).map(e => (
                  <option key={e.id} value={e.id}>{e.name}</option>
                ))}
              </select>
            )}
            <span className="spacer" />
            <span className="muted">Sends your question and the document</span>
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
        <span className="flag">{row.kind === 'correction' ? 'language correction' : 'AI text'}</span>
        <span>
          {row.source === 'paste' ? 'pasted' : 'typed'} · {plural(row.words, 'word')} · {row.match === 'exact' ? 'word for word' : 'close to the answer'}
        </span>
        <span className="muted">{when(row.ts)}</span>
      </span>
      <span className="quote">“{row.excerpt}”</span>
      <span className="muted">
        {row.stillInText ? 'Still in your text' : 'No longer in your text'}
        {row.stillInText && row.line > 0 && (
          <>
            {' · '}
            <button className="link" onClick={() => onJump(row.line)}>line {row.line}</button>
          </>
        )}
        {' · from the answer to '}
        <button className="link" onClick={() => onOpen(row.turn)}>“{row.asked}”</button>
      </span>
      <span className="row">
        <input aria-label={`Comment on “${row.excerpt.slice(0, 40)}”`} placeholder="Comment for your disclosure (optional)" value={note} onChange={e => setNote(e.target.value)} />
        <button onClick={() => onComment(row.id, note)} disabled={note.trim() === row.note.trim()}>Save</button>
      </span>
    </li>
  )
}

type LogProps = { log: LogView; onOpen(id: string): void; onJump(line: number): void; onComment(id: string, note: string): void }

export function LogPanel({ log, onOpen, onJump, onComment }: LogProps) {
  const { stats } = log
  if (log.turns.length === 0 && log.guard.length === 0) {
    return <p className="hint">Nothing is logged yet. Every question you ask the AI and every answer will be listed here.</p>
  }
  // what waits for the author comes first
  const turns = [...log.turns.filter(t => t.isOpen), ...log.turns.filter(t => !t.isOpen)]
  return (
    <div className="log">
      <p className="summary">
        {plural(stats.turns, 'AI input')} · {stats.unannotated} to classify{stats.aiText > 0 ? ` · ${stats.aiText} with AI text` : ''}{log.aiText.length > 0 ? ` · ${plural(log.aiText.length, 'passage')} taken over` : ''}
      </p>
      <ul>
        {turns.map(turn => (
          <li key={turn.id}>
            <button className="turn" onClick={() => onOpen(turn.id)}>
              <span className="top">
                <span className={turn.isOpen ? 'use open' : 'use'}>{turn.use === '' ? (turn.isOpen ? '• to classify' : '· no choice needed') : USE_LABELS[turn.use].button}</span>
                {turn.isAiText && <span className="flag">AI text</span>}
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
          <h3>AI text that entered your document</h3>
          <p className="muted">The editor compares what you paste or type with the AI's answers. Nothing is blocked. Entries stay in the log even when you rewrite the passage; text from other AI tools cannot be detected.</p>
          <ul className="ai-text">
            {log.aiText.map(row => (
              <AiTextItem key={`${row.id}-${row.note}`} row={row} onOpen={onOpen} onJump={onJump} onComment={onComment} />
            ))}
          </ul>
        </>
      )}
      {log.guard.length > 0 && (
        <>
          <h3>Writes the AI tried and the editor refused</h3>
          <ul className="guard">
            {log.guard.map((row, i) => (
              <li key={i}>
                <span className="muted">{when(row.ts)}</span> {row.refused ? row.input : `${row.tool}: ${row.files.join(', ')}`}
              </li>
            ))}
          </ul>
          <p className="muted">Nothing was changed. These are listed in your disclosure.</p>
        </>
      )}
    </div>
  )
}
