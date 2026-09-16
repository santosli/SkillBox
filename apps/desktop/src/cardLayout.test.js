import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const css = await readFile(new URL('./styles.css', import.meta.url), 'utf8');
const colorsCss = await readFile(new URL('./colors.css', import.meta.url), 'utf8');
