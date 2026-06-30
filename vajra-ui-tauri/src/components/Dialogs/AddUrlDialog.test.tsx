import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import AddUrlDialog from './AddUrlDialog';
import { api } from '../../api';

vi.mock('../../api', () => ({
  api: {
    config: vi.fn(),
    inspect: vi.fn(),
  },
  fmtBytes: (b: number) => `${b} B`,
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}));

describe('AddUrlDialog', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (api.config as any).mockResolvedValue({ default_max_connections: 8 });
    localStorage.clear();
  });

  it('renders correctly and sets initial URL', () => {
    render(<AddUrlDialog initialUrl="http://example.com/file.zip" onOk={vi.fn()} onClose={vi.fn()} />);
    const urlInput = screen.getByPlaceholderText('https://') as HTMLInputElement;
    expect(urlInput.value).toBe('http://example.com/file.zip');
  });

  it('triggers inspect when URL is entered', async () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (api.inspect as any).mockResolvedValue({ filename: 'test.zip', size: 1024, type: 'application/zip', support_ranges: true });
    
    render(<AddUrlDialog onOk={vi.fn()} onClose={vi.fn()} />);
    const urlInput = screen.getByPlaceholderText('https://') as HTMLInputElement;
    
    fireEvent.change(urlInput, { target: { value: 'http://example.com/test.zip' } });
    
    await waitFor(() => {
      expect(api.inspect).toHaveBeenCalledWith('http://example.com/test.zip');
    }, { timeout: 1500 }); // debounce is 800ms
  });

  it('auto-fills credentials if domain matches local storage', async () => {
    localStorage.setItem('vajra_site_logins', JSON.stringify([{ host: 'example.com', user: 'admin', pass: 'secret' }]));
    
    render(<AddUrlDialog initialUrl="http://example.com/secret.zip" onOk={vi.fn()} onClose={vi.fn()} />);
    
    // Use Auth checkbox should be checked
    const useAuthCheckbox = screen.getByLabelText(/Basic Authentication/i) as HTMLInputElement;
    expect(useAuthCheckbox.checked).toBe(true);
    
    const usernameInput = screen.getByPlaceholderText('Username') as HTMLInputElement;
    const passwordInput = screen.getByPlaceholderText('Password') as HTMLInputElement;
    
    expect(usernameInput.value).toBe('admin');
    expect(passwordInput.value).toBe('secret');
  });

  it('calls onOk with correct payload', () => {
    const onOk = vi.fn();
    render(<AddUrlDialog initialUrl="http://example.com/file.zip" onOk={onOk} onClose={vi.fn()} />);
    
    // We expect the button to have text 'Download'
    const downloadBtn = screen.getByRole('button', { name: /Download/i });
    fireEvent.click(downloadBtn);
    
    expect(onOk).toHaveBeenCalledWith(expect.objectContaining({
      url: 'http://example.com/file.zip',
      max_connections: 8,
      speed_limit_bps: 0,
      auto_extract: false,
      _startLater: false,
      queue_type: 'Standard',
      sync_interval_secs: 0
    }));
  });
});
