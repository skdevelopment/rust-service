/**
 * Windows 11 upgrade
 *
 * Checks the PC, then starts an in-place upgrade from a Windows 11 ISO.
 * The ISO comes from Microsoft's servers or from a file the technician picks.
 * Supported hardware starts standard Setup. Unsupported hardware uses the
 * compatibility path. Setup's own window does the install.
 */

import { useEffect, useRef, useState } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import * as dialog from '@tauri-apps/plugin-dialog';
import {
  ArrowLeft,
  CheckCircle2,
  Download,
  FolderOpen,
  Loader2,
  MonitorUp,
  RefreshCw,
} from 'lucide-react';

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { Progress } from '@/components/ui/progress';
import { ScrollArea } from '@/components/ui/scroll-area';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Switch } from '@/components/ui/switch';
import { cn, num } from '@/lib/utils';
import { commands } from '@/bindings';
import type {
  Windows11DownloadStatus,
  Windows11IsoCatalog,
  Windows11UpgradeAssessment,
  Windows11UpgradeRoute,
  Windows11UpgradeStart,
} from '@/types';

type CheckTone = 'ready' | 'review' | 'unknown' | 'info';

interface CheckRow {
  label: string;
  detail: string;
  tone: CheckTone;
}

const TONE_LABEL: Record<CheckTone, string> = {
  ready: 'Ready',
  review: 'Review',
  unknown: 'Unknown',
  info: 'Info',
};

const TONE_CLASS: Record<CheckTone, string> = {
  ready: 'border-transparent bg-emerald-500/15 text-emerald-700 dark:text-emerald-300',
  review: 'border-transparent bg-amber-500/15 text-amber-800 dark:text-amber-300',
  unknown: 'text-muted-foreground',
  info: 'text-muted-foreground',
};

export function Windows11Upgrade({ onBack }: { onBack: () => void }) {
  const [assessment, setAssessment] = useState<Windows11UpgradeAssessment | null>(null);
  const [assessError, setAssessError] = useState<string | null>(null);
  const [assessing, setAssessing] = useState(true);
  const [route, setRoute] = useState<Windows11UpgradeRoute>('supported');
  const [advanced, setAdvanced] = useState(false);
  const [catalog, setCatalog] = useState<Windows11IsoCatalog | null>(null);
  const [catalogError, setCatalogError] = useState<string | null>(null);
  const [catalogLoading, setCatalogLoading] = useState(true);
  const [language, setLanguage] = useState('');
  const [isoPath, setIsoPath] = useState('');
  const [downloading, setDownloading] = useState(false);
  const [received, setReceived] = useState(0);
  const [total, setTotal] = useState<number | null>(null);
  const [downloadError, setDownloadError] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);
  const [startResult, setStartResult] = useState<Windows11UpgradeStart | null>(null);
  const [startError, setStartError] = useState<string | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);

  const routeChosen = useRef(false);
  const languageTouched = useRef(false);
  const seenRevision = useRef(0);

  useEffect(() => {
    let cancelled = false;

    const loadAssessment = async () => {
      setAssessing(true);
      setAssessError(null);
      try {
        const next = await commands.assessWindows11Upgrade();
        if (cancelled) return;
        setAssessment(next);
        if (!routeChosen.current) setRoute(next.suggestedRoute);
      } catch (error) {
        if (!cancelled) setAssessError(errorMessage(error));
      } finally {
        if (!cancelled) setAssessing(false);
      }
    };

    void loadAssessment();
    void (async () => {
      setCatalogLoading(true);
      try {
        const next = await commands.listWindows11IsoLanguages();
        if (cancelled) return;
        setCatalog(next);
        setLanguage((current) => {
          if (languageTouched.current && current) return current;
          return next.suggestedLanguage;
        });
      } catch (error) {
        if (!cancelled) setCatalogError(errorMessage(error));
      } finally {
        if (!cancelled) setCatalogLoading(false);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlisten: UnlistenFn | undefined;

    const apply = (status: Windows11DownloadStatus) => {
      if (cancelled || status.revision <= seenRevision.current) return;
      seenRevision.current = status.revision;
      switch (status.phase) {
        case 'idle':
          setDownloading(false);
          break;
        case 'running':
          setDownloading(true);
          setReceived(status.receivedBytes);
          setTotal(status.totalBytes);
          setDownloadError(null);
          break;
        case 'finished':
          setDownloading(false);
          setReceived(status.receivedBytes);
          setIsoPath((current) => current || status.finishedPath || '');
          setDownloadError(null);
          break;
        case 'failed':
          setDownloading(false);
          setDownloadError(status.error);
          break;
      }
    };

    void (async () => {
      const stop = await listen<Windows11DownloadStatus>('windows11-iso-progress', (event) => {
        apply(event.payload);
      });
      if (cancelled) {
        stop();
        return;
      }
      unlisten = stop;
      try {
        apply(await commands.windows11IsoDownloadStatus());
      } catch {
        // The download command still reports its own result.
      }
    })();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const busy = downloading || starting;
  const is32Bit = assessment?.is64Bit === false;
  const canStart = Boolean(isoPath) && !busy && !assessing && !is32Bit;
  const languageValue = catalog?.languages.some((item) => item.id === language) ? language : null;
  const percent = total && total > 0 ? Math.min(100, (received / total) * 100) : null;
  const checks = assessment ? buildChecks(assessment) : [];

  const chooseRoute = (next: Windows11UpgradeRoute) => {
    routeChosen.current = true;
    setRoute(next);
  };

  const checkAgain = async () => {
    setAssessing(true);
    setAssessError(null);
    try {
      const next = await commands.assessWindows11Upgrade();
      setAssessment(next);
      if (!routeChosen.current) setRoute(next.suggestedRoute);
    } catch (error) {
      setAssessError(errorMessage(error));
    } finally {
      setAssessing(false);
    }
  };

  const downloadIso = async () => {
    const savePath = await dialog.save({
      defaultPath: catalog?.releaseName.includes('26H2') ? 'Windows11_26H2.iso' : 'Windows11.iso',
      filters: [{ name: 'Windows ISO', extensions: ['iso'] }],
    });
    if (!savePath) return;

    setDownloadError(null);
    setStartResult(null);
    setStartError(null);
    setDownloading(true);
    setReceived(0);
    setTotal(null);
    try {
      const file = await commands.downloadWindows11Iso(language, savePath);
      setIsoPath(file.path);
    } catch (error) {
      setDownloadError(errorMessage(error));
    } finally {
      setDownloading(false);
    }
  };

  const cancelDownload = () => {
    void commands.cancelWindows11IsoDownload();
  };

  const browseIso = async () => {
    const selected = await dialog.open({
      multiple: false,
      filters: [{ name: 'Windows ISO', extensions: ['iso'] }],
    });
    if (typeof selected !== 'string') return;
    setIsoPath(selected);
    setStartResult(null);
    setStartError(null);
  };

  const startUpgrade = async () => {
    setConfirmOpen(false);
    setStarting(true);
    setStartError(null);
    setStartResult(null);
    try {
      const result = await commands.startWindows11Upgrade(
        isoPath,
        route,
        route === 'unsupported' && advanced,
      );
      setStartResult(result);
    } catch (error) {
      setStartError(errorMessage(error));
    } finally {
      setStarting(false);
    }
  };

  return (
    <div className="h-full flex flex-col overflow-hidden min-h-0">
      <div className="p-4 border-b flex items-center gap-3 flex-shrink-0">
        <Button type="button" variant="ghost" size="sm" onClick={onBack} disabled={busy}>
          <ArrowLeft />
          Back
        </Button>
        <MonitorUp className="h-5 w-5 text-primary" />
        <h2 className="font-semibold">Windows 11 upgrade</h2>
      </div>

      <ScrollArea className="flex-1 min-h-0">
        <div className="p-4 max-w-3xl space-y-4">
          <p className="text-sm text-muted-foreground">
            Check this PC, pick a route, and start Setup from a Windows 11 ISO.
            Nothing is installed until you continue in the Windows Setup window.
          </p>

          {is32Bit && (
            <Alert variant="destructive">
              <AlertTitle>32-bit Windows</AlertTitle>
              <AlertDescription>
                Windows 11 installation media is 64-bit only. This PC is 32-bit.
              </AlertDescription>
            </Alert>
          )}

          <section className="rounded-xl border bg-card p-4 shadow-sm space-y-3">
            <div className="flex items-start justify-between gap-3">
              <div>
                <h3 className="font-medium">This PC</h3>
                {assessment && (
                  <p className="mt-1 text-sm text-muted-foreground">{summaryFor(assessment)}</p>
                )}
              </div>
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() => void checkAgain()}
                disabled={assessing}
              >
                {assessing ? <Loader2 className="animate-spin" /> : <RefreshCw />}
                Check again
              </Button>
            </div>

            {assessError && (
              <Alert variant="destructive">
                <AlertDescription>{assessError}</AlertDescription>
              </Alert>
            )}

            {assessing && !assessment && (
              <p className="text-sm text-muted-foreground">Checking this PC…</p>
            )}

            {assessment && (
              <div className="divide-y">
                {checks.map((check) => (
                  <div key={check.label} className="flex items-start justify-between gap-3 py-2">
                    <div className="min-w-0">
                      <div className="text-sm font-medium">{check.label}</div>
                      <p className="text-sm text-muted-foreground">{check.detail}</p>
                    </div>
                    <Badge variant="outline" className={TONE_CLASS[check.tone]}>
                      {TONE_LABEL[check.tone]}
                    </Badge>
                  </div>
                ))}
              </div>
            )}
          </section>

          <section className="rounded-xl border bg-card p-4 shadow-sm space-y-3">
            <h3 className="font-medium">Route</h3>
            <div className="grid gap-2 sm:grid-cols-2" role="group" aria-label="Upgrade route">
              <RouteButton
                pressed={route === 'supported'}
                suggested={assessment?.suggestedRoute === 'supported'}
                title="Supported"
                detail="Standard Setup and Windows' own hardware checks."
                onClick={() => chooseRoute('supported')}
                disabled={starting}
              />
              <RouteButton
                pressed={route === 'unsupported'}
                suggested={assessment?.suggestedRoute === 'unsupported'}
                title="Unsupported"
                detail="Compatibility path for PCs that miss a Windows 11 check."
                onClick={() => chooseRoute('unsupported')}
                disabled={starting}
              />
            </div>
            <p className="text-sm text-muted-foreground">
              {route === 'supported'
                ? "Standard Setup keeps Windows' own hardware checks."
                : 'The compatibility path skips the TPM, Secure Boot, and CPU generation block. The installed edition is still the client edition on the ISO. Microsoft does not support the result.'}
            </p>
            {route === 'unsupported' && (
              <div className="flex items-center justify-between gap-3 rounded-lg bg-muted/50 p-3">
                <div>
                  <Label htmlFor="advanced-setup">Driver warning flags</Label>
                  <p className="text-xs text-muted-foreground">
                    Ask Setup to ignore driver warnings and migrate drivers. These options are
                    experimental.
                  </p>
                </div>
                <Switch
                  id="advanced-setup"
                  checked={advanced}
                  onCheckedChange={setAdvanced}
                  disabled={starting}
                />
              </div>
            )}
          </section>

          <section className="rounded-xl border bg-card p-4 shadow-sm space-y-3">
            <div>
              <h3 className="font-medium">ISO</h3>
              <p className="mt-1 text-sm text-muted-foreground">
                {catalog
                  ? `Download ${catalog.releaseName}, ${catalog.edition}, ${catalog.architecture} from Microsoft, or choose an ISO that is already on this PC.`
                  : 'Download the current Windows 11 ISO from Microsoft, or choose an ISO that is already on this PC.'}
              </p>
            </div>

            {catalogError && (
              <Alert>
                <AlertTitle>Microsoft's language list did not load</AlertTitle>
                <AlertDescription>
                  {catalogError} You can still choose an ISO from this PC.
                </AlertDescription>
              </Alert>
            )}

            <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
              <Select
                value={languageValue}
                onValueChange={(value) => {
                  if (value === null) return;
                  languageTouched.current = true;
                  setLanguage(value);
                }}
                disabled={catalogLoading || downloading || !catalog}
              >
                <SelectTrigger className="w-full sm:w-72" aria-label="ISO language">
                  <SelectValue placeholder={catalogLoading ? 'Loading languages…' : 'Language'} />
                </SelectTrigger>
                <SelectContent>
                  {catalog?.languages.map((item) => (
                    <SelectItem key={item.id} value={item.id}>
                      {item.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <div className="flex gap-2">
                <Button
                  type="button"
                  onClick={() => void downloadIso()}
                  disabled={downloading || starting || !language || is32Bit}
                >
                  {downloading ? <Loader2 className="animate-spin" /> : <Download />}
                  Download ISO
                </Button>
                {downloading && (
                  <Button type="button" variant="outline" onClick={cancelDownload}>
                    Cancel
                  </Button>
                )}
              </div>
            </div>

            {downloading && (
              <div className="space-y-2">
                <Progress value={percent} />
                <p className="text-xs text-muted-foreground">
                  {formatBytes(received)}
                  {total ? ` of ${formatBytes(total)}` : ''}
                  {percent !== null ? ` (${Math.round(percent)}%)` : ''}
                </p>
              </div>
            )}

            {downloadError && (
              <Alert variant={downloadError === 'Download canceled.' ? 'default' : 'destructive'}>
                <AlertDescription>{downloadError}</AlertDescription>
              </Alert>
            )}

            <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
              <Button
                type="button"
                variant="outline"
                onClick={() => void browseIso()}
                disabled={downloading || starting || is32Bit}
              >
                <FolderOpen />
                Browse for an ISO
              </Button>
              <p className="min-w-0 text-sm text-muted-foreground break-all">
                {isoPath ? `ISO to use: ${isoPath}` : 'No ISO selected yet.'}
              </p>
            </div>
          </section>

          <div className="flex flex-col items-start gap-2">
            <Button type="button" onClick={() => setConfirmOpen(true)} disabled={!canStart}>
              {starting ? <Loader2 className="animate-spin" /> : <MonitorUp />}
              {starting ? 'Starting Setup…' : 'Start Windows Setup'}
            </Button>
            {!isoPath && !is32Bit && (
              <p className="text-sm text-muted-foreground">
                Choose or download an ISO before starting Setup.
              </p>
            )}
          </div>

          {startResult && (
            <Alert>
              <CheckCircle2 />
              <AlertTitle>Setup started</AlertTitle>
              <AlertDescription>{startedMessage(startResult)}</AlertDescription>
            </Alert>
          )}

          {startError && (
            <Alert variant="destructive">
              <AlertTitle>Setup did not start</AlertTitle>
              <AlertDescription>{startError}</AlertDescription>
            </Alert>
          )}
        </div>
      </ScrollArea>

      <AlertDialog open={confirmOpen} onOpenChange={setConfirmOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Start Windows Setup?</AlertDialogTitle>
            <AlertDialogDescription>
              {confirmMessage(route, advanced, isoPath)}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Go back</AlertDialogCancel>
            <AlertDialogAction onClick={() => void startUpgrade()}>
              Start Setup
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

function RouteButton({
  pressed,
  suggested,
  title,
  detail,
  onClick,
  disabled,
}: {
  pressed: boolean;
  suggested: boolean;
  title: string;
  detail: string;
  onClick: () => void;
  disabled: boolean;
}) {
  return (
    <button
      type="button"
      aria-pressed={pressed}
      onClick={onClick}
      disabled={disabled}
      className={cn(
        'rounded-lg border p-3 text-left transition-colors disabled:opacity-50',
        pressed ? 'border-primary bg-primary/5' : 'border-border hover:bg-muted/40',
      )}
    >
      <div className="flex items-center gap-2">
        <span className="text-sm font-medium">{title}</span>
        {suggested && <Badge variant="secondary">Suggested</Badge>}
      </div>
      <p className="mt-1 text-xs text-muted-foreground">{detail}</p>
    </button>
  );
}

function errorMessage(error: unknown): string {
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === 'string' && error) return error;
  return 'Something went wrong.';
}

function formatBytes(bytes: number): string {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  if (unit === 0) return `${Math.round(bytes)} B`;
  return `${value.toFixed(1)} ${units[unit]}`;
}

function formatGb(value: number): string {
  return `${value.toFixed(1)} GB`;
}

function baseName(path: string): string {
  const parts = path.split(/[/\\]/);
  return parts[parts.length - 1] || path;
}

function summaryFor(assessment: Windows11UpgradeAssessment): string {
  if (!assessment.is64Bit) {
    return 'This PC is 32-bit. Windows 11 installation media is 64-bit only.';
  }
  if (assessment.suggestedRoute === 'unsupported') {
    const found = assessment.suggestionReasons.length > 0
      ? assessment.suggestionReasons.join(' ')
      : 'A Windows 11 check did not pass.';
    return `${found} Both routes stay available. The compatibility path skips the TPM, Secure Boot, and CPU generation block.`;
  }
  return 'The checks that choose a route look fine. Both routes stay available, and low disk space is only a warning. Windows Setup still makes the final call.';
}

function buildChecks(assessment: Windows11UpgradeAssessment): CheckRow[] {
  const build = assessment.updateBuild != null
    ? `${assessment.build}.${assessment.updateBuild}`
    : String(assessment.build);
  const edition = [assessment.edition, assessment.displayVersion].filter(Boolean).join(' ');
  const windowsOk = assessment.isWindows10 || assessment.isWindows11;
  const memory = num(assessment.memoryGb);
  const minimumRam = num(assessment.minimumRamGb, 4);
  const recommendedFree = num(assessment.recommendedFreeSpaceGb, 25);

  let memoryTone: CheckTone = 'ready';
  let memoryDetail = `${formatGb(memory)} installed.`;
  if (memory <= 0) {
    memoryTone = 'unknown';
    memoryDetail = 'Could not read installed memory.';
  } else if (memory < minimumRam) {
    memoryTone = 'review';
    memoryDetail = `${formatGb(memory)} installed. Windows 11 asks for ${formatGb(minimumRam)}. Setup can still be started.`;
  }

  let spaceTone: CheckTone = 'unknown';
  let spaceDetail = 'Could not read free space on the Windows drive.';
  if (assessment.freeSpaceGb != null) {
    const freeSpace = num(assessment.freeSpaceGb);
    if (freeSpace >= recommendedFree) {
      spaceTone = 'ready';
      spaceDetail = `${formatGb(freeSpace)} free.`;
    } else {
      spaceTone = 'review';
      spaceDetail = `${formatGb(freeSpace)} free. ${formatGb(recommendedFree)} is recommended. This is a warning only.`;
    }
  }

  return [
    {
      label: 'Windows',
      tone: windowsOk ? 'ready' : 'review',
      detail: windowsOk
        ? `${edition || assessment.windowsName} (build ${build}).`
        : `${edition || assessment.windowsName} (build ${build}). This version is outside Windows 10 and Windows 11.`,
    },
    {
      label: 'Architecture',
      tone: assessment.is64Bit ? 'ready' : 'review',
      detail: assessment.is64Bit
        ? assessment.architecture
        : 'Windows 11 installation media is 64-bit only. This PC is 32-bit.',
    },
    { label: 'Memory', tone: memoryTone, detail: memoryDetail },
    triState(
      'TPM',
      assessment.hasTpm20,
      'TPM 2.0 is present.',
      'TPM 2.0 was not found. The compatibility path skips this check.',
      'Could not read the TPM.',
    ),
    triState(
      'Secure Boot',
      assessment.secureBootEnabled,
      'Secure Boot is on.',
      'Secure Boot is off. The compatibility path skips this check.',
      'Could not read Secure Boot.',
    ),
    { label: 'Free space', tone: spaceTone, detail: spaceDetail },
    {
      label: 'Processor',
      tone: 'info',
      detail: assessment.processorName || 'Could not read the processor name.',
    },
    cpuInstructionRow(assessment),
  ];
}

function triState(
  label: string,
  value: boolean | null,
  ready: string,
  review: string,
  unknown: string,
): CheckRow {
  if (value === true) return { label, tone: 'ready', detail: ready };
  if (value === false) return { label, tone: 'review', detail: review };
  return { label, tone: 'unknown', detail: unknown };
}

function cpuInstructionRow(assessment: Windows11UpgradeAssessment): CheckRow {
  const { hasPopcnt, hasSse42 } = assessment;
  if (hasPopcnt == null && hasSse42 == null) {
    return {
      label: 'POPCNT and SSE4.2',
      tone: 'info',
      detail: 'Not applicable on this processor.',
    };
  }
  if (hasPopcnt === true && hasSse42 === true) {
    return {
      label: 'POPCNT and SSE4.2',
      tone: 'ready',
      detail: 'Both instructions are present, so a current Windows 11 ISO can run here.',
    };
  }
  const missing = [
    hasPopcnt === false ? 'POPCNT' : null,
    hasSse42 === false ? 'SSE4.2' : null,
  ].filter(Boolean).join(' and ');
  return {
    label: 'POPCNT and SSE4.2',
    tone: 'review',
    detail: `${missing || 'One of these instructions'} is missing. Windows 11 24H2 and later will not install, and the compatibility path cannot bypass that.`,
  };
}

function confirmMessage(route: Windows11UpgradeRoute, advanced: boolean, isoPath: string): string {
  const file = baseName(isoPath);
  const backup = 'Back up this PC first. Nothing is installed until you continue in the Windows Setup window. The ISO stays mounted while Setup runs.';
  if (route === 'supported') {
    return `This starts ${file} with standard Setup, which keeps Windows' own hardware checks. ${backup}`;
  }
  const extra = advanced ? ' Driver warning flags are included.' : '';
  return `This starts ${file} with the compatibility path. That skips the TPM, Secure Boot, and CPU generation block. The installed edition is still the client edition on the ISO. Microsoft does not support the result.${extra} ${backup}`;
}

function startedMessage(result: Windows11UpgradeStart): string {
  const command = result.arguments
    ? `${result.setupPath} ${result.arguments}`
    : result.setupPath;
  const build = result.mediaBuild > 0 ? ` Media build ${result.mediaBuild}.` : '';
  return `Started ${command}. Continue in the Windows Setup window. The ISO stays mounted as ${result.drive}.${build}`;
}
