using System;
using System.Windows;
using System.Windows.Interop;
using System.Windows.Media;
using Microsoft.Win32;

namespace Forja.Setup
{
    /// <summary>Paleta clara/oscura siguiendo la preferencia de Windows.</summary>
    internal static class Theme
    {
        private const string PersonalizeKey = @"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
        private const int WmSettingChange = 0x001A;

        internal static bool IsDark
        {
            get
            {
                using (var key = Registry.CurrentUser.OpenSubKey(PersonalizeKey))
                {
                    var value = key == null ? null : key.GetValue("AppsUseLightTheme");
                    if (value is int light)
                    {
                        return light == 0;
                    }
                }

                return false;
            }
        }

        internal static void Apply(ResourceDictionary res, bool dark)
        {
            res["Bg"] = Brush(dark ? "#1B1B20" : "#FFFFFF");
            res["Panel"] = Brush(dark ? "#232329" : "#F6F7F9");
            res["Border"] = Brush(dark ? "#3A3A44" : "#E1E3E8");
            res["Text"] = Brush(dark ? "#F2F3F5" : "#1A1B1F");
            res["Muted"] = Brush(dark ? "#A2A8B4" : "#5B6270");
            res["Accent"] = Brush(dark ? "#F97316" : "#C2410C");
            res["AccentHover"] = Brush(dark ? "#FB923C" : "#9A3208");
            res["AccentText"] = Brush(dark ? "#111114" : "#FFFFFF");
            res["Danger"] = Brush(dark ? "#F87171" : "#B91C1C");
            res["Ok"] = Brush(dark ? "#4ADE80" : "#15803D");
        }

        /// <summary>Reaplica la paleta cuando el usuario cambia el tema en Windows.</summary>
        internal static void Watch(Window window, Action<bool> onChange)
        {
            var source = (HwndSource)PresentationSource.FromVisual(window);
            if (source == null)
            {
                return;
            }

            source.AddHook((IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam, ref bool handled) =>
            {
                if (msg == WmSettingChange && lParam != IntPtr.Zero)
                {
                    onChange(IsDark);
                }

                return IntPtr.Zero;
            });
        }

        private static SolidColorBrush Brush(string hex)
        {
            var brush = new SolidColorBrush((Color)ColorConverter.ConvertFromString(hex));
            brush.Freeze();
            return brush;
        }
    }
}
