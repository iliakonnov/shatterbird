import * as vscode from 'vscode';
import {FsProvider} from "./filesystem/fsProvider.ts";
import {LanguageClient} from "./language/languageClient.ts";

export function activate(context: vscode.ExtensionContext) {
    context.subscriptions.push(new FsProvider());

    let client = new LanguageClient();
    context.subscriptions.push(client)
    client.start();

    vscode.commands.executeCommand('vscode.openFolder', vscode.Uri.parse(`bird://`));
}
