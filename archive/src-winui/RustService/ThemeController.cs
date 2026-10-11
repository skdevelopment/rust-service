using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;
using RustService.Models;
using RustService.Services;
using Windows.UI;

namespace RustService;

public static class ThemeController
{
    public static void Apply()
    {
        if (App.Window?.Content is not FrameworkElement root) return;
        var appearance = AppServices.Settings.Current.Appearance;
        root.RequestedTheme = appearance.Theme switch
        {
            "light" => ElementTheme.Light,
            "dark" => ElementTheme.Dark,
            _ => ElementTheme.Default
        };

        var scheme = ColorSchemes.All.FirstOrDefault(s => s.Id == appearance.ColorScheme) ?? ColorSchemes.All[0];
        if (TryParse(scheme.Primary, out var color) && Application.Current is not null)
        {
            Application.Current.Resources["SystemAccentColor"] = color;
            Application.Current.Resources["AccentFillColorDefaultBrush"] = new SolidColorBrush(color);
        }
    }

    private static bool TryParse(string hex, out Color color)
    {
        color = Colors.Orange;
        try
        {
            hex = hex.TrimStart('#');
            if (hex.Length != 6) return false;
            color = Color.FromArgb(255,
                Convert.ToByte(hex[..2], 16),
                Convert.ToByte(hex[2..4], 16),
                Convert.ToByte(hex[4..6], 16));
            return true;
        }
        catch
        {
            return false;
        }
    }
}
