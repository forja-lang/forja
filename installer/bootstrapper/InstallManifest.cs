using System;
using System.Collections.Generic;
using System.IO;
using System.Text;

namespace Forja.Setup
{
    /// <summary>
    /// Registro de lo instalado (carpeta, alcance y componentes) para que
    /// `forja actualizar` sepa qué reemplazar. Se escribe en ProgramData para
    /// instalaciones de máquina y en %LOCALAPPDATA% para las de usuario.
    /// </summary>
    internal class InstallManifest
    {
        internal string Version { get; set; }

        internal string Path { get; set; }

        internal string Scope { get; set; }

        internal List<string> Features { get; set; }

        internal bool Has(string feature)
        {
            return Features != null && Features.Contains(feature);
        }

        internal static string MachinePath
        {
            get { return System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.CommonApplicationData), @"Forja\install.json"); }
        }

        internal static string UserPath
        {
            get { return System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), @"Forja\install.json"); }
        }

        /// <summary>
        /// Pedido puntual para la próxima ejecución del instalador (carpeta y
        /// componentes). Burn ignora `InstallFolder=` por línea de comandos y
        /// las variables de entorno no sobreviven al relanzamiento con UAC, así
        /// que la UI y `forja actualizar` dejan el pedido en este archivo.
        /// </summary>
        internal static string RequestPath
        {
            get { return System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), @"Forja\setup-request.txt"); }
        }

        internal static void WriteRequest(string folder, string componentes)
        {
            try
            {
                var file = RequestPath;
                var dir = System.IO.Path.GetDirectoryName(file);
                if (!Directory.Exists(dir))
                {
                    Directory.CreateDirectory(dir);
                }

                File.WriteAllText(file,
                    "InstallFolder=" + folder + "\nComponentes=" + (componentes ?? "") + "\n");
            }
            catch
            {
                // sin archivo, la instalación usa la carpeta por defecto
            }
        }

        /// <summary>Lee y borra el pedido pendiente. Devuelve (carpeta, componentes).</summary>
        internal static (string Folder, string Componentes) TakeRequest()
        {
            try
            {
                var file = RequestPath;
                if (!File.Exists(file))
                {
                    return (null, null);
                }

                var folder = "";
                var componentes = "";

                foreach (var line in File.ReadAllLines(file))
                {
                    var parts = line.Split(new[] { '=' }, 2);
                    if (parts.Length != 2)
                    {
                        continue;
                    }

                    if (parts[0].Trim().Equals("InstallFolder", StringComparison.OrdinalIgnoreCase))
                    {
                        folder = parts[1].Trim();
                    }
                    else if (parts[0].Trim().Equals("Componentes", StringComparison.OrdinalIgnoreCase))
                    {
                        componentes = parts[1].Trim();
                    }
                }

                File.Delete(file);
                return (folder, componentes);
            }
            catch
            {
                return (null, null);
            }
        }

        internal static InstallManifest Read()
        {
            foreach (var file in new[] { MachinePath, UserPath })
            {
                try
                {
                    if (File.Exists(file))
                    {
                        return Parse(File.ReadAllText(file));
                    }
                }
                catch
                {
                    // un manifiesto roto no debe impedir instalar
                }
            }

            return null;
        }

        internal void Write()
        {
            var file = Scope == "machine" ? MachinePath : UserPath;

            try
            {
                var dir = System.IO.Path.GetDirectoryName(file);
                if (!Directory.Exists(dir))
                {
                    Directory.CreateDirectory(dir);
                }

                // Sin BOM: el CLI lo lee con serde_json, que no lo tolera.
                File.WriteAllText(file, ToJson(), new UTF8Encoding(false));
            }
            catch
            {
                // sin manifiesto el instalador igual funciona; solo se pierde
                // el "actualizar solo lo que acepté instalar"
            }
        }

        private string ToJson()
        {
            var features = new List<string>();
            foreach (var feature in Features ?? new List<string>())
            {
                features.Add("\"" + Escape(feature) + "\"");
            }

            return "{\n" +
                   "  \"version\": \"" + Escape(Version) + "\",\n" +
                   "  \"path\": \"" + Escape(Path) + "\",\n" +
                   "  \"scope\": \"" + Escape(Scope) + "\",\n" +
                   "  \"features\": [" + string.Join(",", features) + "]\n" +
                   "}\n";
        }

        private static InstallManifest Parse(string json)
        {
            var manifest = new InstallManifest
            {
                Version = StringValue(json, "version"),
                Path = StringValue(json, "path"),
                Scope = StringValue(json, "scope"),
                Features = ArrayValue(json, "features"),
            };

            return manifest.Path == null ? null : manifest;
        }

        private static string StringValue(string json, string key)
        {
            var marker = "\"" + key + "\"";
            var start = json.IndexOf(marker, StringComparison.OrdinalIgnoreCase);
            if (start < 0)
            {
                return null;
            }

            start = json.IndexOf('"', start + marker.Length);
            if (start < 0)
            {
                return null;
            }

            var end = start + 1;
            while (end < json.Length && json[end] != '"')
            {
                end += json[end] == '\\' ? 2 : 1;
            }

            return Unescape(json.Substring(start + 1, end - start - 1));
        }

        private static List<string> ArrayValue(string json, string key)
        {
            var values = new List<string>();
            var marker = "\"" + key + "\"";
            var start = json.IndexOf(marker, StringComparison.OrdinalIgnoreCase);
            if (start < 0)
            {
                return values;
            }

            start = json.IndexOf('[', start);
            var end = start < 0 ? -1 : json.IndexOf(']', start);
            if (start < 0 || end < 0)
            {
                return values;
            }

            foreach (var part in json.Substring(start + 1, end - start - 1).Split(','))
            {
                var value = part.Trim().Trim('"');
                if (value.Length > 0)
                {
                    values.Add(value);
                }
            }

            return values;
        }

        private static string Escape(string value)
        {
            return (value ?? "").Replace("\\", "\\\\").Replace("\"", "\\\"");
        }

        private static string Unescape(string value)
        {
            return value.Replace("\\\\", "\\").Replace("\\\"", "\"");
        }
    }
}
