using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.IO;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Media.Imaging;
using WixToolset.BootstrapperApplicationApi;

namespace Forja.Setup
{
    internal partial class SetupWindow : Window
    {
        private static readonly string[] ProtectedRoots =
        {
            Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles),
            Environment.GetFolderPath(Environment.SpecialFolder.ProgramFilesX86),
            Environment.GetFolderPath(Environment.SpecialFolder.Windows),
            Environment.GetFolderPath(Environment.SpecialFolder.CommonApplicationData),
        };

        private readonly ForjaBootstrapper ba;
        private bool busy;
        private bool finished;

        internal SetupWindow(ForjaBootstrapper ba)
        {
            this.ba = ba;

            InitializeComponent();

            Theme.Apply(Resources, Theme.IsDark);
            Theme.Watch(this, dark =>
            {
                Theme.Apply(Resources, dark);
                Theme.ApplyTitleBar(this, dark);
            });

            // La barra de título necesita el HWND, que recién existe acá.
            SourceInitialized += (s, e) => Theme.ApplyTitleBar(this, Theme.IsDark);

            try
            {
                Logo.Source = new BitmapImage(new Uri("pack://application:,,,/forge.ico"));
            }
            catch
            {
                // sin logo el instalador sigue funcionando
            }

            var version = ba.BundleVersion;
            VersionText.Text = string.IsNullOrEmpty(version)
                ? "Toolchain del lenguaje Forja"
                : "Toolchain del lenguaje Forja · versión " + version;

            LicenseBox.Text = LoadLicense();

            var manifest = InstallManifest.Read();
            DirBox.Text = manifest != null && Directory.Exists(manifest.Path)
                ? manifest.Path
                : DefaultFolder();

            FeatLsp.IsChecked = manifest == null || manifest.Has("LanguageServer");
            FeatDap.IsChecked = manifest == null || manifest.Has("Debugger");
            FeatRt.IsChecked = manifest == null || manifest.Has("Runtime");

            Loaded += (s, e) => AcceptCheck.Focus();
        }

        /// <summary>
        /// Antes de pedir UAC: la ventana se esconde. Windows minimiza la ventana
        /// que lanza el prompt y después queda un ícono colgado en la barra de
        /// tareas mientras corre la instancia elevada, que es otro proceso.
        /// </summary>
        internal void MinimizeForElevation()
        {
            WindowState = WindowState.Minimized;
            ShowInTaskbar = false;
        }

        /// <summary>Si el usuario dice que no en el UAC, la ventana vuelve como estaba.</summary>
        internal void RestoreAfterElevation()
        {
            ShowInTaskbar = true;
            BringToFront();
        }

        /// <summary>
        /// Trae la ventana al frente sin dejarla minimizada. Hace falta cuando
        /// este proceso nace desde el UAC (instancia elevada): el escritorio
        /// seguro puede dejarla atrás o minimizada.
        /// </summary>
        internal void BringToFront()
        {
            if (WindowState == WindowState.Minimized)
            {
                WindowState = WindowState.Normal;
            }

            Show();
            Activate();

            var hwnd = new System.Windows.Interop.WindowInteropHelper(this).Handle;
            if (hwnd != IntPtr.Zero)
            {
                // Toggle de Topmost: si el proceso no es el del frente (volvimos
                // del escritorio seguro del UAC), SetForegroundWindow solo no
                // alcanza.
                Topmost = true;
                Topmost = false;
                SetForegroundWindow(hwnd);
            }
        }

        /// <summary>
        /// Muestra el progreso mientras Windows resuelve el UAC. La ventana puede
        /// venir minimizada por el propio prompt, así que se restaura antes.
        /// </summary>
        internal void ShowElevating()
        {
            if (WindowState == WindowState.Minimized)
            {
                WindowState = WindowState.Normal;
            }

            ShowProgress();
            SetProgress(0);
            AppendActivity("Esperando los permisos de administrador…");
        }

        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool SetForegroundWindow(IntPtr hwnd);

        internal bool Silent { get; set; }

        internal void ShowReady(LaunchAction action)
        {
            var uninstall = action == LaunchAction.Uninstall;
            var repair = action == LaunchAction.Repair || action == LaunchAction.Modify;

            ReadyHeading.Text = uninstall ? "Desinstalar Forja" : repair ? "Reparar Forja" : "Instalar Forja";
            ReadyIntro.Text = uninstall
                ? "Se van a quitar Forja y los componentes instalados en este equipo."
                : repair
                    ? "Se van a restaurar los archivos de Forja que falten o estén dañados."
                    : "Elegí qué componentes instalar y dónde. El compilador forja y la biblioteca estándar son obligatorios.";

            LicenseBox.Visibility = uninstall || repair ? Visibility.Collapsed : Visibility.Visible;
            AcceptCheck.Visibility = uninstall || repair ? Visibility.Collapsed : Visibility.Visible;
            FeatLsp.Visibility = FeatDap.Visibility = FeatRt.Visibility =
                uninstall || repair ? Visibility.Collapsed : Visibility.Visible;
            DirBox.IsEnabled = !uninstall;
            BrowseButton.Visibility = uninstall ? Visibility.Collapsed : Visibility.Visible;

            UpdateScopeBanner();
            InstallButton.Content = uninstall ? "Desinstalar" : repair ? "Reparar" : "Instalar";
            InstallButton.IsEnabled = uninstall || repair || AcceptCheck.IsChecked == true;

            FooterStatus.Text = "";
            ShowPage(ReadyPage);
        }

        /// <summary>Aviso de alcance según los permisos reales del instalador.</summary>
        internal void UpdateScopeBanner()
        {
            ScopeText.Text = ba.IsPrivileged
                ? "Se instala en Archivos de programa para todos los usuarios y el PATH se agrega al entorno del sistema."
                : "Se instala en Archivos de programa para todos los usuarios. Al continuar, Windows va a pedir permisos de administrador (UAC).";
        }

        /// <summary>Tras conceder permisos, la carpeta por defecto pasa a Archivos de programa.</summary>
        internal void ResetFolderToDefault()
        {
            DirBox.Text = DefaultFolder();
        }

        internal void ShowProgress()
        {
            busy = true;
            ProgressHeading.Text = ba.Action == LaunchAction.Uninstall ? "Desinstalando Forja" : "Instalando Forja";
            ProgressIntro.Text = "Detalle de lo que está pasando. No cierres esta ventana.";
            ProgressText.Text = "Preparando…";
            Bar.Value = 0;
            ActivityList.Items.Clear();

            InstallButton.Visibility = Visibility.Collapsed;
            BrowseButton.IsEnabled = false;
            CancelButton.IsEnabled = true;
            FooterStatus.Text = "";

            ShowPage(ProgressPage);
        }

        internal void SetProgress(int percent)
        {
            Bar.Value = Math.Max(0, Math.Min(100, percent));
            ProgressText.Text = percent + " %";
        }

        /// <summary>Agrega una línea al detalle en vivo.</summary>
        internal void AppendActivity(string line)
        {
            if (string.IsNullOrWhiteSpace(line) || ActivityList == null)
            {
                return;
            }

            ActivityList.Items.Add(line.Trim());

            while (ActivityList.Items.Count > 400)
            {
                ActivityList.Items.RemoveAt(0);
            }

            if (ActivityList.Items.Count > 0)
            {
                ActivityList.ScrollIntoView(ActivityList.Items[ActivityList.Items.Count - 1]);
            }
        }

        internal void ShowFinish(LaunchAction action)
        {
            finished = true;
            busy = false;

            FinishHeading.Text = action == LaunchAction.Uninstall ? "Forja se desinstaló" : "Forja quedó instalado";
            FinishHeading.Foreground = (System.Windows.Media.Brush)Resources["Ok"];
            FinishText.Text = action == LaunchAction.Uninstall
                ? "Se quitaron los archivos, la entrada del PATH y la asociación de archivos .fa."
                : "Abrí una consola nueva y ejecutá  forja ayuda  para empezar. Después,  forja actualizar  busca versiones nuevas.";
            FinishHint.Text = ba.IsPrivileged
                ? "El PATH quedó en el entorno del sistema; aplica a las consolas nuevas."
                : "El PATH quedó en tu entorno de usuario; aplica a las consolas nuevas.";

            CancelButton.Visibility = Visibility.Collapsed;
            InstallButton.Visibility = Visibility.Collapsed;
            CloseButton.Visibility = Visibility.Visible;
            FooterStatus.Text = "";

            ShowPage(FinishPage);
        }

        internal void ShowError(string message)
        {
            finished = true;
            busy = false;

            FinishHeading.Text = "No se pudo completar";
            FinishHeading.Foreground = (System.Windows.Media.Brush)Resources["Danger"];
            FinishText.Text = message;
            FinishHint.Text = "";

            CancelButton.Visibility = Visibility.Collapsed;
            InstallButton.Visibility = Visibility.Collapsed;
            CloseButton.Visibility = Visibility.Visible;

            ShowPage(FinishPage);
        }

        private void ShowPage(FrameworkElement page)
        {
            ReadyPage.Visibility = Visibility.Collapsed;
            ProgressPage.Visibility = Visibility.Collapsed;
            FinishPage.Visibility = Visibility.Collapsed;
            page.Visibility = Visibility.Visible;
        }

        private void AcceptCheck_Changed(object sender, RoutedEventArgs e)
        {
            if (InstallButton != null)
            {
                InstallButton.IsEnabled = AcceptCheck.IsChecked == true;
            }
        }

        private void Browse_Click(object sender, RoutedEventArgs e)
        {
            using (var dialog = new System.Windows.Forms.FolderBrowserDialog())
            {
                dialog.Description = "Elegí la carpeta donde instalar Forja";
                dialog.ShowNewFolderButton = true;

                var current = DirBox.Text;
                if (!string.IsNullOrEmpty(current) && Directory.Exists(current))
                {
                    dialog.SelectedPath = current;
                }

                if (dialog.ShowDialog() == System.Windows.Forms.DialogResult.OK)
                {
                    DirBox.Text = dialog.SelectedPath;
                }
            }
        }

        private void Install_Click(object sender, RoutedEventArgs e)
        {
            if (AcceptCheck.Visibility == Visibility.Visible && AcceptCheck.IsChecked != true)
            {
                FooterStatus.Text = "Tenés que aceptar la licencia para continuar.";
                return;
            }

            if (DirBox.IsEnabled)
            {
                var dir = DirBox.Text == null ? "" : DirBox.Text.Trim();
                if (dir.Length == 0 || dir.IndexOfAny(Path.GetInvalidPathChars()) >= 0 || !Path.IsPathRooted(dir))
                {
                    FooterStatus.Text = "Elegí una carpeta de instalación válida (ruta absoluta).";
                    return;
                }

                var problem = ValidateFolder(dir);
                if (problem != null)
                {
                    FooterStatus.Text = problem;
                    return;
                }

                DirBox.Text = dir;
            }

            // Requisito: runtime de Visual C++ (los .exe enlazan VCRUNTIME140.dll).
            // Se avisa y se ofrece instalarlo; sin él, forja.exe no arranca, así
            // que no se sigue hasta que esté.
            if (ForjaBootstrapper.FaltaVCRuntime())
            {
                var elegido = System.Windows.MessageBox.Show(
                    "Forja necesita el runtime de Visual C++ 2015-2022 (x64) y esta máquina no lo tiene.\n\n" +
                    "Los ejecutables lo usan para arrancar; sin él, forja.exe falla al abrirse.\n\n" +
                    "¿Querés abrir la descarga oficial de Microsoft? Instalala y después volvé acá y dale a Instalar.",
                    "Falta un requisito",
                    MessageBoxButton.YesNo,
                    MessageBoxImage.Warning);

                if (elegido == MessageBoxResult.Yes)
                {
                    try
                    {
                        System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo
                        {
                            FileName = "https://aka.ms/vs/17/release/vc_redist.x64.exe",
                            UseShellExecute = true,
                        });
                        FooterStatus.Text = "Instalá el runtime de Visual C++ y después dale a Instalar de nuevo.";
                    }
                    catch (Exception ex)
                    {
                        FooterStatus.Text = "No pude abrir el navegador. Bajalo de https://aka.ms/vs/17/release/vc_redist.x64.exe — " + ex.Message;
                    }
                }
                else
                {
                    FooterStatus.Text = "Sin el runtime de Visual C++ la instalación no va a funcionar.";
                }

                return;
            }

            FooterStatus.Text = "";
            ba.BeginInstall(DirBox.Text, SelectedFeatures());
        }

        /// <summary>
        /// La instalación corre elevada (MSI per-machine), así que cualquier
        /// carpeta absoluta es válida.
        /// </summary>
        private string ValidateFolder(string dir)
        {
            return null;
        }

        private string ValidateFolderLegacy(string dir)
        {
            string full;
            try
            {
                full = Path.GetFullPath(dir).TrimEnd('\\');
            }
            catch
            {
                return "Esa ruta no es válida.";
            }

            foreach (var root in ProtectedRoots)
            {
                if (!string.IsNullOrEmpty(root) &&
                    full.StartsWith(root.TrimEnd('\\'), StringComparison.OrdinalIgnoreCase))
                {
                    return "Esa carpeta requiere permisos de administrador. Elegí una carpeta de tu perfil o usá «Reintentar como administrador».";
                }
            }

            // Probar escritura en el ancestro que ya existe.
            var probe = full;
            while (!Directory.Exists(probe) && Path.GetDirectoryName(probe) != null)
            {
                probe = Path.GetDirectoryName(probe);
            }

            try
            {
                var test = Path.Combine(probe, ".forja-permiso");
                using (File.Create(test))
                {
                }

                File.Delete(test);
            }
            catch (Exception)
            {
                return "No tenés permisos para escribir en esa carpeta. Elegí otra de tu perfil o usá «Reintentar como administrador».";
            }

            return null;
        }

        private List<string> SelectedFeatures()
        {
            var features = new List<string> { "Core" };

            if (FeatLsp.IsChecked == true)
            {
                features.Add("LanguageServer");
            }

            if (FeatDap.IsChecked == true)
            {
                features.Add("Debugger");
            }

            if (FeatRt.IsChecked == true)
            {
                features.Add("Runtime");
            }

            return features;
        }

        private void Cancel_Click(object sender, RoutedEventArgs e)
        {
            if (busy)
            {
                CancelButton.IsEnabled = false;
                FooterStatus.Text = "Cancelando…";
            }

            finished = true;
            ba.Cancel();
            Close();
        }

        private void Close_Click(object sender, RoutedEventArgs e)
        {
            Close();
        }

        protected override void OnClosing(CancelEventArgs e)
        {
            // En modo silencioso el cierre de la ventana oculta no es un cancelar.
            if (!finished && !Silent)
            {
                finished = true;
                ba.Cancel();
            }

            base.OnClosing(e);
        }

        private string UserFolder()
        {
            return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), @"Programs\Forja");
        }

        private string DefaultFolder()
        {
            var configured = ba.Engine.GetVariableString("InstallFolder");
            if (!string.IsNullOrEmpty(configured) && configured.IndexOf('[') < 0)
            {
                return configured;
            }

            return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles), "Forja");
        }

        private static string LoadLicense()
        {
            using (var stream = typeof(SetupWindow).Assembly.GetManifestResourceStream("LICENSE.md"))
            {
                if (stream == null)
                {
                    return "GNU General Public License v3. Ver LICENSE.md en el repositorio.";
                }

                using (var reader = new StreamReader(stream))
                {
                    var text = reader.ReadToEnd();
                    var lines = text.Replace("\r\n", "\n").Split('\n');
                    for (var i = 0; i < lines.Length; i++)
                    {
                        lines[i] = lines[i].TrimStart('#').Trim();
                    }

                    return string.Join(Environment.NewLine, lines);
                }
            }
        }
    }
}
