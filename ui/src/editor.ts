// The writing surface: markdown source, styled in place. Headings look like headings and bold
// looks bold; the marks (`#`, `**`, backticks) are hidden except on the line the cursor is in,
// so what is saved is always plain markdown and nothing is hidden from the author for long.
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands'
import { markdown, markdownLanguage } from '@codemirror/lang-markdown'
import { yamlFrontmatter } from '@codemirror/lang-yaml'
import { HighlightStyle, syntaxHighlighting, syntaxTree } from '@codemirror/language'
import { Compartment, EditorSelection, EditorState, type Range } from '@codemirror/state'
import { Decoration, type DecorationSet, EditorView, keymap, lineNumbers, ViewPlugin, type ViewUpdate } from '@codemirror/view'
import { tags } from '@lezer/highlight'

const styles = HighlightStyle.define([
  { tag: tags.heading1, class: 'md-h1' },
  { tag: tags.heading2, class: 'md-h2' },
  { tag: tags.heading3, class: 'md-h3' },
  { tag: [tags.heading4, tags.heading5, tags.heading6], class: 'md-h4' },
  { tag: tags.strong, class: 'md-strong' },
  { tag: tags.emphasis, class: 'md-em' },
  { tag: tags.strikethrough, class: 'md-strike' },
  { tag: tags.monospace, class: 'md-code' },
  { tag: tags.link, class: 'md-link' },
  { tag: tags.url, class: 'md-url' },
  { tag: tags.quote, class: 'md-quote' },
  { tag: [tags.processingInstruction, tags.contentSeparator], class: 'md-mark' },
  { tag: tags.comment, class: 'md-comment' },
])

/** Syntax nodes that are only marks; hidden where the cursor is not. */
const MARKS = new Set(['HeaderMark', 'EmphasisMark', 'CodeMark', 'StrikethroughMark'])
const hidden = Decoration.replace({})
/** A line of the YAML block at the top: set apart from the prose. */
const frontLine = Decoration.line({ class: 'md-front' })
/** A line of a table: it stays source, in a face of one width, so that its columns stand under each other. */
const tableLine = Decoration.line({ class: 'md-table' })

function hiddenMarks(view: EditorView): DecorationSet {
  const { state } = view
  const active = new Set<number>()
  for (const range of state.selection.ranges) {
    const [from, to] = [state.doc.lineAt(range.from).number, state.doc.lineAt(range.to).number]
    for (let n = from; n <= to; n++) active.add(n)
  }
  const out: Range<Decoration>[] = []
  for (const visible of view.visibleRanges) {
    syntaxTree(state).iterate({
      from: visible.from,
      to: visible.to,
      enter(node) {
        if (node.name === 'Frontmatter') {
          for (let n = state.doc.lineAt(node.from).number; n <= state.doc.lineAt(node.to).number; n++) out.push(frontLine.range(state.doc.line(n).from))
          return
        }
        if (node.name === 'Table') {
          for (let n = state.doc.lineAt(node.from).number; n <= state.doc.lineAt(node.to).number; n++) out.push(tableLine.range(state.doc.line(n).from))
          return
        }
        if (!MARKS.has(node.name) || active.has(state.doc.lineAt(node.from).number)) return
        // a fenced block keeps its fences: they are lines of their own
        if (node.name === 'CodeMark' && node.node.parent?.name === 'FencedCode') return
        // the space after a heading's hashes goes with them
        const to = node.name === 'HeaderMark' && state.sliceDoc(node.to, node.to + 1) === ' ' ? node.to + 1 : node.to
        if (to > node.from) out.push(hidden.range(node.from, to))
      },
    })
  }
  return Decoration.set(out, true)
}

const livePreview = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet
    constructor(view: EditorView) {
      this.decorations = hiddenMarks(view)
    }
    update(update: ViewUpdate) {
      if (update.docChanged || update.selectionSet || update.viewportChanged || syntaxTree(update.startState) !== syntaxTree(update.state)) {
        this.decorations = hiddenMarks(update.view)
      }
    }
  },
  { decorations: plugin => plugin.decorations },
)

/** What a format button writes into the text. */
export type Format = 'title' | 'heading' | 'subheading' | 'bold' | 'italic' | 'bullets' | 'numbers' | 'quote' | 'link' | 'image' | 'table' | 'citation' | 'footnote' | 'comment'

/** How the text is shown: with line numbers or without; styled in place, or as the markdown source it is. */
/** `language` is the one the text is written in ('en', 'de'): the spelling check goes by it. */
export type View = { lineNumbers: boolean; source: boolean; spellcheck: boolean; language: string }

const INLINE: Partial<Record<Format, [mark: string, placeholder: string]>> = { bold: ['**', 'bold text'], italic: ['*', 'italic text'] }
const BLOCK: Partial<Record<Format, (n: number) => string>> = { title: () => '# ', heading: () => '## ', subheading: () => '### ', bullets: () => '- ', numbers: n => `${n}. `, quote: () => '> ' }
/** What a line can start with as a block: heading hashes, a list mark, a quote mark. */
const BLOCK_MARK = /^(#{1,6}\s+|[-*+]\s+|\d+[.)]\s+|>\s?)/
const TABLE = '| Column | Column |\n|--------|--------|\n| cell   | cell   |\n| cell   | cell   |'

/** Marks around what is selected, or around a placeholder that is then selected; a second time takes them off. */
function wrap(view: EditorView, mark: string, placeholder: string) {
  const { state } = view
  view.dispatch(
    state.changeByRange(range => {
      const n = mark.length
      const [before, after] = [state.sliceDoc(range.from - n, range.from), state.sliceDoc(range.to, range.to + n)]
      // one star inside two is bold, not italic
      const isOther = mark === '*' && state.sliceDoc(range.from - 2, range.from) === '**' && state.sliceDoc(range.from - 3, range.from) !== '***'
      if (before === mark && after === mark && !isOther) {
        // the example a first press wrote goes with its marks: nothing is left behind
        if (state.sliceDoc(range.from, range.to) === placeholder) return { changes: { from: range.from - n, to: range.to + n }, range: EditorSelection.cursor(range.from - n) }
        return { changes: [{ from: range.from - n, to: range.from }, { from: range.to, to: range.to + n }], range: EditorSelection.range(range.from - n, range.to - n) }
      }
      const inner = range.empty ? placeholder : state.sliceDoc(range.from, range.to)
      return { changes: { from: range.from, to: range.to, insert: mark + inner + mark }, range: EditorSelection.range(range.from + n, range.from + n + inner.length) }
    }),
    { userEvent: 'input.format', scrollIntoView: true },
  )
}

/** Every selected line starts with this mark, in place of the block mark it had; a second time takes it off. */
function prefix(view: EditorView, markOf: (n: number) => string) {
  const { state } = view
  const { from, to } = state.selection.main
  const lines = []
  for (let n = state.doc.lineAt(from).number; n <= state.doc.lineAt(to).number; n++) lines.push(state.doc.line(n))
  const isOn = lines.every((line, i) => line.text.startsWith(markOf(i + 1)))
  const changes = lines.map((line, i) => ({ from: line.from, to: line.from + (BLOCK_MARK.exec(line.text)?.[0].length ?? 0), insert: isOn ? '' : markOf(i + 1) }))
  const mapped = state.changes(changes)
  // the cursor ends after the last line, where the author goes on writing
  view.dispatch({ changes: mapped, selection: { anchor: mapped.mapPos(lines[lines.length - 1]!.to, 1) }, userEvent: 'input.format', scrollIntoView: true })
}

/** Text in place of the selection, with one part of it selected for the author to type over. */
function put(view: EditorView, text: string, select: string) {
  const { from, to } = view.state.selection.main
  const at = from + text.indexOf(select)
  view.dispatch({ changes: { from, to, insert: text }, selection: { anchor: at, head: at + select.length }, userEvent: 'input.format', scrollIntoView: true })
}

/** The marks of a comment, which is in the file and in no export. */
const [COMMENT_OPEN, COMMENT_CLOSE, COMMENT_EXAMPLE] = ['<!--', '-->', 'comment']

/**
 * A comment around what is selected, or around the line the cursor is in; a second time takes the marks off.
 * Comments cannot lie inside each other (the first `-->` ends them all), so marks inside the selection go: it becomes one comment.
 */
function comment(view: EditorView) {
  const { state } = view
  const text = state.doc.toString()
  let { from, to } = state.selection.main
  /** The comment these two places both lie in, marks included. */
  const around = (from: number, to: number): [number, number] | null => {
    const open = text.lastIndexOf(COMMENT_OPEN, from)
    const close = open === -1 ? -1 : text.indexOf(COMMENT_CLOSE, open + COMMENT_OPEN.length)
    return close !== -1 && to <= close + COMMENT_CLOSE.length ? [open, close + COMMENT_CLOSE.length] : null
  }
  let found = around(from, to)
  if (found === null && from === to) {
    // nothing selected: the line, without the space around it
    const line = state.doc.lineAt(from)
    from = line.from + (line.text.length - line.text.trimStart().length)
    to = Math.max(from, line.from + line.text.trimEnd().length)
    found = from === to ? null : around(from, to)
  }
  const dispatch = (at: number, until: number, insert: string, selectFrom: number, selectTo: number) =>
    view.dispatch({ changes: { from: at, to: until, insert }, selection: { anchor: selectFrom, head: selectTo }, userEvent: 'input.format', scrollIntoView: true })
  if (found !== null) {
    const inner = text.slice(found[0] + COMMENT_OPEN.length, found[1] - COMMENT_CLOSE.length).replace(/^ /, '').replace(/ $/, '')
    // the example a first press wrote goes with its marks
    if (inner === COMMENT_EXAMPLE) dispatch(found[0], found[1], '', found[0], found[0])
    else dispatch(found[0], found[1], inner, found[0], found[0] + inner.length)
    return
  }
  // a comment the selection cuts through is taken whole, or half of its marks would be left behind
  if (from !== to) {
    const [first, last] = [around(from, from), around(to, to)]
    if (first !== null && from < first[1]) from = first[0]
    if (last !== null && to > last[0]) to = last[1]
  }
  const inner = from === to ? COMMENT_EXAMPLE : text.slice(from, to).replace(/<!-- ?| ?-->/g, '')
  dispatch(from, to, `${COMMENT_OPEN} ${inner} ${COMMENT_CLOSE}`, from + COMMENT_OPEN.length + 1, from + COMMENT_OPEN.length + 1 + inner.length)
}

function format(view: EditorView, kind: Format) {
  const { state } = view
  const { from, to, empty } = state.selection.main
  const chosen = state.sliceDoc(from, to)
  const [inline, block] = [INLINE[kind], BLOCK[kind]]
  if (inline !== undefined) wrap(view, inline[0], inline[1])
  else if (block !== undefined) prefix(view, block)
  else if (kind === 'link') put(view, `[${empty ? 'link text' : chosen}](https://)`, 'https://')
  else if (kind === 'image') put(view, `![${empty ? 'description' : chosen}](picture.png)`, 'picture.png')
  else if (kind === 'footnote') put(view, `^[${empty ? 'note' : chosen}]`, empty ? 'note' : chosen)
  else if (kind === 'comment') comment(view)
  else if (kind === 'citation') {
    view.dispatch({ changes: { from, to, insert: '[@]' }, selection: { anchor: from + 2 }, userEvent: 'input.format', scrollIntoView: true })
  } else if (kind === 'table') {
    // a table is a block of its own: an empty line before and after
    const line = state.doc.lineAt(from)
    const before = line.text.trim() === '' ? (line.number > 1 && state.doc.line(line.number - 1).text.trim() !== '' ? '\n' : '') : '\n\n'
    const at = line.text.trim() === '' ? from : line.to
    view.dispatch({ changes: { from: at, to: at, insert: `${before}${TABLE}\n` }, selection: { anchor: at + before.length + 2, head: at + before.length + 8 }, userEvent: 'input.format', scrollIntoView: true })
  }
  view.focus()
}

export type Editor = {
  getText(): string
  setText(text: string): void
  /** Puts text where the cursor is, in place of what is selected. */
  insert(text: string): void
  /** Puts the cursor on a 1-based line and scrolls it into view. */
  jumpToLine(line: number): void
  /** Writes a markdown element at the cursor or around what is selected. */
  format(kind: Format): void
  setView(view: View): void
  focus(): void
  destroy(): void
}

export type EditorHandlers = {
  onChange(): void
  /** Text was pasted or dropped into the document; `line` is where it starts now, 1-based. */
  onPaste(pasted: string, line: number): void
}

export function createEditor(parent: HTMLElement, handlers: EditorHandlers): Editor {
  const [numbering, styling, spelling] = [new Compartment(), new Compartment(), new Compartment()]
  const attributes = (spellcheck: boolean, language: string) => EditorView.contentAttributes.of({ spellcheck: String(spellcheck), lang: language, 'aria-label': 'Document' })
  const view = new EditorView({
    parent,
    state: EditorState.create({
      doc: '',
      extensions: [
        history(),
        keymap.of([
          indentWithTab,
          ...defaultKeymap,
          ...historyKeymap,
        ]),
        // a YAML block at the top is front matter, not a heading
        yamlFrontmatter({ content: markdown({ base: markdownLanguage }) }),
        syntaxHighlighting(styles),
        numbering.of([]),
        styling.of(livePreview),
        EditorView.lineWrapping,
        spelling.of(attributes(true, 'en')),
        EditorView.updateListener.of(update => {
          if (!update.docChanged) return
          handlers.onChange()
          for (const transaction of update.transactions) {
            if (!transaction.isUserEvent('input.paste') && !transaction.isUserEvent('input.drop')) continue
            transaction.changes.iterChanges((_fromA, _toA, fromB, _toB, inserted) => {
              if (inserted.length > 0) handlers.onPaste(inserted.toString(), transaction.newDoc.lineAt(fromB).number)
            })
          }
        }),
      ],
    }),
  })
  return {
    getText: () => view.state.doc.toString(),
    setText: text => view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text }, selection: { anchor: 0 } }),
    insert: text => {
      view.dispatch(view.state.replaceSelection(text))
      view.focus()
    },
    jumpToLine: line => {
      const target = view.state.doc.line(Math.min(Math.max(1, line), view.state.doc.lines))
      view.dispatch({ selection: { anchor: target.from }, effects: EditorView.scrollIntoView(target.from, { y: 'center' }) })
      view.focus()
    },
    format: kind => format(view, kind),
    setView: ({ lineNumbers: isNumbered, source, spellcheck, language }) =>
      view.dispatch({
        effects: [numbering.reconfigure(isNumbered ? lineNumbers() : []), styling.reconfigure(source ? EditorView.editorAttributes.of({ class: 'cm-source' }) : livePreview), spelling.reconfigure(attributes(spellcheck, language))],
      }),
    focus: () => view.focus(),
    destroy: () => view.destroy(),
  }
}
