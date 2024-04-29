import {FilePermission, FileStat, FileSystemError, FileType} from "vscode";
import {Node, DirectoryLike, FileLike} from "./types.ts";
import {Node as ServerNode} from "../server-types/Node";
import {Id} from "../server-types/Id";
import FsClient from "./fsClient.ts";
import * as console from "console";

export class DirNode implements DirectoryLike {
    public readonly name: string;
    public readonly fileType = FileType.Directory;
    private readonly nodeId: Id<ServerNode>;
    private readonly client: FsClient;

    constructor(client: FsClient, nodeId: Id<ServerNode>, name: string) {
        this.client = client;
        this.nodeId = nodeId;
        this.name = name;
    }

    async getChildren(): Promise<Node[]> {
        let node = await this.client.getNode(this.nodeId);
        if (node == null) {
            throw FileSystemError.FileNotFound();
        }
        if (!('Directory' in node.content)) {
            throw FileSystemError.FileNotADirectory();
        }

        const result = []
        for (let key of Object.keys(node.content.Directory.children)) {
            const child = node.content.Directory.children[key];
            if (child.kind == 'Text' || child.kind == 'Blob') {
                result.push(new FileNode(this.client, child._id, key));
            } else if (child.kind == 'Directory') {
                result.push(new DirNode(this.client, child._id, key));
            }
        }
        return result;
    }

    async get(child: string): Promise<Node | null> {
        const children = await this.getChildren();
        const found = children.find(c => c.name == child);
        if (!found) {
            console.log(`[bird] unable to find \`${child}\` in \`${this.name}\` (${this.nodeId})`);
            return null;
        }
        return found;
    }

    async getStat(): Promise<FileStat> {
        return {ctime: 0, mtime: 0, permissions: FilePermission.Readonly, size: 0, type: this.fileType}
    }
}

export class FileNode implements FileLike {
    public readonly name: string;
    public readonly fileType = FileType.File;
    private readonly nodeId: Id<Node>;
    private readonly client: FsClient;

    constructor(client: FsClient, nodeId: Id<ServerNode>, name: string) {
        this.client = client;
        this.nodeId = nodeId;
        this.name = name;
    }

    async getContent(): Promise<Uint8Array> {
        const node = await this.client.getNode(this.nodeId);
        if (node === null) {
            throw FileSystemError.FileNotFound();
        }
        if ('Blob' in node.content) {
            const blob = await this.client.getBlob(node.content.Blob.content);
            if (blob === null) {
                throw FileSystemError.FileNotFound('blob not found');
            }
            return blob;
        }
        if ('Text' in node.content) {
            const text = node.content.Text.lines.map(l => l.text).join('\n');
            return new TextEncoder().encode(text);
        }
        if ('Symlink' in node.content) {
            return new TextEncoder().encode(node.content.Symlink.target);
        }
        if ('Directory' in node.content) {
            throw FileSystemError.FileNotADirectory();
        }
        throw FileSystemError.Unavailable('Unknown file type');
    }

    async getStat(): Promise<FileStat> {
        // TODO: Fetch
        return {
            ctime: 0,
            mtime: 0,
            permissions: FilePermission.Readonly,
            size: 0,
            type: FileType.File
        }
    }
}
