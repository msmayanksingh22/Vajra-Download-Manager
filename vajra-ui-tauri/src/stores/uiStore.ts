import { create } from 'zustand';

interface UiStore {
  // Theme state
  theme: 'dark' | 'light' | 'system';
  setTheme: (theme: 'dark' | 'light' | 'system') => void;

  // RTL/Text direction
  dir: 'ltr' | 'rtl';
  setDir: (dir: 'ltr' | 'rtl') => void;

  // Modal states
  isAddModalOpen: boolean;
  setAddModalOpen: (open: boolean) => void;
  
  isSettingsModalOpen: boolean;
  setSettingsModalOpen: (open: boolean) => void;
  
  isStatsModalOpen: boolean;
  setStatsModalOpen: (open: boolean) => void;

  isDeleteModalOpen: boolean;
  setDeleteModalOpen: (open: boolean) => void;

  isSchedulerModalOpen: boolean;
  setSchedulerModalOpen: (open: boolean) => void;

  isRefreshModalOpen: boolean;
  setRefreshModalOpen: (open: boolean) => void;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  refreshModalItem: any;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  setRefreshModalItem: (item: any) => void;

  isGrabberModalOpen: boolean;
  setGrabberModalOpen: (open: boolean) => void;

  isSpiderModalOpen: boolean;
  setSpiderModalOpen: (open: boolean) => void;

  isImportContainerModalOpen: boolean;
  setImportContainerModalOpen: (open: boolean) => void;

  isPropertiesModalOpen: boolean;
  setPropertiesModalOpen: (open: boolean) => void;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  propertiesModalItem: any;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  setPropertiesModalItem: (item: any) => void;

  isAboutModalOpen: boolean;
  setAboutModalOpen: (open: boolean) => void;

  isHelpModalOpen: boolean;
  setHelpModalOpen: (open: boolean) => void;

  spiderInitialUrl: string;
  spiderInitialExtensions: string;
  setSpiderInitial: (url: string, extensions: string) => void;

  smartLists: Array<{ id: string; name: string; query: string }>;
  addSmartList: (name: string, query: string) => void;
  removeSmartList: (id: string) => void;

  // Sidebar / Layout
  isSidebarOpen: boolean;
  setSidebarOpen: (open: boolean) => void;
}

export const useUiStore = create<UiStore>((set) => ({
  theme: 'system',
  setTheme: (theme) => set({ theme }),

  dir: (typeof localStorage !== 'undefined' ? localStorage.getItem('vajra-dir') as 'ltr' | 'rtl' : 'ltr') || 'ltr',
  setDir: (dir) => {
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('vajra-dir', dir);
    }
    document.documentElement.dir = dir;
    set({ dir });
  },

  spiderInitialUrl: '',
  spiderInitialExtensions: '',
  setSpiderInitial: (url, extensions) => set({ spiderInitialUrl: url, spiderInitialExtensions: extensions }),

  isAddModalOpen: false,
  setAddModalOpen: (open) => set({ isAddModalOpen: open }),

  isSettingsModalOpen: false,
  setSettingsModalOpen: (open) => set({ isSettingsModalOpen: open }),
  
  isStatsModalOpen: false,
  setStatsModalOpen: (open) => set({ isStatsModalOpen: open }),

  isDeleteModalOpen: false,
  setDeleteModalOpen: (open) => set({ isDeleteModalOpen: open }),

  isSchedulerModalOpen: false,
  setSchedulerModalOpen: (open) => set({ isSchedulerModalOpen: open }),

  isRefreshModalOpen: false,
  setRefreshModalOpen: (open) => set({ isRefreshModalOpen: open }),
  refreshModalItem: null,
  setRefreshModalItem: (item) => set({ refreshModalItem: item }),

  isGrabberModalOpen: false,
  setGrabberModalOpen: (open) => set({ isGrabberModalOpen: open }),

  isSpiderModalOpen: false,
  setSpiderModalOpen: (open) => set({ isSpiderModalOpen: open }),

  isImportContainerModalOpen: false,
  setImportContainerModalOpen: (open) => set({ isImportContainerModalOpen: open }),

  isPropertiesModalOpen: false,
  setPropertiesModalOpen: (open) => set({ isPropertiesModalOpen: open }),
  propertiesModalItem: null,
  setPropertiesModalItem: (item) => set({ propertiesModalItem: item }),

  isAboutModalOpen: false,
  setAboutModalOpen: (open) => set({ isAboutModalOpen: open }),

  isHelpModalOpen: false,
  setHelpModalOpen: (open) => set({ isHelpModalOpen: open }),

  isSidebarOpen: true,
  setSidebarOpen: (open) => set({ isSidebarOpen: open }),

  smartLists: JSON.parse(
    (typeof localStorage !== 'undefined' ? localStorage.getItem('vajra-smart-lists') : null) ||
    '[{"id":"large","name":"Large Files (>100MB)","query":"size:>104857600"},{"id":"github","name":"GitHub Sources","query":"url:github.com"},{"id":"fast","name":"Fast Downloads (>2MB/s)","query":"speed:>2097152"}]'
  ),
  addSmartList: (name, query) => set((state) => {
    const newList = [...state.smartLists, { id: Date.now().toString(), name, query }];
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('vajra-smart-lists', JSON.stringify(newList));
    }
    return { smartLists: newList };
  }),
  removeSmartList: (id) => set((state) => {
    const newList = state.smartLists.filter(x => x.id !== id);
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('vajra-smart-lists', JSON.stringify(newList));
    }
    return { smartLists: newList };
  }),
}));
