// The editor's own words, in every language it has.
//
// A language is a folder beside this file (`en/`, `de/`) with one file per part of the window,
// and one line in LANGUAGES below. English is the pattern: every other language must have
// exactly its entries (the compiler says which are missing). An entry is a text, or a function
// when the text has numbers, names or links in it.
//
// Components read `m` when they draw: `m.app.save`, `m.app.words(3)`. Nothing keeps a text in a
// constant, or it would stay in the language the editor started in.
import { de } from './de'
import { en } from './en'

export type Messages = typeof en

export const LANGUAGES = {
  en: { name: 'English', messages: en },
  de: { name: 'Deutsch', messages: de },
} satisfies Record<string, { name: string; messages: Messages }>

export type LanguageId = keyof typeof LANGUAGES

/** The words in the language the editor shows now. A live binding: importers see the change. */
export let m: Messages = en
export let language: LanguageId = 'en'

/** The language for 'system': the computer's, if the editor has it, else English. */
export function languageFor(chosen: 'system' | LanguageId, system: string = navigator.language): LanguageId {
  if (chosen !== 'system') return chosen in LANGUAGES ? chosen : 'en'
  const short = system.toLowerCase().split('-')[0] ?? 'en'
  return short in LANGUAGES ? (short as LanguageId) : 'en'
}

/** Switches the editor's words. Whoever calls this draws the window again. */
export function setLanguage(id: LanguageId) {
  language = id
  m = LANGUAGES[id].messages
  document.documentElement.lang = id
}
