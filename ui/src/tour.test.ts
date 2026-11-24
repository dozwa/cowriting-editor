import { expect, test } from 'vitest'
import { cornerFor } from './tour'

const card = { width: 400, height: 340 }
const window = { width: 1389, height: 835 }

test('the tour card keeps off the buttons it speaks about', () => {
  // nothing to keep free: bottom left, where it does not cover the side panel
  expect(cornerFor(card, window, [])).toBe('left bottom')
  // the two buttons of the start page lie where the card would: it goes to the right
  const open = { left: 312, top: 505, right: 520, bottom: 547 }
  const create = { left: 529, top: 505, right: 691, bottom: 547 }
  expect(cornerFor(card, window, [open, create])).toBe('right bottom')
  // buttons in the bar at the top are not in its way
  expect(cornerFor(card, window, [{ left: 874, top: 70, right: 1107, bottom: 113 }])).toBe('left bottom')
  // in a narrow window the right is taken as well: it goes up
  const narrow = { width: 820, height: 835 }
  expect(cornerFor(card, narrow, [open, create])).toBe('right top')
  // nowhere free: where it always was
  expect(cornerFor(card, { width: 430, height: 420 }, [{ left: 0, top: 0, right: 430, bottom: 420 }])).toBe('left bottom')
})
