/**
 * Dialog shown when a service run needs programs that are not installed.
 * Quick install keeps running after this dialog closes.
 */

import { useEffect, useMemo, useState } from 'react';
import { Check, Download, Loader2 } from 'lucide-react';

import { commands } from '@/bindings';
import type { RequiredProgramDef_Serialize } from '@/bindings';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import {
  InstallProgress,
  isInstallActive,
  programInstallPath,
  useRequiredProgramInstall,
} from '@/components/required-program-install-context';
import type { ServiceDefinition } from '@/types/service';

interface MissingProgramsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  missing: Record<string, string[]> | null;
  onMissingChange: (missing: Record<string, string[]>) => void;
  definitions: ServiceDefinition[];
  enabledServiceIds: string[];
  onStart: () => void;
  onOpenSettings: () => void;
}

export function MissingProgramsDialog({
  open,
  onOpenChange,
  missing,
  onMissingChange,
  definitions,
  enabledServiceIds,
  onStart,
  onOpenSettings,
}: MissingProgramsDialogProps) {
  const { jobs, dataDir, install, installMany } = useRequiredProgramInstall();
  const [programs, setPrograms] = useState<Record<string, RequiredProgramDef_Serialize>>({});
  const [stillMissing, setStillMissing] = useState<Set<string> | null>(null);
  const enabledKey = enabledServiceIds.join('\n');

  const missingIds = useMemo(() => {
    if (!missing) return [];
    return [...new Set(Object.values(missing).flat())];
  }, [missing]);

  const servicesByProgram = useMemo(() => {
    const map = new Map<string, string[]>();
    for (const [serviceId, ids] of Object.entries(missing ?? {})) {
      const name = definitions.find((definition) => definition.id === serviceId)?.name ?? serviceId;
      for (const id of ids) {
        const names = map.get(id) ?? [];
        names.push(name);
        map.set(id, names);
      }
    }
    return map;
  }, [missing, definitions]);

  const doneKey = Object.values(jobs)
    .filter((job) => job.phase === 'done')
    .map((job) => job.programId)
    .sort()
    .join('\n');

  useEffect(() => {
    if (!open || missingIds.length === 0) return;
    let cancelled = false;
    commands.getRequiredProgramsByIds(missingIds)
      .then((defs) => {
        if (cancelled) return;
        setPrograms(Object.fromEntries(defs.map((def) => [def.id, def])));
      })
      .catch((error) => {
        console.error('Failed to load required program details:', error);
      });
    return () => {
      cancelled = true;
    };
  }, [open, missingIds]);

  useEffect(() => {
    if (!open || !doneKey) return;
    let cancelled = false;
    const serviceIds = enabledKey.split('\n').filter((id) => id.length > 0);
    setStillMissing(null);
    commands.validateServiceRequirements(serviceIds)
      .then((next) => {
        if (cancelled) return;
        onMissingChange(next);
        setStillMissing(new Set(Object.values(next).flat()));
      })
      .catch((error) => {
        console.error('Failed to refresh service requirements:', error);
      });
    return () => {
      cancelled = true;
    };
  }, [open, doneKey, enabledKey, onMissingChange]);

  const allClear = open && missing != null && missingIds.length === 0;
  const installingCount = missingIds.filter((id) => isInstallActive(jobs[id])).length;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{allClear ? 'Required programs installed' : 'Missing Required Programs'}</DialogTitle>
          <DialogDescription>
            {allClear
              ? 'The missing programs are in place. You can start the service.'
              : 'Quick install downloads the portable tool and keeps going if you close this dialog. Progress stays visible here and under Settings → Required Programs.'}
          </DialogDescription>
        </DialogHeader>

        {allClear ? null : (
          <div className="max-h-[50vh] space-y-3 overflow-y-auto">
            {missingIds.map((programId) => {
              const program = programs[programId];
              const job = jobs[programId];
              const active = isInstallActive(job);
              const installed = job?.phase === 'done' && !stillMissing?.has(programId);
              const undetected = job?.phase === 'done' && stillMissing?.has(programId);
              const destination = programInstallPath(dataDir, programId);
              const neededBy = servicesByProgram.get(programId) ?? [];

              return (
                <div key={programId} className="rounded-lg border bg-muted/20 p-3 space-y-2">
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
                      <div className="font-medium">{program?.name ?? programId}</div>
                      {program?.description ? (
                        <p className="text-xs text-muted-foreground mt-0.5">{program.description}</p>
                      ) : null}
                      {neededBy.length > 0 ? (
                        <p className="text-xs text-muted-foreground mt-1">
                          Needed by {neededBy.join(', ')}
                        </p>
                      ) : null}
                    </div>
                    {installed ? (
                      <span className="inline-flex items-center gap-1 text-xs text-muted-foreground shrink-0">
                        <Check className="h-3.5 w-3.5" />
                        Installed
                      </span>
                    ) : active ? null : (
                      <Button
                        size="sm"
                        className="shrink-0"
                        onClick={() => { void install(programId); }}
                      >
                        <Download className="h-4 w-4" />
                        {job?.phase === 'error' || undetected ? 'Retry' : 'Quick install'}
                      </Button>
                    )}
                  </div>

                  {active && job ? (
                    <div className="space-y-1.5">
                      <p className="font-mono text-xs text-muted-foreground break-all" title={destination}>
                        {destination}
                      </p>
                      <InstallProgress job={job} />
                    </div>
                  ) : null}

                  {job?.phase === 'error' ? (
                    <p className="text-xs text-destructive whitespace-normal">{job.message}</p>
                  ) : null}
                  {undetected ? (
                    <p className="text-xs text-destructive whitespace-normal">
                      Install finished, but the program was not detected. Retry, or set the path in Settings.
                    </p>
                  ) : null}
                </div>
              );
            })}
          </div>
        )}

        <DialogFooter>
          {allClear ? (
            <>
              <Button variant="outline" onClick={() => onOpenChange(false)}>Close</Button>
              <Button onClick={onStart}>Start service</Button>
            </>
          ) : (
            <>
              {missingIds.length > 1 ? (
                <Button
                  className="sm:mr-auto"
                  onClick={() => { void installMany(missingIds); }}
                  disabled={installingCount === missingIds.length}
                >
                  {installingCount > 0 ? <Loader2 className="h-4 w-4 animate-spin" /> : <Download className="h-4 w-4" />}
                  Install all
                </Button>
              ) : null}
              <Button variant="outline" onClick={onOpenSettings}>
                Go to Settings
              </Button>
              <Button variant="outline" onClick={() => onOpenChange(false)}>
                Close
              </Button>
            </>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
