import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import AddUrlWindow from './windows/AddUrlWindow';
import DownloadCompleteWindow from './windows/DownloadCompleteWindow';
import DownloadFailedWindow from './windows/DownloadFailedWindow';
import ProgressWindow from './windows/ProgressWindow';
import { ThemeProvider } from './ThemeContext';
import './index.css';
import './i18n';

import { ErrorBoundary } from './components/ErrorBoundary';

const params = new URLSearchParams(window.location.search);
const windowType = params.get('window');

let RootComponent = <App />;
if (windowType === 'addUrl') {
  RootComponent = (
    <AddUrlWindow
      initialUrl={params.get('url') || ''}
      initialFilename={params.get('filename') || ''}
    />
  );
} else if (windowType === 'downloadComplete') {
  RootComponent = <DownloadCompleteWindow downloadId={params.get('id')} />;
} else if (windowType === 'downloadFailed') {
  RootComponent = <DownloadFailedWindow downloadId={params.get('id') || ''} />;
} else if (windowType === 'progress') {
  RootComponent = <ProgressWindow downloadId={params.get('id') || ''} />;
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <ErrorBoundary>
    <ThemeProvider>{RootComponent}</ThemeProvider>
  </ErrorBoundary>,
);

// Prevent native web behaviors to make the app feel like a "hard program"
document.addEventListener('contextmenu', (e) => {
  // Allow right click ONLY on input fields
  const target = e.target as HTMLElement;
  if (target.tagName !== 'INPUT' && target.tagName !== 'TEXTAREA') {
    e.preventDefault();
  }
});

document.addEventListener('dragstart', (e) => {
  if ((e.target as HTMLElement).closest('[draggable="true"]')) {
    return;
  }
  e.preventDefault();
});
document.addEventListener('drop', (e) => e.preventDefault());
document.addEventListener('dragover', (e) => e.preventDefault());
