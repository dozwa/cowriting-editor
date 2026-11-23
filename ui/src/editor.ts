// The writing surface: markdown source, styled in place. Headings look like headings and bold
// looks bold; the marks (`#`, `**`, backticks) are hidden except on the line the cursor is in,
// so what is saved is always plain markdown and nothing is hidden from the author for long.
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands'
import { markdown, markdownLanguage } from '@codemirror/lang-markdown'
import { yamlFrontmatter } from '@codemirror/lang-yaml'
import { HighlightStyle, syntaxHighlighting, syntaxTree } from '@codemirror/language'
import { EditorState, type Range } from '@codemirror/state'
import { Decoration, type DecorationSet, EditorView, keymap, ViewPlugin, type ViewUpdate } from '@codemirror/view'
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

export type Editor = {
  getText(): string
  setText(text: string): void
  /** Puts text where the cursor is, in place of what is selected. */
  insert(text: string): void
  /** Puts the cursor on a 1-based line and scrolls it into view. */
  jumpToLine(line: number): void
  focus(): void
  destroy(): void
}

export type EditorHandlers = {
  onChange(): void
  onSave(): void
  /** Text was pasted or dropped into the document; `line` is where it starts now, 1-based. */
  onPaste(pasted: string, line: number): void
}

export function createEditor(parent: HTMLElement, handlers: EditorHandlers): Editor {
  const view = new EditorView({
    parent,
    state: EditorState.create({
      doc: '',
      extensions: [
        history(),
        keymap.of([
          { key: 'Mod-s', preventDefault: true, run: () => (handlers.onSave(), true) },
          indentWithTab,
          ...defaultKeymap,
          ...historyKeymap,
        ]),
        // a YAML block at the top is front matter, not a heading
        yamlFrontmatter({ content: markdown({ base: markdownLanguage }) }),
        syntaxHighlighting(styles),
        livePreview,
        EditorView.lineWrapping,
        EditorView.contentAttributes.of({ spellcheck: 'true', 'aria-label': 'Document' }),
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
    focus: () => view.focus(),
    destroy: () => view.destroy(),
  }
}
