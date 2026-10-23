# Dev setup

How to get a Windows 10/11 x64 machine ready to build Prompt Translator. The required tools and versions are listed in [`development.md`](development.md) section 1; this file only covers installing and verifying them. Day-to-day commands and the branch workflow are in `development.md` too.

Use PowerShell. Installers for Visual Studio Build Tools need admin rights (UAC prompt).

## 1. Install

1. **Git and GitHub CLI** (skip if `git --version` and `gh --version` already work):

   ```powershell
   winget install --id Git.Git -e
   winget install --id GitHub.cli -e
   gh auth login
   ```

2. **Node.js LTS** (includes npm):

   ```powershell
   winget install --id OpenJS.NodeJS.LTS -e
   ```

3. **Visual Studio 2022 Build Tools** with the "Desktop development with C++" workload (the MSVC compiler and linker Rust needs on Windows):

   ```powershell
   winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--passive --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
   ```

   This is a large download (several GB) and takes a while.

4. **Rust** through rustup, with the MSVC toolchain. Install the Build Tools first:

   ```powershell
   winget install --id Rustlang.Rustup -e
   rustup default stable-x86_64-pc-windows-msvc
   ```

5. **WebView2 Runtime.** It ships with Windows 11 and with current Windows 10 updates; nothing to install normally. Check it (read-only):

   ```powershell
   (Get-ItemProperty 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}').pv
   ```

   A version number such as `155.0.4283.62` means it is installed. If the command fails, install the Evergreen runtime from <https://developer.microsoft.com/microsoft-edge/webview2/>.

6. **Optional.** Claude Code (for the Claude provider): see <https://docs.claude.com/en/docs/claude-code>. Ollama (local provider and `eval`) is installed later by the spike ticket that needs it; it is not required for building.

## 2. Open a new shell

Installers change the user PATH, but a terminal that was already open keeps the old one. **Close every terminal (and restart VS Code or your editor) before verifying.** See "PATH not refreshed" below.

## 3. Verify

In a new PowerShell window:

```powershell
node -v
npm -v
rustc -V
cargo -V
rustup show active-toolchain
git --version
gh auth status
```

Expected: Node 24.x (LTS), npm 11.x, a stable Rust release, and the toolchain `stable-x86_64-pc-windows-msvc (default)`. To confirm the compiler and linker are installed, run `cargo build` in `src-tauri` once the project exists (ticket T2 creates it); it must link without errors.

## 4. Common errors

**`rustc`, `cargo` or `node` is not recognized.** PATH not refreshed: the shell was opened before the install. Open a new terminal. If it still fails, check the persistent user PATH:

```powershell
(Get-ItemProperty HKCU:\Environment).Path -split ';'
```

It must contain `%USERPROFILE%\.cargo\bin` (rustup adds it) and the Node.js directory (normally in the machine PATH, `C:\Program Files\nodejs`). If an entry is missing, add it under Settings, System, About, Advanced system settings, Environment Variables, then open a new terminal. In a stuck shell you can add it for that session only:

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
```

Tools that start from an IDE or an agent inherit the PATH of the process that launched them, so restart that process too.

**`link.exe` not found / `linker 'link.exe' not found`.** The MSVC build tools or the C++ workload is missing. Open "Visual Studio Installer", choose Modify on Build Tools 2022, and tick "Desktop development with C++" (with the MSVC v143 x64/x86 build tools and a Windows 10/11 SDK). Then open a new shell. If `link.exe` resolves to Git's `usr\bin\link.exe` instead, MSVC is not first on PATH; build from a normal PowerShell (rustc locates MSVC itself) rather than from Git Bash.

**`rustup` says the toolchain is `-gnu`.** Run `rustup default stable-x86_64-pc-windows-msvc`.

**`npm ci` or scripts blocked by execution policy.** PowerShell may refuse `npm.ps1`. Run `Set-ExecutionPolicy -Scope CurrentUser RemoteSigned` once.

**The first `cargo build` is very slow.** It compiles all Tauri dependencies; later builds are incremental.
