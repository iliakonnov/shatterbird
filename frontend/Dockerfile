FROM node:20-slim as build
ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME:$PATH"
RUN corepack enable

WORKDIR /app

COPY package.json pnpm-lock.yaml /app/
RUN pnpm install

COPY . /app
RUN pnpm run build


FROM caddy:2.7

COPY --from=build /app/dist /srv
COPY Caddyfile /etc/caddy/Caddyfile

EXPOSE 80 443
ENV CADDY_DOMAIN=*
VOLUME /data