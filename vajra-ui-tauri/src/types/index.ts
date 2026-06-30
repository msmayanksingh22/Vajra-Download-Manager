// Type definitions for Vajra UI

export interface Download {
  id: string;
  url: string;
  filename: string;
  status: DownloadStatus;
  bytes_done: number;
  total_bytes: number | null;
  speed_bps: number;
  eta_seconds: number | null;
  progress_pct: number;
  segments?: SegmentInfo[];
  output_path?: string;
  error?: string;
  added_at?: string;
  category?: string;
}

export type DownloadStatus =
  | 'queued'
  | 'connecting'
  | 'downloading'
  | 'paused'
  | 'completed'
  | 'failed'
  | 'error'
  | 'verifying';

export interface SegmentInfo {
  id: number;
  bytes_done: number;
  allocated_bytes: number;
  speed_bps: number;
  status: string;
}

export interface DaemonConfig {
  max_concurrent?: number;
  global_speed_limit_bps?: number | null;
  default_output_dir?: string;
  enable_clipboard_monitor?: boolean;
  scheduler_enabled?: boolean;
  scheduler_start_time?: string;
  scheduler_stop_time?: string;
  auto_extract?: boolean;
  post_queue_action?: string;
}

export interface SSEEvent {
  event: string;
  id?: string;
  download_id?: string;
  status?: string;
  downloaded_bytes?: number;
  bytes_done?: number;
  total_bytes?: number;
  speed_bps?: number;
  eta_seconds?: number | null;
  segments?: SegmentInfo[];
  output_path?: string;
  error?: string;
  url?: string;
  filename?: string;
}
