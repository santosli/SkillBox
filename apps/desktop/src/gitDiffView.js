const HUNK_HEADER_PATTERN = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/;

function makeRow(kind, marker, content, oldLine = null, newLine = null) {
  return {
    kind,
    marker,
    content,
    oldLine,
    newLine
  };
}

function isDiffHeaderMetadata(line) {
  return (
    line.startsWith('diff --git ') ||
    line.startsWith('index ') ||
    line.startsWith('--- ') ||
    line.startsWith('+++ ')
  );
}

export function parseUnifiedDiff(diff) {
  if (!diff) {
    return [];
  }

  const rows = [];
  let oldLine = null;
  let newLine = null;

  const lines = diff.split('\n');
  if (lines[lines.length - 1] === '') {
    lines.pop();
  }

  for (const line of lines) {
    const hunkMatch = line.match(HUNK_HEADER_PATTERN);
    if (hunkMatch) {
      oldLine = Number(hunkMatch[1]);
      newLine = Number(hunkMatch[2]);
      rows.push(makeRow('hunk', '', line));
      continue;
    }

    if (line.startsWith('@@')) {
      oldLine = oldLine ?? 1;
      newLine = newLine ?? 1;
      rows.push(makeRow('hunk', '', line));
      continue;
    }

    if (oldLine === null || newLine === null) {
      if (isDiffHeaderMetadata(line)) {
        continue;
      }
      rows.push(makeRow('meta', '', line));
      continue;
    }

    if (line.startsWith('+') && !line.startsWith('+++')) {
      rows.push(makeRow('addition', '+', line.slice(1), null, newLine));
      newLine += 1;
      continue;
    }

    if (line.startsWith('-') && !line.startsWith('---')) {
      rows.push(makeRow('deletion', '-', line.slice(1), oldLine, null));
      oldLine += 1;
      continue;
    }

    if (line.startsWith(' ')) {
      rows.push(makeRow('context', '', line.slice(1), oldLine, newLine));
      oldLine += 1;
      newLine += 1;
      continue;
    }

    rows.push(makeRow('meta', '', line));
  }

  return rows;
}
