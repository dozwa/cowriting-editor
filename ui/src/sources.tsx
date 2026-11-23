import { useState } from 'react'
import type { CheckedRef, SourceHits, SourcesView } from './api'
import { plural, when } from './text'

type Props = {
  view: SourcesView
  onAdd(): Promise<void>
  onRemove(path: string): Promise<void>
  onSearch(query: string): Promise<SourceHits | null>
  onCheck(): Promise<void>
  /** Puts a citation of this key into the document, where the cursor is. */
  onCite(key: string): void
}

const VERDICT: Record<CheckedRef['status'], string> = {
  ok: 'DOI and title agree',
  mismatch: 'the DOI belongs to something else',
  not_found: 'Crossref does not know this DOI',
  no_doi: 'no DOI',
  error: 'could not be checked',
}

/** Worst first: what the author has to look at is on top. */
const ORDER: CheckedRef['status'][] = ['mismatch', 'not_found', 'error', 'no_doi', 'ok']

/** The author's references: their files, a search, and a check against Crossref. */
export function SourcesPanel({ view, onAdd, onRemove, onSearch, onCheck, onCite }: Props) {
  const [query, setQuery] = useState('')
  const [found, setFound] = useState<SourceHits | null>(null)
  const [isChecking, setChecking] = useState(false)
  const rows = view.checked === null ? [] : [...view.checked.rows].sort((a, b) => ORDER.indexOf(a.status) - ORDER.indexOf(b.status))
  const count = (status: CheckedRef['status']) => rows.filter(row => row.status === status).length

  return (
    <div className="sources">
      <section>
        <h3>Your bibliography</h3>
        {view.files.length === 0 ? (
          <p className="muted">
            No bibliography yet. Name one in the document's front matter (<code>bibliography: refs.bib</code>) or add a file here: BibTeX, CSL JSON, RIS or a PubMed export. The editor reads it and never changes it.
          </p>
        ) : (
          <ul className="bibfiles">
            {view.files.map(file => (
              <li key={file.path} title={file.path}>
                <strong>{file.name}</strong>
                {file.error !== undefined ? <span className="warn">{file.error}</span> : <span className="muted">{file.format}, {plural(file.entries, 'reference')}</span>}
                {file.isFromDocument ? <span className="muted">named in the document</span> : <button className="link" onClick={() => void onRemove(file.path)}>Take off the list</button>}
              </li>
            ))}
          </ul>
        )}
        <button onClick={() => void onAdd()}>Add a bibliography file</button>
        {view.files.length > 0 && (
          <p className="muted">
            The document cites {plural(view.cited, 'reference')}.
            {view.missing.length > 0 && <> Not in your bibliography: {view.missing.map(key => <code key={key}>@{key} </code>)}</>}
          </p>
        )}
      </section>
      <section>
        <h3>Find a reference</h3>
        <form
          onSubmit={async e => {
            e.preventDefault()
            setFound(await onSearch(query))
          }}
        >
          <input aria-label="Search your references" value={query} onChange={e => setQuery(e.target.value)} placeholder="Title, author, year" />
          <button type="submit">Search</button>
        </form>
        {found !== null && (
          <>
            <p className="muted">
              {found.from === 'zotero' ? 'From your Zotero library' : 'From your bibliography files (Zotero is not running)'}: {plural(found.hits.length, 'hit')}.
            </p>
            <ul className="refs">
              {found.hits.map((hit, i) => (
                <li key={i}>
                  <span>
                    {hit.citekey !== null ? <><code>@{hit.citekey}</code> <button className="link" title="Put [@key] into the document where the cursor is" onClick={() => onCite(hit.citekey!)}>Cite</button></> : <span className="muted">no citekey</span>} {hit.authors} {hit.year !== '' && `(${hit.year})`}
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
        <h3>Check the references</h3>
        <p className="muted">
          Looks up every DOI at Crossref and compares title and year. For this the DOI, or title, first author and year of each reference is sent to crossref.org; nothing of your text. No AI is involved.
        </p>
        <button
          disabled={isChecking || view.entries === 0}
          onClick={async () => {
            setChecking(true)
            await onCheck()
            setChecking(false)
          }}
        >
          {isChecking ? 'Checking…' : `Check ${plural(view.entries, 'reference')}`}
        </button>
        {view.checked !== null && (
          <>
            <p role="status">
              Checked {when(view.checked.ts)}: {count('ok')} fine, {count('mismatch') + count('not_found')} to look at, {count('no_doi')} without a DOI{count('error') > 0 && `, ${count('error')} not checked`}.
            </p>
            <ul className="refs">
              {rows.map((row, i) => (
                <li key={i} className={row.status}>
                  <span>
                    {row.derived ? <span className="muted">{row.file}</span> : <code>@{row.key}</code>} <span className={`verdict ${row.status}`}>{VERDICT[row.status]}</span>
                  </span>
                  <span>{row.title}{row.year !== '' && ` (${row.year})`}</span>
                  {row.found !== undefined && <span className="muted">Crossref has: {row.found.title}{row.found.year !== '' && ` (${row.found.year})`}</span>}
                  {row.suggestion !== undefined && <span className="muted">Crossref lists a similar title as doi:{row.suggestion}. Check it yourself before you use it.</span>}
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
