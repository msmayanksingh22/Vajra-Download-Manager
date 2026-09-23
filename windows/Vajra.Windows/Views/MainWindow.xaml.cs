using System.Windows;
using System.Windows.Input;
using Vajra.Windows.Models;
using Vajra.Windows.Services;
using Vajra.Windows.ViewModels;

namespace Vajra.Windows.Views;

public partial class MainWindow : Window
{
    private readonly MainViewModel _viewModel;
    private readonly IDaemonClient _daemonClient;
    private readonly ISystemInteractionService _systemService;
    private readonly ITrayIconService? _trayService;
    private bool _allowExit;
    private bool _balloonShown;

    public MainWindow(
        MainViewModel viewModel,
        IDaemonClient daemonClient,
        ISystemInteractionService systemService,
        ITrayIconService? trayService = null)
    {
        InitializeComponent();
        _viewModel = viewModel;
        _daemonClient = daemonClient;
        _systemService = systemService;
        _trayService = trayService;
        DataContext = _viewModel;

        _viewModel.RequestAddUrlDialog += ShowAddUrlDialog;
        _viewModel.RequestPropertiesDialog += ShowPropertiesDialog;
        _viewModel.ConfirmDeleteCallback = item =>
        {
            var dialog = new DeleteConfirmationDialog(item.Filename)
            {
                Owner = this
            };
            dialog.ShowDialog();
            return dialog.Result;
        };
        _viewModel.ConfirmDeleteMultipleCallback = targets =>
        {
            var title = $"Remove {targets.Count} downloads from list?";
            var summary = string.Join(", ", targets.Take(3).Select(t => t.Filename));
            if (targets.Count > 3) summary += $" and {targets.Count - 3} more";
            var dialog = new DeleteConfirmationDialog(summary, title)
            {
                Owner = this
            };
            dialog.ShowDialog();
            return dialog.Result;
        };

        Loaded += OnLoaded;
        Closing += OnClosing;
    }

    private void OnLoaded(object sender, RoutedEventArgs e)
    {
        _viewModel.Initialize();
    }

    private void OnClosing(object? sender, System.ComponentModel.CancelEventArgs e)
    {
        if (!_allowExit && _trayService != null)
        {
            e.Cancel = true;
            Hide();
            if (!_balloonShown)
            {
                _balloonShown = true;
                _trayService.ShowNotification("Vajra Download Manager", "Vajra is running in the background. Access it from the system tray.");
            }
            return;
        }

        _viewModel.Cleanup();
    }

    public void AllowExit()
    {
        _allowExit = true;
    }

    private void ShowAddUrlDialog()
    {
        var addVm = new AddDownloadViewModel(_daemonClient, _systemService);
        var dialog = new AddDownloadDialog(addVm)
        {
            Owner = this
        };

        bool? result = dialog.ShowDialog();
        if (result == true)
        {
            _ = _viewModel.LoadDownloadsAsync();
        }
    }

    private void ShowPropertiesDialog(DownloadItem item)
    {
        var propVm = new PropertiesViewModel(item, _daemonClient);
        var dialog = new PropertiesWindow(propVm)
        {
            Owner = this
        };
        dialog.ShowDialog();
    }

    private void OnDataGridSelectionChanged(object sender, System.Windows.Controls.SelectionChangedEventArgs e)
    {
        if (sender is System.Windows.Controls.DataGrid grid)
        {
            _viewModel.UpdateSelection(grid.SelectedItems);
        }
    }

    private void OnRowPreviewMouseRightButtonDown(object sender, MouseButtonEventArgs e)
    {
        if (sender is System.Windows.Controls.DataGridRow row && !row.IsSelected)
        {
            row.IsSelected = true;
        }
    }

    private void OnRowDoubleClick(object sender, MouseButtonEventArgs e)
    {
        var selected = _viewModel.SelectedDownload;
        if (selected == null) return;

        if (selected.Status == DownloadStatus.Completed && _viewModel.OpenFileCommand.CanExecute(null))
        {
            _viewModel.OpenFileCommand.Execute(null);
        }
        else if (selected.CanResume && _viewModel.ResumeCommand.CanExecute(null))
        {
            _viewModel.ResumeCommand.Execute(null);
        }
        else if (selected.CanPause && _viewModel.PauseCommand.CanExecute(null))
        {
            _viewModel.PauseCommand.Execute(null);
        }
    }
}
