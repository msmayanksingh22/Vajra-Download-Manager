using System.Globalization;
using System.Windows;
using System.Windows.Data;
using System.Windows.Media;
using Vajra.Windows.Models;

namespace Vajra.Windows.Converters;

public class StatusToForegroundConverter : IValueConverter
{
    private static readonly SolidColorBrush DownloadingBrush = CreateFrozen(Color.FromRgb(0x02, 0x84, 0xC7));
    private static readonly SolidColorBrush ConnectingBrush = CreateFrozen(Color.FromRgb(0x63, 0x66, 0xF1));
    private static readonly SolidColorBrush CompletedBrush = CreateFrozen(Color.FromRgb(0x16, 0xA3, 0x4A));
    private static readonly SolidColorBrush PausedBrush = CreateFrozen(Color.FromRgb(0xD9, 0x77, 0x06));
    private static readonly SolidColorBrush FailedBrush = CreateFrozen(Color.FromRgb(0xDC, 0x26, 0x26));
    private static readonly SolidColorBrush IdleBrush = CreateFrozen(Color.FromRgb(0x64, 0x74, 0x8B));

    private static SolidColorBrush CreateFrozen(Color color)
    {
        var brush = new SolidColorBrush(color);
        brush.Freeze();
        return brush;
    }

    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        if (value is DownloadStatus status)
        {
            return status switch
            {
                DownloadStatus.Downloading => DownloadingBrush,
                DownloadStatus.Connecting => ConnectingBrush,
                DownloadStatus.Completed => CompletedBrush,
                DownloadStatus.Paused => PausedBrush,
                DownloadStatus.Failed => FailedBrush,
                _ => IdleBrush
            };
        }
        return IdleBrush;
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture) => throw new NotImplementedException();
}

public class StatusToBackgroundConverter : IValueConverter
{
    private static readonly SolidColorBrush DownloadingBg = new(Color.FromRgb(0xEB, 0xF3, 0xFC));
    private static readonly SolidColorBrush ConnectingBg = new(Color.FromRgb(0xEE, 0xF8, 0xF9));
    private static readonly SolidColorBrush CompletedBg = new(Color.FromRgb(0xEB, 0xF6, 0xEC));
    private static readonly SolidColorBrush PausedBg = new(Color.FromRgb(0xFC, 0xF4, 0xEB));
    private static readonly SolidColorBrush FailedBg = new(Color.FromRgb(0xFD, 0xEB, 0xEA));
    private static readonly SolidColorBrush IdleBg = new(Color.FromRgb(0xF3, 0xF2, 0xF1));

    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        if (value is DownloadStatus status)
        {
            return status switch
            {
                DownloadStatus.Downloading => DownloadingBg,
                DownloadStatus.Connecting => ConnectingBg,
                DownloadStatus.Completed => CompletedBg,
                DownloadStatus.Paused => PausedBg,
                DownloadStatus.Failed => FailedBg,
                _ => IdleBg
            };
        }
        return IdleBg;
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture) => throw new NotImplementedException();
}

public class ConnectionStateToBrushConverter : IValueConverter
{
    private static readonly SolidColorBrush ConnectedBrush = new(Color.FromRgb(0x10, 0x7C, 0x41)); // Green
    private static readonly SolidColorBrush ReconnectingBrush = new(Color.FromRgb(0xD8, 0x7A, 0x00)); // Amber
    private static readonly SolidColorBrush DisconnectedBrush = new(Color.FromRgb(0xD1, 0x34, 0x38)); // Red

    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        if (value is ConnectionState state)
        {
            return state switch
            {
                ConnectionState.Connected => ConnectedBrush,
                ConnectionState.Reconnecting or ConnectionState.Connecting => ReconnectingBrush,
                _ => DisconnectedBrush
            };
        }
        return DisconnectedBrush;
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture) => throw new NotImplementedException();
}

public class InverseBooleanConverter : IValueConverter
{
    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        return value is bool b ? !b : false;
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        return value is bool b ? !b : false;
    }
}

public class BooleanToVisibilityInvertedConverter : IValueConverter
{
    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        return (value is bool b && b) ? Visibility.Collapsed : Visibility.Visible;
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture) => throw new NotImplementedException();
}
