# Running Headroom as a Service

Instead of starting the proxy manually each session, run it as a background service.

## systemd (Linux)

Create `~/.config/systemd/user/headroom.service`:

```ini
[Unit]
Description=Headroom Context Compression Proxy
After=network.target

[Service]
Type=simple
ExecStart=%h/.local/bin/headroom proxy --port 8787
Restart=on-failure
RestartSec=5
Environment=HEADROOM_LOG_LEVEL=INFO

[Install]
WantedBy=default.target
```

Enable and start:
```bash
systemctl --user daemon-reload
systemctl --user enable --now headroom
systemctl --user status headroom

# Check it's running
curl -s localhost:8787/health
```

## launchd (macOS)

Create `~/Library/LaunchAgents/com.headroom.proxy.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.headroom.proxy</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/headroom</string>
        <string>proxy</string>
        <string>--port</string>
        <string>8787</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/headroom.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/headroom.err</string>
</dict>
</plist>
```

Load:
```bash
launchctl load ~/Library/LaunchAgents/com.headroom.proxy.plist
```

## Quick background (any platform)

```bash
nohup headroom proxy --port 8787 > /tmp/headroom.log 2>&1 &
```
