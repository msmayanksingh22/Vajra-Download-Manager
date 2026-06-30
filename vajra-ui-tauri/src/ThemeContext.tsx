import React, { createContext, useContext, useState, useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';

export type ThemePreference = 'system' | 'light' | 'dark';
export type ComputedTheme = 'light' | 'dark';

interface ThemeContextType {
  themePref: ThemePreference;
  computedTheme: ComputedTheme;
  setThemePref: (pref: ThemePreference) => void;
}

// eslint-disable-next-line react-refresh/only-export-components
export const ThemeContext = createContext<ThemeContextType | null>(null);

function getSystemTheme(): ComputedTheme {
  if (typeof window !== 'undefined' && window.matchMedia) {
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }
  return 'dark'; // Fallback default
}

export const ThemeProvider = ({ children }: { children: React.ReactNode }) => {
  const [themePref, setThemePrefState] = useState<ThemePreference>(() => {
    const stored = localStorage.getItem('vajra-theme-pref') as ThemePreference;
    return stored || 'system';
  });

  const [computedTheme, setComputedTheme] = useState<ComputedTheme>(() => {
    return themePref === 'system' ? getSystemTheme() : themePref;
  });

  // Recompute actual theme whenever preference changes
  useEffect(() => {
    const actual = themePref === 'system' ? getSystemTheme() : themePref;
    setComputedTheme(actual);
  }, [themePref]);

  // Apply computed theme to DOM and Tauri Window
  useEffect(() => {
    const root = document.documentElement;
    // Temporarily disable transition to prevent flash on theme init
    root.style.setProperty('--transition-slow', '0ms');
    if (computedTheme === 'dark') {
      root.classList.add('dark');
      root.classList.remove('light');
      root.style.colorScheme = 'dark';
    } else {
      root.classList.add('light');
      root.classList.remove('dark');
      root.style.colorScheme = 'light';
    }
    // Sync to the key read by the inline script in index.html
    localStorage.setItem('vajra-theme', computedTheme);
    // Re-enable transitions after a brief paint tick
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        root.style.removeProperty('--transition-slow');
      });
    });

    // Attempt to set native window theme if API is available
    try {
      getCurrentWindow().setTheme(computedTheme);
    } catch (e) {
      console.warn("Failed to set native window theme", e);
    }
  }, [computedTheme]);

  // Listen to OS theme changes if 'system' is selected
  useEffect(() => {
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handleChange = (e: MediaQueryListEvent) => {
      if (themePref === 'system') {
        setComputedTheme(e.matches ? 'dark' : 'light');
      }
    };
    
    // Modern API
    if (mediaQuery.addEventListener) {
      mediaQuery.addEventListener('change', handleChange);
      return () => mediaQuery.removeEventListener('change', handleChange);
    } 
    // Fallback for older WebKit
    else if (mediaQuery.addListener) {
      mediaQuery.addListener(handleChange);
      return () => mediaQuery.removeListener(handleChange);
    }
  }, [themePref]);

  // Listen for storage events to sync themes across multiple WebviewWindows
  useEffect(() => {
    const handleStorage = (e: StorageEvent) => {
      if (e.key === 'vajra-theme-pref' && e.newValue) {
        setThemePrefState(e.newValue as ThemePreference);
      }
    };
    window.addEventListener('storage', handleStorage);
    return () => window.removeEventListener('storage', handleStorage);
  }, []);

  const setThemePref = (pref: ThemePreference) => {
    setThemePrefState(pref);
    localStorage.setItem('vajra-theme-pref', pref);
    // Note: storage events don't fire in the window that triggered them,
    // so we update our own state directly above.
  };

  return (
    <ThemeContext.Provider value={{ themePref, computedTheme, setThemePref }}>
      {children}
    </ThemeContext.Provider>
  );
};

// eslint-disable-next-line react-refresh/only-export-components
export const useTheme = () => {
  const context = useContext(ThemeContext);
  if (!context) throw new Error("useTheme must be used within a ThemeProvider");
  return context;
};
