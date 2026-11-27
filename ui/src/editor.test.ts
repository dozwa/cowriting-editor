import { EditorView } from '@codemirror/view'
import { expect, test } from 'vitest'
import { createEditor, type Format } from './editor'

// this test window lays nothing out: where the editor measures text, it gets "no boxes"
Range.prototype.getClientRects = () => [] as unknown as DOMRectList
Range.prototype.getBoundingClientRect = () => new DOMRect()

/** The real editor on a text, with `[` and `]` in it marking what is selected (`[]` is the cursor). */
function editing(marked: string) {
  const parent = document.createElement('div')
  document.body.append(parent)
  const changes: number[] = []
  const editor = createEditor(parent, { onChange: () => void changes.push(1), onPaste: () => {} })
  const [from, to] = [marked.indexOf('['), marked.indexOf(']') - 1]
  editor.setText(marked.replace('[', '').replace(']', ''))
  const view = EditorView.findFromDOM(parent.querySelector<HTMLElement>('.cm-editor')!)!
  view.dispatch({ selection: { anchor: from, head: to } })
  /** The text after a button, with the selection marked the same way. */
  const after = (kind: Format, picture?: string) => {
    editor.format(kind, picture)
    const { from, to } = view.state.selection.main
    const text = editor.getText()
    return `${text.slice(0, from)}[${text.slice(from, to)}]${text.slice(to)}`
  }
  return { editor, parent, after, changes }
}

test('bold and italic go around what is selected, and come off again', () => {
  const { after, changes } = editing('A [strong] claim.')
  expect(after('bold')).toBe('A **[strong]** claim.')
  expect(changes.length).toBeGreaterThan(1) // the document counts as changed
  expect(after('italic')).toBe('A ***[strong]*** claim.')
  expect(after('italic')).toBe('A **[strong]** claim.')
  // italic does not mistake the stars of bold for its own
  expect(after('italic')).toBe('A ***[strong]*** claim.')
  expect(editing('A [strong] claim.').after('italic')).toBe('A *[strong]* claim.')
  expect(editing('**[strong]**').after('bold')).toBe('[strong]')
})

test('with nothing selected a button writes an example to type over', () => {
  const twice = editing('Then: []')
  expect(twice.after('bold')).toBe('Then: **[bold text]**')
  // pressed again, the example is gone and not left in the text
  expect(twice.after('bold')).toBe('Then: []')
  expect(editing('See []').after('link')).toBe('See [link text]([https://])')
  expect(editing('See [the report]').after('link')).toBe('See [the report]([https://])')
  expect(editing('[]').after('image')).toBe('![description]([picture.png])')
  expect(editing('As shown[].').after('citation')).toBe('As shown[@[]].')
  expect(editing('A claim[].').after('footnote')).toBe('A claim^[[note]].')
})

test('a picture the author chose is written with its path, and the description is what is left to type', () => {
  expect(editing('See: []').after('image', 'figures/plot.png')).toBe('See: ![[description]](figures/plot.png)')
  // what was selected is the description; the author goes on after the picture
  expect(editing('[The plot] shows it.').after('image', '<images/my plot.png>')).toBe('![The plot](<images/my plot.png>)[] shows it.')
})

test('a heading, list or quotation marks every selected line, replaces the mark that was there, and comes off again', () => {
  expect(editing('Meth[]ods').after('heading')).toBe('## Methods[]')
  expect(editing('# Meth[]ods').after('subheading')).toBe('### Methods[]')
  expect(editing('## Meth[]ods').after('heading')).toBe('Methods[]')
  expect(editing('[one\ntwo\nthree]').after('numbers')).toBe('1. one\n2. two\n3. three[]')
  expect(editing('[1. one\n2. two]').after('bullets')).toBe('- one\n- two[]')
  expect(editing('- o[]ne').after('quote')).toBe('> one[]')
  expect(editing('[]').after('title')).toBe('# []')
})

test('a comment goes around what is selected or around the line, and comes off again', () => {
  const selected = editing('Keep this. [Not sure about this.] And this.')
  expect(selected.after('comment')).toBe('Keep this. <!-- [Not sure about this.] --> And this.')
  expect(selected.after('comment')).toBe('Keep this. [Not sure about this.] And this.')
  // nothing selected: the line the cursor is in, without the space it is indented by
  const line = editing('One.\n  An old wor[]ding.\nThree.')
  expect(line.after('comment')).toBe('One.\n  <!-- [An old wording.] -->\nThree.')
  expect(line.after('comment')).toBe('One.\n  [An old wording.]\nThree.')
  // the cursor anywhere in a comment takes its marks off, over several lines too
  expect(editing('A <!-- note\nto my[]self --> here.').after('comment')).toBe('A [note\nto myself] here.')
  expect(editing('[<!-- all of it -->]').after('comment')).toBe('[all of it]')
  expect(editing('<!--tight-->[]').after('comment')).toBe('[tight]')
  // an empty line gets an example to type over, which a second press takes away
  const empty = editing('One.\n[]')
  expect(empty.after('comment')).toBe('One.\n<!-- [comment] -->')
  expect(empty.after('comment')).toBe('One.\n[]')
})

test('comments do not lie inside each other: a selection that holds one becomes one comment', () => {
  expect(editing('[One. <!-- two --> Three.]').after('comment')).toBe('<!-- [One. two Three.] -->')
  expect(editing('[One. <!-- two] --> Three.').after('comment')).toBe('<!-- [One. two] --> Three.')
  // a comment that only touches the selection is left alone
  expect(editing('<!-- one -->[ two ]<!-- three -->').after('comment')).toBe('<!-- one --><!-- [ two ] --><!-- three -->')
  expect(editing('One. <!-- two --> Th[]ree.').after('comment')).toBe('<!-- [One. two Three.] -->')
})

test('a table is a block of its own, with a first cell to type over', () => {
  const table = '| [Column] | Column |\n|--------|--------|\n| cell   | cell   |\n| cell   | cell   |\n'
  expect(editing('[]').after('table')).toBe(table)
  expect(editing('Some te[]xt.').after('table')).toBe(`Some text.\n\n${table}`)
  expect(editing('Some text.\n[]').after('table')).toBe(`Some text.\n\n${table}`)
})

test('line numbers and the source view are switched on and off, and neither changes the text', () => {
  const { editor, parent, changes } = editing('# Title\n\nSome **bold** text.[]')
  const before = changes.length
  const shown = () => parent.querySelector('.cm-content')!.textContent
  // styled: the marks of lines the cursor is not in are hidden
  expect(shown()).toContain('Title')
  expect(shown()).not.toContain('# Title')
  expect(parent.querySelector('.cm-lineNumbers')).toBeNull()

  editor.setView({ lineNumbers: true, source: true, spellcheck: false, language: 'de' })
  expect(parent.querySelector('.cm-content')!.getAttribute('spellcheck')).toBe('false')
  expect(shown()).toContain('# Title')
  expect(parent.querySelector('.cm-lineNumbers')).not.toBeNull()
  expect(parent.querySelector('.cm-editor')!.classList.contains('cm-source')).toBe(true)

  editor.setView({ lineNumbers: false, source: false, spellcheck: true, language: 'en' })
  expect(parent.querySelector('.cm-content')!.getAttribute('spellcheck')).toBe('true')
  expect(shown()).not.toContain('# Title')
  expect(parent.querySelector('.cm-lineNumbers')).toBeNull()
  expect(editor.getText()).toBe('# Title\n\nSome **bold** text.')
  expect(changes.length).toBe(before)
})

test('the lines of a table are set in one width, and only those', () => {
  const { parent } = editing('Before.\n\n| Group | n |\n|---|---|\n| A | 24 |\n\nAfter.[]')
  const lines = [...parent.querySelectorAll('.cm-line')].map(line => [line.textContent, line.classList.contains('md-table')])
  expect(lines).toEqual([['Before.', false], ['', false], ['| Group | n |', true], ['|---|---|', true], ['| A | 24 |', true], ['', false], ['After.', false]])
})

/** The find bar of an editor, opened, and what a test does in it. */
function finding(marked: string) {
  const pasted: string[] = []
  const parent = document.createElement('div')
  document.body.append(parent)
  const editor = createEditor(parent, { onChange: () => {}, onPaste: text => void pasted.push(text) })
  const [from, to] = [marked.indexOf('['), marked.indexOf(']') - 1]
  editor.setText(marked.replace('[', '').replace(']', ''))
  const view = EditorView.findFromDOM(parent.querySelector<HTMLElement>('.cm-editor')!)!
  view.dispatch({ selection: { anchor: from, head: to } })
  editor.find()
  const bar = parent.querySelector<HTMLElement>('.search')!
  const field = (name: string) => bar.querySelector<HTMLInputElement>(`input[name="${name}"]`)!
  const type = (name: string, text: string) => {
    field(name).value = text
    field(name).dispatchEvent(new Event('input', { bubbles: true }))
  }
  const press = (label: RegExp) => [...bar.querySelectorAll('button')].find(button => label.test(button.getAttribute('aria-label') ?? ''))!.click()
  const key = (name: string, key: string, shiftKey = false) => field(name).dispatchEvent(new KeyboardEvent('keydown', { key, shiftKey, bubbles: true, cancelable: true }))
  const selected = () => {
    const { from, to } = view.state.selection.main
    const text = editor.getText()
    return `${text.slice(0, from)}[${text.slice(from, to)}]${text.slice(to)}`
  }
  const count = () => bar.querySelector('.count')!.textContent
  return { editor, view, parent, bar, field, type, press, key, selected, count, pasted }
}

test('finding goes from match to match, says how many there are, and takes a selected word as what is looked for', () => {
  const { field, type, press, key, selected, count, parent, view } = finding('The [sample] was small. A Sample of 48. Resampled twice.')
  expect(field('search').value).toBe('sample')
  expect(document.activeElement).toBe(field('search'))
  expect(count()).toBe('1 of 3')
  key('search', 'Enter')
  expect(selected()).toBe('The sample was small. A [Sample] of 48. Resampled twice.')
  expect(count()).toBe('2 of 3')
  key('search', 'Enter', true)
  expect(selected()).toBe('The [sample] was small. A Sample of 48. Resampled twice.')

  // the options narrow it: the case, whole words, a pattern
  press(/upper and lower case/)
  expect(count()).toBe('1 of 2')
  press(/Whole words/)
  expect(count()).toBe('1 of 1')
  press(/upper and lower case/)
  press(/Whole words/)
  press(/Regular expression/)
  type('search', '\\d+')
  expect(selected()).toBe('The sample was small. A Sample of [48]. Resampled twice.')
  type('search', '(')
  expect([count(), field('search').getAttribute('aria-invalid')]).toEqual(['Not a valid expression', 'true'])
  press(/Regular expression/)
  type('search', 'nowhere')
  expect(count()).toBe('No matches')

  // Escape closes the bar and gives the text the cursor back
  key('search', 'Escape')
  expect(parent.querySelector('.search')).toBeNull()
  expect(view.hasFocus).toBe(true)
})

test('replacing one match or all of them is the author\'s own edit: one step to undo, and never taken for a paste', async () => {
  const { undo } = await import('@codemirror/commands')
  const { editor, view, field, type, press, key, selected, count, pasted } = finding('[]Subjects were recruited. The subjects agreed. Each subject signed.')
  type('search', 'subject')
  type('replace', 'participant')
  expect(selected()).toBe('[Subject]s were recruited. The subjects agreed. Each subject signed.')
  key('replace', 'Enter')
  expect(editor.getText()).toBe('participants were recruited. The subjects agreed. Each subject signed.')
  expect(count()).toBe('1 of 2')
  press(/^Replace all$/)
  expect(editor.getText()).toBe('participants were recruited. The participants agreed. Each participant signed.')
  expect(count()).toBe('No matches')
  // the button is switched off now; the cursor is in its field, so Escape still closes the bar
  expect(document.activeElement).toBe(field('replace'))
  undo(view)
  expect(editor.getText()).toBe('participants were recruited. The subjects agreed. Each subject signed.')
  expect(pasted).toEqual([])
})
