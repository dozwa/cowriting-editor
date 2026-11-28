// Development only: the same commands over HTTP, for running the interface in an ordinary
// browser against `cargo run -p cowriting-suites --bin bridge`. The bridge serves one document.
import { type Api, type AskEvent, type AskResult, GUIDES } from './api'

async function call<T>(name: string, args: unknown = {}): Promise<T> {
  const response = await fetch(`/bridge/${name}`, { method: 'POST', body: JSON.stringify(args) })
  const reply = (await response.json()) as { ok?: T; error?: string }
  if (reply.error !== undefined) throw reply.error
  return reply.ok as T
}

export const bridgeApi: Api = {
  getSettings: () => call('get_settings'),
  setSettings: (settings, apiKey) => call('set_settings', { settings, apiKey }),
  setApiKey: () => Promise.reject('The bridge keeps no keys.'),
  testConnection: (settings, apiKey) => call('test_connection', { settings, apiKey }),
  setPreferences: preferences => call('set_preferences', { preferences }),
  showWritten: () => Promise.reject('The bridge shows no files.'),
  showSettingsFolder: () => Promise.reject('The bridge has no settings folder.'),
  pickFolder: async () => null,
  pickDocument: () => call('pick_document'),
  pickNewDocument: () => call('pick_new_document'),
  pickProjectFile: () => call('pick_new_document'),
  pickProject: () => call('pick_project'),
  projectFiles: documentText => call('project_files', { documentText }),
  makeProject: path => call('make_project', { path }),
  addProjectFile: path => call('add_project_file', { path }),
  removeProjectFile: path => call('remove_project_file', { path }),
  orderProjectFiles: paths => call('order_project_files', { paths }),
  renameProjectFile: (path, name) => call('rename_project_file', { path, name }),
  setProjectGoal: (goal, words) => call('set_project_goal', { goal, words }),
  confirmLeave: async () => true,
  openDocument: path => call('open_document', { path }),
  newDocument: path => call('new_document', { path }),
  adoptLog: name => call('adopt_log', { name }),
  pickMove: () => call('pick_move'),
  sendFeedback: () => Promise.reject('The bridge opens no reports. Copy the text instead.'),
  usageAnswered: () => Promise.reject('The bridge sends no usage figures.'),
  usagePreview: async () => ['', ''],
  privacyStatement: async () => '',
  recordSeen: async () => {},
  recentDiagnostics: async () => '',
  clearDiagnostics: async () => {},
  pickReportFile: async () => null,
  writeReportFile: () => Promise.reject('The bridge keeps no technical log.'),
  openGuide: async which => (which === 'repository' || which === 'coffee' ? Promise.reject('The bridge opens guides only.') : void window.open(`https://${GUIDES[which].address}`, '_blank', 'noopener')),
  stateExternal: stated => call('state_external', { stated }),
  pickChat: () => call('pick_chat'),
  readChats: path => call('read_chats', { path }),
  withdrawExternal: id => call('withdraw_external', { id }),
  project: () => call('project'),
  pickLinkedFile: async () => null,
  pickPicture: () => Promise.reject('The bridge chooses no pictures. Type the path into the text.'),
  picture: path => call('picture', { path }),
  copyPicture: (path, into) => call('copy_picture', { path, into: into ?? null }),
  pastePicture: (kind, content) => call('paste_picture', { kind, content }),
  picturesFolder: () => call('pictures_folder'),
  // a browser hands a dropped file to the page, without its path: the editor takes it like a pasted picture
  onFilesDropped: () => () => {},
  linkFile: (path, personas) => call('link_file', { path, personas }),
  unlinkFile: path => call('unlink_file', { path }),
  setProject: () => Promise.reject('The bridge keeps general settings only.'),
  setPrompt: (id, prompt) => call('set_prompt', { id, prompt }),
  supportAnswered: async () => {},
  tourSeen: async () => {},
  countSelection: (text, from, to) => call('count_selection', { text, from, to }),
  picturesToMove: (target, text) => call('pictures_to_move', { target, text }),
  moveDocument: (target, text, pictures) => call('move_document', { target, text, pictures: pictures ?? null }),
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
  keepExportChoice: pdf => call('keep_export_choice', { pdf }),
  layouts: () => call('layouts'),
  keepLayout: (name, pdf) => call('keep_layout', { name, pdf }),
  removeLayout: name => call('remove_layout', { name }),
  importLayout: async () => null,
  exportLayout: async () => null,
  pickExport: (_format, suggested) => call('pick_export', { suggested }),
  previewDocument: (format, documentText, pdf) => call('preview_document', { format, documentText, pdf }),
  exportDocument: (format, target, documentText, pdf) => call('export_document', { format, target, documentText, pdf }),
  sources: documentText => call('sources', { documentText }),
  pickBibliography: () => call('pick_bibliography'),
  addBibliography: (path, documentText) => call('add_bibliography', { path, documentText }),
  removeBibliography: path => call('remove_bibliography', { path }),
  pickStyle: async () => null,
  pickWordTemplate: async () => null,
  setStyle: (path, documentText) => call('set_style', { path, documentText }),
  removeStyle: () => call('remove_style'),
  searchSources: (query, documentText) => call('search_sources', { query, documentText }),
  checkReferences: documentText => call('check_references', { documentText }),
  stopAnswer: () => Promise.reject('The bridge cannot stop an answer.'),
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
  hintMarks: () => call('hint_marks'),
  setHintMarks: (turn, done) => call('set_hint_marks', { turn, done }),
  exportDisclosure: options => call('export_disclosure', { options }),
  party: () => call('party'),
  setBrief: (key, body) => call('set_brief', { key, body }),
  setMember: (id, isMember) => call('set_member', { id, isMember }),
  addExpert: (name, role, looksAt) => call('add_expert', { name, role, looksAt }),
}
