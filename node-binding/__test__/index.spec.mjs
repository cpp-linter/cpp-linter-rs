import test from 'ava'

import { main } from '../index.js'

test('invoke --help', async (t) => {
  t.is(await main(['cpp-linter', '--help']), 0)
})
