import React, { useState, useEffect } from 'react';
import { X, Clock, Play, Square, ArrowUp, ArrowDown } from 'lucide-react';
import { api } from '../../api';
import { cn } from '../../utils';
import { useDialogEscape } from '../../hooks/useDialogEscape';
import { useFocusTrap } from '../../hooks/useFocusTrap';

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export default function SchedulerDialog({ downloads, onClose }: any) {
  const [activeTab, setActiveTab] = useState('Schedule');
  useDialogEscape(onClose);
  const trapRef = useFocusTrap();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [fapEnabled, setFapEnabled] = useState(false);
  const [fapQuotaMb, setFapQuotaMb] = useState(150);
  const [fapWindowHours, setFapWindowHours] = useState(4);
  const [schedulerEnabled, setSchedulerEnabled] = useState(false);
  const [schedulerStartTime, setSchedulerStartTime] = useState('02:00');
  const [schedulerStopTime, setSchedulerStopTime] = useState('06:00');
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [config, setConfig] = useState<any>(null);

  useEffect(() => {
    api.config().then((cfg) => {
      setConfig(cfg);
      setFapEnabled(cfg.fap_enabled ?? false);
      setFapQuotaMb(cfg.fap_quota_mb ?? 150);
      setFapWindowHours(cfg.fap_window_hours ?? 4);
      setSchedulerEnabled(cfg.scheduler_enabled ?? false);
      setSchedulerStartTime(cfg.scheduler_start_time || '02:00');
      setSchedulerStopTime(cfg.scheduler_stop_time || '06:00');
    });
  }, []);

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const queuedDownloads = downloads.filter((d: any) =>
    ['paused', 'error', 'downloading', 'connecting'].includes(d.status),
  );

  const handleApply = async () => {
    if (config)
      await api.setConfig({
        ...config,
        fap_enabled: fapEnabled,
        fap_quota_mb: fapQuotaMb,
        fap_window_hours: fapWindowHours,
        scheduler_enabled: schedulerEnabled,
        scheduler_start_time: schedulerStartTime,
        scheduler_stop_time: schedulerStopTime,
      });
    onClose();
  };
  const handleAction = (id: string, action: 'resume' | 'pause') => api.patch(id, { action });
  const handlePriority = (id: string, priority: 'high' | 'low') => api.patch(id, { priority });

  const TABS = ['Schedule', 'Queue'];
  const Chk = ({
    checked,
    onChange,
    label,
  }: {
    checked: boolean;
    onChange: (v: boolean) => void;
    label: string;
  }) => (
    <label
      className="flex items-center gap-2"
      style={{ cursor: 'default', fontSize: 'var(--text-sm-size)', color: 'var(--color-text-2)' }}
    >
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        style={{ accentColor: 'var(--color-brand)', width: 14, height: 14, cursor: 'default' }}
      />
      {label}
    </label>
  );

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div
        ref={trapRef}
        className="dialog-panel"
        style={{ width: 500, height: 400 }}
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="scheduler-dialog-title"
      >
        {/* Header */}
        <div className="dialog-header">
          <div className="dialog-header-title flex items-center gap-2" id="scheduler-dialog-title">
            <Clock size={16} /> Scheduler
            <span
              style={{
                fontSize: '10px',
                backgroundColor: 'var(--color-warning)',
                color: '#fff',
                padding: '2px 6px',
                borderRadius: '4px',
                fontWeight: 600,
              }}
            >
              Under Development
            </span>
          </div>
          <button className="btn-icon" onClick={onClose} title="Close">
            <X size={15} />
          </button>
        </div>

        {/* Tabs */}
        <div
          role="tablist"
          className="flex gap-5 px-4 shrink-0 drag-region"
          style={{
            borderBottom: '1px solid var(--color-border)',
            backgroundColor: 'var(--color-surface-raised)',
            paddingTop: 8,
          }}
        >
          {TABS.map((tab) => (
            <button
              key={tab}
              type="button"
              role="tab"
              aria-selected={activeTab === tab}
              onClick={() => setActiveTab(tab)}
              className="no-drag relative pb-2"
              style={{
                fontSize: 'var(--text-sm-size)',
                fontWeight: 600,
                cursor: 'default',
                color: activeTab === tab ? 'var(--color-brand)' : 'var(--color-text-3)',
                background: 'none',
                border: 'none',
                fontFamily: 'var(--font-sans)',
                padding: '0 0 8px 0',
              }}
            >
              {tab}
              {activeTab === tab && (
                <div
                  aria-hidden="true"
                  style={{
                    position: 'absolute',
                    bottom: 0,
                    left: 0,
                    right: 0,
                    height: 2,
                    backgroundColor: 'var(--color-brand)',
                    borderRadius: 2,
                  }}
                />
              )}
            </button>
          ))}
        </div>

        {/* Body */}
        <div className="dialog-body" style={{ overflowY: 'auto', gap: 'var(--sp-3)' }}>
          {activeTab === 'Schedule' && (
            <>
              {/* FAP */}
              <div className="card-subtle" style={{ padding: 'var(--sp-3) var(--sp-4)' }}>
                <div className="flex items-center justify-between">
                  <div>
                    <div
                      style={{
                        fontWeight: 600,
                        fontSize: 'var(--text-sm-size)',
                        color: 'var(--color-text-1)',
                      }}
                    >
                      Fair Access Policy
                    </div>
                    <div
                      style={{
                        fontSize: 'var(--text-xs-size)',
                        color: 'var(--color-text-3)',
                        marginTop: 2,
                      }}
                    >
                      Pause downloads when quota exceeded
                    </div>
                  </div>
                  <Chk checked={fapEnabled} onChange={setFapEnabled} label="" />
                </div>
                {fapEnabled && (
                  <div className="flex items-center gap-2 mt-3 animate-fade-in">
                    <span style={{ fontSize: 'var(--text-sm-size)', color: 'var(--color-text-3)' }}>
                      Max
                    </span>
                    <input
                      type="number"
                      className="input-field text-center"
                      style={{ width: 70 }}
                      value={fapQuotaMb}
                      onChange={(e) => setFapQuotaMb(parseInt(e.target.value) || 0)}
                    />
                    <span style={{ fontSize: 'var(--text-sm-size)', color: 'var(--color-text-3)' }}>
                      MB every
                    </span>
                    <input
                      type="number"
                      className="input-field text-center"
                      style={{ width: 55 }}
                      value={fapWindowHours}
                      onChange={(e) => setFapWindowHours(parseInt(e.target.value) || 0)}
                    />
                    <span style={{ fontSize: 'var(--text-sm-size)', color: 'var(--color-text-3)' }}>
                      hrs
                    </span>
                  </div>
                )}
              </div>

              {/* Time Scheduling */}
              <div className="card-subtle mt-4" style={{ padding: 'var(--sp-3) var(--sp-4)' }}>
                <div className="flex items-center justify-between">
                  <div>
                    <div
                      style={{
                        fontWeight: 600,
                        fontSize: 'var(--text-sm-size)',
                        color: 'var(--color-text-1)',
                      }}
                    >
                      Time Scheduling
                    </div>
                    <div
                      style={{
                        fontSize: 'var(--text-xs-size)',
                        color: 'var(--color-text-3)',
                        marginTop: 2,
                      }}
                    >
                      Automatically start and stop all downloads
                    </div>
                  </div>
                  <Chk checked={schedulerEnabled} onChange={setSchedulerEnabled} label="" />
                </div>
                {schedulerEnabled && (
                  <div className="flex items-center gap-3 mt-4 animate-fade-in">
                    <div className="flex flex-col gap-1 flex-1">
                      <label className="text-xs font-semibold text-2 uppercase">Start Time</label>
                      <input
                        type="time"
                        className="input-field"
                        value={schedulerStartTime}
                        onChange={(e) => setSchedulerStartTime(e.target.value)}
                      />
                    </div>
                    <div className="flex flex-col gap-1 flex-1">
                      <label className="text-xs font-semibold text-2 uppercase">Stop Time</label>
                      <input
                        type="time"
                        className="input-field"
                        value={schedulerStopTime}
                        onChange={(e) => setSchedulerStopTime(e.target.value)}
                      />
                    </div>
                  </div>
                )}
              </div>
            </>
          )}

          {activeTab === 'Queue' && (
            <>
              {/* Queue toolbar */}
              <div
                className="card-subtle flex items-center justify-between"
                style={{ padding: 'var(--sp-2) var(--sp-3)' }}
              >
                <span
                  style={{
                    fontSize: 'var(--text-sm-size)',
                    fontWeight: 600,
                    color: 'var(--color-text-2)',
                  }}
                >
                  Queue ({queuedDownloads.length})
                </span>
                <div className="flex gap-1">
                  {[
                    { icon: Play, action: 'resume', title: 'Resume' },
                    { icon: Square, action: 'pause', title: 'Pause' },
                    { icon: ArrowUp, action: 'high', title: 'Higher priority' },
                    { icon: ArrowDown, action: 'low', title: 'Lower priority' },
                  ].map(({ icon: Icon, action, title }) => (
                    <button
                      key={action}
                      disabled={!selectedId}
                      title={title}
                      onClick={() => {
                        if (!selectedId) return;
                        // eslint-disable-next-line @typescript-eslint/no-explicit-any
                        if (action === 'resume' || action === 'pause')
                          handleAction(selectedId, action as any);
                        // eslint-disable-next-line @typescript-eslint/no-explicit-any
                        else handlePriority(selectedId, action as any);
                      }}
                      className="btn-icon"
                      style={{ width: 28, height: 28 }}
                    >
                      <Icon size={14} />
                    </button>
                  ))}
                </div>
              </div>

              {/* Queue list */}
              <div className="flex flex-col gap-1 flex-1 overflow-y-auto">
                {queuedDownloads.length === 0 ? (
                  <p
                    style={{
                      textAlign: 'center',
                      fontSize: 'var(--text-sm-size)',
                      color: 'var(--color-text-4)',
                      padding: 'var(--sp-4)',
                    }}
                  >
                    No items in the queue.
                  </p>
                ) : (
                  // eslint-disable-next-line @typescript-eslint/no-explicit-any
                  queuedDownloads.map((d: any) => (
                    <div
                      key={d.id}
                      onClick={() => setSelectedId(d.id)}
                      className={cn(
                        'card-subtle flex items-center justify-between',
                        selectedId === d.id && 'selected',
                      )}
                      style={{ padding: 'var(--sp-2) var(--sp-3)', cursor: 'default', gap: 8 }}
                    >
                      <span
                        className="truncate"
                        style={{
                          fontSize: 'var(--text-sm-size)',
                          fontWeight: 600,
                          color: selectedId === d.id ? 'var(--color-brand)' : 'var(--color-text-1)',
                        }}
                      >
                        {d.filename || d.file_name || d.url}
                      </span>
                      <div className="flex items-center gap-2 shrink-0">
                        <span className="tag tag-neutral">{d.status}</span>
                        {d.priority === 'high' && (
                          <ArrowUp size={12} style={{ color: 'var(--color-brand)' }} />
                        )}
                        {d.priority === 'low' && (
                          <ArrowDown size={12} style={{ color: 'var(--color-text-4)' }} />
                        )}
                      </div>
                    </div>
                  ))
                )}
              </div>
            </>
          )}
        </div>

        {/* Footer */}
        <div className="dialog-footer">
          <button className="btn-secondary" onClick={onClose}>
            Close
          </button>
          <button className="btn-primary" onClick={handleApply}>
            Apply
          </button>
        </div>
      </div>
    </div>
  );
}
