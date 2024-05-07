import {Disposable} from "vscode";
import {
    AbstractMessageReader, AbstractMessageWriter,
    BaseLanguageClient, DataCallback, Emitter, integer, Message,
    MessageReader,
    MessageTransports,
    MessageWriter, ResponseMessage,
} from "vscode-languageclient";

export class LanguageClient extends BaseLanguageClient {
    constructor() {
        super('shatterbird-vscode', 'Shatterbird', {
            documentSelector: [{scheme: 'bird'}]
        })
    }

    protected async createMessageTransports(encoding: string): Promise<MessageTransports> {
        const reader = new Reader();
        const writer = new Writer(reader);
        return {reader, writer};
    }
}

class Reader extends AbstractMessageReader implements MessageReader {
    callback: DataCallback | null = null;

    listen(callback: DataCallback): Disposable {
        this.callback = callback;
        return new Disposable(() => undefined);
    }

    reply(message: ResponseMessage) {
        if (this.callback == null) {
            return
        }
        this.callback(message)
    }
}

class Writer extends AbstractMessageWriter implements MessageWriter {
    reader: Reader;

    constructor(reader: Reader) {
        super();
        this.reader = reader;
    }

    async write(msg: Message): Promise<void> {
        if (!Message.isRequest(msg)) {
            return;
        }
        const {id, method, params} = msg;
        fetch(`/api/lsp/${method}`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify(params),
        }).then(async response => {
            const data = await response.json() as any;
            this.reader.reply({
                jsonrpc: "2.0",
                id,
                result: data,
            })
        });
    }

    end(): void {
    }
}
