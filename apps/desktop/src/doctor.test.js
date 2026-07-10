import assert from 'node:assert/strict';
import test from 'node:test';
import {
  doctorIssueTone,
  hasStaleDeploymentRecords,
  normalizeDoctorReport,
  normalizeStaleDeploymentRepairResult
} from './doctor.js';

test('normalizes Doctor snake case fields and issue repair previews', () => {
  assert.deepEqual(
    normalizeDoctorReport({
      checked_at: '2026-07-10T12:00:00Z',
      schema_version: 3,
      latest_schema_version: 3,
      healthy: false,
      error_count: 1,
      warning_count: 0,
      repair_preview: true,
      issues: [
        {
          code: 'deployment_target_missing',
          severity: 'error',
          entity_name: 'demo',
          path: '/tmp/demo',
          repairable: true,
          suggested_action: 'Remove the stale deployment record.'
        }
      ]
    }),
    {
      checkedAt: '2026-07-10T12:00:00Z',
      schemaVersion: 3,
      latestSchemaVersion: 3,
      healthy: false,
      errorCount: 1,
      warningCount: 0,
      repairPreview: true,
      issues: [
        {
          code: 'deployment_target_missing',
          severity: 'error',
          entity_name: 'demo',
          entityName: 'demo',
          path: '/tmp/demo',
          repairable: true,
          suggested_action: 'Remove the stale deployment record.',
          suggestedAction: 'Remove the stale deployment record.'
        }
      ]
    }
  );
  assert.equal(doctorIssueTone({ severity: 'error' }), 'red');
  assert.equal(doctorIssueTone({ severity: 'warning' }), 'amber');
});

test('detects stale deployment records and normalizes cleanup results', () => {
  assert.equal(
    hasStaleDeploymentRecords({
      issues: [{ code: 'deployment_record_stale' }]
    }),
    true
  );
  assert.equal(
    hasStaleDeploymentRecords({
      issues: [{ code: 'deployment_target_missing' }]
    }),
    false
  );
  assert.deepEqual(
    normalizeStaleDeploymentRepairResult({ removed_deployment_records: 2 }),
    { removedDeploymentRecords: 2 }
  );
  assert.deepEqual(normalizeStaleDeploymentRepairResult(null), {
    removedDeploymentRecords: 0
  });
});
