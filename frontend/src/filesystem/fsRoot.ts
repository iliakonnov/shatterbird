import {FilePermission, FileStat, FileType} from "vscode";
import RepoRoot from "./repoRoot.ts";
import {DirectoryLike, Node} from "./types.ts";
import FsClient from "./fsClient.ts";

export default class FsRoot implements DirectoryLike {
    private readonly client: FsClient;

    constructor(client: FsClient) {
        this.client = client;

    }

    readonly fileType = FileType.Directory;
    readonly name = "";

    async get(child: string): Promise<Node | null> {
        return null
    }

    async getStat(): Promise<FileStat> {
        return {ctime: 0, mtime: 0, permissions: FilePermission.Readonly, size: 0, type: this.fileType}
    }

    async getChildren(): Promise<RepoRoot[]> {
        const commits = await this.client.listCommits();
        return commits.map(commit => new RepoRoot(this.client, commit));
    }
}