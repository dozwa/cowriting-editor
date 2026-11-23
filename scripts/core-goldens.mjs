// Runs the plugin's own pure functions (plugin/hooks/core.ts) over fixtures/core-cases.json and the
// fixture logs and documents, and prints what they return as JSON. Developer and CI only.
//
//   node scripts/core-goldens.mjs <plugin repo> <fixtures folder>
import { registerHooks } from 'node:module'
import { readFileSync, readdirSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

// the plugin imports its own files without an extension
registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier.startsWith('.') && !/\.[a-z]+$/.test(specifier)) return nextResolve(`${specifier}.ts`, context)
    return nextResolve(specifier, context)
  },
})

const [plugin, fixtures] = process.argv.slice(2).map(p => resolve(p))
const core = await import(pathToFileURL(join(plugin, 'plugin/hooks/core.ts')).href)
const cases = JSON.parse(readFileSync(join(fixtures, 'core-cases.json'), 'utf8'))
const read = (...parts) => readFileSync(join(fixtures, ...parts), 'utf8')
const names = folder => readdirSync(join(fixtures, folder)).sort()
const formatOf = name => (name.endsWith('.tex') ? 'latex' : 'markdown')

// null where the plugin throws: its stats expect every response to list its personas
const statsOf = name => {
  try {
    return core.computeStats(core.parseLog(read('logs', name)), new Date(cases.now))
  } catch {
    return null
  }
}

const out = {
  stats: Object.fromEntries(names('logs').map(name => [name, statsOf(name)])),
  entries: Object.fromEntries(names('logs').map(name => [name, core.parseLog(read('logs', name)).length])),
  words: Object.fromEntries(names('documents').map(name => [name, core.countWords(read('documents', name), formatOf(name))])),
  paragraphs: Object.fromEntries(names('documents').map(name => [name, core.paragraphs(read('documents', name))])),
  changed: Object.fromEntries(
    names('documents')
      .filter(name => name.includes('.before.'))
      .map(name => [name, core.changedParagraphs(read('documents', name), read('documents', name.replace('.before.', '.after.')), formatOf(name))]),
  ),
  category: cases.category.map(c => core.categoryFor(c.command ?? undefined, c.text)),
  readable: cases.readable.map(text => core.readable(text)),
  guard: cases.guard.paths.map(path => ({
    path,
    authorFile: core.authorFileOf(path, cases.guard.root),
    isLog: core.isLogPath(path, cases.guard.root),
    liesIn: core.liesIn(path, cases.guard.root),
  })),
  logFiles: cases.logFiles.map(c => ({ rewritten: core.rewrittenLogFiles(c.before, c.after), stray: core.strayLogFiles(c.before, c.after, c.month) })),
  changedFiles: cases.changedFiles.map(c => core.changedFiles(c.before, c.after)),
  monthFile: cases.monthFile.map(ts => core.monthFile(new Date(ts))),
  briefs: {
    parsed: core.parseBriefs(cases.briefs.text),
    for: Object.fromEntries(cases.briefs.for.map(id => [id, core.briefFor(core.parseBriefs(cases.briefs.text), id)])),
    set: cases.briefs.set.map(c => core.setBrief(cases.briefs.text, c.key, c.body)),
    append: cases.briefs.append.map(c => core.appendBrief(cases.briefs.text, c.key, c.lines)),
    empty: cases.briefs.empty.map(c => core.setBrief('', c.key, c.body)),
  },
}
process.stdout.write(`${JSON.stringify(out, null, 2)}\n`)
