using Microsoft.Data.Sqlite;
using RustService.Core;
using RustService.Models;

namespace RustService.Services;

public sealed class MemoryEntry
{
    public string Id { get; set; } = "";
    public string MemoryType { get; set; } = "fact";
    public string Content { get; set; } = "";
    public string? Metadata { get; set; }
    public string CreatedAt { get; set; } = DateTime.UtcNow.ToString("O");
}

public sealed class AgentMemory : IDisposable
{
    private readonly SqliteConnection connection;

    public AgentMemory()
    {
        var path = DataDirectory.PathTo("agent.db");
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        connection = new SqliteConnection($"Data Source={path}");
        connection.Open();
        using var cmd = connection.CreateCommand();
        cmd.CommandText = """
            CREATE TABLE IF NOT EXISTS command_history (
                id TEXT PRIMARY KEY,
                command TEXT NOT NULL,
                reason TEXT,
                status TEXT NOT NULL,
                output TEXT,
                error TEXT,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS conversation_messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                memory_type TEXT NOT NULL,
                content TEXT NOT NULL,
                metadata TEXT,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_conversation_messages_conv_id ON conversation_messages(conversation_id);
            """;
        cmd.ExecuteNonQuery();
    }

    public void InsertCommand(PendingCommand command)
    {
        using var cmd = connection.CreateCommand();
        cmd.CommandText = "INSERT OR REPLACE INTO command_history(id, command, reason, status, output, error, created_at) VALUES ($id,$command,$reason,$status,$output,$error,$created)";
        cmd.Parameters.AddWithValue("$id", command.Id);
        cmd.Parameters.AddWithValue("$command", command.Command);
        cmd.Parameters.AddWithValue("$reason", command.Reason);
        cmd.Parameters.AddWithValue("$status", command.Status.ToString().ToLowerInvariant());
        cmd.Parameters.AddWithValue("$output", (object?)command.Output ?? DBNull.Value);
        cmd.Parameters.AddWithValue("$error", (object?)command.Error ?? DBNull.Value);
        cmd.Parameters.AddWithValue("$created", command.CreatedAt);
        cmd.ExecuteNonQuery();
    }

    public IReadOnlyList<PendingCommand> History(int limit = 100)
    {
        using var cmd = connection.CreateCommand();
        cmd.CommandText = "SELECT id, command, reason, status, output, error, created_at FROM command_history ORDER BY created_at DESC LIMIT $limit";
        cmd.Parameters.AddWithValue("$limit", limit);
        var list = new List<PendingCommand>();
        using var reader = cmd.ExecuteReader();
        while (reader.Read())
        {
            list.Add(new PendingCommand
            {
                Id = reader.GetString(0),
                Command = reader.GetString(1),
                Reason = reader.IsDBNull(2) ? "" : reader.GetString(2),
                Status = Enum.TryParse<CommandStatus>(reader.GetString(3), true, out var s) ? s : CommandStatus.Pending,
                Output = reader.IsDBNull(4) ? null : reader.GetString(4),
                Error = reader.IsDBNull(5) ? null : reader.GetString(5),
                CreatedAt = reader.GetString(6)
            });
        }

        return list;
    }

    public MemoryEntry SaveMemory(string memoryType, string content, string? metadata = null)
    {
        var entry = new MemoryEntry
        {
            Id = Guid.NewGuid().ToString(),
            MemoryType = memoryType,
            Content = content,
            Metadata = metadata,
            CreatedAt = DateTime.UtcNow.ToString("O")
        };
        using var cmd = connection.CreateCommand();
        cmd.CommandText = "INSERT INTO memories(id, memory_type, content, metadata, created_at) VALUES ($id,$type,$content,$meta,$c)";
        cmd.Parameters.AddWithValue("$id", entry.Id);
        cmd.Parameters.AddWithValue("$type", entry.MemoryType);
        cmd.Parameters.AddWithValue("$content", entry.Content);
        cmd.Parameters.AddWithValue("$meta", (object?)entry.Metadata ?? DBNull.Value);
        cmd.Parameters.AddWithValue("$c", entry.CreatedAt);
        cmd.ExecuteNonQuery();
        return entry;
    }

    public IReadOnlyList<MemoryEntry> SearchMemories(string query, string? memoryType = null, int limit = 50)
    {
        using var cmd = connection.CreateCommand();
        cmd.CommandText = """
            SELECT id, memory_type, content, metadata, created_at FROM memories
            WHERE ($type IS NULL OR memory_type = $type)
              AND ($q = '' OR content LIKE $like)
            ORDER BY created_at DESC LIMIT $limit
            """;
        cmd.Parameters.AddWithValue("$type", string.IsNullOrWhiteSpace(memoryType) ? DBNull.Value : memoryType);
        cmd.Parameters.AddWithValue("$q", query ?? "");
        cmd.Parameters.AddWithValue("$like", "%" + (query ?? "") + "%");
        cmd.Parameters.AddWithValue("$limit", limit);
        return ReadMemories(cmd);
    }

    public IReadOnlyList<MemoryEntry> GetAllMemories(string? memoryType = null, int limit = 100)
    {
        using var cmd = connection.CreateCommand();
        cmd.CommandText = """
            SELECT id, memory_type, content, metadata, created_at FROM memories
            WHERE ($type IS NULL OR memory_type = $type)
            ORDER BY created_at DESC LIMIT $limit
            """;
        cmd.Parameters.AddWithValue("$type", string.IsNullOrWhiteSpace(memoryType) ? DBNull.Value : memoryType);
        cmd.Parameters.AddWithValue("$limit", limit);
        return ReadMemories(cmd);
    }

    public bool DeleteMemory(string id)
    {
        using var cmd = connection.CreateCommand();
        cmd.CommandText = "DELETE FROM memories WHERE id=$id";
        cmd.Parameters.AddWithValue("$id", id);
        return cmd.ExecuteNonQuery() > 0;
    }

    public Conversation CreateConversation(string title = "New conversation")
    {
        var now = DateTime.UtcNow.ToString("O");
        var convo = new Conversation { Id = Guid.NewGuid().ToString(), Title = title, CreatedAt = now, UpdatedAt = now };
        using var cmd = connection.CreateCommand();
        cmd.CommandText = "INSERT INTO conversations(id,title,created_at,updated_at) VALUES ($id,$title,$c,$u)";
        cmd.Parameters.AddWithValue("$id", convo.Id);
        cmd.Parameters.AddWithValue("$title", convo.Title);
        cmd.Parameters.AddWithValue("$c", convo.CreatedAt);
        cmd.Parameters.AddWithValue("$u", convo.UpdatedAt);
        cmd.ExecuteNonQuery();
        return convo;
    }

    public IReadOnlyList<Conversation> ListConversations()
    {
        using var cmd = connection.CreateCommand();
        cmd.CommandText = "SELECT id,title,created_at,updated_at FROM conversations ORDER BY updated_at DESC";
        var list = new List<Conversation>();
        using var reader = cmd.ExecuteReader();
        while (reader.Read())
        {
            list.Add(new Conversation
            {
                Id = reader.GetString(0),
                Title = reader.GetString(1),
                CreatedAt = reader.GetString(2),
                UpdatedAt = reader.GetString(3)
            });
        }

        return list;
    }

    public ConversationWithMessages? GetConversation(string id)
    {
        using var cmd = connection.CreateCommand();
        cmd.CommandText = "SELECT id,title,created_at,updated_at FROM conversations WHERE id=$id";
        cmd.Parameters.AddWithValue("$id", id);
        using var reader = cmd.ExecuteReader();
        if (!reader.Read()) return null;
        var convo = new ConversationWithMessages
        {
            Id = reader.GetString(0),
            Title = reader.GetString(1),
            CreatedAt = reader.GetString(2),
            UpdatedAt = reader.GetString(3)
        };
        reader.Close();

        using var msg = connection.CreateCommand();
        msg.CommandText = "SELECT id,conversation_id,role,content,created_at FROM conversation_messages WHERE conversation_id=$id ORDER BY created_at";
        msg.Parameters.AddWithValue("$id", id);
        using var mr = msg.ExecuteReader();
        while (mr.Read())
        {
            convo.Messages.Add(new ConversationMessage
            {
                Id = mr.GetString(0),
                ConversationId = mr.GetString(1),
                Role = mr.GetString(2),
                Content = mr.GetString(3),
                CreatedAt = mr.GetString(4)
            });
        }

        return convo;
    }

    public void SaveMessages(string conversationId, IEnumerable<ConversationMessage> messages)
    {
        using var del = connection.CreateCommand();
        del.CommandText = "DELETE FROM conversation_messages WHERE conversation_id=$id";
        del.Parameters.AddWithValue("$id", conversationId);
        del.ExecuteNonQuery();
        foreach (var message in messages)
        {
            using var ins = connection.CreateCommand();
            ins.CommandText = "INSERT INTO conversation_messages(id,conversation_id,role,content,created_at) VALUES ($id,$cid,$role,$content,$c)";
            ins.Parameters.AddWithValue("$id", string.IsNullOrEmpty(message.Id) ? Guid.NewGuid().ToString() : message.Id);
            ins.Parameters.AddWithValue("$cid", conversationId);
            ins.Parameters.AddWithValue("$role", message.Role);
            ins.Parameters.AddWithValue("$content", message.Content);
            ins.Parameters.AddWithValue("$c", string.IsNullOrEmpty(message.CreatedAt) ? DateTime.UtcNow.ToString("O") : message.CreatedAt);
            ins.ExecuteNonQuery();
        }

        using var upd = connection.CreateCommand();
        upd.CommandText = "UPDATE conversations SET updated_at=$u WHERE id=$id";
        upd.Parameters.AddWithValue("$u", DateTime.UtcNow.ToString("O"));
        upd.Parameters.AddWithValue("$id", conversationId);
        upd.ExecuteNonQuery();
    }

    public void UpdateTitle(string id, string title)
    {
        using var cmd = connection.CreateCommand();
        cmd.CommandText = "UPDATE conversations SET title=$title, updated_at=$u WHERE id=$id";
        cmd.Parameters.AddWithValue("$title", title);
        cmd.Parameters.AddWithValue("$u", DateTime.UtcNow.ToString("O"));
        cmd.Parameters.AddWithValue("$id", id);
        cmd.ExecuteNonQuery();
    }

    public void DeleteConversation(string id)
    {
        using var cmd = connection.CreateCommand();
        cmd.CommandText = "DELETE FROM conversation_messages WHERE conversation_id=$id; DELETE FROM conversations WHERE id=$id";
        cmd.Parameters.AddWithValue("$id", id);
        cmd.ExecuteNonQuery();
    }

    public void Dispose() => connection.Dispose();

    private static List<MemoryEntry> ReadMemories(SqliteCommand cmd)
    {
        var list = new List<MemoryEntry>();
        using var reader = cmd.ExecuteReader();
        while (reader.Read())
        {
            list.Add(new MemoryEntry
            {
                Id = reader.GetString(0),
                MemoryType = reader.GetString(1),
                Content = reader.GetString(2),
                Metadata = reader.IsDBNull(3) ? null : reader.GetString(3),
                CreatedAt = reader.GetString(4)
            });
        }

        return list;
    }
}
