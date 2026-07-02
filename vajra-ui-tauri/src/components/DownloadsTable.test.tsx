import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import DownloadsTable from './DownloadsTable';

const mockItems = [
  {
    id: '1',
    filename: 'test_file.zip',
    status: 'downloading',
    total_bytes: 1000,
    bytes_done: 500,
    speed_bps: 100,
    progress_pct: 50,
    eta_seconds: 5,
    created_at: 1620000000,
  },
  {
    id: '2',
    filename: 'video.mp4',
    status: 'completed',
    total_bytes: 2000,
    bytes_done: 2000,
    speed_bps: 0,
    progress_pct: 100,
    eta_seconds: null,
    created_at: 1620000100,
  },
];

describe('DownloadsTable', () => {
  it('renders downloads correctly', () => {
    render(
      <DownloadsTable
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        items={mockItems as any}
        activeCategory="All Downloads"
        onSelect={vi.fn()}
        onSelectAll={vi.fn()}
        onDoubleClick={vi.fn()}
        onAction={vi.fn()}
        selectedIds={new Set()}
      />,
    );

    expect(screen.getByText('test_file.zip')).toBeInTheDocument();
    expect(screen.getByText('video.mp4')).toBeInTheDocument();
  });

  it('filters items based on active category', () => {
    render(
      <DownloadsTable
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        items={mockItems as any}
        activeCategory="Completed"
        onSelect={vi.fn()}
        onSelectAll={vi.fn()}
        onDoubleClick={vi.fn()}
        onAction={vi.fn()}
        selectedIds={new Set()}
      />,
    );

    expect(screen.queryByText('test_file.zip')).not.toBeInTheDocument();
    expect(screen.getByText('video.mp4')).toBeInTheDocument();
  });

  it('triggers onSelect when a row is clicked', () => {
    const handleSelect = vi.fn();
    render(
      <DownloadsTable
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        items={mockItems as any}
        activeCategory="All Downloads"
        onSelect={handleSelect}
        onSelectAll={vi.fn()}
        onDoubleClick={vi.fn()}
        onAction={vi.fn()}
        selectedIds={new Set()}
      />,
    );

    const row = screen.getByText('test_file.zip').closest('tr');
    fireEvent.click(row!);

    expect(handleSelect).toHaveBeenCalledWith('1', false, false);
  });
});
