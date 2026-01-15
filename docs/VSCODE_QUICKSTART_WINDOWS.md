# utf8proj — VS Code Quick Start (Windows)

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

No terminal commands. No programming. Just edit and watch for squiggles.

-----

## Step 1: Install Visual Studio Code

1. Go to: **https://code.visualstudio.com**
1. Click **Download for Windows**
1. Run the installer (keep all defaults)
1. Launch VS Code when finished

-----

## Step 2: Download utf8proj

1. Open this link in your browser:

   👉 **https://github.com/alanbld/utf8proj/releases/download/v0.2.0/utf8proj-v0.2.0-x86_64-pc-windows-msvc.zip**
1. Save the file to your Downloads folder
1. **Extract the ZIP file:**

- Right-click the downloaded file
- Select **"Extract All…"**
- Extract to: `C:\utf8proj\`

1. After extraction, you should have:

   ```
   C:\utf8proj\
       utf8proj.exe        ← Command-line tool
       utf8proj-lsp.exe    ← Language server (enables smart editing)
   ```

-----

## Step 3: Configure VS Code for utf8proj

### 3a. Install the Generic LSP Extension

1. Open VS Code
1. Press **Ctrl + Shift + X** (opens Extensions)
1. In the search box, type: **glspc**
1. Find **"Generic LSP Client"** and click **Install**

### 3b. Configure the Extension

1. Press **Ctrl + Shift + P** (opens Command Palette)
1. Type: **settings json**
1. Select: **"Preferences: Open User Settings (JSON)"**
1. Add this configuration (paste inside the `{ }` braces):

```json
"glspc.languageId": "utf8proj",
"glspc.serverCommand": "C:\\utf8proj\\utf8proj-lsp.exe",
"glspc.pathGlob": "**/*.proj"
```

If the file already has content, add a comma after the last entry before pasting.

**Example of complete settings.json:**

```json
{
    "editor.fontSize": 14,
    "glspc.languageId": "utf8proj",
    "glspc.serverCommand": "C:\\utf8proj\\utf8proj-lsp.exe",
    "glspc.pathGlob": "**/*.proj"
}
```

1. Save the file (**Ctrl + S**)
1. **Restart VS Code** (close and reopen)

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

Press **Ctrl + Z** repeatedly to undo until the red squiggle disappears.

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
6. Save (Ctrl + S)
7. Share the file with your team
```

-----

## Troubleshooting

|Problem                   |Solution                                          |
|--------------------------|--------------------------------------------------|
|No syntax colors          |Restart VS Code                                   |
|No red squiggles appearing|Check that `C:\utf8proj\utf8proj-lsp.exe` exists  |
|"Server not found" error  |Verify the path in settings.json uses `\\` not `\`|
|Squiggle message unclear  |Hover longer, or ask your project lead            |
|Made too many mistakes    |Close file **without saving**, then reopen        |

### Still Not Working?

1. Press **Ctrl + Shift + U** to open the Output panel
1. Look for error messages mentioning "utf8proj" or "LSP"
1. Common fix: restart VS Code after any settings change

-----

## One-Sentence Explanation for Colleagues

> "Edit the `.proj` file in VS Code — red squiggles show mistakes instantly, like spell-check in Word."

-----

## Summary

|Component       |What It Does                         |
|----------------|-------------------------------------|
|VS Code         |Your editor                          |
|utf8proj-lsp.exe|Checks your file in real-time        |
|utf8proj.exe    |Advanced command-line tool (optional)|
|.proj file      |Your project schedule                |

-----

## Download Links Reference

|Platform        |Link                                                                                                        |
|----------------|------------------------------------------------------------------------------------------------------------|
|**Windows**     |https://github.com/alanbld/utf8proj/releases/download/v0.2.0/utf8proj-v0.2.0-x86_64-pc-windows-msvc.zip     |
|macOS (Intel)   |https://github.com/alanbld/utf8proj/releases/download/v0.2.0/utf8proj-v0.2.0-x86_64-apple-darwin.tar.gz     |
|macOS (M1/M2/M3)|https://github.com/alanbld/utf8proj/releases/download/v0.2.0/utf8proj-v0.2.0-aarch64-apple-darwin.tar.gz    |
|Linux           |https://github.com/alanbld/utf8proj/releases/download/v0.2.0/utf8proj-v0.2.0-x86_64-unknown-linux-gnu.tar.gz|
