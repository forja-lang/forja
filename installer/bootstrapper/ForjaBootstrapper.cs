using System;
using System.Collections.Generic;
using System.Security.Principal;
using System.Threading;
using System.Windows;
using Microsoft.Win32;
using WixToolset.BootstrapperApplicationApi;

namespace Forja.Setup
{
    internal class ForjaBootstrapper : BootstrapperApplication
    {
        private readonly List<string> errors = new List<string>();
        private Application app;

        public int ExitCode { get; private set; }

        internal IEngine Engine { get; private set; }

        internal IBootstrapperCommand Command { get; private set; }

        internal SetupWindow Window { get; private set; }

        internal LaunchAction Action { get; private set; }

        internal int OverallProgress { get; private set; }

        internal List<string> Features { get; private set; }

        internal string InstallFolder { get; private set; }

        private bool silent;

        /// <summary>
        /// Permisos del proceso del instalador. Se mira el token, no
        /// `Engine.EvaluateCondition("Privileged")`: esa variable del motor
        /// puede venir en 1 sin que este proceso pueda escribir en carpetas
        /// protegidas, y ya nos hizo instalar en el lugar equivocado.
        /// </summary>
        internal bool IsPrivileged
        {
            get
            {
                try
                {
                    using (var identity = WindowsIdentity.GetCurrent())
                    {
                        return new WindowsPrincipal(identity).IsInRole(WindowsBuiltInRole.Administrator);
                    }
                }
                catch
                {
                    return false;
                }
            }
        }

        protected override void OnCreate(CreateEventArgs args)
        {
            base.OnCreate(args);

            Engine = args.Engine;
            Command = args.Command;
            Action = Command.Action;
            silent = Command.Display == Display.None || Command.Display == Display.Embedded;
            Features = new List<string> { "Core", "LanguageServer", "Debugger", "Runtime" };

            Engine.Log(LogLevel.Verbose, "Forja setup: acción " + Action);
        }

        protected override void Run()
        {
            // La UI (y el bombeo de mensajes que entrega los callbacks del
            // engine) vive en su propio hilo STA; este hilo no toca COM.
            var ui = new Thread(UiThread) { IsBackground = true, Name = "ForjaSetupUI" };
            ui.SetApartmentState(ApartmentState.STA);
            ui.Start();
            ui.Join();

            Engine.Quit(ExitCode);
        }

        private void UiThread()
        {
            app = new Application { ShutdownMode = ShutdownMode.OnExplicitShutdown };

            Window = new SetupWindow(this) { Silent = silent };
            app.MainWindow = Window;

            if (!silent)
            {
                Window.Show();
            }

            Engine.Detect();
            app.Run();
        }

        private void ShutdownUi()
        {
            if (app != null)
            {
                app.Dispatcher.BeginInvoke(new Action(() => app.Shutdown()));
            }
        }

        // ── Llamadas desde la UI ────────────────────────────────────────────

        /// <summary>
        /// Pide permisos cuando el usuario quiere instalar en una carpeta
        /// protegida: relanza el bundle elevado (UAC) y cierra esta instancia.
        /// Sin permisos no se puede escribir en Archivos de programa, así que
        /// este es el único momento en que se molesta al usuario.
        /// </summary>
        internal void RestartElevated(string folder, List<string> features)
        {
            var source = Engine.GetVariableString("WixBundleOriginalSource");
            if (string.IsNullOrEmpty(source) || !System.IO.File.Exists(source))
            {
                Ui(() => Window.ShowError("No pude ubicar el instalador para pedir permisos. Ejecutalo de nuevo desde donde lo bajaste."));
                return;
            }

            try
            {
                // El pedido viaja por archivo: con UAC no se heredan variables
                // de entorno y Burn no toma `InstallFolder=` de la línea de comandos.
                InstallManifest.WriteRequest(folder, features == null ? null : string.Join(",", features));

                var info = new System.Diagnostics.ProcessStartInfo(source)
                {
                    UseShellExecute = true,
                    Verb = "runas",
                    Arguments = "/install",
                };

                System.Diagnostics.Process.Start(info);
            }
            catch (Exception ex)
            {
                Engine.Log(LogLevel.Error, ex.ToString());
                Ui(() => Window.ShowError("No se otorgaron los permisos de administrador. Se puede instalar en una carpeta de tu perfil, que no los necesita."));
                return;
            }

            // La ventana nueva hace la instalación; esta se cierra.
            ExitCode = 0;
            Ui(() => Window.Close());
        }

        /// <summary>Carpeta registrada por la instalación actual, si hay una.</summary>
        internal string InstalledFolder
        {
            get
            {
                var manifest = InstallManifest.Read();
                return manifest == null ? null : manifest.Path;
            }
        }

        /// <summary>
        /// Cambiar carpeta o componentes no se puede hacer desde adentro del
        /// propio instalador (Burn no se desinstala a sí mismo), así que se
        /// explica qué hacer. El CLI sí lo automatiza porque corre aparte.
        /// </summary>
        internal void AvisarCambio(string folder)
        {
            var instalada = InstalledFolder;
            var mensaje =
                (string.IsNullOrEmpty(instalada) ? "" : "Forja ya está instalado en:\n" + instalada + "\n\n") +
                "Para cambiar la carpeta o los componentes hay que desinstalar y volver a instalar:\n" +
                "  1) Configuración > Aplicaciones > Forja > Desinstalar\n" +
                "  2) Volver a ejecutar este instalador\n\n" +
                "O desde una consola:  forja desinstalar   y después   forja actualizar";

            Ui(() => Window.ShowError(mensaje));
        }

        /// <summary>
        /// Burn no puede cambiar features de un MSI ya instalado (planifica
        /// None), así que cualquier cambio de componentes o de carpeta pasa por
        /// desinstalar y reinstalar. Devuelve true si hay que reinstalar.
        /// </summary>
        private bool HaceFaltaReinstalar()
        {
            if (Action != LaunchAction.Install)
            {
                return false;
            }

            var manifest = InstallManifest.Read();
            var carpetaInstalada = manifest == null ? null : manifest.Path;

            if (string.IsNullOrEmpty(carpetaInstalada) || !System.IO.Directory.Exists(carpetaInstalada))
            {
                return false;
            }

            var actuales = FeaturesEnDisco(carpetaInstalada);
            var mismosComponentes = actuales.Count == Features.Count && actuales.TrueForAll(f => Features.Contains(f));
            var mismaCarpeta = string.Equals(carpetaInstalada.TrimEnd('\\'),
                                             (InstallFolder ?? "").TrimEnd('\\'),
                                             StringComparison.OrdinalIgnoreCase);

            if (mismosComponentes && mismaCarpeta)
            {
                return false;
            }

            Engine.Log(LogLevel.Verbose, "Forja setup: cambiar requiere reinstalar (en disco: " +
                       string.Join(",", actuales) + " | pedido: " + string.Join(",", Features) + ")");

            return true;
        }

        /// <summary>Componentes que realmente están instalados (por sus .exe).</summary>
        private static List<string> FeaturesEnDisco(string carpeta)
        {
            var features = new List<string>();

            foreach (var par in new[]
                     {
                         new[] { "LanguageServer", "forja-lsp.exe" },
                         new[] { "Debugger", "forja-dap.exe" },
                         new[] { "Runtime", "forja-rt.exe" },
                     })
            {
                if (System.IO.File.Exists(System.IO.Path.Combine(carpeta, par[1])))
                {
                    features.Add(par[0]);
                }
            }

            return features;
        }

        internal void BeginInstall(string targetDir, List<string> features)
        {
            errors.Clear();
            OverallProgress = 0;
            InstallFolder = targetDir;
            Features = features;

            // El runtime de VC++ lo avisa y lo pide la UI (SetupWindow). En
            // silencio no hay a quién preguntarle: se sigue y se deja constancia,
            // porque abortar rompería instalaciones desatendidas que después
            // resuelven el runtime por su cuenta.
            if (silent && FaltaVCRuntime())
            {
                Engine.Log(LogLevel.Error,
                    "Falta el runtime de Visual C++ 2015-2022 (x64): forja.exe no va a arrancar.");
            }

            // Cambiar componentes o carpeta: no se puede hacer en caliente.
            if (HaceFaltaReinstalar())
            {
                AvisarCambio(targetDir);
                return;
            }

            SetSelection();
            Engine.Plan(Action);
        }

        /// <summary>Pasa carpeta y componentes al MSI (ADDLOCAL vía Burn).</summary>
        private void SetSelection()
        {
            // En una desinstalación también hay que pasar la carpeta: sin ella
            // Burn manda INSTALLDIR="" y MSI busca los archivos en la raíz, así
            // que no borra nada y deja todo instalado.
            if (Action == LaunchAction.Uninstall)
            {
                if (string.IsNullOrEmpty(InstallFolder))
                {
                    var manifest = InstallManifest.Read();
                    InstallFolder = manifest != null && !string.IsNullOrEmpty(manifest.Path)
                        ? manifest.Path
                        : DefaultFolderForScope();
                }

                Engine.Log(LogLevel.Verbose, "Forja setup: desinstalando desde " + InstallFolder);
                Engine.SetVariableString("InstallFolder", InstallFolder, false);
                return;
            }

            if (Action != LaunchAction.Install && Action != LaunchAction.Modify && Action != LaunchAction.Repair)
            {
                return;
            }

            // Orden de prioridad: pedido explícito (relanzamiento con permisos
            // o `forja actualizar`) > manifiesto > carpeta por defecto.
            // No se lee `InstallFolder` del engine: Burn restaura el valor de la
            // sesión anterior y pisaba la carpeta nueva.
            // Una instalación silenciosa (o sin manifiesto) llega acá sin
            // carpeta: usar la del alcance en curso y nunca pasar un valor vacío
            // (Burn normaliza un valor vacío a la raíz del disco, C:\).
            if (string.IsNullOrEmpty(InstallFolder))
            {
                InstallFolder = DefaultFolderForScope();
            }

            Engine.Log(LogLevel.Verbose, "Forja setup: carpeta=" + InstallFolder +
                       " privilegiado=" + IsPrivileged);
            Engine.SetVariableString("InstallFolder", InstallFolder, false);

            // Burn arma ADDLOCAL con las variables que llevan el nombre de
            // cada feature del MSI (2 = ausente, 3 = local).
            foreach (var feature in new[] { "LanguageServer", "Debugger", "Runtime" })
            {
                // FeatureState: 1 = ausente, 3 = local (2 = anunciado, que Burn ignora).
                Engine.SetVariableNumeric(feature, Features.Contains(feature) ? 3 : 1);
            }
        }

        /// <summary>
        /// En modo silencioso (por ejemplo `forja actualizar`) se reusa lo que
        /// quedó registrado en el manifiesto: misma carpeta, mismos componentes.
        /// </summary>
        private void LoadManifestSelection()
        {
            var manifest = InstallManifest.Read();

            // Solo se respeta la carpeta del manifiesto si todavía existe: si el
            // usuario la borró, se vuelve a la carpeta por defecto.
            if (manifest != null)
            {
                if (!string.IsNullOrEmpty(manifest.Path) && System.IO.Directory.Exists(manifest.Path))
                {
                    InstallFolder = manifest.Path;
                }

                Features = manifest.Features;
            }

            // Pedido explícito (relanzamiento con permisos o `forja actualizar`).
            // No se consume en una desinstalación: si no, se pierde la carpeta
            // que el usuario eligió para la reinstalación.
            if (Action == LaunchAction.Install || Action == LaunchAction.Modify || Action == LaunchAction.Repair)
            {
                var request = InstallManifest.TakeRequest();

                if (!string.IsNullOrEmpty(request.Folder))
                {
                    InstallFolder = request.Folder;
                }

                if (!string.IsNullOrEmpty(request.Componentes))
                {
                    Features = ParsearComponentes(request.Componentes);
                }
            }

            // `forja actualizar componentes lsp,dap` llega por variable del bundle.
            var componentes = Engine.GetVariableString("Componentes");
            if (!string.IsNullOrEmpty(componentes))
            {
                Features = ParsearComponentes(componentes);
            }

            SetSelection();
        }

        private static List<string> ParsearComponentes(string texto)
        {
            var features = new List<string> { "Core" };

            foreach (var parte in texto.Split(new[] { ',', ';', ' ' }, StringSplitOptions.RemoveEmptyEntries))
            {
                var id = parte.Trim().ToLowerInvariant() switch
                {
                    "lsp" or "lenguaje" or "languageserver" => "LanguageServer",
                    "dap" or "depurador" or "debugger" => "Debugger",
                    "rt" or "runtime" or "forja-rt" => "Runtime",
                    _ => null,
                };

                if (id != null && !features.Contains(id))
                {
                    features.Add(id);
                }
            }

            return features;
        }

        internal void Cancel()
        {
            ExitCode = 1602;
            Engine.Quit(ExitCode);
        }

        internal IntPtr WindowHandle()
        {
            if (Window == null)
            {
                return IntPtr.Zero;
            }

            return new System.Windows.Interop.WindowInteropHelper(Window).EnsureHandle();
        }

        internal string BundleVersion
        {
            get
            {
                var version = Engine.GetVariableString("WixBundleVersion");
                return string.IsNullOrEmpty(version) ? "" : version;
            }
        }

        // ── Callbacks del engine ────────────────────────────────────────────

        private string DefaultFolderForScope()
        {
            // La instalación es para todos los usuarios: Archivos de programa,
            // PATH del sistema y UAC. MSI no puede cambiar de alcance en
            // caliente (lo aprendimos por las malas), así que hay uno solo.
            var carpeta = System.IO.Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles), "Forja");

            Engine.Log(LogLevel.Verbose, "Forja setup: carpeta por defecto '" + carpeta +
                       "' privilegiado=" + IsPrivileged);

            return carpeta;
        }

        protected override void OnDetectMsiFeature(DetectMsiFeatureEventArgs args)
        {
            base.OnDetectMsiFeature(args);

            Engine.Log(LogLevel.Verbose, "Forja setup: feature " + args.FeatureId + " = " + args.State);
        }

        protected override void OnDetectComplete(DetectCompleteEventArgs args)
        {
            base.OnDetectComplete(args);

            if (args.Status != 0)
            {
                Fail("No se pudo detectar el estado de la instalación.", args.HResult);
                return;
            }

            if (silent)
            {
                LoadManifestSelection();

                // En silencio no se puede preguntar: se avisa por el log y se corta.
                if (HaceFaltaReinstalar())
                {
                    Engine.Log(LogLevel.Error, "Cambiar carpeta o componentes requiere desinstalar antes " +
                                               "(forja desinstalar y después forja actualizar).");
                    ExitCode = 1602;
                    ShutdownUi();
                    return;
                }

                Engine.Plan(Action);
                return;
            }

            Ui(() => Window.ShowReady(Action));
        }

        protected override void OnPlanComplete(PlanCompleteEventArgs args)
        {
            base.OnPlanComplete(args);

            if (args.Status != 0)
            {
                if (silent)
                {
                    ExitCode = 1603;
                    ShutdownUi();
                }
                else
                {
                    Ui(() => Window.ShowError("No se pudo preparar la instalación."));
                }

                return;
            }

            // Apply necesita un HWND de la ventana: tocar todo en el hilo de UI.
            Ui(() =>
            {
                if (!silent)
                {
                    Window.ShowProgress();
                }

                Engine.Apply(WindowHandle());
            });
        }

        protected override void OnProgress(ProgressEventArgs args)
        {
            base.OnProgress(args);

            OverallProgress = args.OverallPercentage;

            if (!silent)
            {
                Ui(() => Window.SetProgress(args.OverallPercentage));
            }
        }

        protected override void OnExecutePackageBegin(ExecutePackageBeginEventArgs args)
        {
            base.OnExecutePackageBegin(args);

            if (!silent)
            {
                Ui(() => Window.AppendActivity(Action == LaunchAction.Uninstall
                    ? "Quitando paquete: " + args.PackageId
                    : "Instalando paquete: " + args.PackageId));
            }
        }

        protected override void OnExecuteMsiMessage(ExecuteMsiMessageEventArgs args)
        {
            base.OnExecuteMsiMessage(args);

            if (silent || string.IsNullOrEmpty(args.Message))
            {
                return;
            }

            var text = args.Message;
            if (args.Data != null && args.Data.Count > 0)
            {
                try
                {
                    text = string.Format(text, args.Data);
                }
                catch
                {
                    text = args.Message + " " + string.Join(" ", args.Data);
                }
            }

            Ui(() => Window.AppendActivity(text));
        }

        protected override void OnError(ErrorEventArgs args)
        {
            base.OnError(args);

            var message = args.ErrorMessage;
            if (string.IsNullOrEmpty(message))
            {
                message = "Código de error " + args.ErrorCode;
            }

            errors.Add(message);
            Engine.Log(LogLevel.Error, message);
        }

        protected override void OnApplyComplete(ApplyCompleteEventArgs args)
        {
            base.OnApplyComplete(args);

            if (args.Status == 0)
            {
                // También en modo silencioso: `forja actualizar` depende de esto.
                SaveManifest();
            }

            if (silent)
            {
                ExitCode = args.Status == 0 ? 0 : 1603;
                ShutdownUi();
                return;
            }

            Ui(() =>
            {
                if (args.Status == 0)
                {
                    Window.ShowFinish(Action);
                }
                else
                {
                    Window.ShowError(errors.Count > 0
                        ? string.Join(Environment.NewLine, errors)
                        : "La instalación falló.");
                }
            });
        }

        private void SaveManifest()
        {
            if (Action == LaunchAction.Uninstall)
            {
                try
                {
                    System.IO.File.Delete(InstallManifest.UserPath);
                    System.IO.File.Delete(InstallManifest.MachinePath);
                }
                catch
                {
                    // si no se puede borrar, la próxima detección lo ignora
                }

                return;
            }

            new InstallManifest
            {
                Version = BundleVersion,
                Path = InstallFolder,
                Scope = "user",
                Features = Features,
            }.Write();
        }

        /// <summary>
        /// ¿Falta el runtime de Visual C++ 2015-2022 (x64)? Los ejecutables
        /// enlazan VCRUNTIME140.dll dinámicamente y esa DLL no viene con Windows:
        /// sin el redistribuible, forja.exe no arranca. La clave es la que escribe
        /// el propio redistribuible; se lee la vista de 64 bits (el .exe es x64).
        /// </summary>
        internal static bool FaltaVCRuntime()
        {
            const string clave = @"SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64";
            try
            {
                using (var k = Registry.LocalMachine.OpenSubKey(clave))
                {
                    if (k == null)
                    {
                        return true;
                    }

                    var instalado = k.GetValue("Installed", 0);
                    return Convert.ToInt32(instalado) != 1;
                }
            }
            catch
            {
                // Si no se puede leer, no se bloquea al usuario por las dudas.
                return false;
            }
        }

        private void Fail(string message, int error)
        {
            errors.Add(message + " (" + error + ")");

            if (silent)
            {
                ExitCode = 1603;
                ShutdownUi();
            }
            else
            {
                Ui(() => Window.ShowError(message));
            }
        }

        private void Ui(Action action)
        {
            var window = Window;
            if (window == null)
            {
                action();
                return;
            }

            window.Dispatcher.BeginInvoke(Guarded(action));
        }

        /// <summary>Un callback de la UI nunca debe tumbar la instalación.</summary>
        private Action Guarded(Action action)
        {
            return () =>
            {
                try
                {
                    action();
                }
                catch (Exception ex)
                {
                    Engine.Log(LogLevel.Error, ex.ToString());
                }
            };
        }
    }
}
