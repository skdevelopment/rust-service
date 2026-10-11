/**
 * Windows 11 upgrade types.
 *
 * These mirror the Rust command types. Change the Rust struct, then regenerate
 * bindings, instead of editing the shapes here.
 */

import type * as B from '@/bindings';

export type Windows11UpgradeAssessment = B.Windows11UpgradeAssessment;
export type Windows11UpgradeRoute = B.Windows11UpgradeRoute;
export type Windows11IsoCatalog = B.Windows11IsoCatalog;
export type Windows11IsoLanguage = B.Windows11IsoLanguage;
export type Windows11IsoFile = B.Windows11IsoFile;
export type Windows11DownloadStatus = B.Windows11DownloadStatus;

/** Payload of the `windows11-iso-progress` event. */
export interface Windows11DownloadProgress {
  receivedBytes: number;
  totalBytes: number | null;
}
export type Windows11UpgradeStart = B.Windows11UpgradeStart;
