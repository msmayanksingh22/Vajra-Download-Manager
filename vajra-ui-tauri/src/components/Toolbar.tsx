import React from 'react';
import { useTranslation } from 'react-i18next';
import {
  FilePlus, PlayCircle, PauseCircle, Eraser, Settings2, CalendarClock,
  OctagonX, ListChecks, Radar, Bug, LifeBuoy, Activity, FileText, CaseSensitive, RotateCw
} from 'lucide-react';
import { cn } from '../utils';

// Icon color map — uses CSS variables for semantic meaning, not raw Tailwind colors
const ICON_COLORS: Record<string, string> = {
  add:        'var(--color-success)',
  resume:     'var(--color-success)',
  pause:      'var(--color-warning)',
  stop:       'var(--color-error)',
  progress:   'var(--color-info)',
  properties: 'var(--color-brand)',
  delete:     'var(--color-error)',
  clear:      'var(--color-text-3)',
  settings:   'var(--color-info)',
  scheduler:  'var(--color-brand)',
  grabber:    'var(--color-success)',
  spider:     'var(--color-info)',
  help:       'var(--color-text-4)',
  rename:     'var(--color-brand)',
};

const ActionButton = ({
  icon: Icon,
  label,
  onClick,
  disabled = false,
  colorKey = 'help',
}: {
  icon: React.ElementType;
  label: string;
  onClick: () => void;
  disabled?: boolean;
  colorKey?: string;
}) => {
  const iconColor = ICON_COLORS[colorKey] ?? 'var(--color-text-3)';

  return (
    <button
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
      aria-disabled={disabled}
      title={label}
      className={cn('toolbar-btn', !disabled && 'group')}
    >
      <Icon
        size={18}
        strokeWidth={1.5}
        style={{ color: disabled ? 'var(--color-text-4)' : iconColor }}
        className="transition-transform duration-150 group-hover:-translate-y-px"
      />
      <span className="toolbar-btn-label">{label}</span>
    </button>
  );
};

const Divider = () => (
  <div className="w-px h-8 mx-1 shrink-0" style={{ backgroundColor: 'var(--color-border)' }} />
);

const Toolbar = React.memo(function Toolbar({
   
  selectedIds = new Set(),
  onAdd,
  onResumeSelected,
  onPauseSelected,
  onStopAll,
  onDeleteSelected,
  onDeleteCompleted,
  onOptions,
  onScheduler,
  onGrabber,
  onSpider,
  onBatchRename,
  onHelp,
  canResume,
  resumeLabel,
  canPause,
  canStopAll,
  canDelete,
  canDeleteCompleted = false,
  onShowProgress,
  canShowProgress,
  isSelectedCompleted = false,
  children,
}: {
  selectedIds?: Set<string>;
  onAdd: () => void;
  onResumeSelected: () => void;
  onPauseSelected: () => void;
  onStopAll: () => void;
  onDeleteSelected: () => void;
  onDeleteCompleted: () => void;
  onOptions: () => void;
  onScheduler: () => void;
  onGrabber: () => void;
  onSpider: () => void;
  onBatchRename?: () => void;
  onHelp: () => void;
  canResume: boolean;
  resumeLabel?: string;
  canPause: boolean;
  canStopAll: boolean;
  canDelete: boolean;
  canDeleteCompleted?: boolean;
  onShowProgress: () => void;
  canShowProgress: boolean;
  isSelectedCompleted?: boolean;
  children?: React.ReactNode;
}) {
  const { t } = useTranslation();
  const hasSelection = (selectedIds?.size ?? 0) > 0;

  return (
    <div
      className="flex items-center gap-1 px-2 py-1 shrink-0 select-none overflow-x-auto overflow-y-hidden w-full z-10"
      style={{
        backgroundColor: 'var(--color-surface-raised)',
        borderBottom: '1px solid var(--color-border)',
        minHeight: '58px',
      }}
      role="toolbar"
      aria-label={t('toolbar.aria_label', 'Application Toolbar')}
    >
      {/* Group 1: New Tasks — always visible */}
      <div role="group" aria-label="New Task">
        <ActionButton icon={FilePlus} label={t('toolbar.add_url', 'Add URL')} onClick={onAdd} colorKey="add" />
      </div>
      <Divider />

      {/* Group 2: Playback Controls — contextual (visible when rows selected) */}
      {hasSelection && (
        <>
          <div role="group" aria-label="Playback" className="flex items-center gap-1 toolbar-context-group">
            <ActionButton icon={resumeLabel === 'Retry' ? RotateCw : PlayCircle} label={resumeLabel || t('toolbar.resume', 'Resume')}    onClick={onResumeSelected}  disabled={!canResume}      colorKey="resume" />
            <ActionButton icon={PauseCircle}      label={t('toolbar.pause', 'Pause')}     onClick={onPauseSelected}   disabled={!canPause}       colorKey="pause" />
            <ActionButton
              icon={isSelectedCompleted ? FileText : Activity}
              label={isSelectedCompleted ? t('toolbar.properties', 'Properties') : t('toolbar.progress', 'Progress')}
              onClick={onShowProgress}
              disabled={!canShowProgress}
              colorKey={isSelectedCompleted ? 'properties' : 'progress'}
            />
          </div>
          <Divider />

          {/* Group 3: Delete & Edit — contextual */}
          <div role="group" aria-label="Edit" className="flex items-center gap-1 toolbar-context-group">
            <ActionButton icon={CaseSensitive}   label={t('toolbar.rename', 'Batch Rename')} onClick={() => onBatchRename && onBatchRename()} disabled={selectedIds && selectedIds.size === 0} colorKey="rename" />
            <ActionButton icon={Eraser} label={t('toolbar.delete', 'Delete')}       onClick={onDeleteSelected}  disabled={!canDelete} colorKey="delete" />
          </div>
          <Divider />
        </>
      )}

      {/* Global actions — always visible */}
      <div role="group" aria-label="Global">
        <ActionButton icon={OctagonX}  label={t('toolbar.stop_all', 'Stop All')}       onClick={onStopAll}         disabled={!canStopAll}        colorKey="stop" />
        <ActionButton icon={ListChecks} label={t('toolbar.clear_completed', 'Clear Completed')} onClick={onDeleteCompleted} disabled={!canDeleteCompleted} colorKey="clear" />
      </div>
      <Divider />

      {/* Group 4: Advanced Tools */}
      <div role="group" aria-label="Tools">
        <ActionButton icon={Settings2}  label={t('toolbar.settings', 'Settings')}  onClick={onOptions}   colorKey="settings" />
        <ActionButton icon={CalendarClock}  label={t('toolbar.scheduler', 'Scheduler')} onClick={onScheduler} colorKey="scheduler" />
        <ActionButton icon={Radar}     label={t('toolbar.grabber', 'Grabber')}   onClick={onGrabber}   colorKey="grabber" />
        <ActionButton icon={Bug}   label={t('toolbar.spider', 'Spider')}    onClick={onSpider}    colorKey="spider" />
      </div>
      <Divider />

      {/* Group 5: Help */}
      <div className="ml-auto flex items-center pr-2 gap-1">
        {children}
        <ActionButton icon={LifeBuoy} label={t('toolbar.help', 'Help')} onClick={onHelp} colorKey="help" />
      </div>
    </div>
  );
});

export default Toolbar;
