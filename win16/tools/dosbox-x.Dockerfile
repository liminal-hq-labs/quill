# The DOSBox-X image the Windows 3.11 harness runs in. Build: docker build -t quill16-dosbox-x:local -f win16/tools/dosbox-x.Dockerfile win16/tools
FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends dosbox-x ca-certificates && rm -rf /var/lib/apt/lists/*
