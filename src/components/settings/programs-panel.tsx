/**
 * Required Programs Panel
 *
 * Portable tools live in data/programs next to the app (USB-safe).
 * One-click Install fetches a zip/PE there; it never runs a Windows installer.
 */

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import * as dialog from '@tauri-apps/plugin-dialog';
import {
  ExternalLink,
  RefreshCw,
  Package,
  Loader2,
  Download,
} from 'lucide-react';

import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Skeleton } from '@/components/ui/skeleton';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import type { RequiredProgramStatus } from '@/types/required-programs';

export function ProgramsPanel() {
  const [statuses, setStatuses] = useState<RequiredProgramStatus[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [installingId, setInstallingId] = useState<string | null>(null);
  const [installErrors, setInstallErrors] = useState<Record<string, string>>({});

  const loadStatuses = async () => {
    try {
      const data = await invoke<RequiredProgramStatus[]>('get_required_programs_status');
      setStatuses(data);
    } catch (e) {
      console.error('Failed to load required programs:', e);
    }
  };

  useEffect(() => {
    async function init() {
      setIsLoading(true);
      await loadStatuses();
      setIsLoading(false);
    }
    init();
  }, []);

  const handleRefresh = async () => {
    setIsRefreshing(true);
    await loadStatuses();
    setIsRefreshing(false);
  };

  const handleBrowse = async (programId: string) => {
    try {
      const selected = await dialog.open({
        multiple: false,
        filters: [{ name: 'Executables', extensions: ['exe'] }],
      });
      if (selected) {
        await invoke('set_program_path_override', { program_id: programId, path: selected });
        await loadStatuses();
      }
    } catch (e) {
      console.error('Failed to browse:', e);
    }
  };

  const handleClearOverride = async (programId: string) => {
    try {
      await invoke('set_program_path_override', { program_id: programId, path: null });
      await loadStatuses();
    } catch (e) {
      console.error('Failed to clear override:', e);
    }
  };

  const handleInstall = async (programId: string) => {
    setInstallingId(programId);
    setInstallErrors((prev) => {
      const next = { ...prev };
      delete next[programId];
      return next;
    });
    try {
      await invoke<RequiredProgramStatus>('install_required_program', { id: programId });
      await loadStatuses();
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      setInstallErrors((prev) => ({ ...prev, [programId]: message }));
    } finally {
      setInstallingId(null);
    }
  };

  const foundCount = statuses.filter((s) => s.found).length;
  const missingCount = statuses.length - foundCount;
  const firstError = Object.entries(installErrors)[0];

  if (isLoading) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-8 w-56" />
        <Skeleton className="h-4 w-96" />
        <Skeleton className="h-64 w-full" />
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="flex items-start justify-between gap-4">
        <div className="space-y-1">
          <h3 className="text-lg font-semibold tracking-tight">Required Programs</h3>
          <p className="text-sm text-muted-foreground max-w-2xl">
            Tools are stored in <span className="font-mono text-xs">data/programs</span> next
            to this app so a USB copy stays self-contained. Install never writes to Program
            Files or the registry.
          </p>
        </div>
        <Button variant="outline" size="sm" onClick={handleRefresh} disabled={isRefreshing}>
          {isRefreshing ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <RefreshCw className="h-4 w-4" />
          )}
          Refresh
        </Button>
      </div>

      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <Badge variant="secondary">{foundCount} found</Badge>
        <Badge variant="outline">{missingCount} missing</Badge>
      </div>

      {firstError ? (
        <Alert variant="destructive">
          <AlertTitle>Install failed</AlertTitle>
          <AlertDescription>{firstError[1]}</AlertDescription>
        </Alert>
      ) : null}

      {statuses.length === 0 ? (
        <div className="flex flex-col items-center justify-center rounded-lg border border-dashed py-12 text-center">
          <Package className="h-8 w-8 text-muted-foreground mb-2" />
          <p className="text-sm text-muted-foreground">
            No required programs defined yet.
          </p>
        </div>
      ) : (
        <div className="rounded-lg border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Program</TableHead>
                <TableHead className="w-28">Status</TableHead>
                <TableHead>Location</TableHead>
                <TableHead className="text-right w-56">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {statuses.map((status) => {
                const id = status.definition.id;
                const error = installErrors[id];
                return (
                  <TableRow key={id}>
                    <TableCell>
                      <div className="font-medium">{status.definition.name}</div>
                      <div className="text-xs text-muted-foreground">
                        {status.definition.description}
                      </div>
                    </TableCell>
                    <TableCell>
                      {status.found ? (
                        <Badge variant="secondary">Found</Badge>
                      ) : (
                        <Badge variant="outline">Missing</Badge>
                      )}
                      {status.isCustom ? (
                        <Badge variant="outline" className="ml-1">
                          Custom
                        </Badge>
                      ) : null}
                    </TableCell>
                    <TableCell className="max-w-[18rem]">
                      {status.path ? (
                        <span className="font-mono text-xs truncate block" title={status.path}>
                          {status.path}
                        </span>
                      ) : (
                        <span className="text-xs text-muted-foreground">
                          {status.definition.exeNames.join(', ')}
                        </span>
                      )}
                      {error ? (
                        <p className="text-xs text-destructive mt-1 whitespace-normal">{error}</p>
                      ) : null}
                    </TableCell>
                    <TableCell className="text-right">
                      <div className="inline-flex items-center justify-end gap-1">
                        {!status.found ? (
                          <Button
                            size="sm"
                            onClick={() => handleInstall(id)}
                            disabled={installingId === id}
                          >
                            {installingId === id ? (
                              <Loader2 className="h-4 w-4 animate-spin" />
                            ) : (
                              <Download className="h-4 w-4" />
                            )}
                            Install
                          </Button>
                        ) : null}
                        {status.definition.url ? (
                          <Button
                            variant="ghost"
                            size="icon-sm"
                            onClick={() => window.open(status.definition.url, '_blank')}
                          >
                            <ExternalLink className="h-4 w-4" />
                            <span className="sr-only">Open vendor page</span>
                          </Button>
                        ) : null}
                        {status.isCustom ? (
                          <Button
                            variant="outline"
                            size="sm"
                            onClick={() => handleClearOverride(id)}
                          >
                            Reset
                          </Button>
                        ) : (
                          <Button
                            variant="outline"
                            size="sm"
                            onClick={() => handleBrowse(id)}
                          >
                            Browse
                          </Button>
                        )}
                      </div>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </div>
      )}
    </div>
  );
}
