import { create } from 'zustand';
import { DaemonConfig } from '../types';
import { api } from '../api';

interface ConfigStore {
  config: DaemonConfig | null;
  loading: boolean;
  error: string | null;

  // Actions
  fetchConfig: () => Promise<void>;
  updateConfig: (updates: Partial<DaemonConfig>) => Promise<void>;
}

export const useConfigStore = create<ConfigStore>((set, get) => ({
  config: null,
  loading: false,
  error: null,

  fetchConfig: async () => {
    set({ loading: true, error: null });
    try {
      const config = await api.config();
      set({ config, loading: false });
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    } catch (err: any) {
      set({ error: err.message || 'Failed to fetch config', loading: false });
    }
  },

  updateConfig: async (updates) => {
    set({ loading: true, error: null });
    try {
      await api.setConfig(updates);
      // refetch to ensure we have the latest merged config from server
      await get().fetchConfig();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    } catch (err: any) {
      set({ error: err.message || 'Failed to update config', loading: false });
    }
  }
}));
