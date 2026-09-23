using System.IO;
using System.Windows;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;
using Vajra.Windows.Models;
using Vajra.Windows.ViewModels;
using Vajra.Windows.Views;
using Xunit;

namespace Vajra.Windows.Tests;

public class VisualInspectionTests
{
    private void RunInSta(Action action)
    {
        Exception? ex = null;
        var thread = new Thread(() =>
        {
            try
            {
                action();
            }
            catch (Exception e)
            {
                ex = e;
            }
        });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        thread.Join();

        if (ex != null)
        {
            throw new AggregateException("STA thread failed", ex);
        }
    }

    [Fact]
    public void RenderAllViews_ToVisualArtifacts_ForInspection()
    {
        RunInSta(() =>
        {
            string artifactDir = @"C:\Users\msmay\.gemini\antigravity-ide\brain\3c375e1a-683a-4f96-bd4e-3b5bab6a7a23";
            if (!Directory.Exists(artifactDir))
            {
                Directory.CreateDirectory(artifactDir);
            }

            var daemonClient = new MockDaemonClientForSync();
            var sseService = new MockSseServiceForSync();
            var tokenService = new MockTokenService();
            var systemService = new MockSystemInteractionService();

            // 1. Render Empty State Window
            var vmEmpty = new MainViewModel(daemonClient, sseService, tokenService, systemService)
            {
                ConnectionState = ConnectionState.Connected
            };

            var windowEmpty = new MainWindow(vmEmpty, daemonClient, systemService)
            {
                Width = 960,
                Height = 600
            };

            var helperEmpty = new WindowInteropHelper(windowEmpty);
            helperEmpty.EnsureHandle();
            windowEmpty.Show();
            Dispatcher.CurrentDispatcher.Invoke(() => { }, DispatcherPriority.Render);
            windowEmpty.UpdateLayout();

            int w = Math.Max(1, (int)windowEmpty.ActualWidth);
            int h = Math.Max(1, (int)windowEmpty.ActualHeight);
            var rtbEmpty = new RenderTargetBitmap(w, h, 96, 96, PixelFormats.Pbgra32);
            rtbEmpty.Render(windowEmpty);

            string emptyPath = Path.Combine(artifactDir, "wpf_empty_state.png");
            using (var stream = File.Create(emptyPath))
            {
                var encoder = new PngBitmapEncoder();
                encoder.Frames.Add(BitmapFrame.Create(rtbEmpty));
                encoder.Save(stream);
            }
            windowEmpty.Close();

            // 2. Render Populated State Window
            var vmPop = new MainViewModel(daemonClient, sseService, tokenService, systemService)
            {
                ConnectionState = ConnectionState.Connected
            };

            vmPop.Downloads.Add(new DownloadItem
            {
                Id = "dl-1",
                Filename = "Pritam.and.Pedro.S01.720p.mkv",
                Url = "https://cdn.example.com/shows/Pritam.and.Pedro.S01.720p.mkv",
                Status = DownloadStatus.Downloading,
                TotalBytes = 1073741824, // 1 GB
                BytesDone = 482344960, // 460 MB
                ProgressPct = 44.9,
                SpeedBps = 12 * 1024 * 1024, // 12 MB/s
                EtaSeconds = 49,
                CreatedAt = DateTime.Now.AddMinutes(-12)
            });

            vmPop.Downloads.Add(new DownloadItem
            {
                Id = "dl-2",
                Filename = "ubuntu-24.04-desktop-amd64.iso",
                Url = "https://releases.ubuntu.com/24.04/ubuntu-24.04-desktop-amd64.iso",
                Status = DownloadStatus.Completed,
                TotalBytes = 6120341504, // 5.7 GB
                BytesDone = 6120341504,
                ProgressPct = 100.0,
                SpeedBps = 0,
                CreatedAt = DateTime.Now.AddHours(-2)
            });

            vmPop.Downloads.Add(new DownloadItem
            {
                Id = "dl-3",
                Filename = "dataset_large_archive.zip",
                Url = "https://storage.example.org/archives/dataset_large_archive.zip",
                Status = DownloadStatus.Paused,
                TotalBytes = 2147483648, // 2 GB
                BytesDone = 858993459, // 819 MB
                ProgressPct = 40.0,
                SpeedBps = 0,
                CreatedAt = DateTime.Now.AddMinutes(-45)
            });

            vmPop.Downloads.Add(new DownloadItem
            {
                Id = "dl-4",
                Filename = "nvidia_driver_package.exe",
                Url = "https://us.download.nvidia.com/Windows/driver.exe",
                Status = DownloadStatus.Failed,
                TotalBytes = 734003200,
                BytesDone = 10485760,
                ProgressPct = 1.4,
                Error = "Failed to connect to host: connection refused by remote server",
                CreatedAt = DateTime.Now.AddMinutes(-5)
            });

            vmPop.SelectedDownload = vmPop.Downloads[0];

            var windowPop = new MainWindow(vmPop, daemonClient, systemService)
            {
                Width = 960,
                Height = 600
            };

            var helperPop = new WindowInteropHelper(windowPop);
            helperPop.EnsureHandle();
            windowPop.Show();
            Dispatcher.CurrentDispatcher.Invoke(() => { }, DispatcherPriority.Render);
            windowPop.UpdateLayout();

            int pw = Math.Max(1, (int)windowPop.ActualWidth);
            int ph = Math.Max(1, (int)windowPop.ActualHeight);
            var rtbPop = new RenderTargetBitmap(pw, ph, 96, 96, PixelFormats.Pbgra32);
            rtbPop.Render(windowPop);

            string popPath = Path.Combine(artifactDir, "wpf_populated_view.png");
            using (var stream = File.Create(popPath))
            {
                var encoder = new PngBitmapEncoder();
                encoder.Frames.Add(BitmapFrame.Create(rtbPop));
                encoder.Save(stream);
            }
            windowPop.Close();

            // 3. Render Delete Confirmation Dialog
            var delDialog = new DeleteConfirmationDialog("Pritam.and.Pedro.S01.720p.mkv")
            {
                Width = 490,
                Height = 220
            };
            var helperDel = new WindowInteropHelper(delDialog);
            helperDel.EnsureHandle();
            delDialog.Show();
            Dispatcher.CurrentDispatcher.Invoke(() => { }, DispatcherPriority.Render);
            delDialog.UpdateLayout();

            int dw = Math.Max(1, (int)delDialog.ActualWidth);
            int dh = Math.Max(1, (int)delDialog.ActualHeight);
            var rtbDel = new RenderTargetBitmap(dw, dh, 96, 96, PixelFormats.Pbgra32);
            rtbDel.Render(delDialog);

            string delPath = Path.Combine(artifactDir, "wpf_delete_dialog.png");
            using (var stream = File.Create(delPath))
            {
                var encoder = new PngBitmapEncoder();
                encoder.Frames.Add(BitmapFrame.Create(rtbDel));
                encoder.Save(stream);
            }
            delDialog.Close();

            // 4. Render Add Download Dialog
            var addVm = new AddDownloadViewModel(daemonClient, systemService)
            {
                Url = "https://example.com/files/archive.tar.gz",
                Filename = "archive.tar.gz"
            };
            var addDialog = new AddDownloadDialog(addVm)
            {
                Width = 500,
                Height = 240
            };
            var helperAdd = new WindowInteropHelper(addDialog);
            helperAdd.EnsureHandle();
            addDialog.Show();
            Dispatcher.CurrentDispatcher.Invoke(() => { }, DispatcherPriority.Render);
            addDialog.UpdateLayout();

            int aw = Math.Max(1, (int)addDialog.ActualWidth);
            int ah = Math.Max(1, (int)addDialog.ActualHeight);
            var rtbAdd = new RenderTargetBitmap(aw, ah, 96, 96, PixelFormats.Pbgra32);
            rtbAdd.Render(addDialog);

            string addPath = Path.Combine(artifactDir, "wpf_add_dialog.png");
            using (var stream = File.Create(addPath))
            {
                var encoder = new PngBitmapEncoder();
                encoder.Frames.Add(BitmapFrame.Create(rtbAdd));
                encoder.Save(stream);
            }
            addDialog.Close();

            Assert.True(File.Exists(emptyPath));
            Assert.True(File.Exists(popPath));
            Assert.True(File.Exists(delPath));
            Assert.True(File.Exists(addPath));
        });
    }
}
