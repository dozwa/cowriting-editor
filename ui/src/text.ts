import type { Use } from './api'

/** What the four classifications mean, in the words the buttons use (the plugin's `USE_LABELS`). */
export const USE_LABELS: Record<Use, { button: string; meaning: string }> = {
  adopted: { button: '✓ used', meaning: 'as suggested' },
  adapted: { button: '✎ changed', meaning: 'in your own way' },
  rejected: { button: '✗ not used', meaning: 'decided against it' },
  info: { button: 'ℹ info', meaning: 'only informed you' },
}

/** "10:42" today, "23 Nov 10:42" otherwise, on the author's clock. */
export function when(ts: string, now = new Date()): string {
  const d = new Date(ts)
  if (Number.isNaN(d.getTime())) return ts
  const clock = d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
  return d.toDateString() === now.toDateString() ? clock : `${d.toLocaleDateString([], { day: 'numeric', month: 'short' })} ${clock}`
}

const LINE_REF = /\b(lines?|Zeilen?|l\.)\s*(\d+)(?:\s*[–-]\s*\d+)?/g

/** An answer's mentions of a line ("line 12", "Zeile 3", "lines 4–6") as links the editor can jump to. Code is left alone. */
export function linkLines(markdown: string): string {
  return markdown
    .split(/(```[\s\S]*?```|`[^`\n]*`)/)
    .map((part, i) => (i % 2 === 1 ? part : part.replace(LINE_REF, (match, _word, line) => `[${match}](#line-${line})`)))
    .join('')
}

export const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`
