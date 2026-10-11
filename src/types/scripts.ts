/**
 * Script type definitions
 *
 * Types for managing PowerShell and CMD scripts.
 */


import type * as B from '@/bindings';
/**
 * Script interpreter type
 */
export type ScriptType = B.ScriptType;

/**
 * Represents a saved script managed by RustService
 */
export type Script = B.Script;

/**
 * Sort options for scripts list
 */
export type ScriptSortOption =
  | 'name-asc'
  | 'name-desc'
  | 'most-used'
  | 'recently-added'
  | 'recently-run';

/**
 * Sort option configuration
 */
export interface ScriptSortConfig {
  value: ScriptSortOption;
  label: string;
}

/**
 * Available sort options
 */
export const SCRIPT_SORT_OPTIONS: ScriptSortConfig[] = [
  { value: 'name-asc', label: 'Name A-Z' },
  { value: 'name-desc', label: 'Name Z-A' },
  { value: 'most-used', label: 'Most Used' },
  { value: 'recently-added', label: 'Recently Added' },
  { value: 'recently-run', label: 'Recently Run' },
];

/**
 * Script type display configuration
 */
export const SCRIPT_TYPE_OPTIONS: { value: ScriptType; label: string; description: string }[] = [
  { value: 'powershell', label: 'PowerShell', description: 'Windows PowerShell script' },
  { value: 'cmd', label: 'CMD', description: 'Command Prompt batch script' },
];
