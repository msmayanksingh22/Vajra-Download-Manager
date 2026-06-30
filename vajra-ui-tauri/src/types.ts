export type DownloadStatus = 'downloading' | 'connecting' | 'completed' | 'paused' | 'failed' | 'idle' | 'verifying';
export type Priority = 'high' | 'normal' | 'low';
export type QueueType = 'Standard' | 'Synchronization';

export interface SegmentInfo {
  id: number;
  start: number;
  end: number;
  bytes_done: number;
  allocated_bytes: number;
  status: DownloadStatus;
  thread_index: number;
  speed_bps: number | null;
  retry_count: number;
  error_message: string | null;
}

export interface HashResult {
  matched: boolean;
  algorithm: string;
  computed: string;
}

export interface DownloadInfo {
  id: string;
  status: DownloadStatus;
  url: string;
  output_path: string | null;
  filename: string;
  total_bytes: number | null;
  bytes_done: number;
  speed_bps: number;
  eta_seconds: number | null;
  progress_pct: number;
  connections_active: number;
  segments: SegmentInfo[];
  hash_result: HashResult | null;
  created_at: number;
  started_at: number | null;
  completed_at: number | null;
  error: string | null;
  speed_history: number[];
  priority: Priority;
  queue_type: string;
  sync_interval_secs: number;
  tags?: string[];
  speed_limit_bps?: number | null;
  resume_supported?: boolean;
}

export interface AddDownloadRequest {
  url: string;
  output_dir?: string;
  filename?: string;
  headers?: Record<string, string>;
  expected_hash?: string;
  max_connections?: number;
  speed_limit_bps?: number;
  priority?: Priority;
  schedule_at?: number;
  use_ytdlp?: boolean;
  use_http3?: boolean;
  ytdlp_format?: string;
  ytdlp_subtitles?: boolean;
  ytdlp_playlist?: boolean;
  auto_extract?: boolean;
  post_processing_script?: string;
  queue_type?: QueueType;
  sync_interval_secs?: number;
  tags?: string[];
}

export type DownloadAction = 'pause' | 'resume' | 'cancel' | 'retry';

export interface PatchDownloadRequest {
  action?: DownloadAction;
  speed_limit_bps?: number | null;
  max_connections?: number;
  priority?: Priority;
  url?: string;
  tags?: string[];
  filename?: string;
}

export interface InspectRequest {
  url: string;
  headers?: Record<string, string>;
}

export interface InspectResponse {
  effective_url: string;
  filename: string | null;
  content_type: string | null;
  total_bytes: number | null;
  accepts_ranges: boolean;
  ytdlp_supported: boolean;
}

export interface DownloadList {
  items: DownloadInfo[];
  total: number;
  limit: number;
  offset: number;
}

export interface StatsResponse {
  active_count: number;
  queued_count: number;
  paused_count: number;
  complete_today: number;
  failed_today: number;
  aggregate_speed_bps: number;
  aggregate_limit_bps: number | null;
  total_downloaded_bytes: number;
  daemon_uptime_seconds: number;
  speed_history?: number[];
}

export interface VaultCredentialResponse {
  id: string;
  domain: string;
  username: string;
  created_at: number;
}

export interface AddVaultCredentialRequest {
  domain: string;
  username: string;
  password?: string;
}

export interface CategoryRule {
  extensions: string[];
  output_dir: string;
  label: string;
}

export interface ProxyConfig {
  url: string | null;
  username: string | null;
  password: string | null;
  use_system_proxy: boolean;
  route_via_tor?: boolean;
}

export type PostQueueAction = 'none' | 'exit_app' | 'sleep' | 'hibernate' | 'shutdown';
export type DuplicateAction = 'auto_rename' | 'overwrite' | 'prompt';

export interface DaemonConfig {
  default_output_dir: string;
  max_concurrent_downloads: number;
  global_speed_limit_bps: number | null;
  default_max_connections: number;
  listen_port: number;
  api_token: string | null;
  auto_start_on_login: boolean;
  notifications_enabled: boolean;
  sound_on_complete: boolean;
  default_use_http3: boolean;
  captcha_api_key?: string | null;
  category_rules: CategoryRule[];
  proxy: ProxyConfig;
  post_queue_action: PostQueueAction;
  scheduler_enabled: boolean;
  scheduler_start_time: string | null;
  scheduler_stop_time: string | null;
  duplicate_action: DuplicateAction;
  av_scan_path: string | null;
  av_scan_args: string[];
  temp_dir: string | null;
  fap_enabled?: boolean;
  fap_quota_mb?: number;
  fap_window_hours?: number;
  max_retries?: number;
  dns_over_https?: boolean;
}

export type DaemonEvent =
  | { event: "progress"; download_id: string; url: string; filename: string; total_bytes: number | null; downloaded_bytes: number; speed_bps: number; eta_seconds: number | null; status: DownloadStatus; resume_supported: boolean; segments: SegmentInfo[]; error: string | null; }
  | { event: "state_change"; id: string; status: DownloadStatus; output_path: string | null; error: string | null; }
  | { event: "hash_result"; id: string; matched: boolean; algorithm: string; computed: string; }
  | { event: "added"; id: string; url: string; filename: string; }
  | { event: "removed"; id: string; }
  | { event: "intercepted"; url: string; filename: string; }
;
