// Development only: the same commands over HTTP, for running the interface in an ordinary
// browser against `cargo run -p cowriting-suites --bin bridge`. The bridge serves one document.
import type { Api, AskEvent, AskResult } from './api'

async function call<T>(name: string, args: unknown = {}): Promise<T> {
  const response = await fetch(`/bridge/${name}`, { method: 'POST', body: JSON.stringify(args) })
  const reply = (await response.json()) as { ok?: T; error?: string }
  if (reply.error !== undefined) throw reply.error
  return reply.ok as T
}

export const bridgeApi: Api = {
  getSettings: () => call('get_settings'),
  setSettings: (settings, apiKey) => call('set_settings', { settings, apiKey }),
  testConnection: (settings, apiKey) => call('test_connection', { settings, apiKey }),
  pickDocument: () => call('pick_document'),
  pickNewDocument: async () => null,
  confirmLeave: async () => true,
  openDocument: path => call('open_document', { path }),
  newDocument: path => call('new_document', { path }),
  saveDocument: text => call('save_document', { text }),
  reportPaste: (pasted, line, documentText) => call('report_paste', { pasted, line, documentText }),
  commentAiText: (id, note) => call('comment_ai_text', { id, note }),
  liveView: () => call('live'),
  setLive: (isOn, personas, documentText) => call('set_live', { isOn, personas, documentText }),
  liveReview: documentText => call('live_review', { documentText }),
  scope: documentText => call('scope', { documentText }),
  setScope: (text, documentText) => call('set_scope', { text, documentText }),
  checkPoints: documentText => call('check_points', { documentText }),
  exportSetup: () => call('export_setup'),
  pickExport: (_format, suggested) => call('pick_export', { suggested }),
  exportDocument: (format, target, documentText) => call('export_document', { format, target, documentText }),
  sources: documentText => call('sources', { documentText }),
  pickBibliography: () => call('pick_bibliography'),
  addBibliography: (path, documentText) => call('add_bibliography', { path, documentText }),
  removeBibliography: path => call('remove_bibliography', { path }),
  searchSources: (query, documentText) => call('search_sources', { query, documentText }),
  checkReferences: documentText => call('check_references', { documentText }),
  ask: async (input, onEvent) => {
    const response = await fetch('/bridge/ask', { method: 'POST', body: JSON.stringify({ input }) })
    const reader = response.body!.getReader()
    const decoder = new TextDecoder()
    let pending = ''
    let outcome: { result?: AskResult; error?: string } = { error: 'The bridge closed the connection without an answer.' }
    for (;;) {
      const { done, value } = await reader.read()
      pending += decoder.decode(value, { stream: !done })
      const lines = pending.split('\n')
      pending = lines.pop() ?? ''
      for (const line of lines.filter(l => l.trim() !== '')) {
        const message = JSON.parse(line) as { event?: AskEvent; result?: AskResult; error?: string }
        if (message.event !== undefined) onEvent(message.event)
        else outcome = message
      }
      if (done) break
    }
    if (outcome.result === undefined) throw outcome.error
    return outcome.result
  },
  logView: documentText => call('log_view', { documentText }),
  thread: id => call('thread', { id }),
  annotate: (reference, use, note) => call('annotate', { reference, use, note }),
  exportDisclosure: options => call('export_disclosure', { options }),
  party: () => call('party'),
  setBrief: (key, body) => call('set_brief', { key, body }),
  setMember: (id, isMember) => call('set_member', { id, isMember }),
  addExpert: (name, role, looksAt) => call('add_expert', { name, role, looksAt }),
}
