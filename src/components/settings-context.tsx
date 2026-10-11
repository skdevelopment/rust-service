/**
 * Settings Context
 *
 * Provides global access to application settings from any component.
 * Settings are loaded on mount and cached in React state.
 *
 * Every change goes through `updateSetting(path, value)`, which updates the
 * cached settings immediately (so fast typing never loses characters) and
 * persists only that one field in the backend (so concurrent writers can't
 * overwrite each other). Text inputs should use `useDebouncedSetting` to avoid
 * a disk write per keystroke.
 *
 * @example
 * ```tsx
 * const { settings, updateSetting } = useSettings();
 * await updateSetting('appearance.theme', 'dark');
 * await updateSetting('agent.apiKeys.openai', key);
 * ```
 */

import {
  createContext,
  useContext,
  useEffect,
  useRef,
  useState,
  useCallback,
  type ReactNode,
} from 'react';
import { commands } from '@/bindings';
import type { AppSettings, SettingKey, SettingValueType } from '@/types/settings';
import { DEFAULT_SETTINGS } from '@/types/settings';
import { applyAppFonts } from '@/lib/fonts';

// =============================================================================
// Context Types
// =============================================================================

interface SettingsContextValue {
  /** Current settings object - always available (uses defaults while loading) */
  settings: AppSettings;
  /** Whether settings are currently being loaded from disk */
  isLoading: boolean;
  /** Any error that occurred during load/save */
  error: string | null;
  /** Update one setting by dotted path */
  updateSetting: <K extends SettingKey>(key: K, value: SettingValueType<K>) => Promise<void>;
  /** Reload settings from disk */
  refreshSettings: () => Promise<void>;
}

const SettingsContext = createContext<SettingsContextValue | null>(null);

/** Return a copy of `obj` with the value at dotted `path` replaced. */
function setAtPath<T>(obj: T, path: string, value: unknown): T {
  const [head, ...rest] = path.split('.');
  const current = (obj ?? {}) as Record<string, unknown>;
  return {
    ...current,
    [head]: rest.length === 0 ? value : setAtPath(current[head], rest.join('.'), value),
  } as T;
}

/** Read the value at dotted `path`. */
export function getAtPath(obj: unknown, path: string): unknown {
  return path.split('.').reduce<unknown>(
    (acc, part) => (acc && typeof acc === 'object' ? (acc as Record<string, unknown>)[part] : undefined),
    obj,
  );
}

// =============================================================================
// Provider Component
// =============================================================================

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  // Serialize writes so they reach the backend in the order they were made.
  const writeChain = useRef<Promise<unknown>>(Promise.resolve());

  const loadSettings = useCallback(async () => {
    try {
      setIsLoading(true);
      setError(null);
      setSettings(await commands.getSettings());
    } catch (e) {
      console.error('Failed to load settings:', e);
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  const { uiFont, monoFont } = settings.appearance;
  useEffect(() => {
    applyAppFonts(uiFont ?? '', monoFont ?? '');
  }, [uiFont, monoFont]);

  const updateSetting = useCallback(
    async <K extends SettingKey>(key: K, value: SettingValueType<K>) => {
      setError(null);
      setSettings(prev => setAtPath(prev, key, value));

      const write = writeChain.current.then(() =>
        commands.updateSetting(key, JSON.stringify(value ?? null)),
      );
      writeChain.current = write.catch(() => undefined);
      try {
        await write;
      } catch (e) {
        console.error(`Failed to update setting ${key}:`, e);
        setError(e instanceof Error ? e.message : String(e));
        // The optimistic value was rejected; resync with what is on disk.
        await loadSettings();
        throw e;
      }
    },
    [loadSettings],
  );

  const value: SettingsContextValue = {
    settings,
    isLoading,
    error,
    updateSetting,
    refreshSettings: loadSettings,
  };

  return <SettingsContext.Provider value={value}>{children}</SettingsContext.Provider>;
}

// =============================================================================
// Hooks
// =============================================================================

/**
 * Access settings from any component
 *
 * @throws Error if used outside of SettingsProvider
 */
export function useSettings(): SettingsContextValue {
  const context = useContext(SettingsContext);
  if (!context) {
    throw new Error('useSettings must be used within a SettingsProvider');
  }
  return context;
}

/**
 * Access a single setting value
 *
 * @example
 * ```tsx
 * const [theme, setTheme] = useSetting('appearance.theme');
 * ```
 */
export function useSetting<K extends SettingKey>(
  key: K,
): [SettingValueType<K>, (value: SettingValueType<K>) => Promise<void>] {
  const { settings, updateSetting } = useSettings();
  const value = getAtPath(settings, key) as SettingValueType<K>;
  const setValue = useCallback((v: SettingValueType<K>) => updateSetting(key, v), [key, updateSetting]);
  return [value, setValue];
}

/**
 * Local state for a text input bound to a setting, saved after the user stops
 * typing (and on unmount), instead of on every keystroke.
 *
 * @example
 * ```tsx
 * const [name, setName] = useDebouncedSetting('business.name');
 * <Input value={name} onChange={e => setName(e.target.value)} />
 * ```
 */
export function useDebouncedSetting<K extends SettingKey>(
  key: K,
  delayMs = 600,
): [SettingValueType<K>, (value: SettingValueType<K>) => void] {
  const { settings, updateSetting } = useSettings();
  const saved = getAtPath(settings, key) as SettingValueType<K>;
  const [draft, setDraft] = useState<SettingValueType<K>>(saved);
  const pending = useRef<{ value: SettingValueType<K> } | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Follow external changes while the user isn't editing.
  useEffect(() => {
    if (!pending.current) setDraft(saved);
  }, [saved]);

  const flush = useCallback(() => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
    const p = pending.current;
    pending.current = null;
    if (p) updateSetting(key, p.value).catch(() => undefined);
  }, [key, updateSetting]);

  useEffect(() => flush, [flush]);

  const setValue = useCallback(
    (value: SettingValueType<K>) => {
      setDraft(value);
      pending.current = { value };
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(flush, delayMs);
    },
    [delayMs, flush],
  );

  return [draft, setValue];
}
