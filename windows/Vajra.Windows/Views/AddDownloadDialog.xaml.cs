using System.Windows;
using Vajra.Windows.ViewModels;

namespace Vajra.Windows.Views;

public partial class AddDownloadDialog : Window
{
    public AddDownloadDialog(AddDownloadViewModel viewModel)
    {
        InitializeComponent();
        DataContext = viewModel;

        viewModel.RequestClose += (success) =>
        {
            DialogResult = success;
            Close();
        };

        Loaded += (s, e) =>
        {
            UrlTextBox.Focus();
            if (!string.IsNullOrEmpty(UrlTextBox.Text))
            {
                UrlTextBox.SelectAll();
            }
        };
    }
}
