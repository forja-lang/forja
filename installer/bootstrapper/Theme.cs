using System;
using System.Runtime.InteropServices;
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

        // Atributos de DWM para pintar la barra de título del color de la app.
        private const int DwmUseImmersiveDarkMode = 20;      // Win10 2004+
        private const int DwmUseImmersiveDarkModeLegacy = 19; // Win10 1809/1903
        private const int DwmBorderColor = 34;               // Win11
        private const int DwmCaptionColor = 35;              // Win11
        private const int DwmTextColor = 36;                 // Win11

        [DllImport("dwmapi.dll")]
        private static extern int DwmSetWindowAttribute(IntPtr hwnd, int atributo, ref int valor, int tamano);

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

        /// <summary>
        /// Pinta la barra de título nativa como el resto de la ventana. Sin esto,
        /// el marco y el título quedan claros aunque el contenido sea oscuro.
        /// El modo oscuro inmersivo viene de Windows 10 2004 (atributo 20; en
        /// builds anteriores era 19). Los colores explícitos son de Windows 11:
        /// en Windows 10 la llamada falla y queda el modo oscuro del sistema.
        /// </summary>
        internal static void ApplyTitleBar(Window window, bool dark)
        {
            var hwnd = new WindowInteropHelper(window).Handle;
            if (hwnd == IntPtr.Zero)
            {
                return;
            }

            var oscuro = dark ? 1 : 0;
            if (DwmSetWindowAttribute(hwnd, DwmUseImmersiveDarkMode, ref oscuro, sizeof(int)) != 0)
            {
                DwmSetWindowAttribute(hwnd, DwmUseImmersiveDarkModeLegacy, ref oscuro, sizeof(int));
            }

            var titulo = ColorRef(dark ? "#232329" : "#F6F7F9");
            var texto = ColorRef(dark ? "#F2F3F5" : "#1A1B1F");
            var borde = ColorRef(dark ? "#3A3A44" : "#E1E3E8");
            DwmSetWindowAttribute(hwnd, DwmCaptionColor, ref titulo, sizeof(int));
            DwmSetWindowAttribute(hwnd, DwmTextColor, ref texto, sizeof(int));
            DwmSetWindowAttribute(hwnd, DwmBorderColor, ref borde, sizeof(int));
        }

        /// <summary>DWM espera COLORREF: 0x00BBGGRR, al revés que un color web.</summary>
        private static int ColorRef(string hex)
        {
            var c = (Color)ColorConverter.ConvertFromString(hex);
            return c.R | (c.G << 8) | (c.B << 16);
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
