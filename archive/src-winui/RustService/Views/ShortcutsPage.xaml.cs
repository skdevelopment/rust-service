using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using RustService.Models;
using RustService.Services;

namespace RustService.Views;

public sealed partial class ShortcutsPage : Page
{
    public ShortcutsPage()
    {
        InitializeComponent();
        Rebuild();
    }

    private void Search_Changed(object sender, TextChangedEventArgs e) => Rebuild();

    private void Rebuild()
    {
        Groups.Children.Clear();
        var q = SearchBox.Text?.Trim() ?? "";
        foreach (var group in ShortcutCatalog.Groups)
        {
            var items = group.Items.Where(i =>
                q.Length == 0 ||
                i.Name.Contains(q, StringComparison.OrdinalIgnoreCase) ||
                i.Description.Contains(q, StringComparison.OrdinalIgnoreCase) ||
                i.Command.Contains(q, StringComparison.OrdinalIgnoreCase)).ToList();
            if (items.Count == 0) continue;

            var panel = new VariableSizedWrapGrid { Orientation = Orientation.Horizontal, MaximumRowsOrColumns = 4, ItemWidth = 220, ItemHeight = 88, Margin = new Thickness(0, 8, 0, 0) };
            foreach (var item in items)
            {
                var captured = item;
                var button = new Button
                {
                    HorizontalAlignment = HorizontalAlignment.Stretch,
                    VerticalAlignment = VerticalAlignment.Stretch,
                    Margin = new Thickness(4),
                    Content = new StackPanel
                    {
                        Children =
                        {
                            new TextBlock { Text = captured.Name + (captured.AdminRequired ? " (Admin)" : ""), TextWrapping = TextWrapping.Wrap, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold },
                            new TextBlock { Text = captured.Description, Opacity = 0.75, TextWrapping = TextWrapping.Wrap, FontSize = 12 }
                        }
                    }
                };
                button.Click += (_, _) => Commands.OpenShortcut(captured.Command, captured.Args, captured.AdminRequired);
                panel.Children.Add(button);
            }

            Groups.Children.Add(new Expander
            {
                Header = $"{group.Name} ({items.Count})",
                IsExpanded = true,
                Content = panel
            });
        }
    }
}
