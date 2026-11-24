export const shared = {
  /** What the four classifications mean, in the words the buttons use (the plugin's `USE_LABELS`). */
  use: {
    adopted: { button: '✓ used', meaning: 'as suggested' },
    adapted: { button: '✎ changed', meaning: 'in your own way' },
    rejected: { button: '✗ not used', meaning: 'decided against it' },
    info: { button: 'ℹ info', meaning: 'only informed you' },
  },
  modelUntested: 'Untested model: it may not keep to the assist-only rules. Everything it says is logged all the same, and it cannot change your text.',
  modelFailed: 'This model did not keep to the assist-only rules when tested. Everything it says is logged all the same, and it cannot change your text.',
}
