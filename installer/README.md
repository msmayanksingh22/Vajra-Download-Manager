# Vajra Installer (WiX 4)

This directory contains the WiX 4 installer project for Vajra Download Manager.

## Prerequisites

```powershell
# Install WiX 4 toolset
dotnet tool install -g wix

# Add required extensions
wix extension add WixToolset.UI.wixext
wix extension add WixToolset.Util.wixext
```

## Build Steps

1. **Build the app in Release mode** (in Visual Studio or MSBuild):
   ```powershell
   MSBuild.exe ..\vajra-ui\Vajra.sln /p:Configuration=Release /p:Platform=x64
   ```

2. **Publish the Native Messaging Host**:
   ```powershell
   dotnet publish ..\vajra-nm-host\vajra-nm-host.csproj -c Release -r win-x64 --self-contained true /p:PublishSingleFile=true
   ```

3. **Build the Rust engine DLL**:
   ```powershell
   cargo build --package vajra-ffi --release
   ```

4. **Build the MSI**:
   ```powershell
   cd installer
   wix build Vajra.wxs -o Vajra.msi
   ```

## Important Notes

- **Extension ID**: Before distributing, update the `allowed_origins` in `nm_manifest_installed.json` with your real Chrome Web Store extension ID.
- **App files**: The `.wxs` file references the `Release` build outputs. Make sure all required DLLs are present.
- **Signing**: For production, sign `Vajra.exe`, `vajra_nm_host.exe`, and `Vajra.msi` with a code-signing certificate.

## Development Testing (Without MSI)

Use `install.ps1` (run as Administrator) to copy files and register the NM host for testing:

```powershell
.\install.ps1
```

To uninstall:
```powershell
.\install.ps1 -Uninstall
```

For browser extension development, use `browser-extension\install_nm_dev.reg` to point Chrome/Edge at the local build output directly.
