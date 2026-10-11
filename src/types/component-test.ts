/**
 * Component Test Types
 * 
 * Types for hardware component testing functionality.
 */

import type * as B from '@/bindings';
/**
 * Mouse test state
 */
export interface MouseTestState {
  position: { x: number; y: number };
  leftClicks: number;
  rightClicks: number;
  middleClicks: number;
  scrollUp: number;
  scrollDown: number;
  lastEvent: string;
}

/**
 * Network test result
 */
export type NetworkTestResult = B.NetworkTestResult;

/**
 * Display test patterns
 */
export type DisplayPattern =
  | 'red'
  | 'green'
  | 'blue'
  | 'white'
  | 'black'
  | 'gradient'
  | 'checkerboard'
  | 'grid'
  | 'smpte'
  | 'crosshatch'
  | 'dot-matrix'
  | 'stripes-h'
  | 'stripes-v';
