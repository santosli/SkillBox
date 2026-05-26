export function dashboardTabItems(counts) {
  return [
    { id: 'all', label: 'All', count: counts.total },
    { id: 'user', label: 'User', count: counts.user },
    { id: 'remote', label: 'Remote', count: counts.remote },
    { id: 'updates', label: 'Updates', count: counts.updates }
  ];
}

export function skillMatchesDashboardFilter(skill, filter, remoteUpdates) {
  if (filter === 'all') {
    return true;
  }
  if (filter === 'updates') {
    return hasRemoteUpdate(skill, remoteUpdates);
  }
  return skill.type === filter;
}

function hasRemoteUpdate(skill, remoteUpdates) {
  if (skill?.type !== 'remote') {
    return false;
  }

  const statuses = remoteUpdates?.statuses || [];
  const refreshedStatus = statuses.find((status) => status.skillName === skill.name);
  if (refreshedStatus) {
    return refreshedStatus.state === 'update_available' || refreshedStatus.updateAvailable;
  }

  const normalized = String(skill.status || '').toLowerCase();
  return normalized.includes('update available') || normalized.includes('new version');
}
