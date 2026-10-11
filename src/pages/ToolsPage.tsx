/**
 * Tools Page
 *
 * Utilities that run on their own, outside a service run.
 */

import { useState } from 'react';
import { MonitorUp, Toolbox } from 'lucide-react';

import { ScrollArea } from '@/components/ui/scroll-area';
import { Windows11Upgrade } from './tools/Windows11Upgrade';

export function ToolsPage() {
  const [activeTool, setActiveTool] = useState<string | null>(null);

  if (activeTool === 'windows11-upgrade') {
    return <Windows11Upgrade onBack={() => setActiveTool(null)} />;
  }

  return (
    <div className="h-full flex flex-col overflow-hidden min-h-0">
      <div className="p-4 border-b flex items-center gap-2 flex-shrink-0">
        <Toolbox className="h-5 w-5 text-primary" />
        <div>
          <h2 className="font-semibold">Tools</h2>
          <p className="text-sm text-muted-foreground">
            Utilities that run outside a service run.
          </p>
        </div>
      </div>
      <ScrollArea className="flex-1 min-h-0">
        <div className="p-4 max-w-3xl">
          <button
            type="button"
            onClick={() => setActiveTool('windows11-upgrade')}
            className="w-full rounded-xl border bg-card p-4 text-left shadow-sm transition-colors hover:bg-muted/40 focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] outline-none"
          >
            <div className="flex items-start gap-3">
              <MonitorUp className="mt-0.5 h-5 w-5 text-primary" />
              <div>
                <div className="font-medium">Windows 11 upgrade</div>
                <p className="mt-1 text-sm text-muted-foreground">
                  In-place upgrade from a Windows 11 ISO. Supported PCs use standard Setup.
                  Unsupported PCs use the compatibility path.
                </p>
              </div>
            </div>
          </button>
        </div>
      </ScrollArea>
    </div>
  );
}
