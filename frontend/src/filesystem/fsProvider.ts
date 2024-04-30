import {
    Disposable,
    Event,
    EventEmitter,
    FileChangeEvent,
    FileStat,
    FileSystemProvider,
    FileType,
    Uri,
    workspace
} from "vscode";
import FsProviderBase from "../filesystem/fsProviderBase.ts";

export class FsProvider implements FileSystemProvider, Disposable {
    private readonly disposable: Disposable;
    private fs: FsProviderBase;

    constructor() {
        this.fs = new FsProviderBase();
        this.disposable = Disposable.from(
            workspace.registerFileSystemProvider('bird', this, {
                isCaseSensitive: true,
                isReadonly: true,
            }),
        );
    }

    dispose() {
        this.disposable?.dispose();
    }

    watch(_uri: Uri): Disposable {
        // no-op
        return new Disposable(() => {
        });
    }

    async stat(uri: Uri): Promise<FileStat> {
        return await this.fs.stat(uri);
    }

    async readDirectory(uri: Uri): Promise<[string, FileType][]> {
        return await this.fs.readDirectory(uri);
    }

    async readFile(uri: Uri): Promise<Uint8Array> {
        return await this.fs.readFile(uri);
    }

    createDirectory(_uri: Uri) {
        throw new Error("Filesystem is read-only");
    }

    writeFile(_uri: Uri, _content: Uint8Array) {
        throw new Error("Filesystem is read-only");
    }

    delete(_uri: Uri) {
        throw new Error("Filesystem is read-only");
    }

    rename(_oldUri: Uri, _newUri: Uri) {
        throw new Error("Filesystem is read-only");
    }

    copy(_source: Uri, _destination: Uri) {
        throw new Error("Filesystem is read-only");
    }

    // Files are never changed
    onDidChangeFile: Event<FileChangeEvent[]> = new EventEmitter<FileChangeEvent[]>().event;

}