using System.Windows;
using Vajra.Windows.Services;
using Vajra.Windows.ViewModels;
using Vajra.Windows.Views;

namespace Vajra.Windows;

public partial class App : Application
{
    private ITokenService? _tokenService;
    private IDaemonClient? _daemonClient;
    private ISseService? _sseService;
    private ISystemInteractionService? _systemService;
    private ITrayIconService? _trayService;
    private MainViewModel? _mainViewModel;
    private MainWindow? _mainWindow;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        _tokenService = new TokenService();
        _daemonClient = new DaemonClient(_tokenService);
        _sseService = new SseService(_tokenService);
        _systemService = new SystemInteractionService();
        _trayService = new TrayIconService();

        _mainViewModel = new MainViewModel(_daemonClient, _sseService, _tokenService, _systemService);
        _mainWindow = new MainWindow(_mainViewModel, _daemonClient, _systemService, _trayService);

        _trayService.OnOpenRequested += () =>
        {
            Dispatcher.Invoke(() =>
            {
                if (_mainWindow != null)
                {
                    _mainWindow.Show();
                    if (_mainWindow.WindowState == WindowState.Minimized)
                    {
                        _mainWindow.WindowState = WindowState.Normal;
                    }
                    _mainWindow.Activate();
                }
            });
        };

        _trayService.OnAddDownloadRequested += () =>
        {
            Dispatcher.Invoke(() =>
            {
                if (_mainWindow != null)
                {
                    _mainWindow.Show();
                    if (_mainWindow.WindowState == WindowState.Minimized)
                    {
                        _mainWindow.WindowState = WindowState.Normal;
                    }
                    _mainWindow.Activate();
                    if (_mainViewModel.AddUrlCommand.CanExecute(null))
                    {
                        _mainViewModel.AddUrlCommand.Execute(null);
                    }
                }
            });
        };

        _trayService.OnPauseAllRequested += () =>
        {
            Dispatcher.Invoke(() =>
            {
                if (_mainViewModel?.PauseAllCommand.CanExecute(null) == true)
                {
                    _mainViewModel.PauseAllCommand.Execute(null);
                }
            });
        };

        _trayService.OnResumeAllRequested += () =>
        {
            Dispatcher.Invoke(() =>
            {
                if (_mainViewModel?.ResumeAllCommand.CanExecute(null) == true)
                {
                    _mainViewModel.ResumeAllCommand.Execute(null);
                }
            });
        };

        _trayService.OnExitRequested += () =>
        {
            Dispatcher.Invoke(() =>
            {
                _mainWindow?.AllowExit();
                Shutdown();
            });
        };

        _mainViewModel.PropertyChanged += (_, args) =>
        {
            if (args.PropertyName == nameof(MainViewModel.TotalSpeedBps) ||
                args.PropertyName == nameof(MainViewModel.ActiveCount))
            {
                _trayService.UpdateStatus(_mainViewModel.ActiveCount, _mainViewModel.TotalSpeedBps);
            }
        };

        _trayService.Initialize();
        _mainWindow.Show();
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _trayService?.Dispose();
        base.OnExit(e);
    }
}
