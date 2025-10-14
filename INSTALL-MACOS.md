# LaunchDock launchd Setup Guide

## Launch Agent Plist

Use the attached launchd plist file to have program run at startup. Update the path if your app is installed in a different location.

## Install

```bash
cp tech.qa3.launchdock.plist ~/Library/LaunchAgents/
launchctl load ~/Library/LaunchAgents/tech.qa3.launchdock.plist
```

## Uninstall

```bash
launchctl unload ~/Library/LaunchAgents/tech.qa3.launchdock.plist
rm ~/Library/LaunchAgents/tech.qa3.launchdock.plist
```

## Controlling the Application

**Don't use `launchctl stop/start`.** Instead, control the app directly:

```bash
# Stop the application
launchdock stop

# Start the application
launchdock start
```

The launch agent only handles running the app at login. Use `launchdock` commands for all other control.
