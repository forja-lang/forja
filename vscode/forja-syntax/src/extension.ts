import * as path from 'path';
import { ExtensionContext, workspace, window, commands } from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind
} from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: ExtensionContext) {
    // El servidor LSP se compila desde Rust: target/debug/forja-lsp.exe
    const serverModule = context.asAbsolutePath(
        path.join('..', 'target', 'debug', 'forja-lsp.exe')
    );

    const serverOptions: ServerOptions = {
        run: { command: serverModule, transport: TransportKind.stdio },
        debug: { command: serverModule, transport: TransportKind.stdio }
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'forja' }],
        synchronize: {
            fileEvents: workspace.createFileSystemWatcher('**/*.fa')
        }
    };

    client = new LanguageClient(
        'forja-lsp',
        'Forja Language Server',
        serverOptions,
        clientOptions
    );

    client.start();

    // Registrar comando "Forja: Ejecutar archivo"
    const disposable = commands.registerCommand('forja.run', () => {
        const editor = window.activeTextEditor;
        if (editor) {
            const document = editor.document;
            if (document.languageId === 'forja') {
                // Ejecutar el archivo .fa usando el compilador Forja
                const terminal = window.createTerminal('Forja');
                terminal.sendText(`forja run "${document.fileName}"`);
                terminal.show();
            } else {
                window.showErrorMessage('El archivo activo no es un archivo Forja (.fa)');
            }
        }
    });

    context.subscriptions.push(disposable);
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) return undefined;
    return client.stop();
}
