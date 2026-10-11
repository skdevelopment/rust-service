/**
 * Startup Manager Types
 * 
 * TypeScript interfaces for startup program management.
 */


import type * as B from '@/bindings';
/** Source location of a startup item */
export type StartupSource = B.StartupSource;

/** Startup impact level */
export type StartupImpact = B.StartupImpact;

/** Startup item information */
export type StartupItem = B.StartupItem;

/** Get human-readable source name */
export function getSourceDisplayName(source: StartupSource): string {
  switch (source) {
    case 'registryCurrentUser':
      return 'Registry (User)';
    case 'registryLocalMachine':
      return 'Registry (System)';
    case 'startupFolderUser':
      return 'Startup Folder (User)';
    case 'startupFolderAllUsers':
      return 'Startup Folder (All Users)';
    case 'taskScheduler':
      return 'Task Scheduler';
    default:
      return 'Unknown';
  }
}

/** Get impact badge color */
export function getImpactColor(impact: StartupImpact): string {
  switch (impact) {
    case 'high':
      return 'destructive';
    case 'medium':
      return 'default';
    case 'low':
      return 'secondary';
    default:
      return 'outline';
  }
}
