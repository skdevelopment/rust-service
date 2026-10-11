using System.Security.Cryptography;
using System.Text;
using RustService.Core;
using RustService.Models;

namespace RustService.Services;

public sealed class AgentFiles
{
    public string Read(string path) => File.ReadAllText(path);
    public void Write(string path, string content)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        File.WriteAllText(path, content);
    }

    public IReadOnlyList<string> ListDir(string path) =>
        Directory.Exists(path)
            ? Directory.GetFileSystemEntries(path).Select(Path.GetFileName).OfType<string>().ToList()
            : [];

    public void Move(string src, string dest)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(dest)!);
        File.Move(src, dest, true);
    }

    public void Copy(string src, string dest)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(dest)!);
        File.Copy(src, dest, true);
    }

    public void Edit(string path, string oldText, string newText)
    {
        var content = File.ReadAllText(path);
        File.WriteAllText(path, content.Replace(oldText, newText));
    }

    public IReadOnlyList<string> Grep(string root, string pattern)
    {
        if (!Directory.Exists(root)) return [];
        return Directory.EnumerateFiles(root, "*", SearchOption.AllDirectories)
            .Where(file =>
            {
                try { return File.ReadAllText(file).Contains(pattern, StringComparison.OrdinalIgnoreCase); }
                catch { return false; }
            }).Take(200).ToList();
    }

    public IReadOnlyList<string> Glob(string root, string pattern) =>
        Directory.Exists(root) ? Directory.GetFiles(root, pattern, SearchOption.AllDirectories) : [];

    public IReadOnlyList<string> ListInstruments()
    {
        var dir = DataDirectory.PathTo("instruments");
        return Directory.Exists(dir) ? Directory.GetFiles(dir) : [];
    }

    public IReadOnlyList<string> ListAgentPrograms() =>
        Directory.Exists(DataDirectory.PathTo("programs"))
            ? Directory.GetFiles(DataDirectory.PathTo("programs"), "*.exe", SearchOption.AllDirectories)
            : [];

    public string? FindExe(string name) =>
        Directory.EnumerateFiles(DataDirectory.PathTo("programs"), name, SearchOption.AllDirectories).FirstOrDefault();

    public FileAttachment SaveUpload(string fileName, string mime, string contentBase64)
    {
        var bytes = Convert.FromBase64String(contentBase64);
        var id = Guid.NewGuid().ToString();
        var stored = $"{id}_{fileName}";
        var relative = Path.Combine("agent", "files", "uploaded", stored);
        File.WriteAllBytes(DataDirectory.PathTo(relative), bytes);
        var attachment = new FileAttachment
        {
            Id = id,
            Source = FileSource.Upload,
            OriginalName = fileName,
            StoredName = stored,
            MimeType = mime,
            Size = bytes.Length,
            StoredPath = relative.Replace('\\', '/'),
            Checksum = Convert.ToHexString(SHA256.HashData(bytes))
        };
        JsonUtil.Save(DataDirectory.PathTo("agent", "files", "uploaded", $"{id}.meta.json"), attachment);
        return attachment;
    }

    public FileAttachment Generate(string filename, string content, string description)
    {
        var id = Guid.NewGuid().ToString();
        var stored = $"{id}_{filename}";
        var relative = Path.Combine("agent", "files", "generated", stored);
        File.WriteAllText(DataDirectory.PathTo(relative), content);
        var attachment = new FileAttachment
        {
            Id = id,
            Source = FileSource.Generated,
            OriginalName = filename,
            StoredName = stored,
            MimeType = "text/plain",
            Size = Encoding.UTF8.GetByteCount(content),
            StoredPath = relative.Replace('\\', '/'),
            Content = content,
            Checksum = Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(content)))
        };
        JsonUtil.Save(DataDirectory.PathTo("agent", "files", "generated", $"{id}.meta.json"), attachment);
        return attachment;
    }

    public IReadOnlyList<FileAttachment> List()
    {
        var files = new List<FileAttachment>();
        foreach (var meta in Directory.GetFiles(DataDirectory.PathTo("agent", "files"), "*.meta.json", SearchOption.AllDirectories))
        {
            files.Add(JsonUtil.Load(meta, new FileAttachment()));
        }

        return files;
    }

    public bool Delete(string id)
    {
        var match = List().FirstOrDefault(f => f.Id == id);
        if (match is null) return false;
        var abs = DataDirectory.PathTo(match.StoredPath.Replace('/', Path.DirectorySeparatorChar));
        if (File.Exists(abs)) File.Delete(abs);
        var meta = Path.ChangeExtension(abs, ".meta.json");
        if (!File.Exists(meta))
        {
            meta = Directory.GetFiles(DataDirectory.PathTo("agent", "files"), $"{id}.meta.json", SearchOption.AllDirectories).FirstOrDefault();
        }

        if (meta is not null && File.Exists(meta)) File.Delete(meta);
        return true;
    }

    public object GetFileInfo(string path)
    {
        var info = new FileInfo(path);
        return new
        {
            name = info.Name,
            path = info.FullName,
            exists = info.Exists,
            size = info.Exists ? info.Length : 0,
            isDir = Directory.Exists(path),
            modified = info.Exists ? info.LastWriteTimeUtc.ToString("O") : null
        };
    }

    public string ReadBinaryBase64(string path) => Convert.ToBase64String(File.ReadAllBytes(path));

    public PathValidationResult ValidatePath(string path)
    {
        try
        {
            var full = Path.GetFullPath(path);
            var sandbox = Path.GetFullPath(DataDirectory.Root);
            return new PathValidationResult
            {
                Valid = true,
                SanitizedPath = full,
                WithinSandbox = full.StartsWith(sandbox, StringComparison.OrdinalIgnoreCase)
            };
        }
        catch (Exception ex)
        {
            return new PathValidationResult { Valid = false, Error = ex.Message };
        }
    }
}
