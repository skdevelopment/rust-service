/**
 * Event Log Types
 * 
 * TypeScript interfaces for Windows Event Log viewing.
 */

import type * as B from '@/bindings';
/** Event log source/channel information */
export type EventLogSource = B.EventLogSource;

/** Event severity level */
export type EventLevel = B.EventLevel;

/** Single event log entry */
export type EventLogEntry = B.EventLogEntry;

/** Query filter for event logs */
export type EventLogFilter = B.EventLogFilter;

/** Event log statistics */
export type EventLogStats = B.EventLogStats;

/** Get row color class based on event level */
export function getLevelRowClass(level: EventLevel | string): string {
  const levelLower = level.toLowerCase();
  if (levelLower === 'critical' || levelLower === 'error') {
    return 'bg-destructive/10 border-l-2 border-l-destructive';
  }
  if (levelLower === 'warning') {
    return 'bg-yellow-500/10 border-l-2 border-l-yellow-500';
  }
  return '';
}

/** Get badge variant based on event level */
export function getLevelBadgeVariant(level: EventLevel | string): 'destructive' | 'default' | 'secondary' | 'outline' {
  const levelLower = level.toLowerCase();
  if (levelLower === 'critical' || levelLower === 'error') {
    return 'destructive';
  }
  if (levelLower === 'warning') {
    return 'default';
  }
  return 'secondary';
}

/** Format event timestamp as relative time */
export function formatRelativeTime(isoString: string): string {
  try {
    const date = new Date(isoString);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffSecs = Math.floor(diffMs / 1000);
    const diffMins = Math.floor(diffSecs / 60);
    const diffHours = Math.floor(diffMins / 60);
    const diffDays = Math.floor(diffHours / 24);
    
    if (diffDays > 0) return `${diffDays}d ago`;
    if (diffHours > 0) return `${diffHours}h ago`;
    if (diffMins > 0) return `${diffMins}m ago`;
    return 'Just now';
  } catch {
    return isoString;
  }
}
