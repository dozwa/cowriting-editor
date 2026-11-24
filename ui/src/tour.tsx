// The tour of the editor: a card that goes through the window part by part and marks the part it
// speaks about. It is not a dialog over the window: everything stays usable while it is shown, so
// the author can open a document or try a tab on the way.
import { useEffect, useRef } from 'react'
import { m } from './i18n'
import type { Tab } from './preferences'

type StepId = keyof typeof m.tour.steps

/** The steps in their order: what is marked (`data-tour` in the window), the tab that is shown, and whether the part is only there with a document. */
export const TOUR: { id: StepId; marks: string; tab?: Tab; inDocument?: boolean }[] = [
  { id: 'welcome', marks: '' },
  { id: 'ai', marks: 'settings' },
  { id: 'document', marks: 'open' },
  { id: 'write', marks: 'write', inDocument: true },
  { id: 'ask', marks: 'panel', tab: 'ask', inDocument: true },
  { id: 'experts', marks: 'panel', tab: 'experts', inDocument: true },
  { id: 'live', marks: 'panel', tab: 'live', inDocument: true },
  { id: 'plan', marks: 'panel', tab: 'scope', inDocument: true },
  { id: 'sources', marks: 'panel', tab: 'sources', inDocument: true },
  { id: 'log', marks: 'panel', tab: 'log', inDocument: true },
  { id: 'finish', marks: 'finish', inDocument: true },
  { id: 'around', marks: 'around' },
]

type Props = {
  step: number
  hasDocument: boolean
  /** The AI is set up: the step about it offers nothing to do. */
  isReady: boolean
  onStep(step: number): void
  onSettings(): void
  /** Gone through or closed: either way the tour was seen. */
  onEnd(): void
}

export function Tour({ step, hasDocument, isReady, onStep, onSettings, onEnd }: Props) {
  const t = m.tour
  const now = TOUR[step] ?? TOUR[0]!
  const isLast = step >= TOUR.length - 1
  const title = useRef<HTMLHeadingElement>(null)

  // the part of the window this step is about is marked while the step is shown
  useEffect(() => {
    if (now.marks === '') return
    const marked = [...document.querySelectorAll(`[data-tour~="${now.marks}"]`)]
    marked.forEach(part => part.classList.add('toured'))
    return () => marked.forEach(part => part.classList.remove('toured'))
  }, [now.marks, hasDocument])

  // each step is read from its title
  useEffect(() => title.current?.focus({ preventScroll: true }), [step])

  return (
    <aside className="tour" role="dialog" aria-labelledby="tour-title" onKeyDown={e => e.key === 'Escape' && (e.stopPropagation(), onEnd())}>
      <p className="count">{t.count(step + 1, TOUR.length)}</p>
      <progress value={step + 1} max={TOUR.length} aria-hidden="true" />
      <h2 id="tour-title" tabIndex={-1} ref={title}>{t.steps[now.id].title}</h2>
      <p>{t.steps[now.id].text}</p>
      {now.inDocument === true && !hasDocument && <p className="missing" role="note">{t.needsDocument}</p>}
      {now.id === 'ai' && !isReady && (
        <p>
          <button onClick={onSettings}>{t.setUp}</button>
        </p>
      )}
      <div className="steps">
        {!isLast && <button className="link" onClick={onEnd}>{t.skip}</button>}
        <span className="spacer" />
        {step > 0 && <button onClick={() => onStep(step - 1)}>{t.back}</button>}
        <button className="primary" onClick={() => (isLast ? onEnd() : onStep(step + 1))}>{isLast ? t.done : t.next}</button>
      </div>
    </aside>
  )
}
