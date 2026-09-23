using System.Windows;
using Vajra.Windows.ViewModels;

namespace Vajra.Windows.Views;

public partial class DeleteConfirmationDialog : Window
{
    public DeleteConfirmationResult Result { get; private set; } = DeleteConfirmationResult.Cancel;

    public DeleteConfirmationDialog(string filename, string? title = null)
    {
        InitializeComponent();
        FilenameText.Text = string.IsNullOrWhiteSpace(filename) ? "Unknown Download" : filename;
        if (!string.IsNullOrWhiteSpace(title))
        {
            DialogTitleText.Text = title;
            Title = title;
        }
    }

    private void OnKeepFileClick(object sender, RoutedEventArgs e)
    {
        Result = DeleteConfirmationResult.KeepFile;
        DialogResult = true;
        Close();
    }

    private void OnDeleteFileClick(object sender, RoutedEventArgs e)
    {
        Result = DeleteConfirmationResult.DeleteFile;
        DialogResult = true;
        Close();
    }

    private void OnCancelClick(object sender, RoutedEventArgs e)
    {
        Result = DeleteConfirmationResult.Cancel;
        DialogResult = false;
        Close();
    }
}
