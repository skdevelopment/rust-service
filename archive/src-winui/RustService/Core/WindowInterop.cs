using Microsoft.UI.Xaml;
using WinRT.Interop;

namespace RustService;

public static class WindowInterop
{
    public static nint Handle()
    {
        if (App.Window is null) return 0;
        return WindowNative.GetWindowHandle(App.Window);
    }

    public static void BindPicker(object picker)
    {
        var hwnd = Handle();
        if (hwnd != 0) InitializeWithWindow.Initialize(picker, hwnd);
    }
}
