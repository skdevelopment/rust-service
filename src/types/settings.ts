/**
 * Application settings type definitions
 * Stored in data/settings.json
 * 
 * Settings are organized into categories for scalability.
 * Use the useSettings() hook to access settings from any component.
 */

import type * as B from '@/bindings';
// =============================================================================
// Theme Types
// =============================================================================

/**
 * Valid theme mode options (light/dark/system)
 */
export type ThemeMode = B.ThemeMode;

/**
 * Available color schemes (CSS class names applied to html element)
 * Add new schemes here and in globals.css with .theme-{name} class
 */
export type ColorScheme = 'default' | 'cobalt' | 'forge' | 'circuit' | 'oxide' | 'neon-workshop' | 'techbay' | 'autoservice' | 'amber-minimal' | 'amethyst-haze' | 'catppuccin' | 'supabase' | 'nature' | 'cyberpunk' | 'claude';

/**
 * Combined theme (for backwards compatibility)
 */
export type Theme = ThemeMode;

/**
 * Valid log level options
 */
export type LogLevel = B.LogLevel;

// =============================================================================
// Color Scheme Definitions
// =============================================================================

export interface ColorSchemeInfo {
  id: ColorScheme;
  name: string;
  description: string;
  /** Preview colors for the scheme selector */
  preview: {
    primary: string;
    accent: string;
    background: string;
  };
}

/**
 * Available color schemes with metadata
 * Add new TweakCN themes here
 */
export const COLOR_SCHEMES: ColorSchemeInfo[] = [
  {
    id: 'forge',
    name: 'Forge',
    description: 'Warm ember/copper workshop aesthetic',
    preview: {
      primary: '#d97a2b',
      accent: '#221e1b',
      background: '#1a1614',
    },
  },
  {
    id: 'cobalt',
    name: 'Cobalt',
    description: 'Rich blue, professional diagnostic feel',
    preview: {
      primary: '#3b7dd8',
      accent: '#121822',
      background: '#0c1017',
    },
  },
  {
    id: 'circuit',
    name: 'Circuit',
    description: 'Electric teal diagnostic scanner',
    preview: {
      primary: '#22b8a0',
      accent: '#151c24',
      background: '#0f1419',
    },
  },
  {
    id: 'oxide',
    name: 'Oxide',
    description: 'Burnt terracotta on warm grays',
    preview: {
      primary: '#c05a30',
      accent: '#1e1917',
      background: '#171312',
    },
  },
  {
    id: 'neon-workshop',
    name: 'Neon Workshop',
    description: 'Vivid green workbench LEDs',
    preview: {
      primary: '#3ddc6a',
      accent: '#131a14',
      background: '#0d120e',
    },
  },
  {
    id: 'techbay',
    name: 'Techbay',
    description: 'Coral red accent theme',
    preview: {
      primary: '#ce5d5b',
      accent: '#d77c7a',
      background: '#ffffff',
    },
  },
  {
    id: 'autoservice',
    name: 'AutoService',
    description: 'Original AutoService-inspired dark UI',
    preview: {
      primary: '#4f8cff',
      accent: '#1f2430',
      background: '#0f1115',
    },
  },
  {
    id: 'amber-minimal',
    name: 'Amber Minimal',
    description: 'Warm amber accent theme',
    preview: {
      primary: '#f59e0b',
      accent: '#d97706',
      background: '#ffffff',
    },
  },
  {
    id: 'amethyst-haze',
    name: 'Amethyst Haze',
    description: 'Soft purple tones',
    preview: {
      primary: '#8a79ab',
      accent: '#e6a5b8',
      background: '#f8f7fa',
    },
  },
  {
    id: 'catppuccin',
    name: 'Catppuccin',
    description: 'Pastel color palette',
    preview: {
      primary: '#8839ef',
      accent: '#04a5e5',
      background: '#eff1f5',
    },
  },
  {
    id: 'supabase',
    name: 'Supabase',
    description: 'Green developer theme',
    preview: {
      primary: '#72e3ad',
      accent: '#3b82f6',
      background: '#fcfcfc',
    },
  },
  {
    id: 'nature',
    name: 'Nature',
    description: 'Earthy green tones',
    preview: {
      primary: '#2e7d32',
      accent: '#c8e6c9',
      background: '#f8f5f0',
    },
  },
  {
    id: 'cyberpunk',
    name: 'Cyberpunk',
    description: 'Neon pink and cyan',
    preview: {
      primary: '#ff00c8',
      accent: '#00ffcc',
      background: '#f8f9fa',
    },
  },
  {
    id: 'claude',
    name: 'Claude',
    description: 'Anthropic brand-inspired professional theme',
    preview: {
      primary: '#d97757',
      accent: '#6a9bcc',
      background: '#faf9f5',
    },
  },
  {
    id: 'default',
    name: 'Shadcn',
    description: 'Clean monochrome theme',
    preview: {
      primary: '#1a1a1a',
      accent: '#6b7280',
      background: '#ffffff',
    },
  },
];

// =============================================================================
// Category Interfaces
// =============================================================================

/**
 * Appearance settings for theming and visual preferences
 */
export type AppearanceSettings = B.AppearanceSettings;

/**
 * Data and storage related settings
 */
export type DataSettings = B.DataSettings;

/**
 * Report-related settings
 */
export type ReportsSettings = B.ReportsSettings;

/**
 * Business branding and technician settings
 */
export type BusinessSettings = B.BusinessSettings_Serialize;

/**
 * Required program path overrides
 */
export type ProgramsSettings = B.ProgramsSettings;

/**
 * A single technician tab configuration
 */
export type TechnicianTab = B.TechnicianTab_Serialize;

/**
 * Available preset icons for technician tabs
 */
export const TECHNICIAN_TAB_ICONS = [
  { id: 'globe', name: 'Globe', description: 'Default web icon' },
  { id: 'file-text', name: 'Document', description: 'Documentation or files' },
  { id: 'folder', name: 'Folder', description: 'File management' },
  { id: 'database', name: 'Database', description: 'Data or storage' },
  { id: 'mail', name: 'Mail', description: 'Email or messaging' },
  { id: 'calendar', name: 'Calendar', description: 'Scheduling or dates' },
  { id: 'settings', name: 'Settings', description: 'Configuration' },
  { id: 'user', name: 'User', description: 'Account or profile' },
  { id: 'shopping-cart', name: 'Cart', description: 'Shopping or orders' },
  { id: 'credit-card', name: 'Payment', description: 'Billing or payments' },
  { id: 'bar-chart', name: 'Analytics', description: 'Charts or stats' },
  { id: 'code', name: 'Code', description: 'Development tools' },
  { id: 'terminal', name: 'Terminal', description: 'Console or CLI' },
  { id: 'cloud', name: 'Cloud', description: 'Cloud services' },
  { id: 'lock', name: 'Security', description: 'Security or auth' },
  { id: 'tool', name: 'Tools', description: 'Utilities' },
  { id: 'wrench', name: 'Wrench', description: 'Repair or maintenance' },
  { id: 'monitor', name: 'Monitor', description: 'Display or screen' },
  { id: 'smartphone', name: 'Phone', description: 'Mobile' },
  { id: 'headphones', name: 'Support', description: 'Help or support' },
] as const;

/**
 * Technician tabs settings
 */
export type TechnicianTabsSettings = B.TechnicianTabsSettings_Serialize;

import { DEFAULT_AGENT_SETTINGS } from './agent';

/**
 * Service presets settings (custom presets)
 */
export type PresetsSettings = B.PresetsSettings;

// =============================================================================
// Main Settings Interface
// =============================================================================

/**
 * Main application settings interface
 * 
 * @example
 * ```typescript
 * const { settings } = useSettings();
 * console.log(settings.appearance.theme); // 'system'
 * console.log(settings.appearance.colorScheme); // 'techbay'
 * ```
 */
export type AppSettings = B.AppSettings_Serialize;

// =============================================================================
// Default Values
// =============================================================================

/**
 * Default appearance settings
 */
export const DEFAULT_APPEARANCE: AppearanceSettings = {
  theme: 'system',
  colorScheme: 'forge',
  enableAnimations: true,
};

/**
 * Default data settings
 */
export const DEFAULT_DATA: DataSettings = {
  logLevel: 'info',
};

/**
 * Default reports settings
 */
export const DEFAULT_REPORTS: ReportsSettings = {
  autoSaveReports: true,
  reportRetentionDays: 0,
  includeLogsInReport: true,
};

/**
 * Default business settings (disabled by default)
 */
export const DEFAULT_BUSINESS: BusinessSettings = {
  enabled: false,
  name: '',
  logoPath: undefined,
  address: '',
  phone: '',
  email: '',
  website: '',
  tfn: '',
  abn: '',
  technicians: [],
};

/**
 * Default programs settings
 */
export const DEFAULT_PROGRAMS: ProgramsSettings = {
  overrides: {},
};

/**
 * Default technician tabs settings
 */
export const DEFAULT_TECHNICIAN_TABS: TechnicianTabsSettings = {
  tabs: [],
  useFavicons: true,
};

/**
 * Default presets settings
 */
export const DEFAULT_PRESETS: PresetsSettings = {
  customPresets: [],
};

/**
 * Default application settings
 */
export const DEFAULT_SETTINGS: AppSettings = {
  version: '0.7.0',
  appearance: DEFAULT_APPEARANCE,
  data: DEFAULT_DATA,
  reports: DEFAULT_REPORTS,
  business: DEFAULT_BUSINESS,
  programs: DEFAULT_PROGRAMS,
  technicianTabs: DEFAULT_TECHNICIAN_TABS,
  presets: DEFAULT_PRESETS,
  agent: DEFAULT_AGENT_SETTINGS,
};

// =============================================================================
// Settings Key Types (for update_setting command)
// =============================================================================

type Leaf = string | number | boolean | null | undefined | readonly unknown[];

/** Every dotted path into `T`, descending into nested objects (not arrays). */
type PathsOf<T, Prefix extends string = ''> = {
  [K in keyof T & string]: NonNullable<T[K]> extends Leaf
    ? `${Prefix}${K}`
    : `${Prefix}${K}` | PathsOf<NonNullable<T[K]>, `${Prefix}${K}.`>;
}[keyof T & string];

type ValueAt<T, P extends string> = P extends `${infer Head}.${infer Rest}`
  ? Head extends keyof T
    ? ValueAt<NonNullable<T[Head]>, Rest>
    : never
  : P extends keyof T
    ? T[P]
    : never;

/** Any setting path accepted by the update_setting command, e.g. "agent.apiKeys.openai" */
export type SettingKey = PathsOf<AppSettings>;

/** The value type stored at a setting path */
export type SettingValueType<K extends SettingKey> = ValueAt<AppSettings, K>;

/**
 * Settings category IDs for sidebar navigation
 */
export type SettingsCategory = 'appearance' | 'data' | 'reports' | 'business' | 'programs' | 'technicianTabs' | 'servicePresets' | 'serviceMetrics' | 'agent' | 'about';

