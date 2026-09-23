using System.Windows;
using System.Windows.Input;
using Vajra.Windows.ViewModels;

namespace Vajra.Windows.Views;

public partial class PropertiesWindow : Window
{
    private readonly PropertiesViewModel _viewModel;

    public PropertiesWindow(PropertiesViewModel viewModel)
    {
        InitializeComponent();
        _viewModel = viewModel;
        DataContext = _viewModel;

        Loaded += OnLoaded;
        KeyDown += OnKeyDown;
    }

    private async void OnLoaded(object sender, RoutedEventArgs e)
    {
        await _viewModel.RefreshDetailsAsync();
    }

    private async void OnKeyDown(object sender, KeyEventArgs e)
    {
        if (e.Key == Key.F5)
        {
            await _viewModel.RefreshDetailsAsync();
        }
    }

    private async void OnRefreshClick(object sender, RoutedEventArgs e)
    {
        await _viewModel.RefreshDetailsAsync();
    }

    private void OnCloseClick(object sender, RoutedEventArgs e)
    {
        Close();
    }
}
