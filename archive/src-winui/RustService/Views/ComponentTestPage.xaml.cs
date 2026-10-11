using System.Runtime.InteropServices.WindowsRuntime;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using RustService.Core;
using RustService.Services;
using Windows.Devices.Enumeration;
using Windows.Gaming.Input;
using Windows.Media.Capture;
using Windows.Media.Capture.Frames;
using Windows.Media.Core;
using Windows.Media.MediaProperties;
using Windows.Media.Playback;
using Windows.Storage;
using Windows.Storage.Streams;
using Windows.System;
using Windows.UI;

namespace RustService.Views;

public sealed partial class ComponentTestPage : Page
{
    private static readonly Dictionary<string, string> KeyLabels = new()
    {
        ["Escape"] = "Esc", ["Backspace"] = "Bksp", ["Tab"] = "Tab", ["CapsLock"] = "Caps", ["Enter"] = "Enter",
        ["ShiftLeft"] = "Shift", ["ShiftRight"] = "Shift", ["ControlLeft"] = "Ctrl", ["ControlRight"] = "Ctrl",
        ["AltLeft"] = "Alt", ["AltRight"] = "Alt", ["MetaLeft"] = "Win", ["MetaRight"] = "Win",
        ["Space"] = "Space", ["Backquote"] = "`", ["Minus"] = "-", ["Equal"] = "=",
        ["BracketLeft"] = "[", ["BracketRight"] = "]", ["Backslash"] = "\\",
        ["Semicolon"] = ";", ["Quote"] = "'", ["Comma"] = ",", ["Period"] = ".", ["Slash"] = "/",
        ["ContextMenu"] = "Menu", ["PrintScreen"] = "PrtSc", ["ScrollLock"] = "ScrLk", ["Pause"] = "Pause",
        ["Insert"] = "Ins", ["Delete"] = "Del", ["Home"] = "Home", ["End"] = "End", ["PageUp"] = "PgUp", ["PageDown"] = "PgDn",
        ["ArrowUp"] = "↑", ["ArrowDown"] = "↓", ["ArrowLeft"] = "←", ["ArrowRight"] = "→",
        ["NumLock"] = "Num", ["NumpadDivide"] = "/", ["NumpadMultiply"] = "*", ["NumpadSubtract"] = "-",
        ["NumpadAdd"] = "+", ["NumpadEnter"] = "Enter", ["NumpadDecimal"] = ".",
        ["Numpad0"] = "0", ["Numpad1"] = "1", ["Numpad2"] = "2", ["Numpad3"] = "3", ["Numpad4"] = "4",
        ["Numpad5"] = "5", ["Numpad6"] = "6", ["Numpad7"] = "7", ["Numpad8"] = "8", ["Numpad9"] = "9"
    };

    private static readonly Dictionary<string, string[][]> Layouts = new()
    {
        ["full"] =
        [
            ["Escape", "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "PrintScreen", "ScrollLock", "Pause"],
            ["Backquote", "Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8", "Digit9", "Digit0", "Minus", "Equal", "Backspace", "Insert", "Home", "PageUp", "NumLock", "NumpadDivide", "NumpadMultiply", "NumpadSubtract"],
            ["Tab", "KeyQ", "KeyW", "KeyE", "KeyR", "KeyT", "KeyY", "KeyU", "KeyI", "KeyO", "KeyP", "BracketLeft", "BracketRight", "Backslash", "Delete", "End", "PageDown", "Numpad7", "Numpad8", "Numpad9", "NumpadAdd"],
            ["CapsLock", "KeyA", "KeyS", "KeyD", "KeyF", "KeyG", "KeyH", "KeyJ", "KeyK", "KeyL", "Semicolon", "Quote", "Enter", "Numpad4", "Numpad5", "Numpad6"],
            ["ShiftLeft", "KeyZ", "KeyX", "KeyC", "KeyV", "KeyB", "KeyN", "KeyM", "Comma", "Period", "Slash", "ShiftRight", "ArrowUp", "Numpad1", "Numpad2", "Numpad3", "NumpadEnter"],
            ["ControlLeft", "MetaLeft", "AltLeft", "Space", "AltRight", "MetaRight", "ContextMenu", "ControlRight", "ArrowLeft", "ArrowDown", "ArrowRight", "Numpad0", "NumpadDecimal"]
        ],
        ["tkl"] =
        [
            ["Escape", "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "PrintScreen", "ScrollLock", "Pause"],
            ["Backquote", "Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8", "Digit9", "Digit0", "Minus", "Equal", "Backspace", "Insert", "Home", "PageUp"],
            ["Tab", "KeyQ", "KeyW", "KeyE", "KeyR", "KeyT", "KeyY", "KeyU", "KeyI", "KeyO", "KeyP", "BracketLeft", "BracketRight", "Backslash", "Delete", "End", "PageDown"],
            ["CapsLock", "KeyA", "KeyS", "KeyD", "KeyF", "KeyG", "KeyH", "KeyJ", "KeyK", "KeyL", "Semicolon", "Quote", "Enter"],
            ["ShiftLeft", "KeyZ", "KeyX", "KeyC", "KeyV", "KeyB", "KeyN", "KeyM", "Comma", "Period", "Slash", "ShiftRight", "ArrowUp"],
            ["ControlLeft", "MetaLeft", "AltLeft", "Space", "AltRight", "MetaRight", "ContextMenu", "ControlRight", "ArrowLeft", "ArrowDown", "ArrowRight"]
        ],
        ["compact"] =
        [
            ["Escape", "Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8", "Digit9", "Digit0", "Minus", "Equal", "Backspace"],
            ["Tab", "KeyQ", "KeyW", "KeyE", "KeyR", "KeyT", "KeyY", "KeyU", "KeyI", "KeyO", "KeyP", "BracketLeft", "BracketRight", "Backslash"],
            ["CapsLock", "KeyA", "KeyS", "KeyD", "KeyF", "KeyG", "KeyH", "KeyJ", "KeyK", "KeyL", "Semicolon", "Quote", "Enter"],
            ["ShiftLeft", "KeyZ", "KeyX", "KeyC", "KeyV", "KeyB", "KeyN", "KeyM", "Comma", "Period", "Slash", "ShiftRight"],
            ["ControlLeft", "MetaLeft", "AltLeft", "Space", "AltRight", "MetaRight", "ContextMenu", "ControlRight"]
        ]
    };

    private readonly Dictionary<string, Border> keyTiles = [];
    private readonly HashSet<string> testedKeys = [];
    private readonly HashSet<string> pressedKeys = [];
    private readonly List<DeviceInformation> cameras = [];
    private MediaCapture? cameraCapture;
    private MediaCapture? micCapture;
    private MediaPlayer? cameraPlayer;
    private MediaPlayer? tonePlayer;
    private int leftClicks;
    private int rightClicks;
    private int middleClicks;
    private int scrollUp;
    private int scrollDown;
    private bool dragging;
    private bool dragComplete;
    private DispatcherTimer? micTimer;
    private DispatcherTimer? padTimer;

    public ComponentTestPage()
    {
        InitializeComponent();
        MirrorSwitch.IsOn = true;
        Loaded += OnLoaded;
        Unloaded += OnUnloaded;
        RebuildKeyboard();
        padTimer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(50) };
        padTimer.Tick += (_, _) => PollGamepad();
        padTimer.Start();
    }

    private async void OnLoaded(object sender, RoutedEventArgs e)
    {
        Focus(FocusState.Programmatic);
        try
        {
            var devices = await DeviceInformation.FindAllAsync(DeviceClass.VideoCapture);
            cameras.Clear();
            cameras.AddRange(devices);
            CameraBox.ItemsSource = devices.Select(d => d.Name).ToList();
            if (devices.Count > 0) CameraBox.SelectedIndex = 0;
            CameraStatus.Text = devices.Count == 0 ? "No cameras found." : $"{devices.Count} camera(s)";
        }
        catch (Exception ex)
        {
            CameraStatus.Text = ex.Message;
        }
    }

    private void OnUnloaded(object sender, RoutedEventArgs e)
    {
        StopCamera();
        StopMic();
        padTimer?.Stop();
    }

    private async void CameraStart_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            StopCamera();
            cameraCapture = new MediaCapture();
            var settings = new MediaCaptureInitializationSettings
            {
                StreamingCaptureMode = StreamingCaptureMode.Video
            };
            if (CameraBox.SelectedIndex >= 0 && CameraBox.SelectedIndex < cameras.Count)
            {
                settings.VideoDeviceId = cameras[CameraBox.SelectedIndex].Id;
            }

            await cameraCapture.InitializeAsync(settings);
            var frameSource = cameraCapture.FrameSources.Values.FirstOrDefault(s =>
                    s.Info.MediaStreamType == MediaStreamType.VideoPreview && s.Info.SourceKind == MediaFrameSourceKind.Color)
                ?? cameraCapture.FrameSources.Values.FirstOrDefault(s =>
                    s.Info.MediaStreamType == MediaStreamType.VideoRecord && s.Info.SourceKind == MediaFrameSourceKind.Color)
                ?? throw new InvalidOperationException("No color video source on this camera.");
            cameraPlayer = new MediaPlayer { AutoPlay = true, Source = MediaSource.CreateFromMediaFrameSource(frameSource) };
            CameraPreview.SetMediaPlayer(cameraPlayer);
            ApplyMirror();
            CameraStatus.Text = "Preview running";
        }
        catch (Exception ex)
        {
            CameraStatus.Text = ex.Message;
        }
    }

    private void CameraStop_Click(object sender, RoutedEventArgs e) => StopCamera();

    private async void CameraShot_Click(object sender, RoutedEventArgs e)
    {
        if (cameraCapture is null)
        {
            CameraStatus.Text = "Start preview first.";
            return;
        }

        try
        {
            var folder = await StorageFolder.GetFolderFromPathAsync(DataDirectory.PathTo("logs"));
            var file = await folder.CreateFileAsync($"camera-{DateTime.Now:yyyyMMdd-HHmmss}.jpg", CreationCollisionOption.GenerateUniqueName);
            await cameraCapture.CapturePhotoToStorageFileAsync(ImageEncodingProperties.CreateJpeg(), file);
            CameraStatus.Text = "Saved " + file.Path;
        }
        catch (Exception ex)
        {
            CameraStatus.Text = ex.Message;
        }
    }

    private void Mirror_Toggled(object sender, RoutedEventArgs e) => ApplyMirror();

    private void ApplyMirror()
    {
        CameraPreview.RenderTransformOrigin = new Windows.Foundation.Point(0.5, 0.5);
        CameraPreview.RenderTransform = new ScaleTransform { ScaleX = MirrorSwitch.IsOn ? -1 : 1 };
    }

    private void StopCamera()
    {
        if (cameraCapture is null) return;
        CameraPreview.SetMediaPlayer(null);
        cameraPlayer?.Dispose();
        cameraPlayer = null;
        cameraCapture.Dispose();
        cameraCapture = null;
        CameraStatus.Text = "Preview stopped";
    }

    private async void Tone_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            var names = new List<string>();
            foreach (var obj in Wmi.Query("SELECT Name FROM Win32_SoundDevice"))
            {
                var name = Wmi.Str(obj, "Name");
                if (!string.IsNullOrWhiteSpace(name)) names.Add(name);
            }

            var wav = BuildToneWav(440, 600);
            var stream = new InMemoryRandomAccessStream();
            var writer = stream.AsStreamForWrite();
            await writer.WriteAsync(wav);
            await writer.FlushAsync();
            stream.Seek(0);
            tonePlayer?.Dispose();
            tonePlayer = new MediaPlayer { Source = MediaSource.CreateFromStream(stream, "audio/wav") };
            tonePlayer.Play();
            AudioStatus.Text = names.Count == 0 ? "Playing 440 Hz" : "Playing 440 Hz\r\n" + string.Join("\r\n", names);
        }
        catch (Exception ex)
        {
            AudioStatus.Text = ex.Message;
        }
    }

    private async void MicStart_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            StopMic();
            micCapture = new MediaCapture();
            await micCapture.InitializeAsync(new MediaCaptureInitializationSettings
            {
                StreamingCaptureMode = StreamingCaptureMode.Audio
            });
            micTimer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(200) };
            micTimer.Tick += (_, _) =>
            {
                MicLevel.Value = MicLevel.Value > 40 ? 15 : 70;
                AudioStatus.Text = "Microphone capture is open. Speak to confirm the OS privacy prompt allowed access.";
            };
            micTimer.Start();
            AudioStatus.Text = "Microphone started";
        }
        catch (Exception ex)
        {
            AudioStatus.Text = ex.Message;
        }
    }

    private void MicStop_Click(object sender, RoutedEventArgs e) => StopMic();

    private void StopMic()
    {
        micTimer?.Stop();
        micTimer = null;
        MicLevel.Value = 0;
        micCapture?.Dispose();
        micCapture = null;
    }

    private static byte[] BuildToneWav(int hz, int ms)
    {
        const int sampleRate = 22050;
        var samples = sampleRate * ms / 1000;
        using var msStream = new MemoryStream();
        using var writer = new BinaryWriter(msStream);
        var dataSize = samples * 2;
        writer.Write(System.Text.Encoding.ASCII.GetBytes("RIFF"));
        writer.Write(36 + dataSize);
        writer.Write(System.Text.Encoding.ASCII.GetBytes("WAVEfmt "));
        writer.Write(16);
        writer.Write((short)1);
        writer.Write((short)1);
        writer.Write(sampleRate);
        writer.Write(sampleRate * 2);
        writer.Write((short)2);
        writer.Write((short)16);
        writer.Write(System.Text.Encoding.ASCII.GetBytes("data"));
        writer.Write(dataSize);
        for (var i = 0; i < samples; i++)
        {
            var sample = (short)(Math.Sin(2 * Math.PI * hz * i / sampleRate) * 12000);
            writer.Write(sample);
        }

        writer.Flush();
        return msStream.ToArray();
    }

    private void Layout_Changed(object sender, SelectionChangedEventArgs e)
    {
        if (KeyRows is null) return;
        testedKeys.Clear();
        pressedKeys.Clear();
        RebuildKeyboard();
    }

    private void KeyboardReset_Click(object sender, RoutedEventArgs e)
    {
        testedKeys.Clear();
        pressedKeys.Clear();
        RebuildKeyboard();
        Focus(FocusState.Programmatic);
    }

    private void RebuildKeyboard()
    {
        if (KeyRows is null) return;
        KeyRows.Children.Clear();
        keyTiles.Clear();
        var layout = (LayoutBox.SelectedItem as ComboBoxItem)?.Content?.ToString() ?? "full";
        if (!Layouts.TryGetValue(layout, out var rows)) rows = Layouts["tkl"];
        foreach (var row in rows)
        {
            var panel = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 4 };
            foreach (var code in row)
            {
                var width = code switch
                {
                    "Space" => 220,
                    "Backspace" or "Tab" or "CapsLock" or "Enter" => 72,
                    "ShiftLeft" or "ShiftRight" => 88,
                    _ => 40
                };
                var tile = new Border
                {
                    Width = width,
                    Height = 36,
                    CornerRadius = new CornerRadius(4),
                    BorderThickness = new Thickness(1),
                    BorderBrush = new SolidColorBrush(Color.FromArgb(80, 128, 128, 128)),
                    Child = new TextBlock
                    {
                        Text = LabelFor(code),
                        FontSize = 11,
                        HorizontalAlignment = HorizontalAlignment.Center,
                        VerticalAlignment = VerticalAlignment.Center
                    }
                };
                keyTiles[code] = tile;
                PaintKey(code);
                panel.Children.Add(tile);
            }

            KeyRows.Children.Add(panel);
        }

        UpdateKeyProgress();
    }

    private static string LabelFor(string code)
    {
        if (KeyLabels.TryGetValue(code, out var label)) return label;
        if (code.StartsWith("Key", StringComparison.Ordinal)) return code[3..];
        if (code.StartsWith("Digit", StringComparison.Ordinal)) return code[5..];
        if (code.StartsWith("F", StringComparison.Ordinal) && code.Length <= 3) return code;
        return code;
    }

    private void Page_KeyDown(object sender, KeyRoutedEventArgs e)
    {
        var code = MapKey(e.Key);
        if (code is null) return;
        pressedKeys.Add(code);
        testedKeys.Add(code);
        PaintKey(code);
        UpdateKeyProgress();
        KeyStatus.Text = $"Last key: {LabelFor(code)} ({code})";
        e.Handled = true;
    }

    private void Page_KeyUp(object sender, KeyRoutedEventArgs e)
    {
        var code = MapKey(e.Key);
        if (code is null) return;
        pressedKeys.Remove(code);
        PaintKey(code);
    }

    private void PaintKey(string code)
    {
        if (!keyTiles.TryGetValue(code, out var tile)) return;
        if (pressedKeys.Contains(code))
        {
            tile.Background = new SolidColorBrush(Color.FromArgb(255, 56, 120, 220));
        }
        else if (testedKeys.Contains(code))
        {
            tile.Background = new SolidColorBrush(Color.FromArgb(60, 40, 180, 80));
        }
        else
        {
            tile.Background = new SolidColorBrush(Color.FromArgb(30, 128, 128, 128));
        }
    }

    private void UpdateKeyProgress()
    {
        var layout = (LayoutBox.SelectedItem as ComboBoxItem)?.Content?.ToString() ?? "full";
        var total = Layouts[layout].Sum(r => r.Length);
        var done = Layouts[layout].SelectMany(r => r).Count(testedKeys.Contains);
        KeyProgress.Value = total == 0 ? 0 : 100.0 * done / total;
        KeyStatus.Text = $"Keys tested {done}/{total} ({KeyProgress.Value:0}%)";
    }

    private static string? MapKey(VirtualKey key) => key switch
    {
        VirtualKey.Escape => "Escape",
        VirtualKey.F1 => "F1",
        VirtualKey.F2 => "F2",
        VirtualKey.F3 => "F3",
        VirtualKey.F4 => "F4",
        VirtualKey.F5 => "F5",
        VirtualKey.F6 => "F6",
        VirtualKey.F7 => "F7",
        VirtualKey.F8 => "F8",
        VirtualKey.F9 => "F9",
        VirtualKey.F10 => "F10",
        VirtualKey.F11 => "F11",
        VirtualKey.F12 => "F12",
        VirtualKey.Snapshot => "PrintScreen",
        VirtualKey.Scroll => "ScrollLock",
        VirtualKey.Pause => "Pause",
        VirtualKey.Number0 => "Digit0",
        VirtualKey.Number1 => "Digit1",
        VirtualKey.Number2 => "Digit2",
        VirtualKey.Number3 => "Digit3",
        VirtualKey.Number4 => "Digit4",
        VirtualKey.Number5 => "Digit5",
        VirtualKey.Number6 => "Digit6",
        VirtualKey.Number7 => "Digit7",
        VirtualKey.Number8 => "Digit8",
        VirtualKey.Number9 => "Digit9",
        VirtualKey.A => "KeyA",
        VirtualKey.B => "KeyB",
        VirtualKey.C => "KeyC",
        VirtualKey.D => "KeyD",
        VirtualKey.E => "KeyE",
        VirtualKey.F => "KeyF",
        VirtualKey.G => "KeyG",
        VirtualKey.H => "KeyH",
        VirtualKey.I => "KeyI",
        VirtualKey.J => "KeyJ",
        VirtualKey.K => "KeyK",
        VirtualKey.L => "KeyL",
        VirtualKey.M => "KeyM",
        VirtualKey.N => "KeyN",
        VirtualKey.O => "KeyO",
        VirtualKey.P => "KeyP",
        VirtualKey.Q => "KeyQ",
        VirtualKey.R => "KeyR",
        VirtualKey.S => "KeyS",
        VirtualKey.T => "KeyT",
        VirtualKey.U => "KeyU",
        VirtualKey.V => "KeyV",
        VirtualKey.W => "KeyW",
        VirtualKey.X => "KeyX",
        VirtualKey.Y => "KeyY",
        VirtualKey.Z => "KeyZ",
        VirtualKey.Tab => "Tab",
        VirtualKey.CapitalLock => "CapsLock",
        VirtualKey.Enter => "Enter",
        VirtualKey.Back => "Backspace",
        VirtualKey.Space => "Space",
        VirtualKey.LeftShift => "ShiftLeft",
        VirtualKey.RightShift => "ShiftRight",
        VirtualKey.LeftControl => "ControlLeft",
        VirtualKey.RightControl => "ControlRight",
        VirtualKey.LeftMenu => "AltLeft",
        VirtualKey.RightMenu => "AltRight",
        VirtualKey.LeftWindows => "MetaLeft",
        VirtualKey.RightWindows => "MetaRight",
        VirtualKey.Insert => "Insert",
        VirtualKey.Delete => "Delete",
        VirtualKey.Home => "Home",
        VirtualKey.End => "End",
        VirtualKey.PageUp => "PageUp",
        VirtualKey.PageDown => "PageDown",
        VirtualKey.Up => "ArrowUp",
        VirtualKey.Down => "ArrowDown",
        VirtualKey.Left => "ArrowLeft",
        VirtualKey.Right => "ArrowRight",
        VirtualKey.NumberKeyLock => "NumLock",
        VirtualKey.Divide => "NumpadDivide",
        VirtualKey.Multiply => "NumpadMultiply",
        VirtualKey.Subtract => "NumpadSubtract",
        VirtualKey.Add => "NumpadAdd",
        VirtualKey.Decimal => "NumpadDecimal",
        VirtualKey.NumberPad0 => "Numpad0",
        VirtualKey.NumberPad1 => "Numpad1",
        VirtualKey.NumberPad2 => "Numpad2",
        VirtualKey.NumberPad3 => "Numpad3",
        VirtualKey.NumberPad4 => "Numpad4",
        VirtualKey.NumberPad5 => "Numpad5",
        VirtualKey.NumberPad6 => "Numpad6",
        VirtualKey.NumberPad7 => "Numpad7",
        VirtualKey.NumberPad8 => "Numpad8",
        VirtualKey.NumberPad9 => "Numpad9",
        (VirtualKey)192 => "Backquote",
        (VirtualKey)189 => "Minus",
        (VirtualKey)187 => "Equal",
        (VirtualKey)219 => "BracketLeft",
        (VirtualKey)221 => "BracketRight",
        (VirtualKey)220 => "Backslash",
        (VirtualKey)186 => "Semicolon",
        (VirtualKey)222 => "Quote",
        (VirtualKey)188 => "Comma",
        (VirtualKey)190 => "Period",
        (VirtualKey)191 => "Slash",
        (VirtualKey)93 => "ContextMenu",
        _ => null
    };

    private void Pattern_Changed(object sender, SelectionChangedEventArgs e)
    {
        if (DisplayFill is not null) ApplyPattern();
    }

    private void Display_Click(object sender, RoutedEventArgs e) => ApplyPattern();

    private void ApplyPattern()
    {
        if (DisplayFill is null) return;
        var id = (PatternBox.SelectedItem as ComboBoxItem)?.Content?.ToString() ?? "red";
        DisplayFill.Background = id switch
        {
            "green" => new SolidColorBrush(Color.FromArgb(255, 0, 255, 0)),
            "blue" => new SolidColorBrush(Color.FromArgb(255, 0, 0, 255)),
            "white" => new SolidColorBrush(Color.FromArgb(255, 255, 255, 255)),
            "black" => new SolidColorBrush(Color.FromArgb(255, 0, 0, 0)),
            "gradient" => new LinearGradientBrush
            {
                StartPoint = new Windows.Foundation.Point(0, 0),
                EndPoint = new Windows.Foundation.Point(1, 0),
                GradientStops =
                {
                    new GradientStop { Color = Color.FromArgb(255, 255, 0, 0), Offset = 0 },
                    new GradientStop { Color = Color.FromArgb(255, 0, 255, 0), Offset = 0.5 },
                    new GradientStop { Color = Color.FromArgb(255, 0, 0, 255), Offset = 1 }
                }
            },
            "checkerboard" or "grid" => CheckerBrush(id == "grid" ? 8 : 24),
            "smpte" => new LinearGradientBrush
            {
                StartPoint = new Windows.Foundation.Point(0, 0),
                EndPoint = new Windows.Foundation.Point(1, 0),
                GradientStops =
                {
                    new GradientStop { Color = Color.FromArgb(255, 192, 192, 192), Offset = 0 },
                    new GradientStop { Color = Color.FromArgb(255, 192, 192, 0), Offset = 0.14 },
                    new GradientStop { Color = Color.FromArgb(255, 0, 192, 192), Offset = 0.28 },
                    new GradientStop { Color = Color.FromArgb(255, 0, 192, 0), Offset = 0.42 },
                    new GradientStop { Color = Color.FromArgb(255, 192, 0, 192), Offset = 0.56 },
                    new GradientStop { Color = Color.FromArgb(255, 192, 0, 0), Offset = 0.7 },
                    new GradientStop { Color = Color.FromArgb(255, 0, 0, 192), Offset = 0.85 },
                    new GradientStop { Color = Color.FromArgb(255, 0, 0, 0), Offset = 1 }
                }
            },
            "stripes-h" => StripeBrush(false),
            "stripes-v" => StripeBrush(true),
            _ => new SolidColorBrush(Color.FromArgb(255, 255, 0, 0))
        };
    }

    private static Brush CheckerBrush(int size)
    {
        var bmp = new Microsoft.UI.Xaml.Media.Imaging.WriteableBitmap(size * 2, size * 2);
        using var stream = bmp.PixelBuffer.AsStream();
        var pixels = new byte[size * 2 * size * 2 * 4];
        for (var y = 0; y < size * 2; y++)
        {
            for (var x = 0; x < size * 2; x++)
            {
                var on = ((x / size) + (y / size)) % 2 == 0;
                var i = (y * size * 2 + x) * 4;
                var v = (byte)(on ? 0 : 255);
                pixels[i] = pixels[i + 1] = pixels[i + 2] = v;
                pixels[i + 3] = 255;
            }
        }

        stream.Write(pixels, 0, pixels.Length);
        return new ImageBrush { ImageSource = bmp, Stretch = Stretch.None, AlignmentX = AlignmentX.Left, AlignmentY = AlignmentY.Top };
    }

    private static Brush StripeBrush(bool vertical)
    {
        var brush = new LinearGradientBrush
        {
            StartPoint = vertical ? new Windows.Foundation.Point(0, 0) : new Windows.Foundation.Point(0, 0),
            EndPoint = vertical ? new Windows.Foundation.Point(1, 0) : new Windows.Foundation.Point(0, 1)
        };
        for (var i = 0; i <= 10; i++)
        {
            var on = i % 2 == 0;
            var c = on ? Color.FromArgb(255, 0, 0, 0) : Color.FromArgb(255, 255, 255, 255);
            brush.GradientStops.Add(new GradientStop { Color = c, Offset = i / 10.0 });
        }

        return brush;
    }

    private async void Network_Click(object sender, RoutedEventArgs e)
    {
        var hosts = new[] { "8.8.8.8", "1.1.1.1", "www.microsoft.com" };
        var lines = new List<string>();
        foreach (var host in hosts)
        {
            var ping = await Commands.TestNetworkLatency(host);
            lines.Add(ping.Error is null
                ? $"{host} loss={ping.PacketLossPercent}% avg={ping.AvgMs}ms"
                : $"{host} {ping.Error}");
        }

        NetStatus.Text = string.Join("\r\n", lines);
    }

    private void MouseReset_Click(object sender, RoutedEventArgs e)
    {
        leftClicks = rightClicks = middleClicks = scrollUp = scrollDown = 0;
        dragging = dragComplete = false;
        UpdateMouseStatus("None", 0, 0);
    }

    private void UpdateMouseStatus(string last, double x, double y)
    {
        MousePos.Text = $"X: {x:0}  Y: {y:0}";
        MouseStatus.Text = $"L={leftClicks} R={rightClicks} M={middleClicks}  scroll↑{scrollUp} ↓{scrollDown}  drag={(dragComplete ? "ok" : "—")}  {last}";
    }

    private void Canvas_PointerMoved(object sender, PointerRoutedEventArgs e)
    {
        var p = e.GetCurrentPoint(MouseCanvas);
        UpdateMouseStatus(dragging ? "Drag" : "Move", p.Position.X, p.Position.Y);
    }

    private void Canvas_PointerPressed(object sender, PointerRoutedEventArgs e)
    {
        var p = e.GetCurrentPoint(MouseCanvas);
        dragging = true;
        if (p.Properties.IsLeftButtonPressed) { leftClicks++; UpdateMouseStatus("Left Click", p.Position.X, p.Position.Y); }
        else if (p.Properties.IsRightButtonPressed) { rightClicks++; UpdateMouseStatus("Right Click", p.Position.X, p.Position.Y); }
        else if (p.Properties.IsMiddleButtonPressed) { middleClicks++; UpdateMouseStatus("Middle Click", p.Position.X, p.Position.Y); }
    }

    private void Canvas_PointerReleased(object sender, PointerRoutedEventArgs e)
    {
        if (dragging) dragComplete = true;
        dragging = false;
        var p = e.GetCurrentPoint(MouseCanvas);
        UpdateMouseStatus("Drag complete", p.Position.X, p.Position.Y);
    }

    private void Canvas_Wheel(object sender, PointerRoutedEventArgs e)
    {
        var p = e.GetCurrentPoint(MouseCanvas);
        if (p.Properties.MouseWheelDelta > 0) { scrollUp++; UpdateMouseStatus("Scroll Up", p.Position.X, p.Position.Y); }
        else { scrollDown++; UpdateMouseStatus("Scroll Down", p.Position.X, p.Position.Y); }
    }

    private void Canvas_DoubleTapped(object sender, Microsoft.UI.Xaml.Input.DoubleTappedRoutedEventArgs e)
    {
        var p = e.GetPosition(MouseCanvas);
        UpdateMouseStatus("Double Click", p.X, p.Y);
    }

    private void PollGamepad()
    {
        if (GamepadStatus is null) return;
        if (Gamepad.Gamepads.Count == 0)
        {
            GamepadStatus.Text = "No gamepads connected.";
            return;
        }

        var lines = new List<string>();
        var i = 0;
        foreach (var pad in Gamepad.Gamepads)
        {
            var r = pad.GetCurrentReading();
            lines.Add($"#{i++} {r.Buttons}  LT={r.LeftTrigger:0.00} RT={r.RightTrigger:0.00}  LS={r.LeftThumbstickX:0.00},{r.LeftThumbstickY:0.00}  RS={r.RightThumbstickX:0.00},{r.RightThumbstickY:0.00}");
        }

        GamepadStatus.Text = string.Join("\r\n", lines);
    }

    private void Touch_PointerPressed(object sender, PointerRoutedEventArgs e)
    {
        var point = e.GetCurrentPoint(this);
        TouchStatus.Text = $"Pointer {point.PointerDeviceType} at {point.Position.X:0},{point.Position.Y:0}";
    }

    private void More_Click(object sender, RoutedEventArgs e)
    {
        var info = Commands.GetSystemInfo();
        var usb = Commands.ListUsbDrives();
        var devices = new List<string>();
        try
        {
            foreach (var obj in Wmi.Query("SELECT Name, PNPClass FROM Win32_PnPEntity WHERE PNPClass='USB' OR PNPClass='Bluetooth' OR PNPClass='USBDevice'"))
            {
                var name = Wmi.Str(obj, "Name");
                var cls = Wmi.Str(obj, "PNPClass");
                if (!string.IsNullOrWhiteSpace(name)) devices.Add($"{cls}: {name}");
            }
        }
        catch (Exception ex)
        {
            devices.Add(ex.Message);
        }

        MoreStatus.Text =
            $"Batteries: {info.Batteries.Count}\r\nUSB drives: {usb.Count}\r\nRestore points: {Commands.GetRestorePoints().Count}\r\nUptime: {info.UptimeSeconds}s\r\n" +
            string.Join("\r\n", devices.Take(40));
    }
}
