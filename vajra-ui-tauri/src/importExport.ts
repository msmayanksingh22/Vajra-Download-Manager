import { DownloadInfo, AddDownloadRequest } from './types';
import { api } from './api';

export function exportDownloadsJson(downloads: DownloadInfo[]) {
  const json = JSON.stringify(downloads, null, 2);
  const blob = new Blob([json], { type: 'application/json' });
  triggerDownload(blob, 'vajra-downloads.json');
}

export function exportDownloadsCsv(downloads: DownloadInfo[]) {
  const headers = ['URL', 'Filename', 'Tags', 'Output Path'];
  const rows = downloads.map((d) => [
    d.url,
    d.filename || '',
    (d.tags || []).join(';'),
    d.output_path || '',
  ]);

  const csvContent = [
    headers.join(','),
    ...rows.map((row) => row.map((v) => `"${(v || '').replace(/"/g, '""')}"`).join(',')),
  ].join('\n');

  const blob = new Blob([csvContent], { type: 'text/csv' });
  triggerDownload(blob, 'vajra-downloads.csv');
}

function triggerDownload(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

export async function importDownloads(file: File) {
  const text = await file.text();
  const reqs: AddDownloadRequest[] = [];

  if (file.name.endsWith('.json')) {
    try {
      const data = JSON.parse(text);
      if (Array.isArray(data)) {
        for (const item of data) {
          if (item.url) {
            reqs.push({
              url: item.url,
              filename: item.filename,
              output_dir: item.output_path,
              tags: item.tags,
            });
          }
        }
      }
    } catch (e) {
      console.error('Failed to parse JSON for import', e);
      throw new Error('Invalid JSON format');
    }
  } else if (file.name.endsWith('.csv')) {
    const lines = text.split('\n').filter((l) => l.trim().length > 0);
    if (lines.length > 1) {
      const headers = lines[0].split(',').map((h) => h.trim().replace(/^"|"$/g, ''));
      const urlIdx = headers.findIndex((h) => h.toLowerCase() === 'url');
      const fileIdx = headers.findIndex((h) => h.toLowerCase() === 'filename');
      const tagsIdx = headers.findIndex((h) => h.toLowerCase() === 'tags');
      const outIdx = headers.findIndex((h) => h.toLowerCase() === 'output path');

      for (let i = 1; i < lines.length; i++) {
        // Simple CSV parse line handling basic quotes
        const match = lines[i].match(/(".*?"|[^",\s]+)(?=\s*,|\s*$)/g);
        if (!match) continue;
        const row = match.map((v) => v.replace(/^"|"$/g, '').replace(/""/g, '"'));
        const url = urlIdx >= 0 ? row[urlIdx] : row[0];
        if (url && url.startsWith('http')) {
          reqs.push({
            url,
            filename: fileIdx >= 0 ? row[fileIdx] : undefined,
            tags:
              tagsIdx >= 0 && row[tagsIdx]
                ? row[tagsIdx].split(';').map((t) => t.trim())
                : undefined,
            output_dir: outIdx >= 0 ? row[outIdx] : undefined,
          });
        }
      }
    }
  } else {
    throw new Error('Unsupported file format');
  }

  // Sequentially add downloads
  let added = 0;
  for (const req of reqs) {
    try {
      await api.add(req);
      added++;
    } catch (e) {
      console.error('Failed to add import', req.url, e);
    }
  }
  return added;
}
