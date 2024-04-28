import {Commit} from "../server-types/Commit.ts";
import {FullNode} from "../server-types/FullNode.ts";
import {NodeInfo} from "../server-types/NodeInfo.ts";
import {Node} from "../server-types/Node.ts";
import {Id} from "../server-types/Id.ts";

export default class FsClient {
    readonly gitCommits: Map<string, Commit> = new Map();
    readonly nodes: Map<string, NodeInfo | FullNode> = new Map();

    async listCommits(): Promise<Commit[]> {
        const response = await fetch('http://localhost:3000/fs/commits');
        const data = await response.json() as Commit[];
        for (let commit of data) {
            this.gitCommits.set(commit.oid, commit);
        }
        return data;
    }

    async getCommitFromGit(oid: string): Promise<Commit | null> {
        const found =  this.gitCommits.get(oid);
        if (found) {
            return found;
        }

        const response = await fetch(`http://localhost:3000/fs/commits/by-oid/${oid}`);
        if (response.status === 404) {
            return null;
        }
        const data = await response.json() as Commit;
        this.gitCommits.set(data.oid, data);
        return data as Commit;
    }

    async getNode(nodeId: Id<Node>): Promise<FullNode | null> {
        const found = this.nodes.get(nodeId.$oid);
        if (found && 'content' in found) {
            return found;
        }

        const response = await fetch(`http://localhost:3000/fs/nodes/${nodeId.$oid}`);
        if (response.status === 404) {
            return null;
        }
        const data = await response.json() as FullNode;
        this.nodes.set(data._id.$oid, data);
        return data as FullNode;
    }

    async getNodeShort(nodeId: Id<Node>): Promise<NodeInfo | null> {
        const found = this.nodes.get(nodeId.$oid);
        if (found) {
            return found;
        }

        const response = await fetch(`http://localhost:3000/fs/nodes/${nodeId.$oid}?short=true`);
        if (response.status === 404) {
            return null;
        }
        const data = await response.json() as NodeInfo;
        this.nodes.set(data._id.$oid, data);
        return data;

    }
}