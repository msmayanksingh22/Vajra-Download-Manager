using System.Drawing;
using System.IO;
using System.Runtime.InteropServices;
using System.Windows.Forms;
using Vajra.Windows.Models;

namespace Vajra.Windows.Services;

public class TrayIconService : ITrayIconService
{
    private NotifyIcon? _notifyIcon;
    private ContextMenuStrip? _contextMenu;
    private ToolStripMenuItem? _statsItem;
    private Icon? _createdIcon;
    private IntPtr _hIcon = IntPtr.Zero;

    public event Action? OnOpenRequested;
    public event Action? OnAddDownloadRequested;
    public event Action? OnPauseAllRequested;
    public event Action? OnResumeAllRequested;
    public event Action? OnExitRequested;

    [DllImport("user32.dll", CharSet = CharSet.Auto)]
    private static extern bool DestroyIcon(IntPtr handle);

    public void Initialize()
    {
        _contextMenu = new ContextMenuStrip();

        var openItem = new ToolStripMenuItem("Open Vajra")
        {
            Font = new Font(_contextMenu.Font, FontStyle.Bold)
        };
        openItem.Click += (_, _) => OnOpenRequested?.Invoke();

        var addItem = new ToolStripMenuItem("Add Download...");
        addItem.Click += (_, _) => OnAddDownloadRequested?.Invoke();

        var pauseAllItem = new ToolStripMenuItem("Pause All");
        pauseAllItem.Click += (_, _) => OnPauseAllRequested?.Invoke();

        var resumeAllItem = new ToolStripMenuItem("Resume All");
        resumeAllItem.Click += (_, _) => OnResumeAllRequested?.Invoke();

        _statsItem = new ToolStripMenuItem("Active: 0 | —")
        {
            Enabled = false
        };

        var exitItem = new ToolStripMenuItem("Exit Vajra");
        exitItem.Click += (_, _) => OnExitRequested?.Invoke();

        _contextMenu.Items.Add(openItem);
        _contextMenu.Items.Add(new ToolStripSeparator());
        _contextMenu.Items.Add(addItem);
        _contextMenu.Items.Add(new ToolStripSeparator());
        _contextMenu.Items.Add(pauseAllItem);
        _contextMenu.Items.Add(resumeAllItem);
        _contextMenu.Items.Add(new ToolStripSeparator());
        _contextMenu.Items.Add(_statsItem);
        _contextMenu.Items.Add(new ToolStripSeparator());
        _contextMenu.Items.Add(exitItem);

        _createdIcon = CreateDefaultIcon();

        _notifyIcon = new NotifyIcon
        {
            Icon = _createdIcon,
            Text = "Vajra Download Manager",
            ContextMenuStrip = _contextMenu,
            Visible = true
        };

        _notifyIcon.DoubleClick += (_, _) => OnOpenRequested?.Invoke();
    }

    public void UpdateStatus(int activeCount, ulong speedBps)
    {
        if (_notifyIcon == null) return;

        string speedText = speedBps > 0 ? $"{DownloadItem.FormatBytes(speedBps)}/s" : "—";
        if (_statsItem != null)
        {
            _statsItem.Text = $"Active: {activeCount} | {speedText}";
        }

        string tip = activeCount > 0
            ? $"Vajra: {activeCount} active ({speedText})"
            : "Vajra Download Manager";

        if (tip.Length > 63)
        {
            tip = tip.Substring(0, 60) + "...";
        }

        _notifyIcon.Text = tip;
    }

    public void ShowNotification(string title, string message)
    {
        if (_notifyIcon == null || !string.IsNullOrEmpty(_notifyIcon.BalloonTipText)) return;
        _notifyIcon.BalloonTipTitle = title;
        _notifyIcon.BalloonTipText = message;
        _notifyIcon.BalloonTipIcon = ToolTipIcon.Info;
        _notifyIcon.ShowBalloonTip(3000);
    }

    private Icon CreateDefaultIcon()
    {
        try
        {
            var processPath = Environment.ProcessPath;
            if (!string.IsNullOrEmpty(processPath) && File.Exists(processPath))
            {
                var icon = Icon.ExtractAssociatedIcon(processPath);
                if (icon != null) return icon;
            }
        }
        catch
        {
            // Fall through to dynamic generation
        }

        // Render high-DPI Vajra blue badge with stylized "V" emblem
        using var bitmap = new Bitmap(32, 32);
        using (var g = Graphics.FromImage(bitmap))
        {
            g.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
            using var brush = new SolidBrush(System.Drawing.Color.FromArgb(0, 103, 192));
            g.FillEllipse(brush, 1, 1, 30, 30);
            using var pen = new Pen(System.Drawing.Color.White, 3.5f)
            {
                StartCap = System.Drawing.Drawing2D.LineCap.Round,
                EndCap = System.Drawing.Drawing2D.LineCap.Round,
                LineJoin = System.Drawing.Drawing2D.LineJoin.Round
            };
            g.DrawLines(pen, new[]
            {
                new Point(8, 10),
                new Point(16, 23),
                new Point(24, 10)
            });
        }

        _hIcon = bitmap.GetHicon();
        return Icon.FromHandle(_hIcon);
    }

    public void Dispose()
    {
        if (_notifyIcon != null)
        {
            _notifyIcon.Visible = false;
            _notifyIcon.Dispose();
            _notifyIcon = null;
        }

        _contextMenu?.Dispose();
        _contextMenu = null;

        if (_hIcon != IntPtr.Zero)
        {
            DestroyIcon(_hIcon);
            _hIcon = IntPtr.Zero;
        }

        _createdIcon?.Dispose();
        _createdIcon = null;
    }
}
