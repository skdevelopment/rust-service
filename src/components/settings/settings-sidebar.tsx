/**
 * Settings Sidebar Component
 *
 * Navigation sidebar for settings categories with icon-based items.
 */

import {
  Settings,
  Palette,
  FolderOpen,
  Info,
  FileText,
  Building2,
  Sparkles,
  Package,
  Globe,
  Timer,
  Layers,
  Loader2,
} from 'lucide-react';
import { isInstallActive, useRequiredProgramInstall } from '@/components/required-program-install-context';
import type { LucideIcon } from 'lucide-react';

import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import type { SettingsCategory } from '@/types/settings';

// =============================================================================
// Types
// =============================================================================

export interface SidebarItem {
  id: SettingsCategory;
  label: string;
  description: string;
  icon: LucideIcon;
  iconColor: string;
}

// =============================================================================
// Constants
// =============================================================================

export const SIDEBAR_ITEMS: SidebarItem[] = [
  {
    id: 'appearance',
    label: 'Appearance',
    description: 'Theme and colors',
    icon: Palette,
    iconColor: 'text-muted-foreground',
  },
  {
    id: 'data',
    label: 'Data & Storage',
    description: 'Folders and logs',
    icon: FolderOpen,
    iconColor: 'text-muted-foreground',
  },
  {
    id: 'reports',
    label: 'Reports',
    description: 'Report settings',
    icon: FileText,
    iconColor: 'text-muted-foreground',
  },
  {
    id: 'business',
    label: 'Business',
    description: 'Branding & technicians',
    icon: Building2,
    iconColor: 'text-muted-foreground',
  },
  {
    id: 'programs',
    label: 'Required Programs',
    description: 'Service dependencies',
    icon: Package,
    iconColor: 'text-muted-foreground',
  },
  {
    id: 'servicePresets',
    label: 'Service Presets',
    description: 'Preset configurations',
    icon: Layers,
    iconColor: 'text-muted-foreground',
  },
  {
    id: 'technicianTabs',
    label: 'Technician Tabs',
    description: 'Custom web tabs',
    icon: Globe,
    iconColor: 'text-muted-foreground',
  },
  {
    id: 'serviceMetrics',
    label: 'Service Metrics',
    description: 'Timing & estimates',
    icon: Timer,
    iconColor: 'text-muted-foreground',
  },
  {
    id: 'agent',
    label: 'AI Agent',
    description: 'Provider & execution',
    icon: Sparkles,
    iconColor: 'text-muted-foreground',
  },
  {
    id: 'about',
    label: 'About',
    description: 'Version info',
    icon: Info,
    iconColor: 'text-muted-foreground',
  },
];

// =============================================================================
// Sidebar Component
// =============================================================================

interface SettingsSidebarProps {
  activeCategory: SettingsCategory;
  onCategoryChange: (category: SettingsCategory) => void;
}

export function SettingsSidebar({ activeCategory, onCategoryChange }: SettingsSidebarProps) {
  const { jobs } = useRequiredProgramInstall();
  const installingCount = Object.values(jobs).filter((job) => isInstallActive(job)).length;

  return (
    <div className="w-52 border-r bg-muted/30 flex flex-col">
      <div className="flex items-center gap-2 p-4 border-b">
        <Settings className="h-5 w-5 text-primary" />
        <h2 className="font-semibold">Settings</h2>
      </div>

      <nav className="flex-1 p-2 space-y-1" aria-label="Settings">
        {SIDEBAR_ITEMS.map((item) => {
          const Icon = item.icon;
          const isActive = activeCategory === item.id;

          return (
            <Button
              key={item.id}
              type="button"
              variant="ghost"
              size="sm"
              onClick={() => onCategoryChange(item.id)}
              data-active={isActive ? 'true' : undefined}
              className={cn(
                'w-full justify-start gap-2 px-2 text-muted-foreground hover:text-foreground',
                isActive && 'bg-muted text-foreground'
              )}
            >
              <Icon className={cn('h-4 w-4', item.iconColor)} />
              <span className="truncate">{item.label}</span>
              {item.id === 'programs' && installingCount > 0 ? (
                <>
                  <Loader2 className="ml-auto h-3.5 w-3.5 animate-spin text-primary" aria-hidden />
                  <span className="sr-only">Installing</span>
                </>
              ) : null}
            </Button>
          );
        })}
      </nav>
    </div>
  );
}
