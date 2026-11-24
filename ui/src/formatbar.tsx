import type { ReactNode } from 'react'
import { type Guide, GUIDES } from './api'
import { Dialog } from './dialogs'
import { showKeys } from './preferences'
import type { Format } from './editor'
import { m } from './i18n'

const icon = (children: ReactNode) => (
  <svg viewBox="0 0 16 16" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    {children}
  </svg>
)

/** A button whose name and markdown are read when it is drawn, so they are in the language shown now. */
const format = (kind: Format, symbol: ReactNode) => ({
  kind,
  symbol,
  get name(): string {
    return m.formatbar.formats[kind].name
  },
  get writes(): string {
    return m.formatbar.formats[kind].writes
  },
})

/** The buttons, in groups. `writes` is the markdown a button puts into the text: the tooltip shows it, so the marks are learnt by using them. */
export const FORMATS: { kind: Format; name: string; writes: string; symbol: ReactNode }[][] = [
  [
    format('title', <span className="letters">H1</span>),
    format('heading', <span className="letters">H2</span>),
    format('subheading', <span className="letters">H3</span>),
  ],
  [
    format('bold', <b>B</b>),
    format('italic', <i>I</i>),
  ],
  [
    format('bullets', icon(<><path d="M6 4h8M6 8h8M6 12h8" /><path d="M2.5 4h.01M2.5 8h.01M2.5 12h.01" strokeWidth="2" /></>)),
    format('numbers', icon(<><path d="M7 4h7M7 8h7M7 12h7" /><path d="M2 3l1-.5V6M2 10h1.6L2 12.5h2" strokeWidth="1.1" /></>)),
    format('quote', icon(<><path d="M3 3v10" strokeWidth="2" /><path d="M7 5h7M7 8h7M7 11h4" /></>)),
  ],
  [
    format('link', icon(<><path d="M6.5 9.5a3 3 0 0 0 4.2 0l2-2a3 3 0 0 0-4.2-4.2l-.8.8" /><path d="M9.5 6.5a3 3 0 0 0-4.2 0l-2 2a3 3 0 0 0 4.2 4.2l.8-.8" /></>)),
    format('image', icon(<><rect x="2" y="3" width="12" height="10" rx="1.5" /><path d="M2.5 11.5l3.5-3.5 2.5 2.5 2-2 3 3" /><path d="M10.5 6h.01" strokeWidth="2" /></>)),
    format('table', icon(<><rect x="2" y="3" width="12" height="10" rx="1.5" /><path d="M2 6.5h12M2 9.8h12M8 3v10" /></>)),
  ],
  [
    format('citation', <span className="letters">[@]</span>),
    format('footnote', <span className="letters">a<sup>1</sup></span>),
  ],
]

/** Buttons that write markdown for the author, so nobody has to know the marks to start. */
/** `keys` are the shortcuts as the author has them, named in each button's tooltip. */
export function FormatBar({ keys, onFormat, onHelp }: { keys?: Partial<Record<Format, string>>; onFormat(kind: Format): void; onHelp(): void }) {
  return (
    <div className="formatbar" role="toolbar" aria-label={m.formatbar.toolbar}>
      {FORMATS.map((group, i) => (
        <span key={i} className="set">
          {group.map(({ kind, name, writes, symbol }) => (
            // the text keeps its selection: the button does not take the focus
            <button key={kind} type="button" aria-label={name} title={`${name}: ${writes}${(keys?.[kind] ?? '') === '' ? '' : ` (${showKeys(keys![kind]!)})`}`} onMouseDown={e => e.preventDefault()} onClick={() => onFormat(kind)}>
              {symbol}
            </button>
          ))}
        </span>
      ))}
      <button type="button" className="help" onClick={onHelp}>{m.formatbar.help}</button>
    </div>
  )
}

/** The marks worth knowing, in the order they are shown; what the author types for each is in the catalogue. */
const SYNTAX = ['headings', 'paragraph', 'emphasis', 'bullets', 'numbers', 'quote', 'link', 'image', 'table', 'citation', 'footnote', 'frontMatter'] as const

/** What markdown is and the marks worth knowing, with the way to the full guides. */
export function MarkdownHelp({ onOpen, onClose }: { onOpen(which: Guide): void; onClose(): void }) {
  const t = m.formatbar
  return (
    <Dialog title={t.help} onClose={onClose}>
      <p>{t.intro}</p>
      <dl className="syntax">
        {SYNTAX.map(mark => {
          const { what, typed, note } = t.syntax[mark]
          return (
            <div key={mark}>
              <dt>
                {what}
                {note !== '' && <small>{note}</small>}
              </dt>
              <dd><pre>{typed}</pre></dd>
            </div>
          )
        })}
      </dl>
      <p className="muted">{t.more}</p>
      <ul className="guides">
        {(['markdown', 'pandoc'] as const).map(which => (
          <li key={which}>
            <button className="link" onClick={() => onOpen(which)}>{GUIDES[which].name}</button> <span className="muted">{GUIDES[which].address}</span>
          </li>
        ))}
      </ul>
      <div className="actions">
        <button className="primary" onClick={onClose}>{t.close}</button>
      </div>
    </Dialog>
  )
}
