export function normalizeDoctorReport(report = null) {
  const issues = Array.isArray(report?.issues)
    ? report.issues.map((issue) => ({
        ...issue,
        code: issue.code || '',
        severity: issue.severity || 'warning',
        entityName: issue.entityName || issue.entity_name || '',
        path: issue.path || '',
        repairable: Boolean(issue.repairable),
        suggestedAction: issue.suggestedAction || issue.suggested_action || ''
      }))
    : [];

  return {
    checkedAt: report?.checkedAt || report?.checked_at || '',
    schemaVersion: Number(report?.schemaVersion ?? report?.schema_version) || 0,
    latestSchemaVersion:
      Number(report?.latestSchemaVersion ?? report?.latest_schema_version) || 0,
    healthy: Boolean(report?.healthy),
    errorCount:
      Number(report?.errorCount ?? report?.error_count) ||
      issues.filter((issue) => issue.severity === 'error').length,
    warningCount:
      Number(report?.warningCount ?? report?.warning_count) ||
      issues.filter((issue) => issue.severity === 'warning').length,
    repairPreview: Boolean(report?.repairPreview ?? report?.repair_preview),
    issues
  };
}

export function doctorIssueTone(issue = {}) {
  return issue.severity === 'error' ? 'red' : 'amber';
}

export function hasStaleDeploymentRecords(report = null) {
  return Boolean(
    report?.issues?.some((issue) => issue.code === 'deployment_record_stale')
  );
}

export function normalizeStaleDeploymentRepairResult(result = null) {
  const removedDeploymentRecords = Number(
    result?.removedDeploymentRecords ?? result?.removed_deployment_records
  );

  return {
    removedDeploymentRecords:
      Number.isFinite(removedDeploymentRecords) && removedDeploymentRecords > 0
        ? removedDeploymentRecords
        : 0
  };
}
