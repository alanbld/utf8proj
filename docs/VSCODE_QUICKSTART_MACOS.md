# utf8proj — VS Code Quick Start (macOS)

## Real-Time Error Highlighting for Non-Technical Users

**Version:** 0.2.0
**Last updated:** January 2026

-----

## What You'll Get

|Feature           |Description                  |
|------------------|-----------------------------|
|🔴 Red squiggles   |Errors appear **as you type**|
|🟡 Yellow squiggles|Warnings to review           |
|💡 Suggestions     |Hover for explanations       |
|⌨️ Autocomplete    |Press Ctrl+Space for options |

No Terminal commands. No programming. Just edit and watch for squiggles.

-----

## Step 1: Install Visual Studio Code

1. Go to: **https://code.visualstudio.com**
1. Click **Download for Mac**
1. Open the downloaded `.zip` file
1. Drag **Visual Studio Code** to your **Applications** folder
1. Launch VS Code from Applications

-----

## Step 2: Download utf8proj

### For Apple Silicon Macs (M1, M2, M3, M4)

1. Open this link in your browser:

   👉 **https://github.com/alanbld/utf8proj/releases/download/v0.2.0/utf8proj-v0.2.0-aarch64-apple-darwin.tar.gz**

### For Intel Macs

1. Open this link in your browser:

   👉 **https://github.com/alanbld/utf8proj/releases/download/v0.2.0/utf8proj-v0.2.0-x86_64-apple-darwin.tar.gz**

> **Not sure which Mac you have?** Click the Apple menu () → "About This Mac". Look for "Chip" — if it says M1/M2/M3/M4, use Apple Silicon. If it says Intel, use Intel.

### Extract the Download

1. **Double-click** the downloaded `.tar.gz` file
   - macOS will automatically extract it to a folder
1. **Create the installation folder:**
   - Open **Finder**
   - Press **Cmd + Shift + G**
   - Type: `/usr/local/bin` and press Enter
   - If prompted, click "Create" (you may need to authenticate)
1. **Move the files:**
   - From the extracted folder, drag both files to `/usr/local/bin`:
     - `utf8proj` — Command-line tool
     - `utf8proj-lsp` — Language server (enables smart editing)

> **Alternative location:** If you can't access `/usr/local/bin`, create a folder in your home directory:
> - Open Terminal (Applications → Utilities → Terminal)
> - Type: `mkdir -p ~/bin` and press Enter
> - Move the files to `~/bin` instead

-----

## Step 3: Configure VS Code for utf8proj

### 3a. Install the Generic LSP Extension

1. Open VS Code
1. Press **Cmd + Shift + X** (opens Extensions)
1. In the search box, type: **glspc**
1. Find **"Generic LSP Client"** and click **Install**

### 3b. Configure the Extension

1. Press **Cmd + Shift + P** (opens Command Palette)
1. Type: **settings json**
1. Select: **"Preferences: Open User Settings (JSON)"**
1. Add this configuration (paste inside the `{ }` braces):

**If you installed to /usr/local/bin:**
```json
"glspc.languageId": "utf8proj",
"glspc.serverCommand": "/usr/local/bin/utf8proj-lsp",
"glspc.pathGlob": "**/*.proj"
```

**If you installed to ~/bin:**
```json
"glspc.languageId": "utf8proj",
"glspc.serverCommand": "/Users/YOUR_USERNAME/bin/utf8proj-lsp",
"glspc.pathGlob": "**/*.proj"
```

Replace `YOUR_USERNAME` with your Mac username.

If the file already has content, add a comma after the last entry before pasting.

**Example of complete settings.json:**

```json
{
    "editor.fontSize": 14,
    "glspc.languageId": "utf8proj",
    "glspc.serverCommand": "/usr/local/bin/utf8proj-lsp",
    "glspc.pathGlob": "**/*.proj"
}
```

1. Save the file (**Cmd + S**)
1. **Restart VS Code** (Quit and reopen)

-----

## Step 4: Test It Works

1. In VS Code: **File → Open Folder**
1. Navigate to your project folder (or create a new one)
1. Create a new file: **File → New File**
1. Save it as `test.proj`
1. Type this example:

```
project "Test Project" {
    start: 2026-02-01
}

task hello "Hello World" {
    duration: 5d
}
```

✅ **If working correctly:** No red squiggles = valid file

Now try typing an error:

```
task broken "Missing brace" {
    duration: 5d
```

🔴 **You should see:** A red squiggle indicating the missing `}`

-----

## Step 5: Understanding the Editor

### What the Squiggles Mean

|Visual            |Meaning                     |Action       |
|------------------|----------------------------|-------------|
|🔴 Red underline   |Error — file won't process  |Must fix     |
|🟡 Yellow underline|Warning — review recommended|Should review|
|No underline      |Valid                       |Good to go ✓ |

**Hover over any squiggle** to see a plain-English explanation of what's wrong.

-----

## Editing Guide for Non-Technical Users

### ✅ Safe to Change

|What           |Example         |How to Edit                         |
|---------------|----------------|------------------------------------|
|Dates          |`2026-01-15`    |Use Year-Month-Day format           |
|Durations      |`5d`, `2w`, `1m`|d=days, w=weeks, m=months           |
|Completion %   |`complete: 40%` |Update the number as work progresses|
|Names in quotes|`"Sprint 1"`    |Change text, keep the quotes        |

### ❌ Do Not Change

|What        |Example                     |Why                       |
|------------|----------------------------|--------------------------|
|Keywords    |`task`, `depends`, `project`|System words              |
|Curly braces|`{ }`                       |Structure markers         |
|Task IDs    |`task sprint1`              |Referenced by dependencies|
|Colons      |`duration:`                 |Syntax separators         |

### 🔄 If Something Breaks

Press **Cmd + Z** repeatedly to undo until the red squiggle disappears.

-----

## Quick Reference: Common Edits

### Update task progress:

```
task design "Design Phase" {
    duration: 5d
    complete: 75%    ← change this number
}
```

### Change project dates:

```
project "Website Redesign" {
    start: 2026-02-01    ← change start date
    end: 2026-06-30      ← change end date
}
```

### Add a comment (ignored by the system):

```
# This is a note to myself
# The system ignores these lines
task review "Final Review" {
    duration: 2d
}
```

-----

## Daily Workflow

```
1. Open VS Code
2. Open your project folder (File → Open Folder)
3. Click your .proj file
4. Make your edits (update progress, dates, etc.)
5. Watch for red squiggles → fix any that appear
6. Save (Cmd + S)
7. Share the file with your team
```

-----

## Troubleshooting

|Problem                   |Solution                                                    |
|--------------------------|------------------------------------------------------------|
|No syntax colors          |Restart VS Code                                             |
|No red squiggles appearing|Check that the utf8proj-lsp file exists at the configured path|
|"Server not found" error  |Verify the path in settings.json is correct                 |
|"Permission denied" error |See "Fix Permissions" below                                 |
|Squiggle message unclear  |Hover longer, or ask your project lead                      |
|Made too many mistakes    |Close file **without saving**, then reopen                  |

### Fix Permissions (if needed)

If you see a "permission denied" error:

1. Open **Terminal** (Applications → Utilities → Terminal)
1. Type this command and press Enter:

   **If installed to /usr/local/bin:**
   ```
   chmod +x /usr/local/bin/utf8proj-lsp
   ```

   **If installed to ~/bin:**
   ```
   chmod +x ~/bin/utf8proj-lsp
   ```

1. Restart VS Code

### Still Not Working?

1. Press **Cmd + Shift + U** to open the Output panel
1. Look for error messages mentioning "utf8proj" or "LSP"
1. Common fix: restart VS Code after any settings change

-----

## macOS Keyboard Shortcuts

| Action | Shortcut |
|--------|----------|
| Save | Cmd + S |
| Undo | Cmd + Z |
| Redo | Cmd + Shift + Z |
| Find | Cmd + F |
| Open folder | Cmd + O |
| Command Palette | Cmd + Shift + P |
| Extensions | Cmd + Shift + X |
| Autocomplete | Ctrl + Space |

-----

## One-Sentence Explanation for Colleagues

> "Edit the `.proj` file in VS Code — red squiggles show mistakes instantly, like spell-check in Pages."

-----

## Summary

|Component     |What It Does                         |
|--------------|-------------------------------------|
|VS Code       |Your editor                          |
|utf8proj-lsp  |Checks your file in real-time        |
|utf8proj      |Advanced command-line tool (optional)|
|.proj file    |Your project schedule                |

-----

## Download Links Reference

|Platform        |Link                                                                                                        |
|----------------|------------------------------------------------------------------------------------------------------------|
|**macOS (M1/M2/M3)**|https://github.com/alanbld/utf8proj/releases/download/v0.2.0/utf8proj-v0.2.0-aarch64-apple-darwin.tar.gz|
|**macOS (Intel)**   |https://github.com/alanbld/utf8proj/releases/download/v0.2.0/utf8proj-v0.2.0-x86_64-apple-darwin.tar.gz |
|Windows         |https://github.com/alanbld/utf8proj/releases/download/v0.2.0/utf8proj-v0.2.0-x86_64-pc-windows-msvc.zip     |
|Linux           |https://github.com/alanbld/utf8proj/releases/download/v0.2.0/utf8proj-v0.2.0-x86_64-unknown-linux-gnu.tar.gz|
