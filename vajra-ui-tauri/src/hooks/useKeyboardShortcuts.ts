import { useEffect } from 'react';
import { useDownloadStore } from '../stores/downloadStore';
import { useUiStore } from '../stores/uiStore';

export function useKeyboardShortcuts(
  searchInputRef: React.RefObject<HTMLInputElement>,
  spawnAddUrlWindow: () => void,
) {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Allow default behavior if user is typing in an input
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
        if (e.key === 'Escape') {
          (e.target as HTMLElement).blur();
        }
        return;
      }

      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') {
        e.preventDefault();
        searchInputRef.current?.focus();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'n') {
        e.preventDefault();
        spawnAddUrlWindow();
      } else if (e.key === 'Delete') {
        const store = useDownloadStore.getState();
        if (store.selectedIds.size > 0) {
          e.preventDefault();
          useUiStore.getState().setDeleteModalOpen(true);
        }
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a') {
        e.preventDefault();
        useDownloadStore.getState().selectAll();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [searchInputRef, spawnAddUrlWindow]);
}
