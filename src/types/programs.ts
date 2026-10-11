/**
 * Program type definitions
 * 
 * Types for managing portable programs in the data folder.
 */


import type * as B from '@/bindings';
/**
 * Represents a portable program managed by RustService
 */
export type Program = B.Program;

/**
 * Sort options for programs list
 */
export type ProgramSortOption = 
  | 'name-asc' 
  | 'name-desc' 
  | 'most-used' 
  | 'recently-added' 
  | 'recently-launched';

/**
 * Sort option configuration
 */
export interface ProgramSortConfig {
  value: ProgramSortOption;
  label: string;
}

/**
 * Available sort options
 */
export const PROGRAM_SORT_OPTIONS: ProgramSortConfig[] = [
  { value: 'name-asc', label: 'Name A-Z' },
  { value: 'name-desc', label: 'Name Z-A' },
  { value: 'most-used', label: 'Most Used' },
  { value: 'recently-added', label: 'Recently Added' },
  { value: 'recently-launched', label: 'Recently Launched' },
];
