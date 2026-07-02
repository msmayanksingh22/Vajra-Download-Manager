import React, { useState, useRef, useEffect } from 'react';
import {
  Download as DownloadIcon,
  File,
  FileArchive,
  FileAudio,
  FileVideo,
  Settings as SettingsIcon,
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  Clock,
  CheckCircle2,
  AlertCircle,
  Pause,
  Play,
  RefreshCw,
  Trash2,
  Info,
  Eye,
  Activity,
  ArrowUp,
  ArrowDown,
} from 'lucide-react';
import { cn } from '../utils';
import { api, fmtBytes, fmtSpeed, fmtEta } from '../api';
import { DownloadInfo } from '../types';
import { useUiStore } from '../stores/uiStore';
import { evaluateQuery } from './Sidebar';

const COLUMN_DEFAULTS = {
  checkbox: { label: 'Checkbox', visible: true, width: 32 },
  filename: { label: 'File Name', visible: true, width: 240 },
  progress: { label: 'Progress', visible: true, width: 160 },
  size: { label: 'Size', visible: true, width: 80 },
  status: { label: 'Status', visible: true, width: 95 },
  time: { label: 'Time Left', visible: true, width: 90 },
  speed: { label: 'Transfer Rate', visible: true, width: 100 },
  resume: { label: 'Resume', visible: true, width: 65 },
  category: { label: 'Category', visible: false, width: 90 },
  savePath: { label: 'Save Path', visible: false, width: 160 },
  url: { label: 'URL', visible: false, width: 200 },
  added: { label: 'Added', visible: true, width: 130 },
};

/* ---- Helpers ---- */
const getCategoryLabel = (
  item: any,
  categoryRules: import('../types').CategoryRule[] = [],
): string => {
  const name = item.filename || item.file_name || item.url || '';
  const ext = name.split('.').pop()?.toLowerCase();

  if (ext) {
    for (const rule of categoryRules) {
      if (rule.extensions.map((e) => e.replace(/^\./, '').toLowerCase()).includes(ext)) {
        return rule.label;
      }
    }
  }
  return 'General';
};

const getFileIcon = (filename: string = '') => {
  const ext = filename.split('.').pop()?.toLowerCase();
  if (ext && ['zip', 'rar', '7z', 'tar', 'gz'].includes(ext)) return FileArchive;
  if (ext && ['mp3', 'wav', 'flac'].includes(ext)) return FileAudio;
  if (ext && ['mp4', 'mkv', 'avi', 'mov'].includes(ext)) return FileVideo;
  if (ext && ['exe', 'msi', 'apk', 'dmg'].includes(ext)) return SettingsIcon;
  return File;
};

const STATUS_TAG: Record<string, string> = {
  completed: 'tag tag-success',
  complete: 'tag tag-success',
  downloading: 'tag tag-info',
  connecting: 'tag tag-info',
  paused: 'tag tag-warning',
  failed: 'tag tag-error',
  error: 'tag tag-error',
};

const DownloadsTable = React.memo(function DownloadsTable({
  items,
  selectedIds = new Set(),
  activeCategory,
  categoryRules = [],
  onSelect,
  onSelectAll,
  onDoubleClick,
  onAction,
}: {
  items: DownloadInfo[];
  selectedIds?: Set<string>;
  activeCategory: string;
  categoryRules?: import('../types').CategoryRule[];
  onSelect: (id: string, shiftKey: boolean, ctrlKey: boolean) => void;
  onSelectAll: (ids: string[]) => void;
  onDoubleClick: (item: DownloadInfo) => void;
  onAction: (item: DownloadInfo, action: string) => void;
}) {
  const smartLists = useUiStore((state) => state.smartLists);
  const [columns, setColumns] = useState(() => {
    try {
      const saved = localStorage.getItem('vajra_table_columns_v3');
      if (saved) return JSON.parse(saved);
    } catch {
      /* ignore */
    }
    return COLUMN_DEFAULTS;
  });
  const [sortCol, setSortCol] = useState('added');
  const [sortDesc, setSortDesc] = useState(true);
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    item: DownloadInfo;
  } | null>(null);
  const [headerMenu, setHeaderMenu] = useState<{ x: number; y: number } | null>(null);

  useEffect(() => {
    localStorage.setItem('vajra_table_columns_v3', JSON.stringify(columns));
  }, [columns]);

  // Close menus on any click
  useEffect(() => {
    const close = () => {
      setContextMenu(null);
      setHeaderMenu(null);
    };
    document.addEventListener('click', close);
    document.addEventListener('contextmenu', close);
    return () => {
      document.removeEventListener('click', close);
      document.removeEventListener('contextmenu', close);
    };
  }, []);

  const handleContextMenu = (e: React.MouseEvent, item: DownloadInfo) => {
    e.preventDefault();
    e.stopPropagation();
    onSelect(item.id, false, false);
    const mW = 180,
      mH = 220;
    const x = Math.min(e.clientX, window.innerWidth - mW - 8);
    const y = Math.min(e.clientY, window.innerHeight - mH - 8);
    setContextMenu({ x, y, item });
    setHeaderMenu(null);
  };

  const handleHeaderContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const mW = 180,
      mH = 320;
    const x = Math.min(e.clientX, window.innerWidth - mW - 8);
    const y = Math.min(e.clientY, window.innerHeight - mH - 8);
    setHeaderMenu({ x, y });
    setContextMenu(null);
  };

  const handleSort = (col: string) => {
    if (sortCol === col) setSortDesc(!sortDesc);
    else {
      setSortCol(col);
      setSortDesc(false);
    }
  };

  /* ---- Filtering + Sorting ---- */
  const filteredItems = (() => {
    let out = [...items];
    const matchedSmart = smartLists.find((sl) => sl.name === activeCategory);
    if (activeCategory === 'Completed') out = out.filter((d) => d.status === 'completed');
    else if (activeCategory === 'Unfinished')
      out = out.filter((d) =>
        [
          'downloading',
          'connecting',
          'paused',
          'failed',
          'queued',
          'allocating',
          'fetchingmeta',
        ].includes(d.status),
      );
    else if (activeCategory === 'Grabber') out = [];
    else if (matchedSmart) out = evaluateQuery(out, matchedSmart.query);
    else if (categoryRules.map((r) => r.label).includes(activeCategory))
      out = out.filter((d) => getCategoryLabel(d, categoryRules) === activeCategory);

    out.sort((a, b) => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      let va: any, vb: any;
      switch (sortCol) {
        case 'filename':
          va = (a.filename || a.url || '').toLowerCase();
          vb = (b.filename || b.url || '').toLowerCase();
          break;
        case 'size':
          va = a.total_bytes || 0;
          vb = b.total_bytes || 0;
          break;
        case 'status':
          va = a.status || '';
          vb = b.status || '';
          break;
        case 'time':
          va = a.eta_seconds || Infinity;
          vb = b.eta_seconds || Infinity;
          break;
        case 'speed':
          va = a.speed_bps || 0;
          vb = b.speed_bps || 0;
          break;
        case 'added':
          va = a.created_at || 0;
          vb = b.created_at || 0;
          break;
        case 'category':
          va = getCategoryLabel(a, categoryRules);
          vb = getCategoryLabel(b, categoryRules);
          break;
        case 'savePath':
          va = (a.output_path || '').toLowerCase();
          vb = (b.output_path || '').toLowerCase();
          break;
        case 'url':
          va = (a.url || '').toLowerCase();
          vb = (b.url || '').toLowerCase();
          break;
        default:
          return 0;
      }
      if (va < vb) return sortDesc ? 1 : -1;
      if (va > vb) return sortDesc ? -1 : 1;
      return 0;
    });
    return out;
  })();

  const allSelected = filteredItems.length > 0 && selectedIds.size === filteredItems.length;

  /* ---- Resizable Header ---- */
  const ResizableHeader = ({ col, label }: { col: string; label: string }) => {
    const isResizing = useRef(false);
    const startX = useRef(0);
    const startW = useRef(0);

    const onMouseDown = (e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();
      isResizing.current = true;
      startX.current = e.clientX;
      startW.current = columns[col].width;
      const onMove = (mv: MouseEvent) => {
        if (!isResizing.current) return;
        const diff = mv.clientX - startX.current;
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        setColumns((p: any) => ({
          ...p,
          [col]: { ...p[col], width: Math.max(40, startW.current + diff) },
        }));
      };
      const onUp = () => {
        isResizing.current = false;
        document.removeEventListener('mousemove', onMove);
        document.removeEventListener('mouseup', onUp);
      };
      document.addEventListener('mousemove', onMove);
      document.addEventListener('mouseup', onUp);
    };

    if (!columns[col]?.visible) return null;

    return (
      <th
        style={{
          width: columns[col].width,
          minWidth: columns[col].width,
          maxWidth: columns[col].width,
        }}
        className="table-th relative"
        scope="col"
        aria-sort={sortCol === col ? (sortDesc ? 'descending' : 'ascending') : 'none'}
        onClick={() => handleSort(col)}
        onContextMenu={handleHeaderContextMenu}
      >
        <div
          className="flex items-center gap-1 w-full overflow-hidden"
          style={{ cursor: 'default' }}
        >
          <span className="truncate">{label}</span>
          {sortCol === col &&
            (sortDesc ? (
              <ArrowDown size={10} style={{ color: 'var(--color-brand)', flexShrink: 0 }} />
            ) : (
              <ArrowUp size={10} style={{ color: 'var(--color-brand)', flexShrink: 0 }} />
            ))}
        </div>
        {/* Resize handle — always-visible subtle bar, highlights on hover */}
        <div
          className="col-resize-handle"
          onMouseEnter={(e) => (e.currentTarget.style.backgroundColor = 'var(--color-brand)')}
          onMouseLeave={(e) => (e.currentTarget.style.backgroundColor = 'var(--color-border)')}
          onMouseDown={onMouseDown}
          onClick={(e) => e.stopPropagation()}
          style={{
            position: 'absolute',
            right: 0,
            top: '20%',
            bottom: '20%',
            width: 3,
            borderRadius: 2,
            backgroundColor: 'var(--color-border)',
            cursor: 'col-resize',
            zIndex: 20,
            transition: 'background-color var(--transition-fast)',
          }}
        />
      </th>
    );
  };

  /* ---- Context menus ---- */
  const CtxItem = ({
    icon: Icon,
    label,
    action,
    danger = false,
    disabled = false,
    item,
  }: {
    icon: React.ElementType;
    label: string;
    action: string;
    danger?: boolean;
    disabled?: boolean;
    item: DownloadInfo;
  }) => (
    <button
      disabled={disabled}
      onClick={(e) => {
        e.stopPropagation();
        setContextMenu(null);
        onAction(item, action);
      }}
      className={cn('context-menu-item w-full text-left', danger && 'danger')}
      style={disabled ? { opacity: 0.38, cursor: 'not-allowed' } : {}}
    >
      <Icon size={14} style={{ flexShrink: 0 }} />
      {label}
    </button>
  );

  return (
    <div className="flex-1 overflow-auto" style={{ backgroundColor: 'var(--color-surface)' }}>
      <table
        className="w-full text-left border-collapse table-fixed relative"
        aria-label="Downloads List"
      >
        <thead>
          <tr>
            {/* Checkbox column */}
            <th
              style={{ width: columns.checkbox.width, minWidth: columns.checkbox.width }}
              className="table-th text-center sticky top-0 z-10"
              onContextMenu={handleHeaderContextMenu}
            >
              <input
                type="checkbox"
                checked={allSelected}
                aria-label="Select all downloads"
                style={{
                  accentColor: 'var(--color-brand)',
                  cursor: 'default',
                  width: 13,
                  height: 13,
                }}
                onChange={(e) =>
                  onSelectAll(e.target.checked ? filteredItems.map((i) => i.id) : [])
                }
              />
            </th>
            <ResizableHeader col="filename" label="File Name" />
            <ResizableHeader col="progress" label="Progress" />
            <ResizableHeader col="size" label="Size" />
            <ResizableHeader col="status" label="Status" />
            <ResizableHeader col="time" label="Time Left" />
            <ResizableHeader col="speed" label="Transfer Rate" />
            <ResizableHeader col="resume" label="Resume" />
            <ResizableHeader col="category" label="Category" />
            <ResizableHeader col="savePath" label="Save Path" />
            <ResizableHeader col="url" label="URL" />
            <ResizableHeader col="added" label="Added" />
          </tr>
        </thead>
        <tbody>
          {filteredItems.map((item) => {
            const isSelected = selectedIds.has(item.id);
            const Icon = getFileIcon(item.filename || '');
            const pct = item.progress_pct ?? 0;
            const isComplete = item.status === 'completed';
            const isActive = item.status === 'downloading' || item.status === 'connecting';
            const isFailed = item.status === 'failed';
            const statusLabel = item.status.charAt(0).toUpperCase() + item.status.slice(1);
            const tagClass = STATUS_TAG[item.status] || 'tag tag-neutral';

            return (
              <tr
                key={item.id}
                onClick={(e) => {
                  e.stopPropagation();
                  onSelect(item.id, e.shiftKey, e.metaKey || e.ctrlKey);
                }}
                onDoubleClick={() => onDoubleClick(item)}
                onContextMenu={(e) => handleContextMenu(e, item)}
                className={cn('table-row', isSelected && 'selected')}
                style={{ fontSize: 'var(--text-sm-size)', height: '34px' }}
                aria-selected={isSelected}
                role="row"
              >
                {/* Checkbox */}
                <td
                  className="text-center px-2"
                  style={{ borderRight: '1px solid var(--color-border-subtle)' }}
                  onClick={(e) => {
                    e.stopPropagation();
                    onSelect(item.id, false, true);
                  }}
                >
                  <input
                    type="checkbox"
                    checked={isSelected}
                    readOnly
                    aria-label={`Select ${item.filename || item.url}`}
                    style={{
                      accentColor: 'var(--color-brand)',
                      cursor: 'default',
                      width: 13,
                      height: 13,
                      pointerEvents: 'none',
                    }}
                  />
                </td>

                {/* Filename */}
                {columns.filename?.visible && (
                  <td
                    className="px-2 truncate"
                    style={{
                      maxWidth: columns.filename.width,
                      borderRight: '1px solid var(--color-border-subtle)',
                    }}
                  >
                    <div className="flex items-center gap-1.5 truncate">
                      <Icon
                        size={14}
                        strokeWidth={1.5}
                        style={{
                          color: isComplete ? 'var(--color-success)' : 'var(--color-brand)',
                          flexShrink: 0,
                        }}
                      />
                      <div className="flex flex-col truncate flex-1">
                        <span
                          className="truncate font-medium"
                          style={{ color: 'var(--color-text-1)' }}
                        >
                          {item.filename || item.url}
                        </span>
                        {isFailed && item.error && (
                          <span
                            className="flex items-center gap-1 font-semibold"
                            style={{ color: 'var(--color-error)', fontSize: '10px', marginTop: 1 }}
                            title={item.error}
                          >
                            <AlertCircle size={10} style={{ flexShrink: 0 }} />
                            <span className="truncate">{item.error}</span>
                          </span>
                        )}
                      </div>
                      {item.tags && item.tags.length > 0 && (
                        <div className="flex gap-1 items-center ml-1 flex-shrink-0">
                          {item.tags.map((t) => (
                            <span key={t} className="tag tag-info">
                              {t}
                            </span>
                          ))}
                        </div>
                      )}
                    </div>
                  </td>
                )}

                {/* Progress */}
                {columns.progress?.visible && (
                  <td
                    className="px-2"
                    style={{
                      maxWidth: columns.progress.width,
                      borderRight: '1px solid var(--color-border-subtle)',
                    }}
                  >
                    <div className="flex items-center gap-2">
                      <div className="progress-track flex-1">
                        <div
                          className={cn(
                            'progress-fill',
                            isComplete && 'success',
                            isFailed && 'error',
                          )}
                          style={{ width: `${pct}%` }}
                        />
                      </div>
                      <span
                        style={{
                          fontSize: 'var(--text-xs-size)',
                          fontWeight: 600,
                          color: 'var(--color-text-3)',
                          width: 30,
                          textAlign: 'right',
                        }}
                      >
                        {pct.toFixed(0)}%
                      </span>
                    </div>
                  </td>
                )}

                {/* Size */}
                {columns.size?.visible && (
                  <td
                    className="px-2 truncate"
                    style={{
                      maxWidth: columns.size.width,
                      color: 'var(--color-text-2)',
                      borderRight: '1px solid var(--color-border-subtle)',
                    }}
                  >
                    {item.total_bytes ? fmtBytes(item.total_bytes) : '—'}
                  </td>
                )}

                {/* Status */}
                {columns.status?.visible && (
                  <td
                    className="px-2"
                    style={{
                      maxWidth: columns.status.width,
                      borderRight: '1px solid var(--color-border-subtle)',
                    }}
                  >
                    <span className={tagClass}>{statusLabel}</span>
                  </td>
                )}

                {/* Time Left */}
                {columns.time?.visible && (
                  <td
                    className="px-2 truncate"
                    style={{
                      maxWidth: columns.time.width,
                      color: 'var(--color-text-3)',
                      borderRight: '1px solid var(--color-border-subtle)',
                    }}
                  >
                    {isActive && item.eta_seconds != null ? fmtEta(item.eta_seconds) : ''}
                  </td>
                )}

                {/* Speed */}
                {columns.speed?.visible && (
                  <td
                    className="px-2 truncate"
                    style={{
                      maxWidth: columns.speed.width,
                      color: 'var(--color-text-2)',
                      borderRight: '1px solid var(--color-border-subtle)',
                    }}
                  >
                    {isActive && item.speed_bps ? fmtSpeed(item.speed_bps) : ''}
                  </td>
                )}

                {/* Resume */}
                {columns.resume?.visible && (
                  <td
                    className="px-2 text-center"
                    style={{
                      maxWidth: columns.resume.width,
                      borderRight: '1px solid var(--color-border-subtle)',
                    }}
                  >
                    {isComplete ? (
                      <span className={item.resume_supported ? 'tag tag-success' : 'tag tag-error'}>
                        {item.resume_supported ? 'Yes' : 'No'}
                      </span>
                    ) : item.resume_supported === true ? (
                      <span className="tag tag-success">Yes</span>
                    ) : (
                      <span
                        style={{ color: 'var(--color-text-4)', fontSize: 'var(--text-xs-size)' }}
                      >
                        —
                      </span>
                    )}
                  </td>
                )}

                {/* Category */}
                {columns.category?.visible && (
                  <td
                    className="px-2 truncate"
                    style={{
                      maxWidth: columns.category.width,
                      color: 'var(--color-text-3)',
                      borderRight: '1px solid var(--color-border-subtle)',
                    }}
                  >
                    {getCategoryLabel(item)}
                  </td>
                )}

                {/* Save Path */}
                {columns.savePath?.visible && (
                  <td
                    className="px-2 truncate"
                    style={{
                      maxWidth: columns.savePath.width,
                      color: 'var(--color-text-3)',
                      fontFamily: 'var(--font-mono)',
                      borderRight: '1px solid var(--color-border-subtle)',
                      fontSize: 'var(--text-xs-size)',
                    }}
                  >
                    {item.output_path || ''}
                  </td>
                )}

                {/* URL */}
                {columns.url?.visible && (
                  <td
                    className="px-2 truncate"
                    style={{
                      maxWidth: columns.url.width,
                      color: 'var(--color-text-4)',
                      borderRight: '1px solid var(--color-border-subtle)',
                      fontSize: 'var(--text-xs-size)',
                    }}
                  >
                    {item.url || ''}
                  </td>
                )}

                {/* Added */}
                {columns.added?.visible && (
                  <td
                    className="px-2 truncate"
                    style={{
                      maxWidth: columns.added.width,
                      color: 'var(--color-text-4)',
                      fontSize: 'var(--text-xs-size)',
                    }}
                  >
                    {item.created_at ? new Date(item.created_at * 1000).toLocaleString() : ''}
                  </td>
                )}
              </tr>
            );
          })}
        </tbody>
      </table>

      {/* Empty state */}
      {filteredItems.length === 0 && (
        <div className="empty-state">
          <DownloadIcon
            size={52}
            strokeWidth={1.25}
            style={{ color: 'var(--color-brand)', opacity: 0.35 }}
          />
          <h3>
            {activeCategory === 'All Downloads'
              ? 'No downloads yet'
              : `No ${activeCategory.toLowerCase()} downloads`}
          </h3>
          <p>
            {activeCategory === 'All Downloads'
              ? 'Paste a URL or drag a file here to start downloading.'
              : `Nothing matches "${activeCategory}" right now.`}
          </p>
          {activeCategory === 'All Downloads' && (
            <button
              className="btn-primary"
              style={{ marginTop: 4 }}
              onClick={() => document.dispatchEvent(new CustomEvent('vajra:open-add-url'))}
            >
              <DownloadIcon size={13} style={{ marginRight: 6 }} />
              Add Download
            </button>
          )}
        </div>
      )}

      {/* Header column-picker context menu */}
      {headerMenu && (
        <div
          className="context-menu"
          style={{ top: headerMenu.y, left: headerMenu.x }}
          onClick={(e) => e.stopPropagation()}
        >
          <div
            className="px-3 pb-2 pt-1 flex items-center gap-1.5"
            style={{ borderBottom: '1px solid var(--color-border-subtle)', marginBottom: 4 }}
          >
            <Eye size={12} style={{ color: 'var(--color-brand)' }} />
            <span
              style={{
                fontSize: 'var(--text-xs-size)',
                fontWeight: 700,
                textTransform: 'uppercase',
                letterSpacing: '0.06em',
                color: 'var(--color-text-3)',
              }}
            >
              Columns
            </span>
          </div>
          {Object.keys(columns).map((key) => {
            if (key === 'checkbox' || key === 'filename') return null;
            return (
              <label key={key} className="context-menu-item justify-between cursor-default">
                <span>{columns[key].label}</span>
                <input
                  type="checkbox"
                  checked={columns[key].visible}
                  style={{
                    accentColor: 'var(--color-brand)',
                    cursor: 'default',
                    width: 13,
                    height: 13,
                  }}
                  // eslint-disable-next-line @typescript-eslint/no-explicit-any
                  onChange={() =>
                    setColumns((p: any) => ({
                      ...p,
                      [key]: { ...p[key], visible: !p[key].visible },
                    }))
                  }
                />
              </label>
            );
          })}
          <div className="context-menu-separator" />
          <button
            className="context-menu-item w-full text-left"
            style={{ color: 'var(--color-text-3)' }}
            onClick={() => {
              setColumns(COLUMN_DEFAULTS);
              setHeaderMenu(null);
            }}
          >
            Reset Columns
          </button>
        </div>
      )}

      {/* Row context menu */}
      {contextMenu && (
        <div
          className="context-menu"
          style={{ top: contextMenu.y, left: contextMenu.x }}
          onClick={(e) => e.stopPropagation()}
        >
          {contextMenu.item.status === 'downloading' || contextMenu.item.status === 'connecting' ? (
            <CtxItem icon={Pause} label="Pause" action="pause" item={contextMenu.item} />
          ) : contextMenu.item.status === 'failed' ? (
            <CtxItem icon={RefreshCw} label="Retry" action="resume" item={contextMenu.item} />
          ) : (
            <CtxItem
              icon={Play}
              label="Resume"
              action="resume"
              item={contextMenu.item}
              disabled={contextMenu.item.status === 'completed'}
            />
          )}
          {contextMenu.item.status !== 'completed' && (
            <>
              <CtxItem
                icon={Activity}
                label="Show Progress"
                action="show_progress"
                item={contextMenu.item}
              />
              <div className="context-menu-separator" />
              <div className="px-3 py-1 text-[10px] font-bold text-[var(--color-text-3)] uppercase tracking-wider">
                Speed Limit
              </div>
              <div className="flex gap-1 px-3 pb-2 pt-0.5">
                {[
                  { label: 'Off', val: 0 },
                  { label: '50K', val: 50 * 1024 },
                  { label: '250K', val: 250 * 1024 },
                  { label: '1M', val: 1024 * 1024 },
                  { label: '5M', val: 5 * 1024 * 1024 },
                ].map((opt) => {
                  const isActive = (contextMenu.item.speed_limit_bps || 0) === opt.val;
                  return (
                    <button
                      key={opt.val}
                      onClick={() => {
                        api
                          .patch(contextMenu.item.id, { speed_limit_bps: opt.val })
                          .then(() => {
                            setContextMenu(null);
                            onAction(contextMenu.item, 'refresh_list');
                          })
                          .catch(console.error);
                      }}
                      className={cn(
                        'px-1.5 py-0.5 rounded text-[10px] font-bold border transition-all duration-200',
                        isActive
                          ? 'bg-[var(--color-brand)] text-white border-transparent shadow-sm'
                          : 'bg-[var(--color-surface-raised)] text-[var(--color-text-2)] border-[var(--color-border-subtle)] hover:bg-[var(--color-surface)] hover:text-[var(--color-text-1)]',
                      )}
                    >
                      {opt.label}
                    </button>
                  );
                })}
              </div>
            </>
          )}
          <div className="context-menu-separator" />
          <CtxItem
            icon={RefreshCw}
            label="Refresh Link"
            action="refresh_url"
            item={contextMenu.item}
            disabled={contextMenu.item.status === 'completed'}
          />
          <div className="context-menu-separator" />
          <CtxItem icon={Trash2} label="Delete" action="delete" item={contextMenu.item} danger />
          <CtxItem icon={Info} label="Properties" action="properties" item={contextMenu.item} />
        </div>
      )}
    </div>
  );
});

export default DownloadsTable;
