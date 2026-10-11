/**
 * Required program type definitions
 *
 * Types for managing external programs required by services.
 * Matches Rust types in src-tauri/src/types/required_program.rs
 */

import type * as B from '@/bindings';
/** Status of a required program */
export type RequiredProgramStatus = B.RequiredProgramStatus_Serialize;
