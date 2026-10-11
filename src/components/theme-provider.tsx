/**
 * Theme Provider Component
 *
 * Provides theme context to the application with support for:
 * - Light/Dark/System mode
 * - Color schemes (e.g., Forge, Cobalt)
 *
 * The theme lives in settings (`appearance.theme` / `appearance.colorScheme`);
 * this provider reads and writes it through the settings context, so the two
 * never disagree. Must be rendered inside SettingsProvider.
 */

import { createContext, useContext, useEffect, useState, type ReactNode, useCallback } from 'react';
import { COLOR_SCHEMES, DEFAULT_APPEARANCE } from '@/types/settings';
import type { ThemeMode, ColorScheme } from '@/types/settings';
import { useSettings } from '@/components/settings-context';

interface ThemeProviderContextType {
  /** Current theme mode (light/dark/system) */
  themeMode: ThemeMode;
  /** Current color scheme */
  colorScheme: ColorScheme;
  /** Resolved theme (always 'light' or 'dark') */
  resolvedTheme: 'light' | 'dark';
  /** Set theme mode */
  setThemeMode: (mode: ThemeMode) => void;
  /** Set color scheme */
  setColorScheme: (scheme: ColorScheme) => void;
  /** Legacy: alias for themeMode */
  theme: ThemeMode;
  /** Legacy: alias for setThemeMode */
  setTheme: (theme: ThemeMode) => void;
}

const ThemeProviderContext = createContext<ThemeProviderContextType | undefined>(undefined);

/**
 * Gets the system's preferred color scheme
 */
function getSystemTheme(): 'light' | 'dark' {
  if (typeof window !== 'undefined') {
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }
  return 'light';
}

function isColorScheme(value: unknown): value is ColorScheme {
  return typeof value === 'string' && COLOR_SCHEMES.some(s => s.id === value);
}

/**
 * Apply theme classes to the document root
 */
function applyTheme(resolvedTheme: 'light' | 'dark', colorScheme: ColorScheme) {
  const root = document.documentElement;

  root.classList.remove('light', 'dark');
  root.classList.add(resolvedTheme);

  // Remove old color scheme classes (all theme-* classes)
  Array.from(root.classList)
    .filter(c => c.startsWith('theme-'))
    .forEach(c => root.classList.remove(c));

  // 'default' uses the :root variables
  if (colorScheme !== 'default') {
    root.classList.add(`theme-${colorScheme}`);
  }
}

/**
 * ThemeProvider component that wraps the application and provides theme context
 */
export function ThemeProvider({ children }: { children: ReactNode }) {
  const { settings, updateSetting } = useSettings();
  const themeMode: ThemeMode = settings.appearance?.theme ?? DEFAULT_APPEARANCE.theme ?? 'system';
  const storedScheme = settings.appearance?.colorScheme;
  const colorScheme: ColorScheme = isColorScheme(storedScheme) ? storedScheme : DEFAULT_APPEARANCE.colorScheme as ColorScheme;
  const [systemTheme, setSystemTheme] = useState<'light' | 'dark'>(getSystemTheme);

  // Follow the OS theme while in 'system' mode
  useEffect(() => {
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handleChange = (e: MediaQueryListEvent) => setSystemTheme(e.matches ? 'dark' : 'light');
    mediaQuery.addEventListener('change', handleChange);
    return () => mediaQuery.removeEventListener('change', handleChange);
  }, []);

  const resolvedTheme = themeMode === 'system' ? systemTheme : themeMode;

  useEffect(() => {
    applyTheme(resolvedTheme, colorScheme);
  }, [resolvedTheme, colorScheme]);

  const setThemeMode = useCallback((mode: ThemeMode) => {
    updateSetting('appearance.theme', mode).catch(error => console.warn('Failed to save theme mode:', error));
  }, [updateSetting]);

  const setColorScheme = useCallback((scheme: ColorScheme) => {
    updateSetting('appearance.colorScheme', scheme).catch(error => console.warn('Failed to save color scheme:', error));
  }, [updateSetting]);

  return (
    <ThemeProviderContext.Provider value={{
      themeMode,
      colorScheme,
      resolvedTheme,
      setThemeMode,
      setColorScheme,
      // Legacy aliases
      theme: themeMode,
      setTheme: setThemeMode,
    }}>
      {children}
    </ThemeProviderContext.Provider>
  );
}

/**
 * Hook to access the theme context
 */
export function useTheme() {
  const context = useContext(ThemeProviderContext);
  if (context === undefined) {
    throw new Error('useTheme must be used within a ThemeProvider');
  }
  return context;
}
