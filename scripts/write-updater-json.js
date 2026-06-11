#!/usr/bin/env node

import { readFileSync, writeFileSync } from 'node:fs';
import { parseArgs } from 'node:util';

import { buildLatestJson } from './release.js';

const parsed = parseArgs({
  options: {
    version: { type: 'string' },
    'notes-file': { type: 'string' },
    'pub-date': { type: 'string' },
    url: { type: 'string' },
    'signature-file': { type: 'string' },
    output: { type: 'string' }
  }
});

for (const option of ['version', 'notes-file', 'pub-date', 'url', 'signature-file', 'output']) {
  if (!parsed.values[option]) {
    throw new Error(`--${option} is required.`);
  }
}

const latestJson = buildLatestJson({
  version: parsed.values.version,
  notes: readFileSync(parsed.values['notes-file'], 'utf8'),
  pubDate: parsed.values['pub-date'],
  url: parsed.values.url,
  signature: readFileSync(parsed.values['signature-file'], 'utf8')
});

writeFileSync(parsed.values.output, `${JSON.stringify(latestJson, null, 2)}\n`);
