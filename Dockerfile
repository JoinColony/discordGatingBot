FROM docker.io/rust as builder
RUN curl -fsSL https://deb.nodesource.com/setup_16.x | bash -
RUN apt-get update && apt-get install -y nodejs

WORKDIR /usr/src/discord-gating-bot
COPY . .

WORKDIR /usr/src/discord-gating-bot/backend
RUN cargo build --release 


# Begin of final image
FROM docker.io/debian:bullseye-slim
RUN apt-get update && apt-get install -y ca-certificates bash-completion curl less man vim && rm -rf /var/lib/apt/lists/*

ENV TERM=xterm-256color
COPY --from=builder /usr/src/discord-gating-bot/backend/target/release/discord-gating-bot /usr/local/bin/discord-gating-bot
COPY --from=builder /usr/src/discord-gating-bot/backend/man /usr/local/share/man/man1
COPY --from=builder /usr/src/discord-gating-bot/backend/completion/discord-gating-bot.bash \
/usr/local/share/bash-completion/completions/discord-gating-bot.bash

RUN echo "source /usr/local/share/bash-completion/completions/discord-gating-bot.bash" >> ~/.bashrc && \
    echo "source /etc/profile.d/bash_completion.sh" >> ~/.bashrc && \
    mandb

CMD ["discord-gating-bot"]





