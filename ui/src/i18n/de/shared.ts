import type { shared as pattern } from '../en/shared'

export const shared: typeof pattern = {
  use: {
    adopted: { button: '✓ übernommen', meaning: 'wie vorgeschlagen' },
    adapted: { button: '✎ angepasst', meaning: 'auf eigene Weise' },
    rejected: { button: '✗ verworfen', meaning: 'dagegen entschieden' },
    info: { button: 'ℹ nur Information', meaning: 'hat Sie nur informiert' },
  },
  modelUntested: 'Ungetestetes Modell: Es hält sich womöglich nicht an die Assist-only-Regeln. Alles, was es sagt, wird trotzdem protokolliert, und Ihren Text kann es nicht ändern.',
  modelFailed: 'Dieses Modell hat sich im Test nicht an die Assist-only-Regeln gehalten. Alles, was es sagt, wird trotzdem protokolliert, und Ihren Text kann es nicht ändern.',
}
