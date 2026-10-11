/**
 * In-app navigation
 *
 * Tabs are switched with a `navigate-tab` window event handled by App. A
 * settings category requested together with the Settings tab is remembered
 * until the Settings page mounts and claims it, so it is never lost to the
 * page not having rendered yet.
 */

import type { SettingsCategory } from '@/types/settings';

export const NAVIGATE_TAB_EVENT = 'navigate-tab';
export const NAVIGATE_SETTINGS_CATEGORY_EVENT = 'navigate-settings-category';

let pendingSettingsCategory: SettingsCategory | null = null;

/** Switch to a tab, optionally opening a specific settings category */
export function navigateTo(tab: string, settingsCategory?: SettingsCategory) {
  if (settingsCategory) {
    pendingSettingsCategory = settingsCategory;
    // Reaches the Settings page if it is already mounted
    window.dispatchEvent(new CustomEvent(NAVIGATE_SETTINGS_CATEGORY_EVENT, { detail: settingsCategory }));
  }
  window.dispatchEvent(new CustomEvent(NAVIGATE_TAB_EVENT, { detail: tab }));
}

/** Take (and clear) the category requested before the Settings page mounted */
export function takePendingSettingsCategory(): SettingsCategory | null {
  const category = pendingSettingsCategory;
  pendingSettingsCategory = null;
  return category;
}
