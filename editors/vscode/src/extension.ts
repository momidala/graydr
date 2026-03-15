// graydr VSCode Extension
//
// activate() checks for graydr binary on PATH before starting the LSP client.
// If absent, shows an actionable error notification (SC-5).
// Uses TransportKind.stdio — NOT TransportKind.ipc (ipc is for Node.js servers).

import * as vscode from 'vscode';
import { execFileSync } from 'child_process';
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
  // SC-5: Check for graydr binary before attempting to start the LSP client.
  // Shows an actionable message rather than silently failing.
  if (!isGraydrOnPath()) {
    vscode.window.showErrorMessage(
      'graydr: binary not found on PATH. Install graydr (cargo install graydr) to enable IntelliSense.',
      'Dismiss'
    );
    return;
  }

  // Spawn `graydr lsp` as a child process communicating over stdio.
  // IMPORTANT: Use TransportKind.stdio for external binaries (not TransportKind.ipc).
  const serverOptions: ServerOptions = {
    command: 'graydr',
    args: ['lsp'],
    transport: TransportKind.stdio,
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: 'file', language: 'gmod' },
      { scheme: 'file', language: 'gtpl' },
      { scheme: 'file', language: 'gfrag' },
      { scheme: 'file', language: 'grule' },
    ],
  };

  client = new LanguageClient(
    'graydr',
    'graydr Language Server',
    serverOptions,
    clientOptions
  );
  client.start();
  context.subscriptions.push(client);
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}

function isGraydrOnPath(): boolean {
  try {
    execFileSync('graydr', ['version'], { timeout: 2000 });
    return true;
  } catch {
    return false;
  }
}
