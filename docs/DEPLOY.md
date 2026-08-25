# Deployment

The whole thing is **one container**: the same process serves the compiled game
and relays the lobby. That is why there is no server address to configure - the
client derives `wss://<its own host>/ws` from the page it was loaded from.

Live: <https://elemental-td-375719008085.us-central1.run.app>

## Why one service

Splitting the static site from the lobby means two deployments, a CORS story,
and a mixed-content trap (a page on `https://` cannot open a `ws://` socket).
Serving both from one origin removes all three problems and costs nothing extra:
the static files are a `COPY` into an image that had to exist anyway.

## Cost

| Setting | Value | Why |
|---|---|---|
| `--min-instances` | `0` | Nobody playing costs nothing. This is the single biggest lever. |
| `--max-instances` | `1` | **Correctness, not just cost.** Rooms live in process memory, so two instances would be two different sets of rooms and players in "the same" room would never see each other. |
| `--concurrency` | `800` | One instance holds hundreds of sockets. Room state is well under a kilobyte, so the ceiling is socket buffers, not rooms. |
| `--memory` | `512Mi` | Measured: 1000 players is ~71 KB of room state; the rest is buffers and the runtime. |
| `--cpu` | `1`, throttled | The default (CPU only while a request is in flight) is the cheap one. The server does no background work between messages. |
| `--timeout` | `3600` | Cloud Run's maximum. See the caveat below. |

Idle cost is therefore zero, and an active hour is a fraction of a cent. The
container image itself is a Debian slim base plus one static binary and the
`dist/` bundle.

### The one caveat

Cloud Run caps any request - **including a WebSocket** - at 60 minutes. A room
open longer than that will see its sockets close. The client treats this as a
non-event: the run is simulated locally, so play continues and only the shared
scoreboard stops updating. Rejoining the room restores it.

## Deploying a new version

The wasm is built on the host so the image never carries a Rust or wasm
toolchain:

```sh
trunk build --release
docker build -t us-central1-docker.pkg.dev/game-server-506612/cloud-run-source-deploy/elemental-td:v2 .
docker push  us-central1-docker.pkg.dev/game-server-506612/cloud-run-source-deploy/elemental-td:v2
gcloud run deploy elemental-td \
  --image us-central1-docker.pkg.dev/game-server-506612/cloud-run-source-deploy/elemental-td:v2 \
  --project game-server-506612 --region us-central1 \
  --allow-unauthenticated --port 8080 \
  --cpu 1 --memory 512Mi --min-instances 0 --max-instances 1 \
  --concurrency 800 --timeout 3600
```

`gcloud run deploy --source .` would work too, but it needs
`roles/cloudbuild.builds.builder` on the project's default compute service
account:

```sh
gcloud projects add-iam-policy-binding game-server-506612 \
  --member=serviceAccount:375719008085-compute@developer.gserviceaccount.com \
  --role=roles/cloudbuild.builds.builder
```

## Checks

```sh
curl https://elemental-td-375719008085.us-central1.run.app/health
# ok rooms=0 players=0
```

`/health` reports live room and player counts, which is also the quickest way to
confirm a room really was created.

## GitHub Pages

`.github/workflows/deploy.yml` still publishes the game to Pages on every push
to `main`. That copy cannot guess where the lobby is, so the workflow bakes the
address in via `TD_SERVER`. Update it there if the service ever moves.
