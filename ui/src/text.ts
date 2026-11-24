import type { Use } from './api'
import { language, m } from './i18n'

/** What the four classifications mean, in the words the buttons use (the plugin's `USE_LABELS`). Read when drawn, so they follow the language. */
export const USE_LABELS: Record<Use, { button: string; meaning: string }> = {
  get adopted() { return m.shared.use.adopted },
  get adapted() { return m.shared.use.adapted },
  get rejected() { return m.shared.use.rejected },
  get info() { return m.shared.use.info },
}

/** "10:42" today, "23 Nov 10:42" otherwise, on the author's clock and in the language shown: as the computer writes it where that is its language, else as the language usually does. */
export function when(ts: string, now = new Date()): string {
  const d = new Date(ts)
  if (Number.isNaN(d.getTime())) return ts
  const locale = navigator.language.toLowerCase().startsWith(language) ? [] : language
  const clock = d.toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit' })
  return d.toDateString() === now.toDateString() ? clock : `${d.toLocaleDateString(locale, { day: 'numeric', month: 'short' })} ${clock}`
}

const LINE_REF = /\b(lines?|Zeilen?|l\.)\s*(\d+)(?:\s*[–-]\s*\d+)?/gi

/** An answer's mentions of a line ("line 12", "Zeile 3", "lines 4–6") as links the editor can jump to. Code is left alone. */
export function linkLines(markdown: string): string {
  return markdown
    .split(/(```[\s\S]*?```|`[^`\n]*`)/)
    .map((part, i) => (i % 2 === 1 ? part : part.replace(LINE_REF, (match, _word, line) => `[${match}](#line-${line})`)))
    .join('')
}

export const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`

/** What the author is told about a model the behaviour suite has not passed. Empty for a tested one. */
export const MODEL_NOTICE: Record<'tested' | 'failed' | 'untested', string> = {
  tested: '',
  get untested() { return m.shared.modelUntested },
  get failed() { return m.shared.modelFailed },
}
