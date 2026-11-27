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
