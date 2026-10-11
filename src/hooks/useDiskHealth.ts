/**
 * useDiskHealth Hook
 *
 * Manages disk health (S.M.A.R.T.) data fetching and state.
 */

import { useEffect, useState, useCallback } from 'react';
import type { DiskHealthResponse } from '@/components/system-info/types';
import { commands } from '@/bindings';

export interface UseDiskHealthReturn {
  diskHealth: DiskHealthResponse | null;
  isLoading: boolean;
  refresh: () => void;
}

export function useDiskHealth(): UseDiskHealthReturn {
  const [diskHealth, setDiskHealth] = useState<DiskHealthResponse | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetchDiskHealth = useCallback(async () => {
    setIsLoading(true);
    try {
      const result = await commands.getDiskHealth();
      setDiskHealth(result);
    } catch (err) {
      setDiskHealth({ disks: [], smartctlFound: false, error: String(err) });
    } finally {
      setIsLoading(false);
    }
  }, []);

  // Fetch on mount
  useEffect(() => {
    fetchDiskHealth();
  }, [fetchDiskHealth]);

  return {
    diskHealth,
    isLoading,
    refresh: fetchDiskHealth,
  };
}
