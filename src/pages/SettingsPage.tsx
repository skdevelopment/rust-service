/**
 * Settings Page Component
 *
 * Scalable settings interface with sidebar navigation for categories.
 * Features a modern, spacious layout with cards that fill the available space.
 */

import { useState, useEffect } from 'react';
import { NAVIGATE_SETTINGS_CATEGORY_EVENT, takePendingSettingsCategory } from '@/lib/navigation';

import { ScrollArea } from '@/components/ui/scroll-area';
import type { SettingsCategory } from '@/types/settings';
import { ServiceMetricsPanel } from '@/components/ServiceMetricsPanel';
import {
  SettingsSidebar,
  AppearancePanel,
  DataPanel,
  ReportsPanel,
  BusinessPanel,
  ProgramsPanel,
  ServicePresetsPanel,
  TechnicianTabsPanel,
  AgentPanel,
  AboutPanel,
} from '@/components/settings';

// =============================================================================
// Main Component
// =============================================================================

export function SettingsPage() {
  const [activeCategory, setActiveCategory] = useState<SettingsCategory>(
    () => takePendingSettingsCategory() ?? 'appearance',
  );

  // Listen for external navigation to a specific settings category
  useEffect(() => {
    const handler = (event: Event) => {
      const custom = event as CustomEvent<SettingsCategory>;
      const category = custom.detail;
      if (typeof category === 'string') {
        takePendingSettingsCategory();
        setActiveCategory(category);
      }
    };

    window.addEventListener(NAVIGATE_SETTINGS_CATEGORY_EVENT, handler as EventListener);
    return () => window.removeEventListener(NAVIGATE_SETTINGS_CATEGORY_EVENT, handler as EventListener);
  }, []);

  const renderPanel = () => {
    switch (activeCategory) {
      case 'appearance':
        return <AppearancePanel />;
      case 'data':
        return <DataPanel />;
      case 'reports':
        return <ReportsPanel />;
      case 'business':
        return <BusinessPanel />;
      case 'programs':
        return <ProgramsPanel />;
      case 'servicePresets':
        return <ServicePresetsPanel />;
      case 'technicianTabs':
        return <TechnicianTabsPanel />;
      case 'serviceMetrics':
        return <ServiceMetricsPanel />;
      case 'agent':
        return <AgentPanel />;
      case 'about':
        return <AboutPanel />;
      default:
        return null;
    }
  };

  return (
    <div className="h-full flex min-h-0 overflow-hidden">
      {/* Sidebar */}
      <SettingsSidebar
        activeCategory={activeCategory}
        onCategoryChange={setActiveCategory}
      />

      {/* Content Panel */}
      <ScrollArea className="flex-1 min-h-0">
        <div className="p-4">
          {renderPanel()}
        </div>
      </ScrollArea>
    </div>
  );
}
