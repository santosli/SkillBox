import React from 'react';
import { parseUnifiedDiff } from './gitDiffView.js';

export function GitDiffView({ diff }) {
  const rows = parseUnifiedDiff(diff);

  if (rows.length === 0) {
    return <div className="gitDiffEmpty">No diff to show.</div>;
  }

  return (
    <div className="githubDiffScroller">
      <table className="githubDiffTable" aria-label="Unified diff">
        <tbody>
          {rows.map((row, index) => (
            <tr className={`githubDiffRow ${row.kind}`} key={`${index}-${row.kind}`}>
              <td className="githubDiffLineNumber">{row.oldLine ?? ''}</td>
              <td className="githubDiffLineNumber">{row.newLine ?? ''}</td>
              <td className="githubDiffMarker">{row.marker}</td>
              <td className="githubDiffCode">{row.content || ' '}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
