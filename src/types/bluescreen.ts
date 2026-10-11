/**
 * Bluescreen (BSOD) Analysis Types
 * 
 * TypeScript interfaces for crash dump analysis.
 */

import type * as B from '@/bindings';
/** Summary of a BSOD crash */
export type BsodEntry = B.BsodEntry;

/** Detailed BSOD analysis */
export type BsodDetails = B.BsodDetails;

/** BSOD statistics */
export type BsodStats = B.BsodStats;

/** Format crash timestamp for display */
export function formatCrashTime(isoString: string): string {
  try {
    const date = new Date(isoString);
    return date.toLocaleString();
  } catch {
    return isoString;
  }
}

/** Format file size for display */
export function formatDumpSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Common stop code categories */
export const STOP_CODE_CATEGORIES = {
  driver: ['DRIVER_IRQL_NOT_LESS_OR_EQUAL', 'DRIVER_POWER_STATE_FAILURE', 'SYSTEM_THREAD_EXCEPTION_NOT_HANDLED'],
  memory: ['PAGE_FAULT_IN_NONPAGED_AREA', 'MEMORY_MANAGEMENT', 'PFN_LIST_CORRUPT', 'BAD_POOL_HEADER'],
  hardware: ['WHEA_UNCORRECTABLE_ERROR', 'KERNEL_DATA_INPAGE_ERROR', 'UNEXPECTED_KERNEL_MODE_TRAP'],
  graphics: ['VIDEO_TDR_FAILURE', 'VIDEO_SCHEDULER_INTERNAL_ERROR', 'VIDEO_MEMORY_MANAGEMENT_INTERNAL'],
  system: ['CRITICAL_PROCESS_DIED', 'SYSTEM_SERVICE_EXCEPTION', 'KERNEL_SECURITY_CHECK_FAILURE'],
};

/** Get stop code category */
export function getStopCodeCategory(stopCodeName: string | null): string {
  if (!stopCodeName) return 'Unknown';
  
  for (const [category, codes] of Object.entries(STOP_CODE_CATEGORIES)) {
    if (codes.some(code => stopCodeName.toUpperCase().includes(code))) {
      return category.charAt(0).toUpperCase() + category.slice(1);
    }
  }
  
  return 'Other';
}
