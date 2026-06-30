import { useEffect } from 'react';

/**
 * Closes a dialog when the user presses Escape.
 * Drop this into any dialog component that has an onClose callback.
 */
export function useDialogEscape(onClose: () => void) {
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [onClose]);
}
