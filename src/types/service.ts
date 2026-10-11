/**
 * Service system type definitions
 *
 * Types for the modular service automation system.
 * Matches Rust types in src-tauri/src/types/service.rs
 */

import type * as B from '@/bindings';
// =============================================================================
// Service Definitions
// =============================================================================

/** Definition of a service that can be run */
export type ServiceDefinition = B.ServiceDefinition_Serialize;

// =============================================================================
// Service Presets
// =============================================================================

/** A preset configuration of services */
export type ServicePreset = B.ServicePreset;

// =============================================================================
// Service Queue
// =============================================================================

/** An item in the service run queue */
export type ServiceQueueItem = B.ServiceQueueItem & { id: string };

/** React key for a result (unique even when a service ran twice) */
export function resultKey(result: Pick<ServiceResult, 'serviceId' | 'queueItemId'>, index: number): string {
  return result.queueItemId ?? `${result.serviceId}-${index}`;
}

/**
 * The result produced by a queue entry. Matches on the queue item id (so a
 * service queued twice gets its own result each time), falling back to the
 * service id for reports saved before results carried queue item ids.
 */
export function findResultForItem(
  results: ServiceResult[] | undefined,
  item: { id?: string | null; serviceId: string },
): ServiceResult | undefined {
  if (!results) return undefined;
  return item.id
    ? results.find(r => r.queueItemId === item.id) ??
        results.find(r => !r.queueItemId && r.serviceId === item.serviceId)
    : results.find(r => r.serviceId === item.serviceId);
}

// =============================================================================
// Service Results
// =============================================================================

/** Severity level for a finding */
export type FindingSeverity = B.FindingSeverity;

/** A single finding from a service */
export type ServiceFinding = B.ServiceFinding_Serialize;

/** Result of running a single service */
export type ServiceResult = B.ServiceResult_Serialize;

/** Status of a service run */
export type ServiceRunStatus = B.ServiceRunStatus;

/** A complete service run report */
export type ServiceReport = B.ServiceReport_Serialize;

// =============================================================================
// Service Run State
// =============================================================================

/** Global service run state */
export type ServiceRunState = B.ServiceRunState_Serialize;

// =============================================================================
// UI State
// =============================================================================

/** Current phase of the service page */
export type ServicePhase = 'presets' | 'queue' | 'running' | 'results';

// =============================================================================
// Report Statistics
// =============================================================================

