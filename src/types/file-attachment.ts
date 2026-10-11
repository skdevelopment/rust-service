/**
 * File Attachment Types
 *
 * Type definitions for the file handling system supporting heterogeneous file types
 * with bidirectional flow between user and agent.
 */

import type * as B from '@/bindings';
// =============================================================================
// File Categories
// =============================================================================

/**
 * Categories of files for specialized handling
 */
export type FileCategory = B.FileCategory;   // Binary executables, archives

 // Referenced from filesystem

// =============================================================================
// File Attachment (Unified Schema)
// =============================================================================

/**
 * Core file attachment metadata used across the system
 */
export type FileAttachment = B.FileAttachment_Serialize;

// =============================================================================
// File Upload State
// =============================================================================

/**
 * State tracking for file upload operations
 */
export interface FileUploadState {
  file: File;                  // Browser File object
  id: string;                  // Temporary upload ID
  status: 'pending' | 'uploading' | 'processing' | 'complete' | 'error';
  progress: number;            // 0-100
  bytesUploaded: number;
  totalBytes: number;
  error?: string;
  attachment?: FileAttachment; // Final attachment when complete
}

// =============================================================================
// Helper Functions
// =============================================================================

/**
 * Get file category from extension
 */
export function getCategoryFromExtension(filename: string): FileCategory {
  const ext = filename.split('.').pop()?.toLowerCase() || '';

  const codeExts = [
    'js', 'ts', 'jsx', 'tsx', 'py', 'rs', 'java', 'cpp', 'c', 'h', 'hpp',
    'go', 'rb', 'php', 'swift', 'kt', 'scala', 'r', 'm', 'mm', 'cs', 'vb',
    'fs', 'hs', 'lua', 'pl', 'sh', 'bash', 'zsh', 'fish', 'ps1', 'cmd',
    'bat', 'sql', 'html', 'css', 'scss', 'sass', 'less', 'xml', 'yaml',
    'yml', 'toml', 'ini', 'conf', 'config', 'json', 'md', 'markdown'
  ];
  if (codeExts.includes(ext)) return 'code';

  const imageExts = ['png', 'jpg', 'jpeg', 'gif', 'bmp', 'webp', 'svg', 'ico', 'tiff', 'raw'];
  if (imageExts.includes(ext)) return 'image';

  const mediaExts = ['mp3', 'mp4', 'wav', 'avi', 'mov', 'mkv', 'flv', 'wmv', 'webm', 'ogg', 'ogv'];
  if (mediaExts.includes(ext)) return 'media';

  const docExts = ['pdf', 'doc', 'docx', 'xls', 'xlsx', 'ppt', 'pptx', 'odt', 'ods', 'odp', 'rtf', 'txt'];
  if (docExts.includes(ext)) return 'document';

  return 'binary';
}

/**
 * Format file size for display
 */
export function formatFileSize(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

/**
 * Get icon name for file category
 */
export function getCategoryIcon(category: FileCategory): string {
  switch (category) {
    case 'text': return 'FileText';
    case 'code': return 'FileCode';
    case 'document': return 'FileText';
    case 'image': return 'Image';
    case 'media': return 'Film';
    case 'binary': return 'File';
    default: return 'File';
  }
}

/**
 * Maximum file sizes (in bytes)
 */
export const FILE_SIZE_LIMITS = {
  SMALL: 10 * 1024 * 1024,      // 10 MB - direct upload
  LARGE: 100 * 1024 * 1024,     // 100 MB - chunked upload
  HUGE: 1024 * 1024 * 1024,     // 1 GB - streaming only
} as const;

