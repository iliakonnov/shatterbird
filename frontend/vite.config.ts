import {defineConfig} from "vite";
import {viteStaticCopy} from "vite-plugin-static-copy";
import * as path from "path";

export default defineConfig({
    server: {
        hmr: {
            overlay: false,
        }
    },
    build: {
        lib: {
            entry: "./src/extension.ts",
            formats: ["cjs"],
            fileName: (format, name) => `shatterbird/${name}.js`,
        },
        rollupOptions: {
            external: ["vscode"],
        },
        sourcemap: true,
    },
    preview: {
        proxy: {
            "/api": "http://localhost:3000/"
        }
    },
    plugins: [
        viteStaticCopy({
            targets: [
                {
                    src: path.resolve(__dirname, './src/manifest.json'),
                    rename: 'package.json',
                    dest: './shatterbird',
                },
                {
                    src: path.resolve(__dirname, './node_modules/vscode-web/dist') + '/[!.]*',
                    dest: './vscode-web',
                }
            ],
            watch: {
                reloadPageOnChange: true
            }
        }),
    ]
});