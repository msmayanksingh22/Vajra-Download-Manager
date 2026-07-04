import React, { useEffect, useState } from 'react';
import { api, fmtSpeed } from '../api';
import { StatsResponse } from '../types';
import { FolderDown, ArrowRightLeft, Bolt, Database, Activity, Timer } from 'lucide-react';

interface DashboardProps {
  onNavigate?: (category: string) => void;
}

/* Skeleton card shown during initial load */
const SkeletonCard = () => (
  <div className="card-subtle kpi-card">
    <div className="skeleton" style={{ width: 80, height: 12, borderRadius: 'var(--radius-sm)' }} />
    <div className="skeleton" style={{ width: 56, height: 28, borderRadius: 'var(--radius-sm)' }} />
    <div className="skeleton" style={{ width: 40, height: 10, borderRadius: 'var(--radius-sm)' }} />
  </div>
);

const SkeletonChart = () => (
  <div className="card-subtle dashboard-chart-card">
    <div
      className="skeleton"
      style={{ width: 160, height: 14, borderRadius: 'var(--radius-sm)' }}
    />
    <div className="skeleton" style={{ flex: 1, borderRadius: 'var(--radius-md)' }} />
  </div>
);

interface KpiCard {
  label: string;
  value: string;
  sub?: string;
  icon: React.ReactNode;
  accent: string;
}

export default function Dashboard({ onNavigate }: DashboardProps) {
  const [stats, setStats] = useState<StatsResponse | null>(null);
  const [hoverIndex, setHoverIndex] = useState<number | null>(null);
  const [hoverPos, setHoverPos] = useState<{ x: number; y: number; val: number } | null>(null);

  useEffect(() => {
    const fetchStats = async () => {
      try {
        const s = await api.stats();
        setStats(s);
      } catch (e) {
        console.error('Failed to fetch stats', e);
      }
    };
    fetchStats();
    const int = setInterval(fetchStats, 1000);
    return () => clearInterval(int);
  }, []);

  const openAddUrl = () => document.dispatchEvent(new CustomEvent('vajra:open-add-url'));
  const goToDownloads = () => onNavigate?.('All Downloads');

  /* ── Loading skeleton ── */
  if (!stats) {
    return (
      <div className="dashboard-root">
        {/* Header skeleton */}
        <div className="dashboard-header">
          <div
            className="skeleton"
            style={{ width: 180, height: 22, borderRadius: 'var(--radius-sm)' }}
          />
          <div className="flex" style={{ gap: 8 }}>
            <div
              className="skeleton"
              style={{ width: 130, height: 30, borderRadius: 'var(--radius-md)' }}
            />
            <div
              className="skeleton"
              style={{ width: 140, height: 30, borderRadius: 'var(--radius-md)' }}
            />
          </div>
        </div>
        {/* KPI skeletons */}
        <div className="dashboard-kpi-grid">
          <SkeletonCard />
          <SkeletonCard />
          <SkeletonCard />
          <SkeletonCard />
        </div>
        {/* Chart skeleton */}
        <SkeletonChart />
      </div>
    );
  }

  const speedHistory = stats.speed_history || [];
  const maxSpeed = Math.max(...speedHistory, 1024 * 1024); // at least 1MB/s scale

  const handleMouseMove = (e: React.MouseEvent<SVGSVGElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;
    const pct = Math.max(0, Math.min(1, mouseX / rect.width));
    const idx = Math.min(speedHistory.length - 1, Math.round(pct * (speedHistory.length - 1)));
    if (idx >= 0 && idx < speedHistory.length) {
      const x = (idx / Math.max(1, speedHistory.length - 1)) * rect.width;
      const speedVal = speedHistory[idx];
      const y = rect.height - 25 - (speedVal / maxSpeed) * (rect.height - 45);
      setHoverIndex(idx);
      setHoverPos({ x, y, val: speedVal });
    }
  };

  const handleMouseLeave = () => {
    setHoverIndex(null);
    setHoverPos(null);
  };

  // Generate SVG paths
  const width = 500;
  const height = 150;
  const chartHeight = height - 25; // space for labels

  const points = speedHistory.map((speed, i) => {
    const x = (i / Math.max(1, speedHistory.length - 1)) * width;
    const y = chartHeight - (speed / maxSpeed) * (chartHeight - 20);
    return { x, y };
  });

  const linePath =
    points.length > 0
      ? `M ${points[0].x} ${points[0].y} ` +
        points
          .slice(1)
          .map((p) => `L ${p.x} ${p.y}`)
          .join(' ')
      : '';

  const areaPath =
    points.length > 0
      ? `${linePath} L ${points[points.length - 1].x} ${chartHeight} L ${points[0].x} ${chartHeight} Z`
      : '';

  const totalGb = (stats.total_downloaded_bytes / (1024 * 1024 * 1024)).toFixed(2);
  const currentSpeedMbs = (stats.aggregate_speed_bps / (1024 * 1024)).toFixed(2);

  const kpiCards: KpiCard[] = [
    {
      label: 'Active Downloads',
      value: String(stats.active_count),
      sub: 'in progress',
      icon: <Activity size={18} />,
      accent: 'var(--color-brand)',
    },
    {
      label: 'Queued',
      value: String(stats.queued_count),
      sub: 'waiting',
      icon: <Timer size={18} />,
      accent: 'var(--color-warning)',
    },
    {
      label: 'Completed Today',
      value: String(stats.complete_today ?? 0),
      sub: 'finished',
      icon: <Bolt size={18} />,
      accent: 'var(--color-success)',
    },
    {
      label: 'Failed Today',
      value: String(stats.failed_today ?? 0),
      sub: 'errors',
      icon: <ArrowRightLeft size={18} />,
      accent: 'var(--color-error)',
    },
    {
      label: 'Total Downloaded',
      value: `${totalGb} GB`,
      sub: 'all time',
      icon: <Database size={18} />,
      accent: 'var(--color-success)',
    },
    {
      label: 'Current Speed',
      value: `${currentSpeedMbs} MB/s`,
      sub: fmtSpeed(stats.aggregate_speed_bps),
      icon: <Bolt size={18} />,
      accent: 'var(--color-brand)',
    },
  ];

  return (
    <div className="dashboard-root">
      {/* ── Header ── */}
      <div className="dashboard-header">
        <div>
          <h2 className="dashboard-title">Dashboard</h2>
          <p className="dashboard-subtitle">Live download statistics</p>
        </div>
        <div className="flex" style={{ gap: 'var(--sp-2)' }}>
          <button
            className="btn-primary flex items-center gap-2"
            onClick={openAddUrl}
            title="Open Add Download dialog"
          >
            <FolderDown size={14} /> Add Download
          </button>
          <button
            className="btn-secondary flex items-center gap-2"
            onClick={goToDownloads}
            title="Go to Downloads list"
          >
            <ArrowRightLeft size={14} /> Go to Downloads
          </button>
        </div>
      </div>

      {/* ── KPI Cards ── */}
      <div className="dashboard-kpi-grid">
        {kpiCards.map(({ label, value, sub, icon, accent }) => (
          <div key={label} className="card-subtle kpi-card">
            <div className="kpi-card-header">
              <span className="kpi-label">{label}</span>
              <span style={{ color: accent, opacity: 0.8 }}>{icon}</span>
            </div>
            <span className="kpi-value">{value}</span>
            {sub && <span className="kpi-sub">{sub}</span>}
          </div>
        ))}
      </div>

      {/* ── Speed Chart ── */}
      <div
        className="card-subtle dashboard-chart-card"
        style={{ height: 280, display: 'flex', flexDirection: 'column' }}
      >
        <div className="dashboard-chart-header">
          <h3 className="dashboard-chart-title">Global Speed History</h3>
          <span className="dashboard-chart-meta">
            {hoverPos
              ? `Speed: ${fmtSpeed(hoverPos.val)}`
              : `Last ${speedHistory.length} samples (Max: ${fmtSpeed(maxSpeed)})`}
          </span>
        </div>
        <div style={{ flex: 1, position: 'relative', marginTop: 12 }} className="select-none">
          {speedHistory.length > 0 ? (
            <div style={{ width: '100%', height: '100%', position: 'relative' }}>
              <svg
                width="100%"
                height="100%"
                viewBox={`0 0 ${width} ${height}`}
                preserveAspectRatio="none"
                onMouseMove={handleMouseMove}
                onMouseLeave={handleMouseLeave}
                style={{ overflow: 'visible', cursor: 'crosshair' }}
              >
                <defs>
                  <linearGradient id="dashGrad" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor="var(--color-brand)" stopOpacity={0.25} />
                    <stop offset="100%" stopColor="var(--color-brand)" stopOpacity={0.0} />
                  </linearGradient>
                </defs>

                {/* Gridlines */}
                <line
                  x1="0"
                  y1={chartHeight}
                  x2={width}
                  y2={chartHeight}
                  stroke="var(--color-border-subtle)"
                  strokeWidth={1}
                />
                <line
                  x1="0"
                  y1={chartHeight / 2}
                  x2={width}
                  y2={chartHeight / 2}
                  stroke="var(--color-border-subtle)"
                  strokeWidth={1}
                  strokeDasharray="3 3"
                />
                <line
                  x1="0"
                  y1={15}
                  x2={width}
                  y2={15}
                  stroke="var(--color-border-subtle)"
                  strokeWidth={1}
                  strokeDasharray="3 3"
                />

                {/* Area & Line */}
                {areaPath && <path d={areaPath} fill="url(#dashGrad)" />}
                {linePath && (
                  <path d={linePath} fill="none" stroke="var(--color-brand)" strokeWidth={1.5} />
                )}

                {/* Y-Axis Labels */}
                <text x={4} y={12} fill="var(--color-text-4)" fontSize={9} fontWeight={600}>
                  {fmtSpeed(maxSpeed)}
                </text>
                <text
                  x={4}
                  y={chartHeight / 2 + 3}
                  fill="var(--color-text-4)"
                  fontSize={9}
                  fontWeight={600}
                >
                  {fmtSpeed(maxSpeed / 2)}
                </text>
                <text
                  x={4}
                  y={chartHeight - 4}
                  fill="var(--color-text-4)"
                  fontSize={9}
                  fontWeight={600}
                >
                  0 KB/s
                </text>

                {/* Interactive Tooltip Cursor */}
                {hoverPos && (
                  <>
                    <line
                      x1={(hoverIndex! / Math.max(1, speedHistory.length - 1)) * width}
                      y1={0}
                      x2={(hoverIndex! / Math.max(1, speedHistory.length - 1)) * width}
                      y2={chartHeight}
                      stroke="var(--color-brand)"
                      strokeWidth={1}
                      strokeDasharray="2 2"
                    />
                    <circle
                      cx={(hoverIndex! / Math.max(1, speedHistory.length - 1)) * width}
                      cy={hoverPos.y}
                      r={4}
                      fill="var(--color-brand)"
                      stroke="var(--color-surface)"
                      strokeWidth={1.5}
                    />
                  </>
                )}
              </svg>

              {/* Floating Tooltip HTML Overlay */}
              {hoverPos && (
                <div
                  style={{
                    position: 'absolute',
                    left: hoverPos.x + 12,
                    top: hoverPos.y - 30,
                    backgroundColor: 'var(--color-surface-elevated)',
                    border: '1px solid var(--color-border)',
                    borderRadius: 'var(--radius-md)',
                    padding: '4px 8px',
                    fontSize: 10,
                    fontWeight: 700,
                    color: 'var(--color-text-1)',
                    boxShadow: 'var(--shadow-md)',
                    pointerEvents: 'none',
                    zIndex: 10,
                    transform: hoverPos.x > width * 0.7 ? 'translateX(-110%)' : 'none',
                    transition: 'left 0.05s ease, top 0.05s ease',
                  }}
                >
                  {fmtSpeed(hoverPos.val)}
                </div>
              )}
            </div>
          ) : (
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                height: '100%',
                color: 'var(--color-text-4)',
                fontSize: 'var(--text-sm-size)',
              }}
            >
              No speed data available
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
