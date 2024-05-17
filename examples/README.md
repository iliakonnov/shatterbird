Это файлы индексов, построенные с помощью коммита `58170725e4c25b2a7983ae7f8475a7c1ea2c3e10`

Их можно загрузить в базу данных:
```shell
cd backend/
env RUST_LOG=info,mongodb=info \
  cargo run -p shatterbird-indexer -- \
  --db-url mongodb://127.0.0.1:27017/db \
  lsif \
  --input ./index.lsif.json \
  --roots=/Users/iliako/Documents/shatterbird=58170725e4c25b2a7983ae7f8475a7c1ea2c3e10 \
  --save
```