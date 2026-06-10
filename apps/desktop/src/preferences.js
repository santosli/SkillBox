import {
  normalizeDashboardTagOverrides,
  normalizeFavoriteNames
} from './dashboardMetadata.js';
import {
  normalizeRemoteUpdateTimeoutSeconds,
  normalizeStatusRefreshIntervalMinutes
} from './skillStatusRefresh.js';

export const previewPreferenceStorageKey = 'skillbox.skipLocalImportConfirmation';
export const previewStatusRefreshIntervalStorageKey = 'skillbox.statusRefreshIntervalMinutes';
export const previewRemoteUpdateTimeoutStorageKey = 'skillbox.remoteUpdateTimeoutSeconds';
export const dashboardFavoriteStorageKey = 'skillbox.dashboardFavorites';
export const dashboardTagStorageKey = 'skillbox.dashboardTags';

export function normalizePreferences(preferences) {
  return {
    skipLocalImportConfirmation: Boolean(
      preferences?.skipLocalImportConfirmation ?? preferences?.skip_local_import_confirmation
    ),
    statusRefreshIntervalMinutes: normalizeStatusRefreshIntervalMinutes(
      preferences?.statusRefreshIntervalMinutes ?? preferences?.status_refresh_interval_minutes
    ),
    remoteUpdateTimeoutSeconds: normalizeRemoteUpdateTimeoutSeconds(
      preferences?.remoteUpdateTimeoutSeconds ?? preferences?.remote_update_timeout_seconds
    )
  };
}

export function readPreviewPreferences() {
  try {
    const statusRefreshIntervalMinutes = window.localStorage.getItem(
      previewStatusRefreshIntervalStorageKey
    );
    const remoteUpdateTimeoutSeconds = window.localStorage.getItem(
      previewRemoteUpdateTimeoutStorageKey
    );

    return {
      skipLocalImportConfirmation: window.localStorage.getItem(previewPreferenceStorageKey) === 'true',
      statusRefreshIntervalMinutes: normalizeStatusRefreshIntervalMinutes(
        statusRefreshIntervalMinutes
      ),
      remoteUpdateTimeoutSeconds: normalizeRemoteUpdateTimeoutSeconds(
        remoteUpdateTimeoutSeconds
      )
    };
  } catch {
    return normalizePreferences(null);
  }
}

export function readDashboardFavorites() {
  try {
    return normalizeFavoriteNames(window.localStorage.getItem(dashboardFavoriteStorageKey));
  } catch {
    return [];
  }
}

export function readDashboardTagOverrides() {
  try {
    return normalizeDashboardTagOverrides(window.localStorage.getItem(dashboardTagStorageKey));
  } catch {
    return {};
  }
}
