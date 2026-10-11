/**
 * RustService Main Application
 * 
 * Windows desktop toolkit for computer repair technicians.
 * Tab-based navigation for work, diagnostics, and library, plus custom technician tabs.
 */

import { useState, useEffect, ReactNode, useMemo } from 'react';
import {
  Wrench,
  Monitor,
  TestTube,
  Zap,
  AppWindow,
  ScrollText,
  FileText,
  Settings,
  Globe,

  Rocket,
  Network,
  Skull,
  Bot,
  Toolbox,
  // Icon picker icons
  Folder,
  Database,
  Mail,
  Calendar,
  ShoppingCart,
  CreditCard,
  BarChart,
  Code,
  Terminal,
  Cloud,
  Lock,
  Smartphone,
  Headphones,
  User,
} from 'lucide-react';

import { ScrollArea } from '@/components/ui/scroll-area';
import { Separator } from '@/components/ui/separator';
import { Button } from '@/components/ui/button';
import { TooltipProvider } from '@/components/ui/tooltip';
import { cn } from '@/lib/utils';
import { ThemeProvider } from '@/components/theme-provider';
import { SettingsProvider, useSettings } from '@/components/settings-context';
import { AnimationProvider, useAnimation, motion, AnimatePresence, tabContentVariants } from '@/components/animation-context';
import { ServiceRunProvider } from '@/components/service-run-context';
import { RequiredProgramInstallProvider } from '@/components/required-program-install-context';
import { AgentSessionProvider } from '@/components/agent-session-context';
import { NAVIGATE_TAB_EVENT } from '@/lib/navigation';
import { FloatingServiceStatus } from '@/components/floating-service-status';
import { Titlebar } from '@/components/titlebar';
import { IframeTabContent } from '@/components/IframeTabContent';
import { ErrorBoundary } from '@/components/error-boundary';
import {
  AgentPage,
  ServicePage,
  SystemInfoPage,
  ComponentTestPage,
  ShortcutsPage,
  ProgramsPage,
  ScriptsPage,
  ReportsPage,
  SettingsPage,
  NetworkDiagnosticsPage,
  StartupManagerPage,
  EventLogPage,
  BluescreenPage,
  ToolsPage,
} from '@/pages';
import type { TechnicianTab } from '@/types/settings';

import '@/styles/globals.css';
import { commands } from '@/bindings';

/** Maps icon ID strings to Lucide icon components */
function getIconComponent(iconId: string): React.ComponentType<{ className?: string }> {
  const iconMap: Record<string, React.ComponentType<{ className?: string }>> = {
    'globe': Globe,
    'file-text': FileText,
    'folder': Folder,
    'database': Database,
    'mail': Mail,
    'calendar': Calendar,
    'settings': Settings,
    'user': User,
    'shopping-cart': ShoppingCart,
    'credit-card': CreditCard,
    'bar-chart': BarChart,
    'code': Code,
    'terminal': Terminal,
    'cloud': Cloud,
    'lock': Lock,
    'tool': Wrench,
    'wrench': Wrench,
    'monitor': Monitor,
    'smartphone': Smartphone,
    'headphones': Headphones,
  };
  return iconMap[iconId] ?? Globe;
}

/** Gets favicon URL for a website using DuckDuckGo's favicon service */
function getFaviconUrl(siteUrl: string): string {
  try {
    const url = new URL(siteUrl);
    // Use DuckDuckGo's favicon service - more reliable
    return `https://icons.duckduckgo.com/ip3/${url.hostname}.ico`;
  } catch {
    return '';
  }
}

/** Component that displays either a preset icon or the website's favicon */
function TechnicianTabIcon({ tab, useFavicons }: { tab: TechnicianTab; useFavicons: boolean }) {
  const [faviconError, setFaviconError] = useState(false);
  
  // If tab has a preset icon, use it
  if (tab.icon) {
    const IconComponent = getIconComponent(tab.icon);
    return <IconComponent className="h-4 w-4" />;
  }
  
  // Otherwise try favicon if enabled
  if (useFavicons && !faviconError) {
    const faviconUrl = getFaviconUrl(tab.url);
    if (faviconUrl) {
      return (
        <img 
          src={faviconUrl} 
          alt=""
          className="h-4 w-4"
          onError={() => setFaviconError(true)}
        />
      );
    }
  }
  
  // Fallback to globe icon
  return <Globe className="h-4 w-4" />;
}

const WORK_TABS = [
  { id: 'agent', label: 'Agent', icon: Bot, component: AgentPage },
  { id: 'service', label: 'Service', icon: Wrench, component: ServicePage },
  { id: 'programs', label: 'Programs', icon: AppWindow, component: ProgramsPage },
  { id: 'scripts', label: 'Scripts', icon: ScrollText, component: ScriptsPage },
  { id: 'shortcuts', label: 'Shortcuts', icon: Zap, component: ShortcutsPage },
  { id: 'tools', label: 'Tools', icon: Toolbox, component: ToolsPage },
] as const;

const DIAGNOSTICS_TABS = [
  { id: 'system-info', label: 'System Info', icon: Monitor, component: SystemInfoPage },
  { id: 'component-test', label: 'Component Test', icon: TestTube, component: ComponentTestPage },
  { id: 'network-diagnostics', label: 'Network Diagnostics', icon: Network, component: NetworkDiagnosticsPage },
  { id: 'startup-manager', label: 'Startup Manager', icon: Rocket, component: StartupManagerPage },
  { id: 'event-log', label: 'Event Log', icon: ScrollText, component: EventLogPage },
  { id: 'bluescreen', label: 'Bluescreen Analysis', icon: Skull, component: BluescreenPage },
] as const;

const LIBRARY_TABS = [
  { id: 'reports', label: 'Reports', icon: FileText, component: ReportsPage },
  { id: 'settings', label: 'Settings', icon: Settings, component: SettingsPage },
] as const;

const ALL_TABS = [...WORK_TABS, ...DIAGNOSTICS_TABS, ...LIBRARY_TABS];

const NAV_SECTIONS = [
  { label: 'Work', items: WORK_TABS },
  { label: 'Diagnostics', items: DIAGNOSTICS_TABS },
  { label: 'Library', items: LIBRARY_TABS },
] as const;

function NavButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      onClick={onClick}
      data-active={active ? 'true' : undefined}
      className={cn(
        'w-full justify-start gap-2 px-2 text-muted-foreground hover:text-foreground',
        active && 'bg-muted text-foreground'
      )}
    >
      {children}
    </Button>
  );
}

/**
 * Animated tab content wrapper
 */
function AnimatedTabContent({ id, children }: { id: string; children: ReactNode }) {
  const { animationsEnabled } = useAnimation();

  if (!animationsEnabled) {
    return <>{children}</>;
  }

  return (
    <motion.div
      key={id}
      variants={tabContentVariants}
      initial="hidden"
      animate="show"
      exit="exit"
      className="flex-1 flex flex-col min-h-0"
    >
      {children}
    </motion.div>
  );
}

/**
 * App content with animations
 */
function AppContent() {
  const [activeTab, setActiveTab] = useState('service');
  const { animationsEnabled } = useAnimation();
  const { settings } = useSettings();

  // Get technician tabs from settings
  const technicianTabs = useMemo(() => 
    settings.technicianTabs?.tabs ?? [],
    [settings.technicianTabs?.tabs]
  );
  const useFavicons = settings.technicianTabs?.useFavicons ?? true;

  useEffect(() => {
    const handler = (event: Event) => {
      const custom = event as CustomEvent<string>;
      const nextTab = custom.detail;
      if (typeof nextTab !== 'string') return;
      // Check both regular tabs and technician tabs
      if (ALL_TABS.some((t) => t.id === nextTab) || technicianTabs.some(t => `tech-${t.id}` === nextTab)) {
        setActiveTab(nextTab);
      }
    };

    window.addEventListener(NAVIGATE_TAB_EVENT, handler as EventListener);
    return () => window.removeEventListener(NAVIGATE_TAB_EVENT, handler as EventListener);
  }, [technicianTabs]);

  return (
    <div className="h-screen flex flex-col bg-background overflow-hidden">
      <Titlebar />

      <div className="flex flex-1 min-h-0">
        <nav
          aria-label="Main"
          className="w-52 shrink-0 border-r bg-muted/40 flex flex-col min-h-0"
        >
          <ScrollArea className="flex-1">
            <div className="p-2 space-y-4">
              {NAV_SECTIONS.map((section) => (
                <div key={section.label} className="space-y-1">
                  <div className="px-2 py-1 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                    {section.label}
                  </div>
                  {section.items.map(({ id, label, icon: Icon }) => (
                    <NavButton
                      key={id}
                      active={activeTab === id}
                      onClick={() => setActiveTab(id)}
                    >
                      <Icon className="h-4 w-4 shrink-0" />
                      <span className="truncate">{label}</span>
                    </NavButton>
                  ))}
                </div>
              ))}

              {technicianTabs.length > 0 ? (
                <div className="space-y-1">
                  <Separator />
                  <div className="px-2 py-1 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                    Technician
                  </div>
                  {technicianTabs.map((tab) => (
                    <NavButton
                      key={`tech-${tab.id}`}
                      active={activeTab === `tech-${tab.id}`}
                      onClick={() => setActiveTab(`tech-${tab.id}`)}
                    >
                      <TechnicianTabIcon tab={tab} useFavicons={useFavicons} />
                      <span className="truncate">{tab.name}</span>
                    </NavButton>
                  ))}
                </div>
              ) : null}
            </div>
          </ScrollArea>
        </nav>

        <main className="flex-1 overflow-hidden min-h-0 relative">
          <AnimatePresence mode="wait" initial={false}>
            {ALL_TABS.map(({ id, component: Component, label }) => (
              activeTab === id && (
                <div key={id} className="absolute inset-0 flex flex-col">
                  <AnimatedTabContent id={animationsEnabled ? id : 'static'}>
                    <ErrorBoundary label={label}>
                      <Component />
                    </ErrorBoundary>
                  </AnimatedTabContent>
                </div>
              )
            ))}
          </AnimatePresence>

          {technicianTabs.map((tab) => {
            const isActive = activeTab === `tech-${tab.id}`;
            return (
              <div
                key={`tech-${tab.id}`}
                className={`absolute inset-0 ${isActive ? 'flex flex-col z-10' : 'invisible pointer-events-none z-0'}`}
              >
                <IframeTabContent url={tab.url} name={tab.name} />
              </div>
            );
          })}
        </main>
      </div>

      <FloatingServiceStatus activeTab={activeTab} />
    </div>
  );
}

/**
 * Main application component with tab-based navigation
 */
function App() {
  // Ensure data directory exists on startup
  useEffect(() => {
    async function initializeDataDir() {
      try {
        await commands.ensureDataDir();
      } catch (error) {
        console.error('Failed to initialize data directory:', error);
      }
    }
    initializeDataDir();
  }, []);

  return (
    <SettingsProvider>
      <ThemeProvider>
        <AnimationProvider>
          <ServiceRunProvider>
            <RequiredProgramInstallProvider>
              <AgentSessionProvider>
                <ErrorBoundary>
                  <TooltipProvider>
                    <AppContent />
                  </TooltipProvider>
                </ErrorBoundary>
              </AgentSessionProvider>
            </RequiredProgramInstallProvider>
          </ServiceRunProvider>
        </AnimationProvider>
      </ThemeProvider>
    </SettingsProvider>
  );
}

export default App;

