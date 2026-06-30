import React, { useState } from 'react';
import {
  PackageOpen, Timer, CircleCheck, Braces,
  FileText, GitBranch, SlidersHorizontal, Eraser, Plus, X, Check,
  LayoutGrid
} from 'lucide-react';
import { cn } from '../utils';
import { useTranslation } from 'react-i18next';
import { DownloadInfo } from '../types';
import { useUiStore } from '../stores/uiStore';

export const evaluateQuery = (downloads: DownloadInfo[], query: string): DownloadInfo[] => {
  let out = [...downloads];
  const parts = query.split(/\s+/);
  for (const part of parts) {
    if (!part.trim()) continue;
    if (part.startsWith('url:')) {
      const val = part.slice(4).toLowerCase();
      out = out.filter(d => d.url.toLowerCase().includes(val));
    } else if (part.startsWith('filename:')) {
      const val = part.slice(9).toLowerCase();
      out = out.filter(d => (d.filename || '').toLowerCase().includes(val));
    } else if (part.startsWith('status:')) {
      const val = part.slice(7).toLowerCase();
      out = out.filter(d => d.status.toLowerCase() === val);
    } else if (part.startsWith('size:>')) {
      const val = parseInt(part.slice(6), 10);
      out = out.filter(d => d.total_bytes ? d.total_bytes > val : false);
    } else if (part.startsWith('size:<')) {
      const val = parseInt(part.slice(6), 10);
      out = out.filter(d => d.total_bytes ? d.total_bytes < val : false);
    } else if (part.startsWith('speed:>')) {
      const val = parseInt(part.slice(7), 10);
      out = out.filter(d => d.speed_bps ? d.speed_bps > val : false);
    } else if (part.startsWith('speed:<')) {
      const val = parseInt(part.slice(7), 10);
      out = out.filter(d => d.speed_bps ? d.speed_bps < val : false);
    } else {
      const val = part.toLowerCase();
      out = out.filter(d => d.url.toLowerCase().includes(val) || (d.filename || '').toLowerCase().includes(val));
    }
  }
  return out;
};

const Sidebar = React.memo(function Sidebar({
  downloads = [],
  activeCategory,
  onSelectCategory,
  categoryRules = [],
}: {
  downloads?: DownloadInfo[];
  activeCategory: string;
  onSelectCategory: (category: string) => void;
  categoryRules?: import('../types').CategoryRule[];
}) {
  const { t } = useTranslation();
  const { smartLists, addSmartList, removeSmartList } = useUiStore();
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);
  const [showAddForm, setShowAddForm] = useState(false);
  const [newListName, setNewListName] = useState('');
  const [newListQuery, setNewListQuery] = useState('');

  const allCount        = downloads.length;
  const unfinishedCount = downloads.filter(d => d.status !== 'completed').length;
  const completedCount  = downloads.filter(d => d.status === 'completed').length;

  const getExtCount = (exts: string[]) =>
    downloads.filter(d => {
      const name = d.filename || d.url || '';
      const ext  = name.split('.').pop()?.toLowerCase();
      return ext ? exts.includes(ext) : false;
    }).length;

  const categories = [
    { name: t('Dashboard'),      icon: LayoutGrid, count: 0,     desc: 'Overview' },
    { name: t('All Downloads'),  icon: PackageOpen,  count: allCount, desc: 'Everything' },
    { name: t('Unfinished'),     icon: Timer,        count: unfinishedCount, desc: 'In progress' },
    { name: t('Completed'),      icon: CircleCheck,  count: completedCount,  desc: 'Done' },
    { name: t('Grabber'),        icon: Braces,       count: 0,     desc: 'Mass grab' },
  ];

  const queues = [
    {
      name: t('Main Queue'),
      icon: GitBranch,
      count: downloads.filter(d => ['paused','idle','queued'].includes(d.status)).length,
    },
  ];

  const types = categoryRules.map(r => ({
    name: r.label,
    icon: SlidersHorizontal,
    count: getExtCount(r.extensions.map(ext => ext.replace(/^\./, '').toLowerCase()))
  }));

  const smartItems = smartLists.map(sl => ({
    id: sl.id,
    name: sl.name,
    icon: SlidersHorizontal,
    count: evaluateQuery(downloads, sl.query).length,
    isSmart: true,
  }));

  const renderItem = (item: { id?: string; name: string; icon: React.ElementType; count?: number; isSmart?: boolean }) => {
    const Icon       = item.icon;
    const isActive   = activeCategory === item.name;
    const isConfirming = item.isSmart && confirmDeleteId === item.id;

    return (
      <div
        key={item.name}
        role="button"
        tabIndex={isConfirming ? -1 : 0}
        aria-current={isActive ? 'page' : undefined}
        onClick={() => !isConfirming && onSelectCategory(item.name)}
        onKeyDown={(e) => {
          if (!isConfirming && (e.key === 'Enter' || e.key === ' ')) {
            e.preventDefault();
            onSelectCategory(item.name);
          }
        }}
        className={cn('sidebar-item group', isActive && 'active')}
      >
        <div className="flex items-center gap-2.5 min-w-0 flex-1">
          <div className={cn(
            'sidebar-icon-wrap',
            isActive && 'active'
          )}>
            <Icon size={13} strokeWidth={isActive ? 2.2 : 1.8} />
          </div>
          <span className="truncate text-[11.5px] leading-tight">{item.name}</span>
        </div>
        <div className="flex items-center gap-1 flex-shrink-0 ml-2">
          {item.count !== undefined && item.count > 0 && !isConfirming && (
            <span className={cn('sidebar-item-count', isActive && 'active')}>
              {item.count}
            </span>
          )}
          {item.isSmart && !isConfirming && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                setConfirmDeleteId(item.id!);
              }}
              aria-label={`Delete smart list ${item.name}`}
              className="sidebar-delete-btn"
              title="Delete Smart List"
            >
              <Eraser size={11} />
            </button>
          )}
          {isConfirming && (
            <>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  removeSmartList(item.id!);
                  if (isActive) onSelectCategory('All Downloads');
                  setConfirmDeleteId(null);
                }}
                aria-label="Confirm delete"
                className="sidebar-confirm-btn confirm"
                title="Confirm delete"
              >
                <Check size={11} />
              </button>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  setConfirmDeleteId(null);
                }}
                aria-label="Cancel delete"
                className="sidebar-confirm-btn cancel"
                title="Cancel"
              >
                <X size={11} />
              </button>
            </>
          )}
        </div>
      </div>
    );
  };

  const renderSection = (
    title: string | null,
    items: any[],
    action?: React.ReactNode
  ) => (
    <div className="sidebar-section">
      {title && (
        <div className="sidebar-section-header">
          <span className="sidebar-section-title">{title}</span>
          {action}
        </div>
      )}
      {items.map(renderItem)}
    </div>
  );

  return (
    <nav
      aria-label="Application navigation"
      className="sidebar-nav"
    >
      {renderSection(null, categories)}
      <div className="sidebar-divider" />
      {renderSection('Queues', queues)}
      {types.length > 0 && (
        <>
          <div className="sidebar-divider" />
          {renderSection('File Types', types)}
        </>
      )}
      <div className="sidebar-divider" />
      {renderSection(
        'Smart Lists',
        smartItems,
        <button
          onClick={(e) => {
            e.stopPropagation();
            setShowAddForm(f => !f);
            setNewListName('');
            setNewListQuery('');
          }}
          className="sidebar-section-action"
          title="Create Smart List"
        >
          <Plus size={12} />
        </button>
      )}
      {showAddForm && (
        <div className="smart-list-dialog" style={{ position: 'relative', top: 0, right: 'auto', marginTop: 4 }}>
          <div style={{ fontSize: '11px', fontWeight: 700, color: 'var(--color-text-2)', letterSpacing: '0.02em' }}>
            New Smart List
          </div>
          <input
            className="search-input w-full"
            placeholder="List name"
            value={newListName}
            onChange={e => setNewListName(e.target.value)}
            autoFocus
          />
          <input
            className="search-input w-full"
            placeholder='Filter query (e.g. url:github)'
            value={newListQuery}
            onChange={e => setNewListQuery(e.target.value)}
          />
          <div className="flex gap-2 justify-end">
            <button
              className="btn-secondary"
              style={{ height: 26, fontSize: 'var(--text-xs-size)' }}
              onClick={() => setShowAddForm(false)}
            >
              Cancel
            </button>
            <button
              className="btn-primary"
              style={{ height: 26, fontSize: 'var(--text-xs-size)' }}
              disabled={!newListName.trim() || !newListQuery.trim()}
              onClick={() => {
                if (!newListName.trim() || !newListQuery.trim()) return;
                let finalQuery = newListQuery;
                const sizeMatch = newListQuery.match(/size:([><])(\d+)(kb|mb|gb)?/i);
                if (sizeMatch) {
                  const op = sizeMatch[1];
                  let sizeVal = parseInt(sizeMatch[2], 10);
                  const unit = (sizeMatch[3] || '').toLowerCase();
                  if (unit === 'kb') sizeVal *= 1024;
                  else if (unit === 'mb') sizeVal *= 1024 * 1024;
                  else if (unit === 'gb') sizeVal *= 1024 * 1024 * 1024;
                  finalQuery = newListQuery.replace(sizeMatch[0], `size:${op}${sizeVal}`);
                }
                addSmartList(newListName.trim(), finalQuery);
                setShowAddForm(false);
                setNewListName('');
                setNewListQuery('');
              }}
            >
              Add
            </button>
          </div>
        </div>
      )}
    </nav>
  );
});

export default Sidebar;
