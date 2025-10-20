# LaunchDock - macOS Installation Guide

Complete setup instructions for LaunchDock on macOS using Automator Quick Actions. This method offers several advantages:

- **No system daemons** - Everything runs in user-space
- **No launchctl** - Avoids macOS launch agent complexity
- **Version independent** - Works across all modern macOS versions
- **On-demand** - Daemon starts only when you need it
- **Easy to modify** - Simple script you can edit anytime
- **No admin rights** - No sudo or system modifications required
- **Clean uninstall** - Just delete one workflow file

The daemon automatically starts when you press your hotkey (if not already running), then shows the UI. After a restart, the first hotkey press will start the daemon—subsequent presses instantly show the UI.

## Prerequisites

- macOS 10.14 or later
- LaunchDock v0.4.0+ installed (via `cargo install` or from releases)

## Installation Steps

### Step 1: Verify LaunchDock Installation

Open Terminal and verify launchdock is accessible:

```bash
launchdock status
```

You should see output like:

```
Daemon: not running
UI: not visible
```

If you get "command not found", ensure launchdock is in your PATH. If installed via cargo, it should be at `~/.cargo/bin/launchdock`.

### Step 2: Create Automator Quick Action

1. Open **Automator** (press ⌘+Space, type "Automator", press Enter)

2. Click **New Document** (or File → New if Automator is already open)

3. Choose **Quick Action** (or "Service" on older macOS versions)

4. At the top of the workflow pane, configure:
   - **Workflow receives**: `no input`
   - **in**: `any application`

5. In the left sidebar, search for `Run Shell Script`

6. Drag the **Run Shell Script** action into the workflow area

7. Configure the shell script action:
   - **Shell**: `/bin/bash`
   - **Pass input**: `as arguments`

8. **Replace the default script** with this:

```bash
#!/bin/bash

# LaunchDock Toggle Script
# Checks if daemon is running, starts if needed, then shows UI

# Check if launchdock daemon is running by parsing status output
if ! launchdock status | grep -q "Daemon: running"; then
    # Daemon not running - start it
    launchdock start

    # Give daemon time to initialize
    sleep 0.1
fi

# Show the launcher UI
launchdock show
```

9. **Save** the workflow (⌘+S) with the name: **"Launch LaunchDock"**

10. **Close Automator**

### Step 3: Assign Keyboard Shortcut

1. Open **System Settings** (or System Preferences on older macOS)

2. Navigate to **Keyboard**

3. Click **Keyboard Shortcuts** (or the "Shortcuts" button)

4. Select **Services** in the left sidebar

5. Scroll down to the **General** section

6. Find **"Launch LaunchDock"** in the list

7. Click on the service name, then click **Add Shortcut**

8. Press your desired keyboard combination
   - Recommended: **⌥+Space** (Option+Space)
   - Alternative: **⌃+⌥+Space** (Control+Option+Space)
   - Note: Avoid ⌘+Space if you use Spotlight

9. Close System Settings

## Usage

Press your configured keyboard shortcut from anywhere in macOS:

1. First press: Script checks if daemon is running, starts it if needed, shows UI
2. Subsequent presses: Instantly shows UI (daemon already running)

**In the LaunchDock UI:**

- Type to search for applications (fuzzy matching)
- Use arrow keys or number shortcuts (1-7) to select
- Press **Enter** to launch
- Press **Escape** to close

## Editing Your Workflow

To modify the script later (e.g., adjust sleep duration):

**Method 1: Direct Access**

1. Open **Finder**
2. Press **⌘+⇧+G** (Go to Folder)
3. Type: `~/Library/Services/`
4. Find **Launch LaunchDock.workflow**
5. **Double-click** to open in Automator
6. Edit the script and save (⌘+S)

**Method 2: From Automator**

1. Open **Automator**
2. **File → Open** (⌘+O)
3. Navigate to `~/Library/Services/`
4. Select **Launch LaunchDock.workflow**
5. Edit and save

## Troubleshooting

### Keyboard shortcut doesn't work

- **Conflict**: Try a different key combination that doesn't conflict with other apps
- **Not saved**: Ensure you saved the Quick Action in Automator
- **Needs restart**: Restart your Mac if the shortcut isn't registering
- **Check assignment**: Verify the shortcut is assigned in System Settings → Keyboard → Keyboard Shortcuts

### "Command not found" error

LaunchDock isn't in your PATH. Fix by using the full path:

1. Find where launchdock is installed:

   ```bash
   which launchdock
   ```

2. Edit your workflow (see "Editing Your Workflow" above)

3. Replace all instances of `launchdock` with the full path:
   ```bash
   # Example if installed via cargo:
   /Users/yourname/.cargo/bin/launchdock status
   /Users/yourname/.cargo/bin/launchdock start
   /Users/yourname/.cargo/bin/launchdock show
   ```

### Daemon won't start

Check the logs:

```bash
launchdock logs
```

Manually test:

```bash
launchdock start
launchdock status
launchdock show
```

### UI appears slowly after restart

The `sleep 0.1` gives the daemon time to initialize. If the UI doesn't appear or appears too slowly:

1. Edit your workflow (see instructions above)
2. Change `sleep 0.1` to `sleep 1.0` (or higher)
3. Save and test

### Workflow not visible in Services

Quick Actions are saved to `~/Library/Services/`. If you can't find it:

1. Check the folder: `open ~/Library/Services/`
2. Verify the file exists: `Launch LaunchDock.workflow`
3. If missing, recreate it following Step 2

## Uninstallation

1. **Remove keyboard shortcut**:
   - System Settings → Keyboard → Keyboard Shortcuts → Services
   - Find "Launch LaunchDock" and delete the shortcut

2. **Delete the workflow**:

   ```bash
   rm -rf ~/Library/Services/Launch\ LaunchDock.workflow
   ```

3. **Stop the daemon**:

   ```bash
   launchdock stop
   ```

4. **Uninstall LaunchDock** (optional):
   ```bash
   cargo uninstall launchdock
   ```
