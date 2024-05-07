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

USER www-data

COPY --from=build /app/dist /var/www

EXPOSE 8080
CMD ["busybox", "httpd", "-f", "-v", "-p", "8080"]