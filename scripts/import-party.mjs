// Writes crates/core/party/catalog.json from the plugin's catalogue (plugin/hooks/personas.ts) and
// its agent files (plugin/agents/<id>.md): who the experts are, how they look, and what each is
// told about its perspective. The plugin's ground rules and output format are left out: the
// editor has its own, for one document and no slash commands. Developer only.
//
//   node scripts/import-party.mjs <plugin repo>
import { registerHooks } from 'node:module'
import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier.startsWith('.') && !/\.[a-z]+$/.test(specifier)) return nextResolve(`${specifier}.ts`, context)
    return nextResolve(specifier, context)
  },
})

const plugin = resolve(process.argv[2])
const { PERSONAS, SHARED_PALETTE, DEFAULT_PARTY } = await import(pathToFileURL(join(plugin, 'plugin/hooks/personas.ts')).href)

/** The agent file's own part: after the frontmatter, before the shared rules. */
const perspective = id => {
  const text = readFileSync(join(plugin, 'plugin/agents', `${id}.md`), 'utf8')
  const body = text.replace(/^---\n[\s\S]*?\n---\n/, '')
  return body.split(/^## Ground rules/m)[0].trim()
}

const out = {
  sharedPalette: SHARED_PALETTE,
  personas: PERSONAS.map(p => ({
    id: p.id,
    name: p.name,
    short: p.short,
    role: p.role,
    category: p.category,
    isDefaultMember: DEFAULT_PARTY.includes(p.id),
    palette: p.palette,
    sprite: p.sprite,
    prompt: perspective(p.id),
  })),
}
const target = join(dirname(fileURLToPath(import.meta.url)), '../crates/core/party/catalog.json')
writeFileSync(target, `${JSON.stringify(out, null, 2)}\n`)
console.log(`${out.personas.length} personas written to ${target}`)
