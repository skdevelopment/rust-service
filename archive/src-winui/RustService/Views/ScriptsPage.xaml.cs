using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using RustService.Models;
using RustService.Services;

namespace RustService.Views;

public sealed partial class ScriptsPage : Page
{
    private List<ScriptItem> shown = [];
    private bool creating;

    public ScriptsPage()
    {
        InitializeComponent();
        Reload();
    }

    private void Search_Changed(object sender, TextChangedEventArgs e) => Reload();
    private void Sort_Changed(object sender, SelectionChangedEventArgs e)
    {
        if (List is null) return;
        Reload();
    }

    private void Reload()
    {
        IEnumerable<ScriptItem> items = Commands.GetScripts();
        var q = SearchBox.Text?.Trim() ?? "";
        if (q.Length > 0)
        {
            items = items.Where(s =>
                s.Name.Contains(q, StringComparison.OrdinalIgnoreCase) ||
                s.Description.Contains(q, StringComparison.OrdinalIgnoreCase) ||
                s.Content.Contains(q, StringComparison.OrdinalIgnoreCase));
        }

        shown = ((SortBox.SelectedItem as ComboBoxItem)?.Tag?.ToString() switch
        {
            "name-desc" => items.OrderByDescending(s => s.Name),
            "used" => items.OrderByDescending(s => s.RunCount),
            "added" => items.OrderByDescending(s => s.CreatedAt),
            "run" => items.OrderByDescending(s => s.LastRun ?? DateTime.MinValue),
            _ => items.OrderBy(s => s.Name)
        }).ToList();

        List.ItemsSource = shown.Select(s => new CardRow
        {
            Title = s.Name,
            Subtitle = s.Description,
            Meta = $"{s.ScriptType}  runs={s.RunCount}",
            Badge = s.RunAsAdmin ? "Admin" : s.ScriptType
        }).ToList();
    }

    private ScriptItem? Selected() =>
        !creating && List.SelectedIndex >= 0 && List.SelectedIndex < shown.Count ? shown[List.SelectedIndex] : null;

    private void List_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        creating = false;
        var item = Selected();
        if (item is null) return;
        NameBox.Text = item.Name;
        DescBox.Text = item.Description;
        ContentBox.Text = item.Content;
        AdminBox.IsChecked = item.RunAsAdmin;
        TypeBox.SelectedIndex = item.ScriptType == "cmd" ? 1 : 0;
    }

    private void New_Click(object sender, RoutedEventArgs e)
    {
        creating = true;
        List.SelectedIndex = -1;
        NameBox.Text = "";
        DescBox.Text = "";
        ContentBox.Text = "";
        AdminBox.IsChecked = false;
        TypeBox.SelectedIndex = 0;
    }

    private void Save_Click(object sender, RoutedEventArgs e)
    {
        var type = TypeBox.SelectedIndex == 1 ? "cmd" : "powershell";
        var existing = Selected();
        if (existing is null)
        {
            Commands.AddScript(new ScriptItem
            {
                Name = NameBox.Text,
                Description = DescBox.Text,
                Content = ContentBox.Text,
                ScriptType = type,
                RunAsAdmin = AdminBox.IsChecked == true
            });
        }
        else
        {
            existing.Name = NameBox.Text;
            existing.Description = DescBox.Text;
            existing.Content = ContentBox.Text;
            existing.ScriptType = type;
            existing.RunAsAdmin = AdminBox.IsChecked == true;
            Commands.UpdateScript(existing);
        }

        creating = false;
        Reload();
    }

    private async void Run_Click(object sender, RoutedEventArgs e)
    {
        var item = Selected();
        if (item is null) return;
        var result = await Commands.RunScript(item.Id);
        OutputBox.Text = result.Combined;
        Reload();
    }

    private void Delete_Click(object sender, RoutedEventArgs e)
    {
        var item = Selected();
        if (item is not null) Commands.DeleteScript(item.Id);
        Reload();
    }
}
