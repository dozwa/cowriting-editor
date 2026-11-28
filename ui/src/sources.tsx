import { useState } from 'react'
import { type CheckedRef, GUIDES, type SourceHits, type SourcesView } from './api'
import { m } from './i18n'
import { when } from './text'
import { More } from './more'

type Props = {
  view: SourcesView
  onAdd(): Promise<void>
  onRemove(path: string): Promise<void>
  /** Asks the author for a .csl file and makes it this document's reference style. */
  onStyle(): Promise<void>
  onNoStyle(): Promise<void>
  /** Shows where styles can be downloaded, in the browser. */
  onStyles(): void
  onSearch(query: string): Promise<SourceHits | null>
  onCheck(): Promise<void>
  /** Puts a citation of this key into the document, where the cursor is. */
  onCite(key: string): void
}

/** Worst first: what the author has to look at is on top. */
const ORDER: CheckedRef['status'][] = ['mismatch', 'not_found', 'error', 'no_doi', 'ok']

/** The author's references: their files, a search, and a check against Crossref. */
export function SourcesPanel({ view, onAdd, onRemove, onStyle, onNoStyle, onStyles, onSearch, onCheck, onCite }: Props) {
  const style = view.style
  const [query, setQuery] = useState('')
  const [found, setFound] = useState<SourceHits | null>(null)
  const [isChecking, setChecking] = useState(false)
  const rows = view.checked === null ? [] : [...view.checked.rows].sort((a, b) => ORDER.indexOf(a.status) - ORDER.indexOf(b.status))
  const t = m.sources
  const count = (status: CheckedRef['status']) => rows.filter(row => row.status === status).length

  return (
    <div className="sources">
      <section>
        <h3>{t.bibliographyHeading}</h3>
        {view.files.length === 0 ? (
          <More more={t.noBibliographyMore(<code>bibliography: refs.bib</code>)}>{t.noBibliography}</More>
        ) : (
          <ul className="bibfiles">
            {view.files.map(file => (
              <li key={file.path} title={file.path}>
                <strong>{file.name}</strong>
                {file.error !== undefined ? <span className="warn">{file.error}</span> : <span className="muted">{t.fileSummary(file.format, file.entries)}</span>}
                {file.isFromDocument ? <span className="muted">{t.namedInDocument}</span> : <button className="link" onClick={() => void onRemove(file.path)}>{t.takeOffList}</button>}
              </li>
            ))}
          </ul>
        )}
        {view.files.length === 0 && view.cited > 0 && <p className="hint" role="note">{t.citesWithoutFile(view.cited)}</p>}
        <button onClick={() => void onAdd()}>{t.addFile}</button>
        {view.files.length > 0 && (
          <p className="muted">
            {t.cites(view.cited)}
            {view.missing.length > 0 && <> {t.notInBibliography(view.missing.map(key => <code key={key}>@{key} </code>))}</>}
          </p>
        )}
      </section>
      <section>
        <h3>{t.styleHeading}</h3>
        {style === null ? (
          <More more={t.noStyleMore(<code>csl: apa.csl</code>)}>{t.noStyle}</More>
        ) : (
          <ul className="bibfiles">
            <li title={style.path}>
              <strong>{style.title !== '' ? style.title : style.name}</strong>
              {style.error !== undefined ? <span className="warn">{style.name} {style.error}</span> : style.title !== '' && <span className="muted">{style.name}</span>}
              {style.isFromDocument ? <span className="muted">{t.namedInDocument}</span> : <button className="link" onClick={() => void onNoStyle()}>{t.takeOffStyle}</button>}
            </li>
          </ul>
        )}
        {style?.isFromDocument !== true && <button onClick={() => void onStyle()}>{style === null ? t.chooseStyle : t.chooseOtherStyle}</button>}
        <More more={t.styleNoteMore}>{t.styleNote(<button className="link" onClick={onStyles}>{GUIDES.styles.name}</button>)}</More>
      </section>
      <section>
        <h3>{t.findHeading}</h3>
        <form
          onSubmit={async e => {
            e.preventDefault()
            setFound(await onSearch(query))
          }}
        >
          <input aria-label={t.searchLabel} value={query} onChange={e => setQuery(e.target.value)} placeholder={t.searchPlaceholder} />
          <button type="submit">{t.searchButton}</button>
        </form>
        {found !== null && (
          <>
            <p className="muted">{t.found(found.from, found.hits.length)}</p>
            <ul className="refs">
              {found.hits.map((hit, i) => (
                <li key={i}>
                  <span>
                    {hit.citekey !== null ? <><code>@{hit.citekey}</code> <button className="link" title={t.citeTitle} onClick={() => onCite(hit.citekey!)}>{t.cite}</button></> : <span className="muted">{t.noCitekey}</span>} {hit.authors} {hit.year !== '' && `(${hit.year})`}
                  </span>
                  <span>{hit.title}</span>
                  {hit.doi !== '' && <span className="muted">doi:{hit.doi}</span>}
                </li>
              ))}
            </ul>
          </>
        )}
      </section>
      <section>
        <h3>{t.checkHeading}</h3>
        <More more={t.checkNoteMore}>{t.checkNote}</More>
        <button
          disabled={isChecking || view.entries === 0}
          onClick={async () => {
            setChecking(true)
            await onCheck()
            setChecking(false)
          }}
        >
          {isChecking ? t.checking : view.entries === 0 ? t.checkNothing : t.checkButton(view.entries)}
        </button>
        {view.entries === 0 && <p className="muted">{t.checkNeedsFile}</p>}
        {view.checked !== null && (
          <>
            <p role="status">{t.checked(when(view.checked.ts), count('ok'), count('mismatch') + count('not_found'), count('no_doi'), count('error'))}</p>
            <ul className="refs">
              {rows.map((row, i) => (
                <li key={i} className={row.status}>
                  <span>
                    {row.derived ? <span className="muted">{row.file}</span> : <code>@{row.key}</code>} <span className={`verdict ${row.status}`}>{t.verdict[row.status]}</span>
                  </span>
                  <span>{row.title}{row.year !== '' && ` (${row.year})`}</span>
                  {row.found !== undefined && <span className="muted">{t.crossrefHas(row.found.title, row.found.year)}</span>}
                  {row.suggestion !== undefined && <span className="muted">{t.suggestion(row.suggestion)}</span>}
                  {row.detail !== undefined && <span className="muted">{row.detail}</span>}
                </li>
              ))}
            </ul>
          </>
        )}
      </section>
    </div>
  )
}
