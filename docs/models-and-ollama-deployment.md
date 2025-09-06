# ocodex Models and Ollama Deployment Guide

This guide explains how models work in ocodex, practical use cases by model size, how ocodex selects models by default, and multiple ways to run and deploy Ollama-backed OSS models efficiently, with notes on bandwidth, latency, storage, and cost.

## Overview

- ocodex supports two main provider families:
  - Open‑source via Ollama (built‑in `oss` provider)
  - OpenAI (Responses API; opt‑in)
- You can choose a model explicitly with `-m <model>`, or let ocodex pick a default for you when using `--oss`.
- For OSS models, ocodex talks to a local or remote Ollama server using an OpenAI‑compatible interface.

## Model Selection in ocodex

- CLI flags and overrides:
  - `--oss`: use the built‑in open‑source provider (Ollama‑compatible)
  - `-m <model>`: choose a specific model (e.g., `-m gpt-oss:20b`)
  - `-c model_provider=openai`: use the OpenAI provider (requires `--openai`)
  - `--openai`: allow providers that require OpenAI auth
- Dynamic default for `--oss`:
  - ocodex tries to pick the heaviest reasonable default model based on host memory.
  - Heuristic: hosts with >= 64 GiB RAM default to `gpt-oss:120b`; otherwise, `gpt-oss:20b`.
  - You can always override with `-m`.
- Ensure/pull behavior:
  - When `--oss` is used, ocodex checks that the selected model is present on the Ollama server and pulls it if missing.

## Matching Use Cases to Model Sizes

- Small/Medium models (e.g., 7B–20B; `gpt-oss:20b`)
  - Use for quick code edits, refactors, shell helpers, short explanations, and most iterative dev workflows.
  - Lower memory/storage, faster cold starts, lower inference cost.
- Large models (e.g., 70B–120B; `gpt-oss:120b`)
  - Use for complex multi‑file changes, refactoring with broad context, deep design reviews, and synthesis tasks.
  - Higher memory/storage/CPU/GPU needs; better reasoning and consistency.
- Practical approach
  - Default to a lighter model for fast feedback loops; escalate to a heavy model as needed for key tasks.

## Tradeoffs: Bandwidth and Latency

- Local vs remote:
  - Local Ollama minimizes network latency and egress/ingress of prompts and outputs.
  - Remote Ollama can centralize storage/compute, but adds WAN latency and bandwidth costs.
  - Co‑locate the model with data (e.g., codebase) when possible to reduce bytes over the network.
- Prompt size matters:
  - Large prompts (code + docs) can be hundreds of KB per turn; across WAN this adds latency and cost.
  - Keep context tight with project filters and smaller diffs.
- Streaming:
  - ocodex uses streaming where supported; RTT and packet loss on WAN directly impact perceived latency.

## Using Ollama with ocodex

ocodex talks to Ollama via an OpenAI‑compatible base URL.

- Environment knobs (read by the built‑in `oss` provider):
  - `CODEX_OSS_BASE_URL`: e.g., `http://localhost:11434/v1` (recommended)
  - `CODEX_OSS_PORT`: if you only want to change the port (default `11434`)
- Point ocodex to a remote Ollama:
  - SSH tunnel (recommended for security): on the cloud server
    - `ssh -N -L 11434:localhost:11434 user@your-pc`
    - Then set `CODEX_OSS_BASE_URL=http://localhost:11434/v1`
  - VPN or private network: set `CODEX_OSS_BASE_URL=http://<pc-ip>:11434/v1`
  - Reverse proxy with TLS + auth: terminate HTTPS/auth at the proxy and forward to Ollama

### Storage Location for Ollama Models

- Default on Linux: usually `/usr/share/ollama/.ollama/models` or `/var/lib/ollama` (varies by install)
- Default on macOS: `~/.ollama/models`
- Override location with `OLLAMA_MODELS=/path/to/models` (server and CLI honor this)
  - Use this to place models on larger or faster volumes (e.g., SSD, network storage)

### Running Ollama Locally

1) Install Ollama (see Ollama docs)
2) Serve locally (default bind):
```
ollama serve
```
3) Pull a model:
```
ollama pull llama3.1:8b
```
4) Use with ocodex:
```
export CODEX_OSS_BASE_URL=http://localhost:11434/v1
ocodex exec --oss -m llama3.1:8b -- json "Your prompt here"
```

Bind to a specific host/port:
```
export OLLAMA_HOST=0.0.0.0:11434
ollama serve
```
Security: do not expose Ollama publicly without a proxy that enforces TLS and authentication.

### Running Ollama with Docker

docker‑compose example (persist models, control bind):
```yaml
services:
  ollama:
    image: ollama/ollama:latest
    restart: unless-stopped
    ports:
      - "11434:11434"
    environment:
      - OLLAMA_HOST=0.0.0.0:11434
    volumes:
      - ollama-models:/root/.ollama

volumes:
  ollama-models:
```
Then from ocodex:
```
export CODEX_OSS_BASE_URL=http://localhost:11434/v1
```

Point models to a custom host path (e.g., large SSD):
```yaml
    volumes:
      - /mnt/fastdisk/ollama:/root/.ollama
```

### Running Ollama on Kubernetes

Minimal Deployment and Service (attach a PersistentVolumeClaim):
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ollama
spec:
  replicas: 1
  selector:
    matchLabels: { app: ollama }
  template:
    metadata:
      labels: { app: ollama }
    spec:
      containers:
        - name: ollama
          image: ollama/ollama:latest
          env:
            - name: OLLAMA_HOST
              value: 0.0.0.0:11434
          ports:
            - containerPort: 11434
          volumeMounts:
            - name: models
              mountPath: /root/.ollama
      volumes:
        - name: models
          persistentVolumeClaim:
            claimName: ollama-models-pvc
---
apiVersion: v1
kind: Service
metadata:
  name: ollama
spec:
  selector: { app: ollama }
  ports:
    - port: 11434
      targetPort: 11434
      protocol: TCP
```
Expose via Ingress and add auth/TLS at the edge. From clients (ocodex):
```
export CODEX_OSS_BASE_URL=https://ollama.example.com/v1
```

### Using Network‑Mounted Storage

- If disk is scarce on compute nodes, mount a network share (NFS/SMB/SSHFS) and set:
```
export OLLAMA_MODELS=/mnt/shared/ollama
ollama serve
```
- Pros: centralizes models; no per‑host downloads.
- Cons: I/O latency and throughput depend on the network; prefer fast LAN and SSD‑backed storage.

### Remote Use Without Exposing Ollama Publicly

- Use SSH tunnels to forward `11434` from the consumer to the host. Example on the cloud host:
```
ssh -N -L 11434:localhost:11434 user@your-pc
export CODEX_OSS_BASE_URL=http://localhost:11434/v1
```
- This preserves privacy and avoids running a public service.

## Cost‑Effective and Efficient Patterns

- Two‑tier model strategy
  - Default to a smaller model for everyday tasks; escalate to a large model for key operations.
  - In ocodex, use `-m` to override per run; or set a profile in your config.
- Pre‑pull and cache
  - Pull commonly used models during provisioning to avoid on‑demand downloads during work hours.
  - Snapshot/cache the model volume (Docker/K8s PV) to speed up new nodes.
- Keep context small
  - Limit embedded files and diffs; rely on iterative turns to “drill down”.
- Co‑locate compute and data
  - Run Ollama near the code/workspace to minimize WAN transfers.
- Quantization and variants
  - Prefer quantized variants (e.g., 4‑bit/5‑bit) for CPU‑only hosts or tight RAM. Balance quality vs speed.

## Security Considerations

- Do not expose Ollama without TLS and authentication.
- Prefer SSH tunnels, VPNs, or reverse proxies that enforce auth.
- Restrict firewalls to trusted clients; avoid 0.0.0.0 exposure on public networks.

## Troubleshooting

- "No running Ollama server detected"
  - Ensure `ollama serve` is running and reachable; verify `CODEX_OSS_BASE_URL`.
- Pulls are slow or fail
  - Verify storage space and network; consider pre‑pulling or using a local mirror.
- Wrong endpoint used
  - For OSS via Ollama, ensure base URL looks like `http://host:11434/v1`.
- High latency
  - Reduce prompt size, move server closer to client, or switch to local.

## Quick Reference

- Select OSS provider with dynamic default:
```
ocodex exec --oss -- json "Your prompt"
```
- Force a specific OSS model:
```
ocodex exec --oss -m gpt-oss:20b -- json "Your prompt"
```
- Point to a remote Ollama:
```
export CODEX_OSS_BASE_URL=http://host:11434/v1
```
- Move Ollama model storage:
```
export OLLAMA_MODELS=/mnt/fastdisk/ollama
ollama serve
```

