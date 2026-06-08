Check the Headroom proxy and report token savings.

Run the health and stats endpoints, then summarize:

```bash
curl -s -m 2 http://127.0.0.1:8787/health || echo "offline"
```

If the proxy responds, fetch the stats:

```bash
curl -s -m 2 http://127.0.0.1:8787/stats
```

Present a short summary of:

- input/output tokens saved
- compression ratio
- estimated cost savings
- uptime

If the proxy is offline, respond with:

```
Headroom proxy is not running on 127.0.0.1:8787.

Start it with:
    headroom proxy

Install it with:
    uv tool install headroom-ai

Then re-run /whetstone-headroom.
```
