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

export function skillMatchesDashboardFilters(skill, filters = {}) {
  const type = filters.type || filters.filter || 'all';

  if (!skillMatchesDashboardFilter(skill, type, filters.remoteSkillUpdates)) {
    return false;
  }

  if (filters.favoritesOnly && !skill.isFavorite) {
    return false;
  }

  if (filters.tag && filters.tag !== 'all' && !(skill.displayTags || []).includes(filters.tag)) {
    return false;
  }

  if (filters.agent && filters.agent !== 'all' && skill.agentLabel !== filters.agent) {
    return false;
  }

  const query = String(filters.query || '').trim().toLowerCase();
  if (!query) {
    return true;
  }

  return [
    skill.name,
    skill.description,
    skill.sourceRoot,
    skill.sourceLabel,
    skill.agentLabel,
    skill.status,
    skill.statusLabel,
    skill.type,
    ...(skill.displayTags || []),
    ...(skill.installedAgents || []).flatMap((agent) => [agent.id, agent.label])
  ]
    .filter(Boolean)
    .some((value) => String(value).toLowerCase().includes(query));
}

export function sortDashboardSkills(skills = []) {
  return [...skills].sort((left, right) => {
    if (Boolean(left.isFavorite) !== Boolean(right.isFavorite)) {
      return left.isFavorite ? -1 : 1;
    }

    const nameCompare = String(left.name || '').localeCompare(String(right.name || ''), undefined, {
      sensitivity: 'base'
    });
    if (nameCompare !== 0) {
      return nameCompare;
    }

    return String(left.sourceRoot || '').localeCompare(String(right.sourceRoot || ''), undefined, {
      sensitivity: 'base'
    });
  });
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
