using System;
using WixToolset.BootstrapperApplicationApi;

namespace Forja.Setup
{
    internal static class Program
    {
        // Sin [STAThread]: el hilo principal habla COM en MTA con Burn; la UI
        // corre en su propio hilo STA (ver ForjaBootstrapper.UiThread).
        private static int Main()
        {
            var application = new ForjaBootstrapper();
            ManagedBootstrapperApplication.Run(application);
            return application.ExitCode;
        }
    }
}
