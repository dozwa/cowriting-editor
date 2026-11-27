import { expect, test } from 'vitest'
import css from './styles.css?raw'

/** What the stylesheet declares for a selector, over all the rules that name exactly it. */
function declared(selector: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  return [...css.matchAll(new RegExp(`(?:^|[}/])\\s*${escaped}\\s*\\{([^}]*)\\}`, 'gm'))].map((rule) => rule[1]).join(';')
}

test('a table in an answer keeps a short column whole and scrolls when it is too wide', () => {
  // A long word or an address must not widen the panel, so an answer breaks anywhere. In a table cell that would
  // let the column shrink to one character: line numbers then stand one digit to a line (seen in the window).
  // The test environment lays nothing out, so this holds the rules that the browser was measured with.
  expect(css.length).toBeGreaterThan(1000)
  expect(declared('.answer')).toMatch(/overflow-wrap:\s*anywhere/)
  expect(declared('.answer th, .answer td')).toMatch(/overflow-wrap:\s*break-word/)
  expect(declared('.answer table')).toMatch(/display:\s*block/)
  expect(declared('.answer table')).toMatch(/overflow-x:\s*auto/)
})
