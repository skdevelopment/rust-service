using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace RustService.Views;

public sealed partial class TechnicianTabPage : Page
{
    public TechnicianTabPage()
    {
        InitializeComponent();
    }

    protected override async void OnNavigatedTo(NavigationEventArgs e)
    {
        if (e.Parameter is not string url || string.IsNullOrWhiteSpace(url)) return;
        try
        {
            await Web.EnsureCoreWebView2Async();
            Web.Source = new Uri(url);
        }
        catch
        {
            // WebView2 runtime may be missing on some machines.
        }
    }
}
