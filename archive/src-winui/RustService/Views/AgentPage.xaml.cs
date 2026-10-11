using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using RustService.Services;

namespace RustService.Views;

public sealed partial class AgentPage : Page
{
    private string? conversationId;

    public AgentPage()
    {
        InitializeComponent();
        ReloadConvos();
        ReloadPending();
        ReloadMemory();
        _ = Commands.GetCommandHistory();
        _ = Commands.ListAgentFiles();
        _ = Commands.ListInstruments();
        _ = Commands.ListAgentPrograms();
        _ = Commands.GetPendingCommands();
        _ = Commands.GetAgentSettings();
    }

    private void ReloadConvos()
    {
        var items = Commands.ListConversations();
        ConvoBox.ItemsSource = items.Select(c => c.Title).ToList();
        if (items.Count > 0 && conversationId is null)
        {
            conversationId = items[0].Id;
            ConvoBox.SelectedIndex = 0;
            LoadTranscript();
        }
    }

    private void LoadTranscript()
    {
        if (conversationId is null) return;
        var convo = Commands.GetConversation(conversationId);
        Transcript.Text = string.Join("\r\n\r\n", convo?.Messages.Select(m => $"{m.Role}: {m.Content}") ?? []);
    }

    private void ReloadPending() =>
        PendingList.ItemsSource = Commands.GetPendingCommands().Select(p => new CardRow
        {
            Title = p.Command,
            Subtitle = p.Reason,
            Meta = p.Id,
            Badge = p.Status.ToString()
        }).ToList();

    private List<MemoryEntry> memories = [];

    private void MemorySearch_Changed(object sender, TextChangedEventArgs e) => ReloadMemory();
    private void MemoryType_Changed(object sender, SelectionChangedEventArgs e)
    {
        if (MemoryList is null) return;
        ReloadMemory();
    }

    private void ReloadMemory()
    {
        var type = (MemoryTypeBox.SelectedItem as ComboBoxItem)?.Content?.ToString();
        if (type is "all" or null) type = null;
        var q = MemoryQueryBox?.Text?.Trim() ?? "";
        memories = (q.Length == 0
            ? AppServices.Memory.GetAllMemories(type)
            : AppServices.Memory.SearchMemories(q, type)).ToList();
        MemoryList.ItemsSource = memories.Select(m => new CardRow
        {
            Title = m.MemoryType,
            Subtitle = m.Content,
            Meta = m.CreatedAt,
            Badge = "Memory"
        }).ToList();
    }

    private void New_Click(object sender, RoutedEventArgs e)
    {
        var convo = Commands.CreateConversation($"Chat {DateTime.Now:t}");
        conversationId = convo.Id;
        ReloadConvos();
        Transcript.Text = "";
    }

    private void ConvoBox_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        var items = Commands.ListConversations();
        if (ConvoBox.SelectedIndex >= 0 && ConvoBox.SelectedIndex < items.Count)
        {
            conversationId = items[ConvoBox.SelectedIndex].Id;
            LoadTranscript();
        }
    }

    private async void Send_Click(object sender, RoutedEventArgs e)
    {
        if (conversationId is null) New_Click(sender, e);
        if (conversationId is null || string.IsNullOrWhiteSpace(InputBox.Text)) return;
        var text = InputBox.Text;
        if (text.StartsWith("/search ", StringComparison.OrdinalIgnoreCase))
        {
            var hits = await Commands.SearchWeb(text[8..]);
            Transcript.Text += "\r\n" + string.Join("\r\n", hits.Select(h => h.Title + " " + h.Url));
        }
        else if (text.StartsWith("/tavily ", StringComparison.OrdinalIgnoreCase))
        {
            var key = Commands.GetAgentSettings().TavilyApiKey ?? "";
            var hits = await Commands.SearchTavily(text[8..], key);
            Transcript.Text += "\r\n" + string.Join("\r\n", hits.Select(h => h.Title));
        }
        else if (text.StartsWith("/searx ", StringComparison.OrdinalIgnoreCase))
        {
            var url = Commands.GetAgentSettings().SearxngUrl ?? "";
            var hits = await Commands.SearchSearxng(text[7..], url);
            Transcript.Text += "\r\n" + string.Join("\r\n", hits.Select(h => h.Title));
        }
        else if (text.StartsWith("/exec ", StringComparison.OrdinalIgnoreCase))
        {
            var ran = await Commands.ExecuteAgentCommand(text[6..], "agent-page");
            Transcript.Text += "\r\n" + (ran.Output ?? ran.Error);
        }
        else
        {
            await AppServices.Agent.ChatAsync(conversationId, text);
            if (conversationId is not null)
            {
                var convo = Commands.GetConversation(conversationId);
                if (convo is not null) Commands.SaveConversationMessages(conversationId, convo.Messages);
            }
        }

        InputBox.Text = "";
        LoadTranscript();
    }

    private void Queue_Click(object sender, RoutedEventArgs e)
    {
        if (string.IsNullOrWhiteSpace(InputBox.Text)) return;
        Commands.QueueAgentCommand(InputBox.Text, "manual");
        ReloadPending();
    }

    private async void Approve_Click(object sender, RoutedEventArgs e)
    {
        var item = Commands.GetPendingCommands().ElementAtOrDefault(PendingList.SelectedIndex);
        if (item is null) return;
        await Commands.ApproveCommand(item.Id);
        ReloadPending();
    }

    private void Reject_Click(object sender, RoutedEventArgs e)
    {
        var item = Commands.GetPendingCommands().ElementAtOrDefault(PendingList.SelectedIndex);
        if (item is null) return;
        Commands.RejectCommand(item.Id);
        ReloadPending();
    }

    private void ClearPending_Click(object sender, RoutedEventArgs e)
    {
        Commands.ClearPendingCommands();
        ReloadPending();
    }

    private void Rename_Click(object sender, RoutedEventArgs e)
    {
        if (conversationId is null || string.IsNullOrWhiteSpace(InputBox.Text)) return;
        Commands.UpdateConversationTitle(conversationId, InputBox.Text);
        ReloadConvos();
    }

    private void DeleteChat_Click(object sender, RoutedEventArgs e)
    {
        if (conversationId is null) return;
        Commands.DeleteConversation(conversationId);
        conversationId = null;
        ReloadConvos();
        Transcript.Text = "";
    }

    private void SaveMemory_Click(object sender, RoutedEventArgs e)
    {
        if (string.IsNullOrWhiteSpace(MemoryBox.Text)) return;
        var type = (MemoryTypeBox.SelectedItem as ComboBoxItem)?.Content?.ToString() ?? "fact";
        AppServices.Memory.SaveMemory(type, MemoryBox.Text);
        ReloadMemory();
    }

    private void DeleteMemory_Click(object sender, RoutedEventArgs e)
    {
        if (MemoryList.SelectedIndex < 0 || MemoryList.SelectedIndex >= memories.Count) return;
        AppServices.Memory.DeleteMemory(memories[MemoryList.SelectedIndex].Id);
        ReloadMemory();
    }

    private void ReadFile_Click(object sender, RoutedEventArgs e)
    {
        if (string.IsNullOrWhiteSpace(FilePathBox.Text)) return;
        var valid = Commands.ValidateFilesystemPath(FilePathBox.Text);
        FileOut.Text = valid.Valid ? Commands.AgentReadFile(valid.SanitizedPath ?? FilePathBox.Text) : valid.Error ?? "invalid";
        if (valid.Valid) _ = Commands.ReadFilesystemFile(valid.SanitizedPath ?? FilePathBox.Text);
        _ = Commands.GetFileInfo(FilePathBox.Text);
        _ = Commands.ReadFileContent(FilePathBox.Text);
        try { _ = Commands.ReadFileBinary(FilePathBox.Text); } catch { /* binary optional */ }
    }

    private void ListDir_Click(object sender, RoutedEventArgs e)
    {
        var path = string.IsNullOrWhiteSpace(FilePathBox.Text) ? Commands.GetDataDir() : FilePathBox.Text;
        FileOut.Text = string.Join("\r\n", Commands.AgentListDir(path));
    }

    private void WriteFile_Click(object sender, RoutedEventArgs e)
    {
        if (string.IsNullOrWhiteSpace(FilePathBox.Text)) return;
        Commands.AgentWriteFile(FilePathBox.Text, MemoryBox.Text);
        FileOut.Text = "Wrote " + FilePathBox.Text;
    }

    private void SaveTranscript_Click(object sender, RoutedEventArgs e)
    {
        var generated = Commands.GenerateAgentFile("transcript.txt", Transcript.Text, "chat transcript");
        var abs = Path.Combine(Commands.GetDataDir(), generated.StoredPath.Replace('/', Path.DirectorySeparatorChar));
        var copy = abs + ".copy";
        Commands.AgentCopyFile(abs, copy);
        Commands.AgentEditFile(copy, "role", "Role");
        var moved = copy + ".moved";
        Commands.AgentMoveFile(copy, moved);
        var uploaded = Commands.SaveUploadedFile("transcript.txt", "text/plain", Convert.ToBase64String(System.Text.Encoding.UTF8.GetBytes(Transcript.Text ?? "")));
        Commands.DeleteAgentFile(uploaded.Id);
        _ = Commands.AgentFindExe("cmd.exe");
        FileOut.Text = generated.StoredPath;
    }

    private void Grep_Click(object sender, RoutedEventArgs e)
    {
        var root = Commands.GetDataDir();
        FileOut.Text = string.Join("\r\n", Commands.AgentGrep(root, FilePathBox.Text));
    }

    private void Glob_Click(object sender, RoutedEventArgs e)
    {
        FileOut.Text = string.Join("\r\n", Commands.AgentGlob(Commands.GetDataDir(), string.IsNullOrWhiteSpace(FilePathBox.Text) ? "*.json" : FilePathBox.Text));
    }
}
