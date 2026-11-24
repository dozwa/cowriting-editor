import type { ReactNode } from 'react'

export const plan = {
  // a time between reviews, as it reads after "every"
  span: (ms: number) => (ms === 60_000 ? 'minute' : ms % 60_000 === 0 ? `${ms / 60_000} minutes` : `${Math.round(ms / 1000)} seconds`),
  line: (n: number) => `line ${n}`,

  // live review
  liveReview: 'Live review',
  liveExplained: (seconds: number, gap: string, perHour: number) =>
    `While it is on, the paragraphs you change are sent to the AI once you pause for ${seconds} seconds: at most one review every ${gap} and ${perHour} an hour. You can change these numbers in the settings. Each review is logged. The experts give hints; they do not write for you.`,
  hints: 'Hints',
  noHintsYet: 'No hints yet. Keep writing.',
  turnOnForHints: 'Turn live review on to get hints while you write.',
  classifyOrReply: 'Classify or reply',
  whoWatches: 'Who watches',

  // progress
  ofYourPlan: (share: ReactNode) => <>{share} of your plan</>,
  daysLeft: (n: number) => `${n} ${n === 1 ? 'day' : 'days'} left`,
  daysPast: (n: number) => `${n} ${n === 1 ? 'day' : 'days'} past the deadline`,
  wordsPerDay: (n: number) => `${n} words a day to get there`,
  doneOf: (done: number, target: number) => `${done} of ${target}`,
  words: 'Words',
  figures: 'Figures',
  tables: 'Tables',
  references: 'References',
  openMarkers: (n: number) => `${n} ${n === 1 ? 'open marker' : 'open markers'} (TODO, ??, UNVERIFIED…) in the text`,
  sections: 'Sections',
  sectionWords: (words: number, target: number | undefined) => `${words}${target !== undefined ? ` of ${target}` : ''} words`,
  noHeadingYet: 'no heading of that name yet',
  markers: (n: number) => `${n} ${n === 1 ? 'marker' : 'markers'}`,
  points: 'Points',
  tickedByYou: '✓ ticked by you',
  notChecked: '· not checked',
  covered: '✓ covered',
  partial: '◐ partly',
  missing: '○ missing',
  lastChecked: (time: string) => `Last checked ${time}. A check is the AI's reading, not a verdict.`,

  // the plan itself
  planExplained: 'Your plan, in plain text: a goal, targets, sections and the points the text must make. It is yours; the AI cannot change it.',
  yourPlan: 'Your plan',
  savePlan: 'Save the plan',
  cancel: 'Cancel',
  noPlanYet: 'No plan yet. Say what the text should reach (words, sections, the points it must make) and see how far you are while you write.',
  makePlan: 'Make a plan',
  editPlan: 'Edit the plan',
  noPointsYet: 'Your plan has no points yet',
  checking: 'Checking…',
  checkPoints: 'Check the points',
  checkExplained: "The check sends your plan's points and the document to the AI and asks which points the text makes. It is logged.",
}
