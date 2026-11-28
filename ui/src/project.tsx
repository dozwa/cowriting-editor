import { useState } from 'react'
import type { ProjectOverview } from './api'
import { m } from './i18n'
import { More } from './more'

type Props = {
  /** null for a document that is a project of its own. */
  view: ProjectOverview | null
  onMake(): void
  onOpen(path: string): void
  onAdd(): void
  onAddNew(): void
  onRemove(path: string): void
  onOrder(paths: string[]): void
  /** For a file that is not open; the open one is renamed with the window's "Move…". */
  onRename(path: string, name: string): Promise<boolean>
  onMoveOpen(): void
  onGoal(goal: string, words: number | null): Promise<boolean>
  /** The author allows or forbids the AI to read the project's other files. */
  onAiReads(may: boolean): void
}

const percent = (share: number) => `${Math.round(share * 100)} %`
const fileName = (name: string) => name.split('/').pop() ?? name

/** The project of several files: the whole against its target, and every file with its words and its plan. */
export function ProjectPanel({ view, onMake, onOpen, onAdd, onAddNew, onRemove, onOrder, onRename, onMoveOpen, onGoal, onAiReads }: Props) {
  const [goal, setGoal] = useState<{ goal: string; words: string } | null>(null)
  const [renaming, setRenaming] = useState<{ path: string; name: string } | null>(null)
  if (view === null) {
    return (
      <div className="project">
        <section>
          <More more={m.project.aloneHow}>{m.project.alone}</More>
          <button onClick={onMake}>{m.project.make}</button>
        </section>
      </div>
    )
  }
  const paths = view.files.map(file => file.path)
  const swapped = (at: number, with_: number) => paths.map((path, i) => (i === at ? paths[with_]! : i === with_ ? paths[at]! : path))
  const planned = view.files.reduce((sum, file) => sum + (file.target ?? 0), 0)
  return (
    <div className="project">
      <section>
        <h3>{m.project.of(<strong>{view.name}</strong>)}</h3>
        {goal === null ? (
          <>
            <p className={view.goal === '' ? 'muted' : undefined}>{view.goal === '' ? m.project.noGoal : view.goal}</p>
            <p className="numbers" title={m.project.chars(view.chars)}>{m.project.total(view.words, view.wordsTarget)}</p>
            {view.wordsTarget !== undefined && <progress aria-label={m.project.totalWords} max={view.wordsTarget} value={Math.min(view.words, view.wordsTarget)} />}
            {planned > 0 && <p className="muted">{m.project.targetsSum(planned)}</p>}
            <button className="link" onClick={() => setGoal({ goal: view.goal, words: view.wordsTarget === undefined ? '' : String(view.wordsTarget) })}>{m.project.editGoal}</button>
          </>
        ) : (
          <form
            className="goal"
            onSubmit={async e => {
              e.preventDefault()
              const words = Number.parseInt(goal.words.replace(/\D/g, ''), 10)
              if (await onGoal(goal.goal, Number.isFinite(words) && words > 0 ? words : null)) setGoal(null)
            }}
          >
            <label>
              {m.project.goal}
              <textarea rows={3} value={goal.goal} onChange={e => setGoal({ ...goal, goal: e.target.value })} />
            </label>
            <label>
              {m.project.wordsTarget}
              <input inputMode="numeric" value={goal.words} onChange={e => setGoal({ ...goal, words: e.target.value })} />
            </label>
            <span className="row">
              <button className="primary" type="submit">{m.project.saveGoal}</button>
              <button type="button" onClick={() => setGoal(null)}>{m.project.cancel}</button>
            </span>
          </form>
        )}
      </section>
      <section>
        <h3>{m.project.files}</h3>
        <More more={m.project.filesHintMore}>{m.project.filesHint}</More>
        <ol className="files" aria-label={m.project.files}>
          {view.files.map((file, at) => (
            <li key={file.path} className={file.isOpen ? 'current' : undefined}>
              <span className="top">
                {file.isOpen || file.isMissing ? (
                  <strong aria-current={file.isOpen ? 'true' : undefined}>{file.name}</strong>
                ) : (
                  <button className="link" title={m.project.open(file.name)} onClick={() => onOpen(file.path)}>{file.name}</button>
                )}
                {file.isOpen && <span className="muted">{m.project.isOpen}</span>}
                {file.isMissing && <span className="flag">{m.project.missing}</span>}
              </span>
              {!file.isMissing && (
                <span className="muted numbers">
                  {m.project.wordsOf(file.words, file.target)}
                  {' · '}
                  {file.progress === undefined ? m.project.noPlan : m.project.ofPlan(percent(file.progress))}
                  {file.points > 0 && ` · ${m.project.points(file.pointsMade, file.points)}`}
                </span>
              )}
              {file.target !== undefined && <progress aria-label={m.project.wordsOf(file.words, file.target)} max={file.target} value={Math.min(file.words, file.target)} />}
              {renaming?.path === file.path ? (
                <form
                  className="row"
                  onSubmit={async e => {
                    e.preventDefault()
                    if (await onRename(file.path, renaming.name)) setRenaming(null)
                  }}
                >
                  <input aria-label={m.project.newName(file.name)} value={renaming.name} autoFocus onChange={e => setRenaming({ ...renaming, name: e.target.value })} />
                  <button className="primary" type="submit">{m.project.rename}</button>
                  <button type="button" onClick={() => setRenaming(null)}>{m.project.cancel}</button>
                </form>
              ) : (
                <span className="row actions">
                  <button aria-label={m.project.up(file.name)} title={m.project.up(file.name)} disabled={at === 0} onClick={() => onOrder(swapped(at, at - 1))}>↑</button>
                  <button aria-label={m.project.down(file.name)} title={m.project.down(file.name)} disabled={at === view.files.length - 1} onClick={() => onOrder(swapped(at, at + 1))}>↓</button>
                  {!file.isMissing && <button className="link" title={m.project.renameTitle(file.name)} onClick={() => (file.isOpen ? onMoveOpen() : setRenaming({ path: file.path, name: fileName(file.name) }))}>{m.project.rename}</button>}
                  <button className="link" title={m.project.removeTitle(file.name)} disabled={view.files.length === 1} onClick={() => onRemove(file.path)}>{m.project.remove}</button>
                </span>
              )}
            </li>
          ))}
        </ol>
        <span className="row">
          <button onClick={onAdd}>{m.project.add}</button>
          <button onClick={onAddNew}>{m.project.addNew}</button>
        </span>
      </section>
      <section>
        <label className="check">
          <input type="checkbox" checked={view.aiReadsFiles} onChange={e => onAiReads(e.target.checked)} /> {m.project.aiReads}
        </label>
        <p className="muted">{m.project.aiReadsNote}</p>
      </section>
    </div>
  )
}
