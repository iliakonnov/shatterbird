import {FileStat, FileSystemError, FileType, Uri} from "vscode";
import FsRoot from "./fsRoot.ts";
import RepoRoot from "./repoRoot.ts";
import {Node} from "./types.ts";
import FsClient from "./fsClient.ts";

export default class FsProviderBase {
    public readonly client = new FsClient();
    private readonly root = new FsRoot(this.client);


    async stat(uri: Uri): Promise<FileStat> {
        console.log(`[bird] stat of ${uri.path} (${uri})`);
        if (uri.path === '/') {
            return await this.root.getStat();
        }

        const node = await this.resolve(uri);
        return await node.getStat();
    }

    async readDirectory(uri: Uri): Promise<[string, FileType][]> {
        console.log(`[bird] readdir of ${uri.path} (${uri})`);
        if (uri.path === '/') {
            const roots = await this.root.getChildren();
            return roots.map(r => [r.name, FileType.Directory]);
        }

        const node = await this.resolve(uri);
        if (node.fileType != FileType.Directory) {
            throw FileSystemError.FileNotADirectory(uri);
        }

        return (await node.getChildren()).map(child => [child.name, child.fileType]);
    }

    async readFile(uri: Uri): Promise<Uint8Array> {
        console.log(`[bird] read of ${uri.path} (${uri})`);
        if (uri.path === '/') {
            throw FileSystemError.FileIsADirectory(uri);
        }
        const node = await this.resolve(uri);
        if (node.fileType == FileType.File) {
            return node.getContent()
        }
        if (node.fileType == FileType.SymbolicLink) {
            throw FileSystemError.Unavailable("is a symlink");
        }
        if (node.fileType == FileType.Directory) {
            throw FileSystemError.FileIsADirectory(uri);
        }
        throw FileSystemError.Unavailable("unknown file type");
    }

    private async resolve(uri: Uri): Promise<Node> {
        let splitted = uri.path.split('/')
        let commitId = splitted[1];

        if (commitId == '.vscode' || commitId == '.git') {
            throw FileSystemError.FileNotFound(uri);
        }

        const commit = await this.client.getCommitFromGit(commitId)
        if (commit == null) {
            throw FileSystemError.FileNotFound(uri);
        }
        let curr: Node = new RepoRoot(this.client, commit);

        for (let next of splitted.slice(2)) {
            if (curr.fileType !== FileType.Directory) {
                throw FileSystemError.FileNotFound(uri);
            }
            const nextNode = await curr.get(next);
            if (!nextNode) {
                throw FileSystemError.FileNotFound(uri);
            }
            curr = nextNode
        }
        return curr
    }

}