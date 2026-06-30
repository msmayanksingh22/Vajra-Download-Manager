import React, { useState, useEffect } from 'react';
import { 
  X, Settings, FileBox, Wifi, HardDriveDownload, 
  Network, Lock, Phone, MonitorPlay, FileCheck2, 
  Globe, Fingerprint, Activity, Server, AlertCircle, 
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  Check, Save, Trash2, FolderOpen
} from 'lucide-react';
import { cn } from '../../utils';
import { api } from '../../api';
import { open } from '@tauri-apps/plugin-dialog';
import { useTheme, ThemePreference } from '../../ThemeContext';
import { useUiStore } from '../../stores/uiStore';
import { useDialogEscape } from '../../hooks/useDialogEscape';
import { useFocusTrap } from '../../hooks/useFocusTrap';

export default function OptionsDialog({ onClose }: { onClose: () => void }) {
  const { dir, setDir } = useUiStore();
  useDialogEscape(onClose);
  const trapRef = useFocusTrap();
  const [activeTab, setActiveTab] = useState('General');
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [config, setConfig] = useState<any>(null);
  const [deletePreference, setDeletePreference] = useState(() => localStorage.getItem('vajra_delete_preference'));
  const [isSaving, setIsSaving] = useState(false);
  
  const { themePref, setThemePref } = useTheme();

  // Site Logins
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [logins, setLogins] = useState<any[]>([]);

  const [newLoginHost, setNewLoginHost] = useState('');
  const [newLoginUser, setNewLoginUser] = useState('');
  const [newLoginPass, setNewLoginPass] = useState('');




  // Dialup Settings
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [dialup, setDialup] = useState<any>(() => {
    try {
      const saved = localStorage.getItem('vajra_dialup_settings');
      if (saved) return JSON.parse(saved);
    } catch(e) { /* ignore */ }
    return { connectionName: '', username: '', password: '', redialCount: 5 };
  });

  // Sound Config
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [soundConfig, setSoundConfig] = useState<any>(() => {
    try {
      const saved = localStorage.getItem('vajra_sounds_config');
      if (saved) return JSON.parse(saved);
    } catch(e) { /* ignore */ }
    return { onComplete: true, onFail: false, onQueueStart: false };
  });

  useEffect(() => {
    const loadConfig = () => api.config().then(setConfig).catch(e => { console.error("Config load error:", e); setConfig({ _error: e.toString() }); });
    loadConfig();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (window as any)._retryConfig = loadConfig;
    api.getVault().then(setLogins).catch(console.error);
  }, []);

  useEffect(() => {
    localStorage.setItem('vajra_dialup_settings', JSON.stringify(dialup));
  }, [dialup]);

  useEffect(() => {
    localStorage.setItem('vajra_sounds_config', JSON.stringify(soundConfig));
  }, [soundConfig]);

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const updateConfig = (updates: any) => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    setConfig((prev: any) => {
      const next = { ...prev, ...updates };
      setIsSaving(true);
      api.setConfig(next)
        .then(() => setTimeout(() => setIsSaving(false), 500))
        .catch(console.error);
      return next;
    });
  };

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const updateProxy = (updates: any) => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    setConfig((prev: any) => {
      const next = { ...prev, proxy: { ...prev.proxy, ...updates } };
      setIsSaving(true);
      api.setConfig(next)
        .then(() => setTimeout(() => setIsSaving(false), 500))
        .catch(console.error);
      return next;
    });
  };

  const Toggle = ({ active, onClick, label }: { active: boolean; onClick: () => void; label?: string }) => (
    <label className="flex items-center justify-between" style={{ cursor: 'default' }}>
      {label && <span style={{ fontSize: 'var(--text-sm-size)', fontWeight: 500, color: 'var(--color-text-1)' }}>{label}</span>}
      <button
        type="button"
        role="switch"
        aria-checked={active}
        onClick={onClick}
        aria-label={label || 'Toggle'}
        style={{
          position: 'relative', display: 'inline-flex', height: 20, width: 36,
          borderRadius: 10, transition: 'background-color var(--transition-normal)',
          backgroundColor: active ? 'var(--color-brand)' : 'var(--color-border)',
          cursor: 'default', flexShrink: 0, padding: 0, border: 'none',
        }}
      >
        <span style={{
          position: 'absolute', top: 2, left: 2, width: 16, height: 16,
          borderRadius: '50%', backgroundColor: 'var(--color-surface)',
          boxShadow: 'var(--shadow-sm)', transition: 'transform var(--transition-normal)',
          transform: active ? 'translateX(16px)' : 'translateX(0)',
          pointerEvents: 'none',
        }} />
      </button>
    </label>
  );

  const tabs = [
    { id: 'General', icon: Settings, desc: 'Startup, browser & dialogs' },
    { id: 'File Types', icon: FileBox, desc: 'Auto-downloads & blacklists' },
    { id: 'Connection', icon: Wifi, desc: 'Limits & speed tuning' },
    { id: 'Downloads', icon: HardDriveDownload, desc: 'Directories & actions' },
    { id: 'Categories', icon: FolderOpen, desc: 'Sidebar categories & auto-routing' },
    { id: 'Automation & Security', icon: Activity, desc: 'Scheduling & AV' },
    { id: 'Proxy / Socks', icon: Network, desc: 'Proxy configuration' },
    { id: 'Site Logins', icon: Lock, desc: 'Saved credentials' },
    { id: 'Dial-up', icon: Phone, desc: 'VPN & Dial-up settings' },
  ];

  const handleBrowseFolder = async (field: string) => {
    try {
      const selected = await open({ directory: true });
      if (selected) {
        updateConfig({ [field]: selected });
      }
    } catch (err) {
      console.error("Folder picker not available", err);
    }
  };

  const handleBrowseFile = async (field: string) => {
    try {
      const selected = await open({ directory: false });
      if (selected) {
        updateConfig({ [field]: selected });
      }
    } catch (err) {
      console.error("File picker not available", err);
    }
  };

  const addLogin = async () => {
    if (newLoginHost.trim() && newLoginUser.trim() && newLoginPass.trim()) {
      try {
        const added = await api.addVault({
          domain: newLoginHost.trim(),
          username: newLoginUser.trim(),
          password: newLoginPass.trim()
        });
        setLogins(prev => [...prev, added].sort((a, b) => a.domain.localeCompare(b.domain)));
        setNewLoginHost('');
        setNewLoginUser('');
        setNewLoginPass('');
      } catch (err) {
        console.error("Failed to add site login", err);
      }
    }
  };

  const removeLogin = async (id: string) => {
    try {
      await api.deleteVault(id);
      setLogins(prev => prev.filter(l => l.id !== id));
    } catch (err) {
      console.error("Failed to remove site login", err);
    }
  };

  // Modern Input Component
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const ConfigInput = ({ label, value, onChange, placeholder, type = "text", icon: Icon }: any) => (
    <div className="flex flex-col gap-1.5 w-full">
      <label className="text-xs font-semibold text-2 uppercase tracking-wider">{label}</label>
      <div className="relative">
        {Icon && <Icon className="absolute left-3 top-1/2 -translate-y-1/2 text-3" size={14} />}
        <input 
          type={type}
          placeholder={placeholder}
          className={cn(
            "w-full bg-surface-elevated border border-muted rounded-lg px-3 py-2 text-sm text-1",
            "focus:outline-none focus:ring-2 focus:ring-brand focus:border-brand transition-all placeholder:text-4",
            Icon ? "pl-9" : ""
          )}
          value={value}
          onChange={onChange}
        />
      </div>
    </div>
  );

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div ref={trapRef} className="dialog-panel" style={{ width: 860, height: 580, maxWidth: '92vw', maxHeight: '90vh', flexDirection: 'column' }} onClick={e => e.stopPropagation()}
        role="dialog" aria-modal="true" aria-labelledby="options-dialog-title"
      >
        
        {/* Header */}
        <div className="dialog-header" style={{ padding: '12px 20px' }}>
          <div className="dialog-header-title" style={{ gap: 10 }}>
            <div style={{ backgroundColor: 'var(--color-brand-dim)', padding: 8, borderRadius: 'var(--radius-lg)', color: 'var(--color-brand)', display: 'flex' }}>
              <Settings size={18} />
            </div>
            <div>
              <div id="options-dialog-title" style={{ fontWeight: 700, fontSize: 'var(--text-base-size)', color: 'var(--color-text-1)' }}>Settings</div>
              <div style={{ fontSize: 'var(--text-xs-size)', color: 'var(--color-text-3)', marginTop: 2 }}>Configure Vajra's core behavior</div>
            </div>
          </div>
          <div className="flex items-center gap-3">
            {isSaving && (
              <span style={{ fontSize: 'var(--text-xs-size)', color: 'var(--color-brand)', display: 'flex', alignItems: 'center', gap: 4 }}>
                <Activity size={12} className="animate-pulse" /> Savingâ€¦

              </span>
            )}
            <button className="btn-icon" onClick={onClose} title="Close"><X size={16} /></button>
          </div>
        </div>
        
        <div className="flex flex-1 overflow-hidden">
          {/* Sidebar nav */}
          <div role="tablist" aria-label="Settings categories" style={{ width: 220, flexShrink: 0, borderRight: '1px solid var(--color-border)', backgroundColor: 'var(--color-sidebar)', display: 'flex', flexDirection: 'column', padding: '8px 6px', gap: 2, overflowY: 'auto' }}>
            {tabs.map(t => {
              const isActive = activeTab === t.id;
              return (
                <button
                  key={t.id}
                  type="button"
                  role="tab"
                  aria-selected={isActive}
                  onClick={() => setActiveTab(t.id)}
                  style={{
                    display: 'flex', alignItems: 'flex-start', gap: 10,
                    width: '100%', textAlign: 'left', padding: '8px 10px',
                    borderRadius: 'var(--radius-lg)', border: 'none', cursor: 'default',
                    backgroundColor: isActive ? 'var(--color-brand-dim)' : 'transparent',
                    position: 'relative', transition: 'background-color var(--transition-fast)',
                    fontFamily: 'var(--font-sans)',
                  }}
                  onMouseEnter={e => { if (!isActive) (e.currentTarget as HTMLElement).style.backgroundColor = 'var(--color-surface-raised)'; }}
                  onMouseLeave={e => { if (!isActive) (e.currentTarget as HTMLElement).style.backgroundColor = 'transparent'; }}
                >
                  {isActive && (
                    <div style={{ position: 'absolute', left: 0, top: '50%', transform: 'translateY(-50%)', width: 3, height: 22, backgroundColor: 'var(--color-brand)', borderRadius: '0 2px 2px 0' }} />
                  )}
                  <t.icon size={16} style={{ marginTop: 2, color: isActive ? 'var(--color-brand)' : 'var(--color-text-3)', flexShrink: 0 }} />
                  <div>
                    <div style={{ fontWeight: 600, fontSize: 'var(--text-sm-size)', color: isActive ? 'var(--color-brand)' : 'var(--color-text-1)' }}>{t.id}</div>
                    <div style={{ fontSize: 'var(--text-xs-size)', color: 'var(--color-text-4)', marginTop: 1 }}>{t.desc}</div>
                  </div>
                </button>
              );
            })}
          </div>

          {/* Main Content Area */}
          <div className="flex-1 overflow-y-auto relative" style={{ padding: 'var(--sp-6)', backgroundColor: 'var(--color-surface)' }}>
            {!config ? (
              <div className="absolute inset-0 flex flex-col items-center justify-center text-3 gap-4">
                <Activity className="animate-spin text-brand" size={32} />
                <span className="font-medium text-sm">Loading Configuration...</span>
              </div>
            ) : config._error ? (
              <div className="absolute inset-0 flex flex-col items-center justify-center gap-5" style={{ padding: 32, textAlign: 'center' }}>
                <div style={{ width: 56, height: 56, borderRadius: '50%', backgroundColor: 'var(--color-error-dim)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                  <AlertCircle size={28} style={{ color: 'var(--color-error)' }} />
                </div>
                <div>
                  <div style={{ fontWeight: 700, fontSize: 'var(--text-md-size)', color: 'var(--color-text-1)', marginBottom: 6 }}>Vajra Daemon is Offline</div>
                  <div style={{ fontSize: 'var(--text-sm-size)', color: 'var(--color-text-3)', maxWidth: 280, lineHeight: 1.6 }}>
                    Settings cannot be loaded because the backend is not running.<br/>
                    Start <strong style={{ color: 'var(--color-text-2)' }}>dev.bat</strong> and wait for the daemon to start, then retry.
                  </div>
                  <div style={{ marginTop: 8, fontSize: 'var(--text-xs-size)', color: 'var(--color-text-4)', fontFamily: 'var(--font-mono)' }}>{config._error}</div>
                </div>
                {/* eslint-disable-next-line @typescript-eslint/no-explicit-any */}
                <button className="btn-primary px-6 py-2" onClick={() => { setConfig(null); (window as any)._retryConfig?.(); }}>
                  Retry Connection
                </button>
              </div>

            ) : (
              <div className="max-w-[500px] mx-auto animate-in slide-in-from-bottom-4 fade-in duration-300">
                
                {/* --- GENERAL TAB --- */}
                {activeTab === 'General' && (
                  <div className="flex flex-col gap-8">
                    <section>
                      <h3 className="text-sm font-bold text-1 mb-4 flex items-center gap-2">
                        <MonitorPlay size={16} style={{ color: "var(--color-brand)" }}/> Appearance & Theme
                      </h3>
                      <div className="bg-surface border border-muted rounded-2xl p-5 shadow-sm flex flex-col gap-6">
                        <div className="flex items-center justify-between">
                          <div className="pr-4">
                            <div className="font-semibold text-sm text-1">Theme Mode</div>
                            <div className="text-xs text-3 mt-1">Select your preferred color scheme for Vajra.</div>
                          </div>
                          <div className="flex items-center bg-surface-raised rounded-lg p-1">
                            {(['system', 'light', 'dark'] as ThemePreference[]).map((mode) => (
                              <button
                                key={mode}
                                onClick={() => setThemePref(mode)}
                                className={cn(
                                  "px-3 py-1.5 rounded-md text-xs font-medium capitalize transition-colors duration-200",
                                  themePref === mode ? "bg-brand text-white shadow-sm" : "text-3 hover:text-1"
                                )}
                              >
                                {mode}
                              </button>
                            ))}
                          </div>
                        </div>
                        <div className="h-px bg-surface-raised w-full" />
                        <div className="flex items-center justify-between">
                          <div className="pr-4">
                            <div className="font-semibold text-sm text-1">Text Direction (RTL)</div>
                            <div className="text-xs text-3 mt-1">Configure layout direction for RTL languages like Arabic and Hebrew.</div>
                          </div>
                          <div className="flex items-center bg-surface-raised rounded-lg p-1">
                            {([
                              { id: 'ltr', label: 'LTR' },
                              { id: 'rtl', label: 'RTL' }
                            ] as const).map((dirItem) => (
                              <button
                                key={dirItem.id}
                                onClick={() => setDir(dirItem.id)}
                                className={cn(
                                  "px-3 py-1.5 rounded-md text-xs font-medium transition-colors duration-200",
                                  dir === dirItem.id ? "bg-brand text-white shadow-sm" : "text-3 hover:text-1"
                                )}
                              >
                                {dirItem.id.toUpperCase()}
                              </button>
                            ))}
                          </div>
                        </div>
                      </div>
                    </section>

                    <section>
                      <h3 className="text-sm font-bold text-1 mb-4 flex items-center gap-2">
                        <MonitorPlay size={16} style={{ color: "var(--color-brand)" }}/> System Integration
                      </h3>
                      <div className="bg-surface border border-muted rounded-2xl p-5 shadow-sm flex flex-col gap-6">
                        <div className="flex items-center justify-between">
                          <div className="pr-4">
                            <div className="font-semibold text-sm text-1">Launch on Startup</div>
                            <div className="text-xs text-3 mt-1">Automatically start Vajra in the background when Windows boots up.</div>
                          </div>
                          <Toggle active={config.auto_start_on_login} onClick={() => updateConfig({ auto_start_on_login: !config.auto_start_on_login })} />
                        </div>
                        <div className="h-px bg-surface-raised w-full" />
                        <div className="flex items-center justify-between">
                          <div className="pr-4">
                            <div className="font-semibold text-sm text-1">Browser Extension Integration</div>
                            <div className="text-xs text-3 mt-1">Captures downloads directly from Chrome, Edge, and Firefox.</div>
                          </div>
                          <button 
                            className="btn-ghost text-xs px-4 py-2 rounded-lg" 
                            onClick={() => api.openBrowserSetup()}
                          >
                            Configure
                          </button>
                        </div>
                        <div className="h-px bg-surface-raised w-full" />
                        <div className="flex items-center justify-between">
                          <div className="pr-4">
                            <div className="font-semibold text-sm text-1">Clipboard Monitoring</div>
                            <div className="text-xs text-3 mt-1">Automatically detect and capture copied URLs.</div>
                          </div>
                          <Toggle active={config.enable_clipboard_monitor !== false} onClick={() => updateConfig({ enable_clipboard_monitor: config.enable_clipboard_monitor === false ? true : false })} />
                        </div>
                      </div>
                    </section>

                    <section>
                      <h3 className="text-sm font-bold text-1 mb-4 flex items-center gap-2">
                        <AlertCircle size={16} style={{ color: "var(--color-brand)" }}/> Confirmation Dialogs
                      </h3>
                      <div className="bg-surface border border-muted rounded-2xl p-5 shadow-sm flex flex-col gap-6">
                        <div className="flex items-center justify-between">
                          <div className="pr-4">
                            <div className="font-semibold text-sm text-1">Confirm before deleting</div>
                            <div className="text-xs text-3 mt-1">Show a safety prompt when deleting files or clearing history.</div>
                          </div>
                          <Toggle active={!deletePreference} onClick={() => {
                            if (deletePreference) {
                              localStorage.removeItem('vajra_delete_preference');
                              setDeletePreference(null);
                            } else {
                              localStorage.setItem('vajra_delete_preference', 'list_only');
                              setDeletePreference('list_only');
                            }
                          }} />
                        </div>
                      </div>
                    </section>

                    <section>
                      <h3 className="text-sm font-bold text-1 mb-4 flex items-center gap-2">
                        <Activity size={16} style={{ color: "var(--color-brand)" }}/> Interception Hotkeys
                      </h3>
                      <div className="bg-surface border border-muted rounded-2xl p-5 shadow-sm flex flex-col gap-4">
                        <div className="flex items-center justify-between bg-surface-raised p-3 rounded-xl border border-muted">
                          <span className="text-sm text-2">Hold <kbd className=" bg-surface-elevated border border-muted px-2 py-0.5 rounded shadow-sm font-bold text-1 mx-1">Alt</kbd> during click</span>
                          <span className="text-brand font-bold text-xs uppercase tracking-wide bg-brand-dim px-2 py-1 rounded-md">Force Intercept</span>
                        </div>
                        <div className="flex items-center justify-between bg-surface-raised p-3 rounded-xl border border-muted">
                          <span className="text-sm text-2">Hold <kbd className=" bg-surface-elevated border border-muted px-2 py-0.5 rounded shadow-sm font-bold text-1 mx-1">Ins</kbd> during click</span>
                          <span className="text-warning font-bold text-xs uppercase tracking-wide bg-warning-dim px-2 py-1 rounded-md">Bypass Vajra</span>
                        </div>
                      </div>
                    </section>
                  </div>
                )}

                {/* --- FILE TYPES TAB --- */}
                {activeTab === 'File Types' && (
                  <div className="flex flex-col gap-8">
                    <section>
                      <h3 className="text-sm font-bold text-1 mb-4 flex items-center gap-2">
                        <FileCheck2 size={16} style={{ color: "var(--color-brand)" }}/> Interception Rules
                      </h3>
                      <div className="bg-surface border border-muted rounded-2xl p-5 shadow-sm flex flex-col gap-6">
                        <div className="flex flex-col gap-2">
                          <label className="text-sm font-semibold text-1" id="label-extensions">Auto-download Extensions</label>
                          <p className="text-xs text-3" id="desc-extensions">Automatically grab these file types from the browser.</p>
                          <textarea
                            aria-labelledby="label-extensions"
                            aria-describedby="desc-extensions"
                            className="w-full bg-surface-elevated border border-muted rounded-xl p-3 text-sm  text-brand focus:outline-none focus:ring-2 focus:ring-brand transition-all resize-none mt-2"
                            rows={3}
                            // eslint-disable-next-line @typescript-eslint/no-explicit-any
                            value={config.category_rules ? config.category_rules.map((r: any) => r.extensions.join(' ')).join(' ') : 'ZIP RAR EXE MP3 MP4 PDF MSI DMG'}
                            onChange={(e) => {
                              const exts = e.target.value.toLowerCase().split(/\s+/).filter(Boolean);
                              updateConfig({
                                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                                category_rules: config.category_rules?.map((rule: any, idx: number) => idx === 0 ? { ...rule, extensions: exts } : rule)
                              });
                            }}
                          />
                        </div>
                        
                        <div className="h-px bg-surface-raised w-full" />

                        <div className="flex flex-col gap-2">
                          <label className="text-sm font-semibold text-1" id="label-blacklist">Domain Blacklist</label>
                          <p className="text-xs text-3" id="desc-blacklist">Never intercept downloads originating from these domains.</p>
                          <textarea
                            aria-labelledby="label-blacklist"
                            aria-describedby="desc-blacklist"
                            placeholder="e.g. google.com, internal-site.net"
                            className="w-full bg-surface-elevated border border-muted rounded-xl p-3 text-sm  text-warning focus:outline-none focus:ring-2 focus:ring-brand transition-all resize-none mt-2"
                            rows={3}
                            value={config.blacklist_domains ? config.blacklist_domains.join(', ') : ''}
                            onChange={(e) => {
                              const domains = e.target.value.split(',').map(d => d.trim()).filter(Boolean);
                              updateConfig({ blacklist_domains: domains });
                            }}
                          />
                        </div>
                      </div>
                    </section>
                  </div>
                )}

                {/* --- CONNECTION TAB --- */}
                {activeTab === 'Connection' && (
                  <div className="flex flex-col gap-8">
                    <section>
                      <h3 className="text-sm font-bold text-1 mb-4 flex items-center gap-2">
                        <Server size={16} style={{ color: "var(--color-brand)" }}/> Performance Tuning
                      </h3>
                      <div className="bg-surface border border-muted rounded-2xl p-5 shadow-sm flex flex-col gap-6">
                        <div className="flex flex-col gap-2">
                          <label className="text-sm font-semibold text-1">Default Max. Connections</label>
                          <p className="text-xs text-3 mb-2">Number of parallel segments to open per download by default.</p>
                          <select 
                            className="select-field w-full"
                            value={config.default_max_connections}
                            onChange={(e) => updateConfig({ default_max_connections: parseInt(e.target.value, 10) })}
                          >
                            <option value="1">1 Connection (Slowest, safest)</option>
                            <option value="4">4 Connections</option>
                            <option value="8">8 Connections (Recommended)</option>
                            <option value="16">16 Connections</option>
                            <option value="24">24 Connections</option>
                            <option value="32">32 Connections (Aggressive)</option>
                          </select>
                        </div>

                        <div className="flex flex-col gap-2">
                          <label className="text-sm font-semibold text-1">Auto-Retry Attempts</label>
                          <p className="text-xs text-3 mb-2">Number of automatic download attempts on failure before displaying failure window.</p>
                          <select 
                            className="select-field w-full"
                            value={config.max_retries ?? 2}
                            onChange={(e) => updateConfig({ max_retries: parseInt(e.target.value, 10) })}
                          >
                            <option value="0">Disabled (No retries)</option>
                            <option value="1">1 Retry Attempt</option>
                            <option value="2">2 Retry Attempts (Default)</option>
                            <option value="3">3 Retry Attempts</option>
                            <option value="5">5 Retry Attempts</option>
                            <option value="10">10 Retry Attempts</option>
                          </select>
                        </div>


                        <div className="h-px bg-surface-raised w-full" />
                        <div className="flex items-center justify-between">
                          <div className="pr-4">
                            <div className="font-semibold text-sm text-1">HTTP/3 (QUIC) Prior Knowledge</div>
                            <div className="text-xs text-3 mt-1">Enable QUIC protocols by default for supported servers. Falling back to HTTP/2 on fail.</div>
                          </div>
                          <Toggle active={config.default_use_http3 || false} onClick={() => updateConfig({ default_use_http3: !config.default_use_http3 })} />
                        </div>
                      </div>
                    </section>


                  </div>
                )}

                {/* --- DOWNLOADS TAB --- */}
                {activeTab === 'Downloads' && (
                  <div className="flex flex-col gap-8">
                    <section>
                      <h3 className="text-sm font-bold text-1 mb-4 flex items-center gap-2">
                        <FolderOpen size={16} style={{ color: "var(--color-brand)" }}/> Directory Management
                      </h3>
                      <div className="bg-surface border border-muted rounded-2xl p-5 shadow-sm flex flex-col gap-6">
                        <div className="flex flex-col gap-2">
                          <label className="text-sm font-semibold text-1">Default Output Folder</label>
                          <div className="flex gap-2">
                            <input 
                              type="text" 
                              className="flex-1 bg-surface-elevated border border-muted rounded-lg px-3 py-2 text-sm text-1 focus:outline-none focus:ring-2 focus:ring-brand transition-all" 
                              value={config.default_output_dir} 
                              onChange={(e) => updateConfig({ default_output_dir: e.target.value })}
                            />
                            <button 
                              className="btn-secondary px-4 rounded-lg text-sm font-semibold"
                              onClick={() => handleBrowseFolder('default_output_dir')}
                            >
                              Browse
                            </button>
                          </div>
                        </div>

                        <div className="flex flex-col gap-2">
                          <label className="text-sm font-semibold text-1">Temporary Files Folder</label>
                          <div className="flex gap-2">
                            <input 
                              type="text" 
                              placeholder="Leave empty to use output folder"
                              className="flex-1 bg-surface-elevated border border-muted rounded-lg px-3 py-2 text-sm text-1 focus:outline-none focus:ring-2 focus:ring-brand transition-all placeholder:text-4" 
                              value={config.temp_dir || ''} 
                              onChange={(e) => updateConfig({ temp_dir: e.target.value || null })}
                            />
                            <button 
                              className="btn-secondary px-4 rounded-lg text-sm font-semibold"
                              onClick={() => handleBrowseFolder('temp_dir')}
                            >
                              Browse
                            </button>
                          </div>
                        </div>
                      </div>
                    </section>

                    <section>
                      <h3 className="text-sm font-bold text-1 mb-4 flex items-center gap-2">
                        <Activity size={16} style={{ color: "var(--color-brand)" }}/> Advanced Behaviors
                      </h3>
                      <div className="bg-surface border border-muted rounded-2xl p-5 shadow-sm flex flex-col gap-6">
                        <div className="flex flex-col gap-2">
                          <label className="text-sm font-semibold text-1">Duplicate Filename Strategy</label>
                          <select 
                            className="select-field w-full"
                            value={config.duplicate_action}
                            onChange={(e) => updateConfig({ duplicate_action: e.target.value })}
                          >
                            <option value="auto_rename">Auto-Rename (e.g. file(1).zip)</option>
                            <option value="overwrite">Overwrite existing file</option>
                            <option value="prompt">Prompt me every time</option>
                          </select>
                        </div>
                        
                        <div className="h-px bg-surface-raised w-full" />

                        <div className="flex items-center justify-between">
                          <div className="pr-4">
                            <div className="font-semibold text-sm text-1">Audio Alerts</div>
                            <div className="text-xs text-3 mt-1">Play sounds on completion or error.</div>
                          </div>
                          {/* eslint-disable-next-line @typescript-eslint/no-explicit-any */}
                          <Toggle active={soundConfig.onComplete} onClick={() => setSoundConfig((prev:any) => ({ ...prev, onComplete: !prev.onComplete }))} />
                        </div>
                      </div>
                    </section>
                  </div>
                )}

                {/* --- CATEGORIES TAB --- */}
                {activeTab === 'Categories' && (
                  <div className="flex flex-col gap-8">
                    <section>
                      <h3 className="text-sm font-bold text-1 mb-4 flex items-center gap-2">
                        <FolderOpen size={16} style={{ color: "var(--color-brand)" }}/> Custom Categories
                      </h3>
                      <div className="bg-surface border border-muted rounded-2xl p-5 shadow-sm flex flex-col gap-6">
                        <div className="text-sm text-3 mb-2">
                          Categories automatically route downloads to specific folders based on file extensions. They also appear in the sidebar.
                        </div>
                        
                        {config.category_rules?.map((rule: any, i: number) => (
                          <div key={i} className="flex flex-col gap-3 p-4 rounded-xl border border-muted bg-surface-raised relative">
                            <button 
                              className="absolute top-3 right-3 text-4 hover:text-error transition-colors"
                              onClick={() => {
                                const newRules = [...config.category_rules];
                                newRules.splice(i, 1);
                                updateConfig({ category_rules: newRules });
                              }}
                            >
                              <Trash2 size={16} />
                            </button>
                            
                            <div className="flex gap-4 items-center">
                              <label className="text-xs font-semibold text-2 w-20">Name</label>
                              <input 
                                className="flex-1 bg-surface-elevated border border-muted rounded-md px-2 py-1 text-sm text-1 focus:border-brand outline-none"
                                value={rule.label}
                                onChange={e => {
                                  const newRules = [...config.category_rules];
                                  newRules[i] = { ...rule, label: e.target.value };
                                  updateConfig({ category_rules: newRules });
                                }}
                              />
                            </div>
                            
                            <div className="flex gap-4 items-center">
                              <label className="text-xs font-semibold text-2 w-20">Extensions</label>
                              <input 
                                className="flex-1 bg-surface-elevated border border-muted rounded-md px-2 py-1 text-sm text-1 focus:border-brand outline-none"
                                value={rule.extensions.join(', ')}
                                placeholder="mp4, mkv, avi"
                                onChange={e => {
                                  const newRules = [...config.category_rules];
                                  newRules[i] = { ...rule, extensions: e.target.value.split(',').map(s => s.trim().replace(/^\./, '')).filter(Boolean) };
                                  updateConfig({ category_rules: newRules });
                                }}
                              />
                            </div>

                            <div className="flex gap-4 items-center">
                              <label className="text-xs font-semibold text-2 w-20">Save To</label>
                              <div className="flex flex-1 gap-2">
                                <input 
                                  className="flex-1 bg-surface-elevated border border-muted rounded-md px-2 py-1 text-sm text-1 focus:border-brand outline-none"
                                  value={rule.output_dir}
                                  onChange={e => {
                                    const newRules = [...config.category_rules];
                                    newRules[i] = { ...rule, output_dir: e.target.value };
                                    updateConfig({ category_rules: newRules });
                                  }}
                                />
                                <button 
                                  className="btn-secondary px-3 py-1 rounded-md text-xs font-semibold"
                                  onClick={async () => {
                                    try {
                                      const selected = await open({ directory: true });
                                      if (selected) {
                                        const newRules = [...config.category_rules];
                                        newRules[i] = { ...rule, output_dir: selected };
                                        updateConfig({ category_rules: newRules });
                                      }
                                    } catch (err) { console.error("Folder picker not available", err); }
                                  }}
                                >
                                  Browse
                                </button>
                              </div>
                            </div>
                          </div>
                        ))}

                        <button 
                          className="btn-secondary py-2 rounded-lg font-semibold w-full mt-2"
                          onClick={() => {
                            const newRules = [...(config.category_rules || []), {
                              label: 'New Category',
                              extensions: [],
                              output_dir: config.default_output_dir || ''
                            }];
                            updateConfig({ category_rules: newRules });
                          }}
                        >
                          + Add Category
                        </button>
                      </div>
                    </section>
                  </div>
                )}

                {/* --- AUTOMATION & SECURITY TAB --- */}
                {activeTab === 'Automation & Security' && (
                  <div className="flex flex-col gap-8">
                    <section>
                      <h3 className="text-sm font-bold text-1 mb-4 flex items-center gap-2">
                        <Activity size={16} style={{ color: "var(--color-brand)" }}/> Global Scheduler
                      </h3>
                      <div className="bg-surface border border-muted rounded-2xl p-5 shadow-sm flex flex-col gap-6">
                        <div className="flex items-center justify-between">
                          <div className="pr-4">
                            <div className="font-semibold text-sm text-1">Enable Queue Scheduler</div>
                            <div className="text-xs text-3 mt-1">Automatically pause and resume the download queue based on time of day.</div>
                          </div>
                          <Toggle active={config.scheduler_enabled || false} onClick={() => updateConfig({ scheduler_enabled: !config.scheduler_enabled })} />
                        </div>
                        
                        <div className={cn("transition-all duration-300 overflow-hidden flex gap-4", config.scheduler_enabled ? "opacity-100 max-h-[100px]" : "opacity-50 pointer-events-none max-h-[100px]")}>
                          <div className="flex-1 flex flex-col gap-2">
                            <label className="text-sm font-semibold text-1">Start Time</label>
                            <input 
                              type="time" 
                              className="bg-surface-elevated border border-muted rounded-lg px-3 py-2 text-sm text-1 focus:outline-none focus:ring-2 focus:ring-brand transition-all" 
                              value={config.scheduler_start_time || '02:00'} 
                              onChange={(e) => updateConfig({ scheduler_start_time: e.target.value })}
                            />
                          </div>
                          <div className="flex-1 flex flex-col gap-2">
                            <label className="text-sm font-semibold text-1">Stop Time</label>
                            <input 
                              type="time" 
                              className="bg-surface-elevated border border-muted rounded-lg px-3 py-2 text-sm text-1 focus:outline-none focus:ring-2 focus:ring-brand transition-all" 
                              value={config.scheduler_stop_time || '08:00'} 
                              onChange={(e) => updateConfig({ scheduler_stop_time: e.target.value })}
                            />
                          </div>
                        </div>
                      </div>
                    </section>

                    <section>
                      <h3 className="text-sm font-bold text-1 mb-4 flex items-center gap-2">
                        <FileCheck2 size={16} style={{ color: "var(--color-brand)" }}/> Security Integrations
                      </h3>
                      <div className="bg-surface border border-muted rounded-2xl p-5 shadow-sm flex flex-col gap-6">
                        <div className="flex flex-col gap-2">
                          <div className="flex items-center justify-between mb-2">
                            <label className="text-sm font-semibold text-1">Auto-Extract Archives</label>
                            {config.auto_extract && <span className="text-xs font-bold uppercase tracking-wider tag-brand rounded-full">Active</span>}
                          </div>
                          <div className="flex items-center justify-between">
                            <div className="text-xs text-3 pr-4">Automatically extract .zip and .7z archives upon successful download.</div>
                            <Toggle active={config.auto_extract || false} onClick={() => updateConfig({ auto_extract: !config.auto_extract })} />
                          </div>
                        </div>

                        <div className="h-px bg-surface-raised w-full" />

                        <div className="flex flex-col gap-2">
                          <div className="flex items-center justify-between mb-2">
                            <label className="text-sm font-semibold text-1">Post-Processing Script</label>
                            {config.post_process_script && <span className="text-xs font-bold uppercase tracking-wider tag-brand rounded-full">Active</span>}
                          </div>
                          <div className="flex gap-2">
                            <input 
                              type="text" 
                              placeholder="Path to script (.bat, .ps1, .sh)"
                              className="flex-1 bg-surface-elevated border border-muted rounded-lg px-3 py-2 text-sm text-1 focus:outline-none focus:ring-2 focus:ring-brand transition-all placeholder:text-4" 
                              value={config.post_process_script || ''} 
                              onChange={(e) => updateConfig({ post_process_script: e.target.value || null })}
                            />
                            <button 
                              className="btn-secondary px-4 rounded-lg text-sm font-semibold"
                              onClick={() => handleBrowseFile('post_process_script')}
                            >
                              Browse
                            </button>
                          </div>
                        </div>

                        <div className="h-px bg-surface-raised w-full" />

                        <div className="flex flex-col gap-2">
                          <div className="flex items-center justify-between mb-2">
                            <label className="text-sm font-semibold text-1">Antivirus Post-Scan</label>
                            {config.av_scan_path && <span className="text-xs font-bold uppercase tracking-wider tag-brand rounded-full">Active</span>}
                          </div>
                          <div className="flex gap-2">
                            <input 
                              type="text" 
                              placeholder="Path to Antivirus CLI (e.g. MpCmdRun.exe)"
                              className="flex-1 bg-surface-elevated border border-muted rounded-lg px-3 py-2 text-sm text-1 focus:outline-none focus:ring-2 focus:ring-brand transition-all placeholder:text-4" 
                              value={config.av_scan_path || ''} 
                              onChange={(e) => updateConfig({ av_scan_path: e.target.value || null })}
                            />
                            <button 
                              className="btn-secondary px-4 rounded-lg text-sm font-semibold"
                              onClick={() => handleBrowseFile('av_scan_path')}
                            >
                              Browse
                            </button>
                          </div>
                          <div className="mt-2">
                            <label className="text-xs font-semibold text-2 uppercase tracking-wider mb-1 block">Custom CLI Arguments</label>
                            <input 
                              type="text" 
                              placeholder="e.g. -Scan -ScanType 3 -File {FILE}"
                              className="w-full bg-surface-elevated border border-muted rounded-lg px-3 py-2 text-sm text-1 focus:outline-none focus:ring-2 focus:ring-brand transition-all placeholder:text-4" 
                              value={config.av_scan_args || ''} 
                              onChange={(e) => updateConfig({ av_scan_args: e.target.value || null })}
                            />
                            <p className="text-xs text-3 mt-1">Use <code style={{ color: "var(--color-brand)" }}>{"{FILE}"}</code> to represent the downloaded file path.</p>
                          </div>
                        </div>
                      </div>
                    </section>

                    <section>
                      <h3 className="text-sm font-bold text-1 mb-4 flex items-center gap-2">
                        <Lock size={16} style={{ color: "var(--color-brand)" }}/> Captcha Solver
                      </h3>
                      <div className="bg-surface border border-muted rounded-2xl p-5 shadow-sm flex flex-col gap-6">
                        <div className="flex flex-col gap-2">
                          <div className="flex items-center justify-between mb-2">
                            <label className="text-sm font-semibold text-1">2captcha API Key</label>
                            {config.captcha_api_key && config.captcha_api_key !== '' && <span className="text-xs font-bold uppercase tracking-wider tag-brand rounded-full">Securely Vaulted</span>}
                          </div>
                          <div className="flex gap-2">
                            <input 
                              type="password" 
                              placeholder="Enter 2captcha API Key to store in encrypted vault"
                              className="flex-1 bg-surface-elevated border border-muted rounded-lg px-3 py-2 text-sm text-1 focus:outline-none focus:ring-2 focus:ring-brand transition-all placeholder:text-4" 
                              value={config.captcha_api_key || ''} 
                              onChange={(e) => updateConfig({ captcha_api_key: e.target.value })}
                            />
                            {config.captcha_api_key && config.captcha_api_key !== '' && (
                              <button 
                                className="btn-danger"
                                onClick={() => updateConfig({ captcha_api_key: '' })}
                              >
                                Clear
                              </button>
                            )}
                          </div>
                          <p className="text-xs text-3 mt-1">
                            The API key is encrypted using XChaCha20-Poly1305 and stored locally in the secure database vault. It will be used automatically when downloads encounter a captcha block.
                          </p>
                        </div>
                      </div>
                    </section>
                  </div>
                )}

                {/* --- PROXY TAB --- */}
                {activeTab === 'Proxy / Socks' && (
                  <div className="flex flex-col gap-8">
                    <section>
                      <h3 className="text-sm font-bold text-1 mb-4 flex items-center gap-2">
                        <Network size={16} style={{ color: "var(--color-brand)" }}/> Network Routing
                      </h3>
                      
                      <div className="bg-surface border border-muted rounded-2xl shadow-sm flex flex-col overflow-hidden">
                        
                        <label className={cn("flex items-center gap-4 p-5 cursor-pointer transition-colors border-b border-subtle", (!config.proxy?.use_system_proxy && !config.proxy?.url) ? "bg-brand-dim" : "hover:bg-surface-raised")}>
                          <div className="relative flex items-center justify-center w-5 h-5">
                            <input 
                              type="radio" 
                              className="peer sr-only"
                              checked={!config.proxy?.use_system_proxy && !config.proxy?.url} 
                              onChange={() => updateProxy({ use_system_proxy: false, url: null })}
                            />
                            <div className="w-5 h-5 border-2 border-border rounded-full peer-checked:border-brand transition-all" />
                            <div className="absolute w-2.5 h-2.5 rounded-full bg-brand scale-0 peer-checked:scale-100 transition-transform" />
                          </div>
                          <div>
                            <div className="font-semibold text-sm text-1">No Proxy</div>
                            <div className="text-xs text-3 mt-0.5">Direct connection to the internet</div>
                          </div>
                        </label>

                        <label className={cn("flex items-center gap-4 p-5 cursor-pointer transition-colors border-b border-subtle", config.proxy?.use_system_proxy ? "bg-brand-dim" : "hover:bg-surface-raised")}>
                          <div className="relative flex items-center justify-center w-5 h-5">
                            <input 
                              type="radio" 
                              className="peer sr-only"
                              checked={config.proxy?.use_system_proxy}
                              onChange={() => updateProxy({ use_system_proxy: true, url: null })}
                            />
                            <div className="w-5 h-5 border-2 border-border rounded-full peer-checked:border-brand transition-all" />
                            <div className="absolute w-2.5 h-2.5 rounded-full bg-brand scale-0 peer-checked:scale-100 transition-transform" />
                          </div>
                          <div>
                            <div className="font-semibold text-sm text-1">System Default</div>
                            <div className="text-xs text-3 mt-0.5">Use Windows Internet Options proxy</div>
                          </div>
                        </label>

                        <label className={cn("flex items-center gap-4 p-5 cursor-pointer transition-colors", (!config.proxy?.use_system_proxy && !!config.proxy?.url) ? "bg-brand-dim" : "hover:bg-surface-raised")}>
                          <div className="relative flex items-center justify-center w-5 h-5">
                            <input 
                              type="radio" 
                              className="peer sr-only"
                              checked={!config.proxy?.use_system_proxy && !!config.proxy?.url}
                              onChange={() => updateProxy({ use_system_proxy: false, url: config.proxy?.url || 'http://' })}
                            />
                            <div className="w-5 h-5 border-2 border-border rounded-full peer-checked:border-brand transition-all" />
                            <div className="absolute w-2.5 h-2.5 rounded-full bg-brand scale-0 peer-checked:scale-100 transition-transform" />
                          </div>
                          <div>
                            <div className="font-semibold text-sm text-1">Manual Configuration</div>
                            <div className="text-xs text-3 mt-0.5">Specify HTTP / SOCKS proxy details</div>
                          </div>
                        </label>

                      </div>

                      <div className={cn(
                        "transition-all duration-300 overflow-hidden", 
                        (!config.proxy?.use_system_proxy && !!config.proxy?.url) ? "max-h-[500px] opacity-100 mt-2" : "max-h-0 opacity-0 pointer-events-none"
                      )}>
                        <div className="bg-surface border border-muted rounded-2xl p-5 shadow-sm flex flex-col gap-4">
                          <ConfigInput 
                            label="Proxy Server URL" 
                            placeholder="http://192.168.1.1:8080"
                            value={config.proxy?.url || ''}
                            // eslint-disable-next-line @typescript-eslint/no-explicit-any
                            onChange={(e:any) => updateProxy({ url: e.target.value })}
                            icon={Network}
                          />
                          <div className="flex gap-4">
                            <ConfigInput 
                              label="Username" 
                              placeholder="Optional"
                              value={config.proxy?.username || ''}
                              // eslint-disable-next-line @typescript-eslint/no-explicit-any
                              onChange={(e:any) => updateProxy({ username: e.target.value || null })}
                              icon={Fingerprint}
                            />
                            <ConfigInput 
                              label="Password" 
                              type="password"
                              placeholder="Optional"
                              value={config.proxy?.password || ''}
                              // eslint-disable-next-line @typescript-eslint/no-explicit-any
                              onChange={(e:any) => updateProxy({ password: e.target.value || null })}
                              icon={Lock}
                            />
                          </div>
                        </div>
                      </div>

                    </section>

                    <section className="mt-6">
                      <h3 className="text-sm font-bold text-1 mb-4 flex items-center gap-2">
                        <AlertCircle size={16} style={{ color: "var(--color-brand)" }}/> Tor Network Integration
                      </h3>
                      <div className="bg-surface border border-muted rounded-2xl p-5 shadow-sm flex flex-col gap-4">
                        <div className="flex items-center justify-between">
                          <div>
                            <div className="font-semibold text-sm text-1">Tor Network Routing (SOCKS5h)</div>
                            <div className="text-xs text-3 mt-1">Route all download traffic through a local Tor proxy (127.0.0.1:9050) with remote DNS.</div>
                          </div>
                          <Toggle 
                            active={config.proxy?.route_via_tor || false} 
                            onClick={() => updateProxy({ route_via_tor: !config.proxy?.route_via_tor })} 
                          />
                        </div>
                      </div>
                    </section>

                    <section className="mt-6">
                      <h3 className="text-sm font-bold text-1 mb-4 flex items-center gap-2">
                        <Globe size={16} style={{ color: "var(--color-brand)" }}/> DNS Privacy
                      </h3>
                      <div className="bg-surface border border-muted rounded-2xl p-5 shadow-sm flex flex-col gap-4">
                        <div className="flex items-center justify-between">
                          <div>
                            <div className="font-semibold text-sm text-1">DNS over HTTPS (DoH)</div>
                            <div className="text-xs text-3 mt-1">Resolve hostname queries securely over HTTPS via Cloudflare DNS.</div>
                          </div>
                          <Toggle active={config.dns_over_https} onClick={() => updateConfig({ dns_over_https: !config.dns_over_https })} />
                        </div>
                      </div>
                    </section>
                  </div>
                )}

                {/* --- SITE LOGINS TAB --- */}
                {activeTab === 'Site Logins' && (
                  <div className="flex flex-col gap-8">
                    <section>
                      <h3 className="text-sm font-bold text-1 mb-4 flex items-center gap-2">
                        <Fingerprint size={16} style={{ color: "var(--color-brand)" }}/> Site Credentials
                      </h3>
                      
                      <div className="bg-surface border border-muted rounded-2xl shadow-sm overflow-hidden flex flex-col h-[380px]">
                        
                        <div className="bg-surface-raised p-4 border-b border-muted flex flex-col gap-3">
                          <p className="text-xs text-3">These credentials will automatically be injected into HTTP Basic Auth requests for matching domains.</p>
                          <div className="flex gap-2 items-center">
                            <input 
                              type="text" placeholder="Server / Host" 
                              className="flex-1 min-w-0 bg-surface border border-muted rounded-lg px-3 py-2 text-xs text-1 focus:outline-none focus:border-brand"
                              value={newLoginHost} onChange={e => setNewLoginHost(e.target.value)}
                            />
                            <input 
                              type="text" placeholder="Username" 
                              className="w-[120px] bg-surface border border-muted rounded-lg px-3 py-2 text-xs text-1 focus:outline-none focus:border-brand"
                              value={newLoginUser} onChange={e => setNewLoginUser(e.target.value)}
                            />
                            <input 
                              type="password" placeholder="Password" 
                              className="w-[120px] bg-surface border border-muted rounded-lg px-3 py-2 text-xs text-1 focus:outline-none focus:border-brand"
                              value={newLoginPass} onChange={e => setNewLoginPass(e.target.value)}
                            />
                            <button 
                              className="btn-primary"
                              onClick={addLogin}
                              disabled={!newLoginHost || !newLoginUser || !newLoginPass}
                            >
                              <Check size={14} /> Add
                            </button>
                          </div>
                        </div>

                        <div className="flex-1 overflow-y-auto">
                          {logins.length === 0 ? (
                            <div className="h-full flex flex-col items-center justify-center text-3 gap-3 opacity-50">
                              <Lock size={32} />
                              <span className="text-sm font-medium">No site credentials stored.</span>
                            </div>
                          ) : (
                            <div className="divide-y divide-muted">
                              {logins.map((item) => (
                                <div key={item.id} className="flex justify-between items-center px-5 py-3 hover:bg-surface-raised transition-colors group">
                                  <div className="flex flex-col">
                                    <span className="font-semibold text-sm text-1">{item.domain}</span>
                                    <span className="text-xs text-3 mt-0.5">User: {item.username}</span>
                                  </div>
                                  <button 
                                    className="p-2 rounded-lg text-3 hover:text-error hover:bg-error-dim transition-colors opacity-0 group-hover:opacity-100"
                                    onClick={() => removeLogin(item.id)}
                                    title="Delete credential"
                                  >
                                    <Trash2 size={16} />
                                  </button>
                                </div>
                              ))}
                            </div>
                          )}
                        </div>

                      </div>
                    </section>
                  </div>
                )}

                {/* --- DIAL-UP TAB --- */}
                {activeTab === 'Dial-up' && (
                  <div className="flex flex-col gap-8">
                    <section>
                      <h3 className="text-sm font-bold text-1 mb-4 flex items-center gap-2">
                        <Phone size={16} style={{ color: "var(--color-brand)" }}/> Dial-up & VPN Integrations
                      </h3>
                      <div className="bg-surface border border-muted rounded-2xl p-5 shadow-sm flex flex-col gap-4">
                        
                        <div className="flex flex-col gap-2 mb-2">
                          <label className="text-xs font-semibold text-2 uppercase tracking-wider">Windows Connection Profile</label>
                          <select 
                            className="w-full bg-surface-elevated border border-muted rounded-lg px-3 py-2.5 text-sm font-medium text-1 focus:outline-none focus:ring-2 focus:ring-brand transition-all cursor-pointer appearance-none"
                            value={dialup.connectionName}
                            // eslint-disable-next-line @typescript-eslint/no-explicit-any
                            onChange={(e) => setDialup((prev:any) => ({ ...prev, connectionName: e.target.value }))}
                          >
                            <option value="">-- No Active Connections Found --</option>
                            <option value="vpn-1">Primary Broadband / WAN Profile</option>
                            <option value="pppoe">PPPoE Link Profile</option>
                          </select>
                        </div>

                        <div className="flex gap-4">
                          <ConfigInput 
                            label="Username" 
                            value={dialup.username}
                            // eslint-disable-next-line @typescript-eslint/no-explicit-any
                            onChange={(e:any) => setDialup((prev:any) => ({ ...prev, username: e.target.value }))}
                          />
                          <ConfigInput 
                            label="Password" 
                            type="password"
                            value={dialup.password}
                            // eslint-disable-next-line @typescript-eslint/no-explicit-any
                            onChange={(e:any) => setDialup((prev:any) => ({ ...prev, password: e.target.value }))}
                          />
                        </div>
                        
                        <div className="h-px bg-surface-raised w-full my-2" />

                        <div className="flex items-center justify-between">
                          <div className="pr-4">
                            <div className="font-semibold text-sm text-1">Redial on drop</div>
                            <div className="text-xs text-3 mt-1">Number of times to attempt reconnecting if connection is lost.</div>
                          </div>
                          <input 
                            type="number" 
                            min="0"
                            className="input-field w-20 text-center !py-2 !px-3 font-semibold text-brand" 
                            value={dialup.redialCount}
                            // eslint-disable-next-line @typescript-eslint/no-explicit-any
                            onChange={(e) => setDialup((prev:any) => ({ ...prev, redialCount: parseInt(e.target.value, 10) || 0 }))}
                          />
                        </div>

                      </div>
                    </section>
                  </div>
                )}


              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

