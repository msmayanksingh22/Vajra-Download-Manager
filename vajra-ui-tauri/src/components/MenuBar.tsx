import React, { useState, useRef, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTheme } from '../ThemeContext';
import { MoonStar, SunMedium, MonitorSmartphone } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useDownloadStore } from '../stores/downloadStore';
import { exportDownloadsJson, exportDownloadsCsv, importDownloads } from '../importExport';
import { toast } from 'sonner';

export default function MenuBar({
  onAdd,
  onGrabber,
  onOptions,
  onPauseAll,
  onResumeAll,
  onHelp,
  onAbout,
  onSpider,
  onScheduler,
  onBatchRename,
  onClearCompleted,
}: {
  onAdd: () => void;
  onGrabber: () => void;
  onOptions: () => void;
  onPauseAll: () => void;
  onResumeAll: () => void;
  onHelp: () => void;
  onAbout: () => void;
  onSpider: () => void;
  onScheduler: () => void;
  onBatchRename: () => void;
  onClearCompleted: () => void;
}) {
  const { t } = useTranslation();
  const [activeMenu, setActiveMenu] = useState<string | null>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const { themePref, setThemePref } = useTheme();

  const handleImport = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    try {
      await importDownloads(file);
      toast.success('Downloads imported successfully');
    } catch (err) {
      console.error(err);
      toast.error('Import failed. Check the file format.');
    }
    if (fileInputRef.current) fileInputRef.current.value = '';
  };

  const handleThemeToggle = () => {
    if (themePref === 'system') setThemePref('light');
    else if (themePref === 'light') setThemePref('dark');
    else setThemePref('system');
  };

  const handleMenuClick = (name: string) => setActiveMenu(activeMenu === name ? null : name);

  const handleAction = (action: string) => {
    setActiveMenu(null);
    if (action === 'add') onAdd();
    else if (action === 'batch') onGrabber();
    else if (action === 'spider') onSpider();
    else if (action === 'options') onOptions();
    else if (action === 'pauseAll') onPauseAll();
    else if (action === 'resumeAll') onResumeAll();
    else if (action === 'scheduler') onScheduler();
    else if (action === 'rename') onBatchRename();
    else if (action === 'clear') onClearCompleted();
    else if (action === 'exportJson') {
      const downloads = useDownloadStore.getState().getSortedAndFilteredDownloads();
      exportDownloadsJson(downloads);
      toast.success('Exported as JSON');
    } else if (action === 'exportCsv') {
      const downloads = useDownloadStore.getState().getSortedAndFilteredDownloads();
      exportDownloadsCsv(downloads);
      toast.success('Exported as CSV');
    } else if (action === 'import') {
      fileInputRef.current?.click();
    } else if (action === 'exit') invoke('cmd_quit_app').catch(console.error);
    else if (action === 'help') onHelp();
    else if (action === 'about') onAbout();
  };

  const menuBtnClass = (name: string) => `menu-bar-btn${activeMenu === name ? ' active' : ''}`;

  const dropdownStyle: React.CSSProperties = {
    position: 'absolute',
    top: 'calc(100% + 4px)',
    left: 0,
    minWidth: '210px',
    padding: '4px',
    backgroundColor: 'var(--color-surface)',
    border: '1px solid var(--color-border)',
    borderRadius: 'var(--radius-lg)',
    boxShadow: 'var(--shadow-lg)',
    zIndex: 50,
    animation: 'fadeInScale var(--transition-fast) ease',
  };

  // Use a ref to track if we're processing an item click — avoids race
  // with handleClickOutside when the dropdown unmounts on action.
  const isProcessingRef = useRef(false);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (isProcessingRef.current) return;
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setActiveMenu(null);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const MenuItem = ({
    label,
    action,
    shortcut,
  }: {
    label: string;
    action: string;
    shortcut?: string;
  }) => (
    <button
      className="menu-dropdown-item"
      onMouseDown={(e) => {
        e.stopPropagation();
        e.preventDefault();
        handleAction(action);
      }}
    >
      <span>{label}</span>
      {shortcut && <span className="menu-dropdown-shortcut">{shortcut}</span>}
    </button>
  );

  const MenuDivider = () => <div className="menu-dropdown-divider" />;

  const themeIcon =
    themePref === 'dark' ? MoonStar : themePref === 'light' ? SunMedium : MonitorSmartphone;
  const ThemeIcon = themeIcon;
  const themeLabel = themePref === 'system' ? 'System' : themePref === 'light' ? 'Light' : 'Dark';

  return (
    <div ref={menuRef} className="menubar-root">
      {/* Left: Brand + Menu Items */}
      <div className="menubar-left">
        {/* Brand */}
        <div className="menubar-brand">
          <div className="menubar-logo">
            <img
              src="/logo.png"
              alt="Vajra"
              style={{ width: '100%', height: '100%', objectFit: 'contain', borderRadius: 3 }}
            />
          </div>
          <span className="menubar-brand-text">VAJRA</span>
          <span
            style={{
              fontSize: '8px',
              fontWeight: 800,
              backgroundColor: 'var(--color-brand)',
              color: '#fff',
              padding: '2px 4px',
              borderRadius: '4px',
              marginLeft: '4px',
              lineHeight: 1,
            }}
          >
            BETA
          </span>
        </div>

        {/* File menu */}
        <div className="menu-item-wrapper">
          <button className={menuBtnClass('File')} onClick={() => handleMenuClick('File')}>
            {t('File')}
          </button>
          {activeMenu === 'File' && (
            <div style={dropdownStyle}>
              <MenuItem label={t('Add New Download')} action="add" shortcut="Ctrl+N" />
              <MenuItem label={t('Add Batch Download')} action="batch" />
              <MenuItem label={t('Spider Download')} action="spider" />
              <MenuDivider />
              <MenuItem label={t('Import Downloads')} action="import" shortcut="Ctrl+I" />
              <MenuItem label={t('Export as JSON')} action="exportJson" />
              <MenuItem label={t('Export as CSV')} action="exportCsv" />
              <MenuDivider />
              <MenuItem label={t('Exit Application')} action="exit" shortcut="Alt+F4" />
              <input
                type="file"
                accept=".json,.csv"
                style={{ display: 'none' }}
                ref={fileInputRef}
                onChange={handleImport}
              />
            </div>
          )}
        </div>

        {/* Actions menu */}
        <div className="menu-item-wrapper">
          <button className={menuBtnClass('Actions')} onClick={() => handleMenuClick('Actions')}>
            {t('Actions')}
          </button>
          {activeMenu === 'Actions' && (
            <div style={dropdownStyle}>
              <MenuItem label={t('Pause All Active')} action="pauseAll" />
              <MenuItem label={t('Resume All Paused')} action="resumeAll" />
              <MenuDivider />
              <MenuItem label={t('Scheduler')} action="scheduler" />
              <MenuItem label={t('Batch Rename')} action="rename" />
              <MenuItem label={t('Clear Completed')} action="clear" />
              <MenuDivider />
              <MenuItem label={t('Settings & Options')} action="options" shortcut="Ctrl+," />
            </div>
          )}
        </div>

        {/* Help menu */}
        <div className="menu-item-wrapper">
          <button className={menuBtnClass('Help')} onClick={() => handleMenuClick('Help')}>
            {t('Help')}
          </button>
          {activeMenu === 'Help' && (
            <div style={dropdownStyle}>
              <MenuItem label={t('Help Documentation')} action="help" shortcut="F1" />
              <MenuItem label={t('About Vajra')} action="about" />
            </div>
          )}
        </div>
      </div>

      {/* Right: Theme Toggle */}
      <div className="menubar-right">
        <button
          onClick={handleThemeToggle}
          title={`Theme: ${themeLabel} — click to switch`}
          aria-label={`Theme: ${themeLabel}. Click to switch.`}
          className="theme-toggle-btn"
        >
          <ThemeIcon size={12} strokeWidth={2} />
          <span className="theme-toggle-label">{themeLabel}</span>
        </button>
      </div>
    </div>
  );
}
