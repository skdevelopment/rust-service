using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using RustService.Services;

namespace RustService;

public partial class App : Application
{
    public static Window? Window { get; private set; }

    public App()
    {
        InitializeComponent();
        UnhandledException += (_, e) =>
        {
            Log("unhandled " + e.Exception);
            try { MainWindow.Shell?.ShowStatus("Unhandled error", e.Exception.Message, InfoBarSeverity.Error); } catch { /* keep going */ }
            e.Handled = true;
        };
        TaskScheduler.UnobservedTaskException += (_, e) =>
        {
            Log("unobserved-task " + e.Exception);
            e.SetObserved();
        };
        AppDomain.CurrentDomain.UnhandledException += (_, e) =>
            Log("domain-unhandled " + e.ExceptionObject);
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        try
        {
            Log("launch-start");
            AppServices.Initialize(startMcp: false);
            _ = Commands.EnsureDataDir();
            _ = Commands.GetSettings();
            Log("services-ready");

            if (Environment.GetCommandLineArgs().Any(a => a.Equals("--headless-init", StringComparison.OrdinalIgnoreCase)))
            {
                Log("headless-init");
                Log(SmokeCheck.Run());
                Environment.Exit(0);
                return;
            }

            Log("creating-window");
            Window = new MainWindow();
            Log("window-constructed");
            Window.Activate();
            Log("main-window-ok");
            ThemeController.Apply();
            AppServices.StartMcpIfEnabled();
            Log("mcp-ready");

            if (Environment.GetCommandLineArgs().Any(a => a.Equals("--ui-smoke", StringComparison.OrdinalIgnoreCase))
                && Window is MainWindow shell)
            {
                shell.DispatcherQueue.TryEnqueue(() =>
                {
                    try { shell.VisitAllTabs(); }
                    catch (Exception ex) { Log("ui-smoke-failed " + ex); }
                });
            }
        }
        catch (Exception ex)
        {
            Log("launch-failed " + ex);
            try
            {
                Window ??= new Window { Title = "RustService" };
                Window.Content = new TextBlock
                {
                    Text = "RustService failed to start:\n" + ex,
                    Margin = new Thickness(24),
                    TextWrapping = TextWrapping.Wrap
                };
                Window.Activate();
            }
            catch (Exception inner)
            {
                Log("fallback-window-failed " + inner);
            }
        }
    }

    public static void Log(string message)
    {
        try
        {
            var dir = Path.Combine(AppContext.BaseDirectory, "data", "logs");
            Directory.CreateDirectory(dir);
            File.AppendAllText(Path.Combine(dir, "startup.log"), $"{DateTime.Now:O} {message}{Environment.NewLine}");
        }
        catch
        {
            // logging must never throw
        }
    }
}
