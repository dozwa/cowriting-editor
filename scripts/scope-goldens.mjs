// Runs the plugin's own scope and live-review functions over fixtures/scope-cases.json and prints
// what they return as JSON. Developer and CI only.
//
//   node scripts/scope-goldens.mjs <compiled plugin hooks> <fixtures folder>
//
// The scope functions live in the plugin's hooks module (register.tsx), which needs Claude Code to
// run. scripts/make-goldens.py compiles it to plain JavaScript first; here its host is replaced by
// a stand-in, since the functions called are pure.
import { registerHooks } from 'node:module'
import { readFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

const HOST = `data:text/javascript,${encodeURIComponent('const f = () => f; export const atom = f, read = f, update = f;')}`
registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier === 'claude-code') return { url: HOST, shortCircuit: true }
    if (specifier.startsWith('.') && !/\.[a-z]+$/.test(specifier)) return nextResolve(`${specifier}.js`, context)
    return nextResolve(specifier, context)
  },
})
globalThis.h = () => null // what the compiled JSX calls; nothing here draws

const [compiled, fixtures] = process.argv.slice(2).map(p => resolve(p))
const load = name => import(pathToFileURL(join(compiled, name)).href)
const [hooks, core, personas] = [await load('register.js'), await load('core.js'), await load('personas.js')]
const cases = JSON.parse(readFileSync(join(fixtures, 'scope-cases.json'), 'utf8'))
const plans = Object.fromEntries(Object.entries(cases.plans).map(([name, text]) => [name, hooks.parseScope(text)]))

const out = {
  plans,
  isEmpty: Object.fromEntries(Object.entries(plans).map(([name, plan]) => [name, hooks.isEmptyPlan(plan)])),
  progress: cases.progress.map(c => {
    const text = c.text ?? readFileSync(join(fixtures, 'documents', c.document), 'utf8')
    return hooks.scopeProgress(plans[c.plan], [{ file: c.document ?? 'text.md', text, format: 'markdown' }], c.checked, c.checkedAt, c.today ?? cases.today)
  }),
  check: cases.check.map(c => hooks.parseCheck(c.text, c.count)),
  numbered: cases.numbered.map(c => hooks.numberedText(c.files.map(f => ({ ...f, format: 'markdown' })), c.maxChars)),
  checkSystem: { en: hooks.scopeCheckSystem('en'), de: hooks.scopeCheckSystem('de') },
  wordsPerPage: hooks.WORDS_PER_PAGE,
  pauseMs: hooks.PAUSE_MS,
  watchDefault: core.WATCH_DEFAULT,
  hints: cases.hints.map(c => core.parseHints(c.text, c.personas)),
  excerpt: cases.excerpt.map(c => core.watchExcerpt(c.parts, c.maxChars)),
  system: cases.system.map(c => core.watchSystem(c.personas.map(id => personas.personaById(id)), c.language, c.profile, core.parseBriefs(c.briefs))),
}
process.stdout.write(`${JSON.stringify(out, null, 2)}\n`)
