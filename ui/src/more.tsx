import { type ReactNode, useId, useState } from 'react'
import { m } from './i18n'

/**
 * An explanation with more to it: the one way the window shows further information on request.
 *
 * The first line is always there. The rest opens in place when "More" is asked for and closes
 * again with "Less"; it is a button, so the keyboard and a screen reader reach it like any other.
 *
 * What goes where: the first line holds what someone needs to act safely (what is sent to whom,
 * what cannot be undone, what a choice costs) and is one sentence, two at most. `more` holds what
 * is good to know once and what is there to look up. What someone agrees to is never put behind it.
 */
export function More({ children, more, as: Tag = 'p', className = 'muted' }: { children: ReactNode; more: ReactNode; as?: 'p' | 'small'; className?: string }) {
  const [isOpen, setOpen] = useState(false)
  const id = useId()
  return (
    <Tag className={`${className} more`}>
      {children}{' '}
      <button type="button" className="link more-toggle" aria-expanded={isOpen} aria-controls={id} onClick={() => setOpen(!isOpen)}>
        {isOpen ? m.shared.less : m.shared.more}
      </button>
      <span id={id} className="more-text" hidden={!isOpen}>{more}</span>
    </Tag>
  )
}
