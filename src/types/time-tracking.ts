/**
 * Service Time Tracking Types (Enhanced)
 * 
 * TypeScript interfaces for service execution time tracking,
 * PC fingerprinting, and statistical analysis.
 * 
 * Features:
 * - Extended PC specs (AVX2, GPU, power, network, CPU load)
 * - Ridge regression with Z-score normalization
 * - Time decay weighting for recency bias
 */

import type * as B from '@/bindings';
import { num } from '@/lib/utils';
// =============================================================================
// Network Type
// =============================================================================

export type NetworkType = B.NetworkType;

export function getNetworkTypeLabel(type: NetworkType | null | undefined): string {
  switch (type) {
    case 'ethernet': return 'Ethernet';
    case 'wifi': return 'Wi-Fi';
    case 'cellular': return 'Cellular';
    default: return 'Unknown';
  }
}

// =============================================================================
// PC Fingerprint (Enhanced)
// =============================================================================

/**
 * Extended PC specifications for correlating with execution times
 */
export type PcFingerprint = B.PcFingerprint;

// =============================================================================
// Statistics
// =============================================================================

/**
 * Aggregated statistics for a single service
 */
export type ServiceTimeStats = B.ServiceTimeStats_Serialize;

// =============================================================================
// Main Metrics Structure
// =============================================================================

/**
 * Complete service time metrics data
 */
export type ServiceTimeMetrics = B.ServiceTimeMetrics_Serialize;

// =============================================================================
// Utility Functions
// =============================================================================

/**
 * Format milliseconds to human-readable duration
 */
export function formatDuration(value: number | null | undefined): string {
  const ms = num(value);
  if (ms < 1000) {
    return `${Math.round(ms)}ms`;
  }
  if (ms < 60000) {
    return `${(ms / 1000).toFixed(1)}s`;
  }
  const minutes = Math.floor(ms / 60000);
  const seconds = Math.round((ms % 60000) / 1000);
  return `${minutes}m ${seconds}s`;
}

/**
 * Get confidence color class
 */
export function getConfidenceColor(confidence: string): string {
  switch (confidence) {
    case 'high':
      return 'text-green-500';
    case 'medium':
      return 'text-yellow-500';
    case 'low':
      return 'text-orange-500';
    default:
      return 'text-muted-foreground';
  }
}

/**
 * Get confidence badge variant
 */
export function getConfidenceBadge(confidence: string): 'default' | 'secondary' | 'outline' {
  switch (confidence) {
    case 'high':
      return 'default';
    case 'medium':
      return 'secondary';
    default:
      return 'outline';
  }
}

/**
 * Format model quality as percentage
 */
export function formatModelQuality(rSquared: number | null | undefined): string {
  return `${(num(rSquared) * 100).toFixed(0)}%`;
}

/**
 * Get model quality color
 */
export function getModelQualityColor(value: number | null | undefined): string {
  const rSquared = num(value);
  if (rSquared >= 0.7) return 'text-green-500';
  if (rSquared >= 0.4) return 'text-yellow-500';
  return 'text-orange-500';
}
