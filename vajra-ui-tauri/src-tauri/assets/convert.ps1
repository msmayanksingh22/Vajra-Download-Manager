Add-Type -AssemblyName System.Drawing
$b1 = [System.Drawing.Image]::FromFile("d:\Project\Project-Vajra\vajra-ui-tauri\src-tauri\assets\sidebar.png")
$b1.Save("d:\Project\Project-Vajra\vajra-ui-tauri\src-tauri\assets\sidebar.bmp", [System.Drawing.Imaging.ImageFormat]::Bmp)
$b1.Dispose()
$b2 = [System.Drawing.Image]::FromFile("d:\Project\Project-Vajra\vajra-ui-tauri\src-tauri\assets\header.png")
$b2.Save("d:\Project\Project-Vajra\vajra-ui-tauri\src-tauri\assets\header.bmp", [System.Drawing.Imaging.ImageFormat]::Bmp)
$b2.Dispose()
