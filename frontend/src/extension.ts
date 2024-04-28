import * as vscode from 'vscode';
import {FsProvider} from "./bridge/fsProvider.ts";

export function activate(context: vscode.ExtensionContext) {
    context.subscriptions.push(new FsProvider());
    vscode.commands.executeCommand('vscode.openFolder', vscode.Uri.parse(`bird://`));
}
