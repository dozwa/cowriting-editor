// Find and replace in the document: a bar over the text, in the editor's own look and words.
// The searching itself is CodeMirror's (`@codemirror/search`); the bar is ours, so that it can
// say how many matches there are and reads in the language of the window.
import { SearchQuery, closeSearchPanel, findNext, findPrevious, getSearchQuery, openSearchPanel, replaceAll, replaceNext, search, setSearchQuery } from '@codemirror/search'
import type { Extension } from '@codemirror/state'
import { type EditorView, type Panel, type ViewUpdate, keymap } from '@codemirror/view'
import { m } from './i18n'

/** Counting stops here: beyond it the bar says "more than". */
const MOST = 5000

/** How many matches the document has, and which of them is selected (0: none). */
function matches(view: EditorView): { count: number; at: number } | null {
  const query = getSearchQuery(view.state)
  if (!query.valid || query.search === '') return null
  const { from, to } = view.state.selection.main
  const cursor = query.getCursor(view.state)
  let [count, at] = [0, 0]
  for (let found = cursor.next(); !found.done && count <= MOST; found = cursor.next()) {
    count += 1
    if (found.value.from === from && found.value.to === to) at = count
  }
  return { count, at }
}

function element<K extends keyof HTMLElementTagNameMap>(tag: K, attributes: Record<string, string>, text = ''): HTMLElementTagNameMap[K] {
  const made = document.createElement(tag)
  for (const [name, value] of Object.entries(attributes)) made.setAttribute(name, value)
  made.textContent = text
  return made
}

function bar(view: EditorView): Panel {
  const words = m.search
  const query = getSearchQuery(view.state)
  const find = element('input', { type: 'text', name: 'search', 'main-field': 'true', placeholder: words.find, 'aria-label': words.find, spellcheck: 'false', autocomplete: 'off' })
  const replace = element('input', { type: 'text', name: 'replace', placeholder: words.replaceWith, 'aria-label': words.replaceWith, spellcheck: 'false', autocomplete: 'off' })
  const count = element('span', { class: 'count', role: 'status' })
  const toggle = (label: string, shown: string) => element('button', { type: 'button', class: 'toggle', title: label, 'aria-label': label, 'aria-pressed': 'false' }, shown)
  const [matchCase, wholeWord, regexp] = [toggle(words.matchCase, 'Aa'), toggle(words.wholeWord, words.wholeWordShown), toggle(words.regexp, '.*')]
  const button = (label: string, shown = label) => element('button', { type: 'button', title: label, 'aria-label': label }, shown)
  const [previous, next, close] = [button(words.previous, '‹'), button(words.next, '›'), button(words.close, '×')]
  const [one, all] = [button(words.replaceOne), button(words.replaceAll)]
  const dom = element('div', { class: 'search', role: 'search', 'aria-label': words.title })
  const rows = [element('div', { class: 'row' }), element('div', { class: 'row' })]
  rows[0]!.append(find, count, previous, next, matchCase, wholeWord, regexp, close)
  rows[1]!.append(replace, one, all)
  dom.append(...rows)

  const isOn = (which: HTMLElement) => which.getAttribute('aria-pressed') === 'true'
  const show = () => {
    const found = matches(view)
    const isValid = find.value === '' || getSearchQuery(view.state).valid
    find.setAttribute('aria-invalid', String(!isValid))
    count.textContent = !isValid ? words.notAPattern : found === null ? '' : found.count === 0 ? words.none : words.count(found.at, Math.min(found.count, MOST), found.count > MOST)
    for (const needs of [previous, next, one, all]) needs.disabled = found === null || found.count === 0
  }
  /** What the bar says goes to the editor; the first match from the cursor on is selected. */
  const commit = (select: boolean) => {
    const wanted = new SearchQuery({ search: find.value, replace: replace.value, caseSensitive: isOn(matchCase), wholeWord: isOn(wholeWord), regexp: isOn(regexp) })
    if (!wanted.eq(getSearchQuery(view.state))) view.dispatch({ effects: setSearchQuery.of(wanted) })
    if (select && wanted.valid && wanted.search !== '') {
      const from = view.state.selection.main.from
      const here = wanted.getCursor(view.state, from).next()
      const first = here.done ? wanted.getCursor(view.state).next() : here
      if (!first.done) view.dispatch({ selection: { anchor: first.value.from, head: first.value.to }, scrollIntoView: true, userEvent: 'select.search' })
    }
    show()
  }
  const fill = (from: SearchQuery) => {
    find.value = from.search
    replace.value = from.replace
    matchCase.setAttribute('aria-pressed', String(from.caseSensitive))
    wholeWord.setAttribute('aria-pressed', String(from.wholeWord))
    regexp.setAttribute('aria-pressed', String(from.regexp))
  }
  const leave = () => {
    closeSearchPanel(view)
    view.focus()
  }
  fill(query)
  find.addEventListener('input', () => commit(true))
  replace.addEventListener('input', () => commit(false))
  for (const option of [matchCase, wholeWord, regexp]) {
    option.addEventListener('click', () => {
      option.setAttribute('aria-pressed', String(!isOn(option)))
      commit(true)
    })
  }
  // a button that was pressed gives the cursor back to its field: Enter and Escape go on working there,
  // also when the button is switched off by what it did
  const press = (button: HTMLButtonElement, run: (view: EditorView) => boolean, field: HTMLInputElement) =>
    button.addEventListener('click', () => {
      run(view)
      field.focus()
    })
  press(previous, findPrevious, find)
  press(next, findNext, find)
  press(one, replaceNext, replace)
  press(all, replaceAll, replace)
  close.addEventListener('click', leave)
  dom.addEventListener('keydown', event => {
    const isCommand = event.metaKey || event.ctrlKey
    if (event.key === 'Escape') leave()
    else if (event.key === 'Enter' && event.target === find) void (event.shiftKey ? findPrevious(view) : findNext(view))
    else if (event.key === 'Enter' && event.target === replace) void replaceNext(view)
    else if (isCommand && event.key.toLowerCase() === 'g') void (event.shiftKey ? findPrevious(view) : findNext(view))
    else return
    event.preventDefault()
  })
  return {
    dom,
    top: true,
    mount: () => {
      find.focus()
      find.select()
      show()
    },
    update: (update: ViewUpdate) => {
      const now = getSearchQuery(update.state)
      const isAsked = update.transactions.some(transaction => transaction.effects.some(effect => effect.is(setSearchQuery)))
      // a query set from elsewhere (opening the bar with a word selected) is shown; what is being typed is left alone
      if (isAsked && document.activeElement !== find && document.activeElement !== replace) fill(now)
      if (isAsked || update.docChanged || update.selectionSet) show()
    },
  }
}

/** The bar, the marks on what was found, and the keys that go on from match to match in the text. */
export const searching = (): Extension => [
  search({ top: true, createPanel: bar }),
  keymap.of([
    { key: 'Mod-g', run: findNext, shift: findPrevious, preventDefault: true },
    { key: 'F3', run: findNext, shift: findPrevious, preventDefault: true },
    { key: 'Escape', run: closeSearchPanel },
  ]),
]

/** Opens the bar with the cursor in it; a word that is selected is what is looked for. */
export const openSearch = (view: EditorView) => void openSearchPanel(view)
