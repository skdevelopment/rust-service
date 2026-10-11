using System.Runtime.InteropServices.WindowsRuntime;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media.Imaging;
using RustService.Models;
using RustService.Services;
using Windows.Storage.Pickers;

namespace RustService.Views;

public sealed class ProgramRow
{
    public string Id { get; set; } = "";
    public string Title { get; set; } = "";
    public string Meta { get; set; } = "";
    public BitmapImage? Icon { get; set; }
}

public sealed partial class ProgramsPage : Page
{
    private List<ProgramItem> shown = [];

    public ProgramsPage()
    {
        InitializeComponent();
        Reload();
    }

    private string SortKey => (SortBox.SelectedItem as ComboBoxItem)?.Tag?.ToString() ?? "name-asc";

    private void Reload()
    {
        IEnumerable<ProgramItem> items = Commands.GetPrograms();
        var q = SearchBox.Text?.Trim() ?? "";
        if (q.Length > 0)
        {
            items = items.Where(p =>
                p.Name.Contains(q, StringComparison.OrdinalIgnoreCase) ||
                p.Description.Contains(q, StringComparison.OrdinalIgnoreCase) ||
                p.ExePath.Contains(q, StringComparison.OrdinalIgnoreCase));
        }

        shown = SortKey switch
        {
            "name-desc" => items.OrderByDescending(p => p.Name).ToList(),
            "most-used" => items.OrderByDescending(p => p.LaunchCount).ToList(),
            "recently-added" => items.OrderByDescending(p => p.CreatedAt).ToList(),
            "recently-launched" => items.OrderByDescending(p => p.LastLaunched ?? DateTime.MinValue).ToList(),
            _ => items.OrderBy(p => p.Name).ToList()
        };

        List.ItemsSource = shown.Select(p => new ProgramRow
        {
            Id = p.Id,
            Title = $"{p.Name}{(p.IsCli ? "  · CLI" : "")}",
            Meta = $"{p.ExePath}  launches={p.LaunchCount}" + (string.IsNullOrWhiteSpace(p.Description) ? "" : $"  — {p.Description}"),
            Icon = LoadIcon(p)
        }).ToList();
    }

    private static BitmapImage? LoadIcon(ProgramItem item)
    {
        var path = Commands.GetProgramIcon(item.IconPath);
        if (string.IsNullOrWhiteSpace(path) || !File.Exists(path)) return null;
        try
        {
            var image = new BitmapImage();
            using var stream = File.OpenRead(path);
            image.SetSource(stream.AsRandomAccessStream());
            return image;
        }
        catch
        {
            return null;
        }
    }

    private ProgramItem? Selected()
    {
        if (List.SelectedIndex < 0 || List.SelectedIndex >= shown.Count) return null;
        return shown[List.SelectedIndex];
    }

    private void Search_Changed(object sender, TextChangedEventArgs e) => Reload();
    private void Sort_Changed(object sender, SelectionChangedEventArgs e)
    {
        if (List is null) return;
        Reload();
    }

    private void List_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        var item = Selected();
        if (item is null) return;
        NameBox.Text = item.Name;
        DescBox.Text = item.Description;
        PathBox.Text = item.ExePath;
        CliBox.IsChecked = item.IsCli;
    }

    private void List_DoubleTapped(object sender, DoubleTappedRoutedEventArgs e) => Launch_Click(sender, e);

    private async void Browse_Click(object sender, RoutedEventArgs e)
    {
        if (App.Window is null) return;
        var picker = new FileOpenPicker();
        var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(App.Window);
        WinRT.Interop.InitializeWithWindow.Initialize(picker, hwnd);
        picker.FileTypeFilter.Add(".exe");
        picker.FileTypeFilter.Add("*");
        var file = await picker.PickSingleFileAsync();
        if (file is not null) PathBox.Text = file.Path;
    }

    private void Add_Click(object sender, RoutedEventArgs e)
    {
        if (string.IsNullOrWhiteSpace(PathBox.Text)) return;
        var item = Commands.AddProgram(
            string.IsNullOrWhiteSpace(NameBox.Text) ? Path.GetFileNameWithoutExtension(PathBox.Text) : NameBox.Text,
            DescBox.Text,
            "",
            PathBox.Text,
            CliBox.IsChecked == true);
        Commands.ExtractProgramIcon(PathBox.Text, item.Id);
        Reload();
    }

    private void Save_Click(object sender, RoutedEventArgs e)
    {
        var item = Selected();
        if (item is null) return;
        item.Name = NameBox.Text;
        item.Description = DescBox.Text;
        item.ExePath = PathBox.Text;
        item.IsCli = CliBox.IsChecked == true;
        item.IconPath = Commands.ExtractProgramIcon(item.ExePath, item.Id) ?? item.IconPath;
        Commands.UpdateProgram(item);
        Reload();
    }

    private void Launch_Click(object sender, RoutedEventArgs e)
    {
        var item = Selected();
        if (item is not null) Commands.LaunchProgram(item.Id);
        Reload();
    }

    private void Reveal_Click(object sender, RoutedEventArgs e)
    {
        var item = Selected();
        if (item is not null) Commands.RevealProgram(item.Id);
    }

    private void Delete_Click(object sender, RoutedEventArgs e)
    {
        var item = Selected();
        if (item is not null) Commands.DeleteProgram(item.Id);
        Reload();
    }

    private void Import_Click(object sender, RoutedEventArgs e)
    {
        Commands.ImportLegacyPrograms();
        Reload();
    }
}
