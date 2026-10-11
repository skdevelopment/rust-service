using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace RustService.Views;

public sealed partial class TechnicianWebPage : Page
{
    public TechnicianWebPage()
    {
        InitializeComponent();
    }

    protected override async void OnNavigatedTo(NavigationEventArgs e)
    {
        if (e.Parameter is string url && Uri.TryCreate(url, UriKind.Absolute, out var uri))
        {
            await Web.EnsureCoreWebView2Async();
            Web.Source = uri;
        }
    }
}
