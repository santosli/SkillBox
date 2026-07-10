import {
  normalizeDashboardTagOverrides,
  normalizeEditableTags,
  normalizeFavoriteNames
} from './dashboardMetadata.js';

export function normalizeSkillUserMetadata(rows = []) {
  const favoriteNames = [];
  const tagOverrides = {};

  for (const row of Array.isArray(rows) ? rows : []) {
    const skillName = String(row?.skillName || row?.skill_name || '').trim();
    if (!skillName) continue;
    if (row.favorite) favoriteNames.push(skillName);
    tagOverrides[skillName] = normalizeEditableTags(row.tags);
  }

  return {
    favoriteNames: normalizeFavoriteNames(favoriteNames).sort((left, right) =>
      left.localeCompare(right)
    ),
    tagOverrides: normalizeDashboardTagOverrides(tagOverrides)
  };
}

export function mergeSkillUserMetadataRow(favoriteNames = [], tagOverrides = {}, row = null) {
  const skillName = String(row?.skillName || row?.skill_name || '').trim();
  if (!skillName) {
    return { favoriteNames, tagOverrides };
  }

  const normalized = normalizeSkillUserMetadata([row]);
  const nextFavoriteNames = favoriteNames.filter((name) => name !== skillName);
  if (normalized.favoriteNames.includes(skillName)) {
    nextFavoriteNames.push(skillName);
    nextFavoriteNames.sort((left, right) => left.localeCompare(right));
  }

  return {
    favoriteNames: nextFavoriteNames,
    tagOverrides: {
      ...tagOverrides,
      [skillName]: normalized.tagOverrides[skillName] || []
    }
  };
}

export function legacySkillUserMetadataUpdates(favoriteNames = [], tagOverrides = {}) {
  const favorites = new Set(normalizeFavoriteNames(favoriteNames));
  const tagsBySkill = normalizeDashboardTagOverrides(tagOverrides);
  const skillNames = [...new Set([...favorites, ...Object.keys(tagsBySkill)])].sort((left, right) =>
    left.localeCompare(right)
  );

  return skillNames
    .map((skillName) => ({
      skill_name: skillName,
      favorite: favorites.has(skillName),
      tags: tagsBySkill[skillName] || []
    }))
    .filter((item) => item.favorite || item.tags.length > 0);
}
