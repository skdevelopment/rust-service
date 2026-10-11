/**
 * Required-program installs.
 *
 * The download lives here, above the tabs, so closing the missing-programs
 * dialog or leaving Settings does not cancel it. Progress events update every
 * listener, including a dialog that is opened again mid-install.
 */

import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, ReactNode } from 'react';
import { listen } from '@tauri-apps/api/event';

import { commands } from '@/bindings';
import { cn } from '@/lib/utils';

export const REQUIRED_PROGRAM_INSTALL_EVENT = 'required-program-install-progress';

export type InstallPhase = 'downloading' | 'extracting' | 'done' | 'error';

export interface ProgramInstallJob {
  programId: string;
  phase: InstallPhase;
  bytesReceived: number;
  bytesTotal: number | null;
  message: string | null;
}

interface InstallProgressEvent {
  programId: string;
  phase: InstallPhase;
  bytesReceived: number;
  bytesTotal: number | null;
  message?: string | null;
}

interface RequiredProgramInstallContextValue {
  jobs: Record<string, ProgramInstallJob>;
  /** Absolute data directory, used to show where a program is being written. */
  dataDir: string | null;
  install: (programId: string) => Promise<void>;
  installMany: (programIds: string[]) => Promise<void>;
}

const RequiredProgramInstallContext = createContext<RequiredProgramInstallContextValue | null>(null);

export function isInstallActive(job: ProgramInstallJob | undefined): boolean {
  return job?.phase === 'downloading' || job?.phase === 'extracting';
}

export function programInstallPath(dataDir: string | null, programId: string): string {
  if (!dataDir) return `data\\programs\\${programId}`;
  const base = dataDir.replace(/[\\/]+$/, '');
  return `${base}\\programs\\${programId}`;
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return '0 B';
  if (bytes < 1024) return `${Math.round(bytes)} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function progressLabel(job: ProgramInstallJob): string {
  if (job.phase === 'extracting') return job.message ?? 'Extracting…';
  if (job.phase === 'done') return 'Installed';
  if (job.phase === 'error') return job.message ?? 'Install failed';
  const received = formatBytes(job.bytesReceived);
  if (job.bytesTotal && job.bytesTotal > 0) {
    const percent = Math.min(100, Math.round((job.bytesReceived / job.bytesTotal) * 100));
    return `Downloading ${percent}% · ${received} / ${formatBytes(job.bytesTotal)}`;
  }
  return `Downloading ${received}…`;
}

export function InstallProgress({ job }: { job: ProgramInstallJob }) {
  const total = job.bytesTotal;
  const percent = total && total > 0
    ? Math.min(100, Math.round((job.bytesReceived / total) * 100))
    : null;
  const showBar = job.phase === 'downloading' || job.phase === 'extracting';
  const label = progressLabel(job);

  return (
    <div className="space-y-1 min-w-0">
      {showBar ? (
        <div
          className="bg-primary/20 relative h-1.5 w-full overflow-hidden rounded-full"
          role="progressbar"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={percent ?? undefined}
          aria-label={label}
        >
          <div
            className={cn('bg-primary h-full transition-[width]', percent == null && 'w-1/3 animate-pulse')}
            style={percent != null ? { width: `${percent}%` } : undefined}
          />
        </div>
      ) : null}
      <p className={cn('text-xs', job.phase === 'error' ? 'text-destructive' : 'text-muted-foreground')}>
        {label}
      </p>
    </div>
  );
}

function errorMessage(error: unknown): string {
  if (error instanceof Error && error.message) return error.message;
  return String(error);
}

function jobFromEvent(payload: InstallProgressEvent, previous: ProgramInstallJob | undefined): ProgramInstallJob {
  const keepBytes = payload.phase === 'error' && payload.bytesReceived === 0 && previous;
  return {
    programId: payload.programId,
    phase: payload.phase,
    bytesReceived: keepBytes ? previous.bytesReceived : payload.bytesReceived,
    bytesTotal: payload.bytesTotal ?? previous?.bytesTotal ?? null,
    message: payload.message ?? null,
  };
}

export function RequiredProgramInstallProvider({ children }: { children: ReactNode }) {
  const [jobs, setJobs] = useState<Record<string, ProgramInstallJob>>({});
  const [dataDir, setDataDir] = useState<string | null>(null);
  const inflight = useRef<Map<string, Promise<void>>>(new Map());

  useEffect(() => {
    let cancelled = false;
    commands.getDataDir()
      .then((dir) => {
        if (!cancelled) setDataDir(dir);
      })
      .catch((error) => {
        console.error('Failed to read data directory:', error);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    const unsubscribers: (() => void)[] = [];
    const track = (unsub: () => void) => {
      if (disposed) unsub();
      else unsubscribers.push(unsub);
    };

    const setup = async () => {
      const unsub = await listen<InstallProgressEvent>(REQUIRED_PROGRAM_INSTALL_EVENT, (event) => {
        const payload = event.payload;
        if (!payload?.programId) return;
        setJobs((prev) => {
          const current = prev[payload.programId];
          if (
            current &&
            (current.phase === 'done' || current.phase === 'error') &&
            (payload.phase === 'downloading' || payload.phase === 'extracting')
          ) {
            return prev;
          }
          return {
            ...prev,
            [payload.programId]: jobFromEvent(payload, current),
          };
        });
      });
      track(unsub);
    };

    setup().catch((error) => {
      console.error('Failed to listen for program install progress:', error);
    });

    return () => {
      disposed = true;
      for (const unsub of unsubscribers) unsub();
    };
  }, []);

  const install = useCallback((programId: string) => {
    const existing = inflight.current.get(programId);
    if (existing) return existing;

    const task = (async () => {
      setJobs((prev) => {
        const current = prev[programId];
        if (current && isInstallActive(current)) return prev;
        return {
          ...prev,
          [programId]: {
            programId,
            phase: 'downloading',
            bytesReceived: 0,
            bytesTotal: null,
            message: null,
          },
        };
      });
      try {
        await commands.installRequiredProgram(programId);
        setJobs((prev) => {
          const current = prev[programId];
          if (current?.phase === 'error') return prev;
          return {
            ...prev,
            [programId]: {
              programId,
              phase: 'done',
              bytesReceived: current?.bytesReceived ?? 0,
              bytesTotal: current?.bytesTotal ?? null,
              message: null,
            },
          };
        });
      } catch (error) {
        const message = errorMessage(error);
        if (message.toLowerCase().includes('already installing')) return;
        setJobs((prev) => ({
          ...prev,
          [programId]: {
            programId,
            phase: 'error',
            bytesReceived: prev[programId]?.bytesReceived ?? 0,
            bytesTotal: prev[programId]?.bytesTotal ?? null,
            message,
          },
        }));
      } finally {
        inflight.current.delete(programId);
      }
    })();

    inflight.current.set(programId, task);
    return task;
  }, []);

  const installMany = useCallback((programIds: string[]) => {
    const unique = [...new Set(programIds)];
    return Promise.allSettled(unique.map((id) => install(id))).then(() => undefined);
  }, [install]);

  const value = useMemo(
    () => ({ jobs, dataDir, install, installMany }),
    [jobs, dataDir, install, installMany],
  );

  return (
    <RequiredProgramInstallContext.Provider value={value}>
      {children}
    </RequiredProgramInstallContext.Provider>
  );
}

export function useRequiredProgramInstall(): RequiredProgramInstallContextValue {
  const context = useContext(RequiredProgramInstallContext);
  if (!context) {
    throw new Error('useRequiredProgramInstall must be used within a RequiredProgramInstallProvider');
  }
  return context;
}
