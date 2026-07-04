Add-Type -AssemblyName System.Drawing

# ── Header BMP: 150x57 ─────────────────────────────────────────────────────
$header = New-Object System.Drawing.Bitmap(150, 57)
$g = [System.Drawing.Graphics]::FromImage($header)
$g.SmoothingMode = 'AntiAlias'
$g.TextRenderingHint = 'ClearTypeGridFit'

# Dark navy background
$bgBrush = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
    (New-Object System.Drawing.Rectangle(0, 0, 150, 57)),
    [System.Drawing.Color]::FromArgb(255, 15, 23, 42),
    [System.Drawing.Color]::FromArgb(255, 23, 37, 84),
    [System.Drawing.Drawing2D.LinearGradientMode]::Horizontal
)
$g.FillRectangle($bgBrush, 0, 0, 150, 57)

# Left accent bar (indigo)
$accentBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 99, 102, 241))
$g.FillRectangle($accentBrush, 0, 0, 4, 57)

# VAJRA title
$fontTitle = New-Object System.Drawing.Font('Segoe UI', 14, [System.Drawing.FontStyle]::Bold)
$whiteBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 248, 250, 252))
$g.DrawString('VAJRA', $fontTitle, $whiteBrush, 12.0, 7.0)

# Subtitle
$fontSub = New-Object System.Drawing.Font('Segoe UI', 6, [System.Drawing.FontStyle]::Regular)
$grayBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 148, 163, 184))
$g.DrawString('Download Manager  v0.2.0-beta', $fontSub, $grayBrush, 14.0, 37.0)

$g.Dispose()
$outPath = 'D:\Project\Project-Vajra\vajra-ui-tauri\src-tauri\assets\header.bmp'
$header.Save($outPath, [System.Drawing.Imaging.ImageFormat]::Bmp)
$header.Dispose()
Write-Host "header.bmp saved ($outPath)"

# ── Sidebar BMP: 164x314 ───────────────────────────────────────────────────
$sidebar = New-Object System.Drawing.Bitmap(164, 314)
$g2 = [System.Drawing.Graphics]::FromImage($sidebar)
$g2.SmoothingMode = 'AntiAlias'
$g2.TextRenderingHint = 'ClearTypeGridFit'

# Dark background
$bgBrush2 = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
    (New-Object System.Drawing.Rectangle(0, 0, 164, 314)),
    [System.Drawing.Color]::FromArgb(255, 15, 23, 42),
    [System.Drawing.Color]::FromArgb(255, 8, 12, 26),
    [System.Drawing.Drawing2D.LinearGradientMode]::Vertical
)
$g2.FillRectangle($bgBrush2, 0, 0, 164, 314)

# Top accent bar
$accentBrush2 = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 99, 102, 241))
$g2.FillRectangle($accentBrush2, 0, 0, 164, 4)

# Lightning bolt polygon (VAJRA logo shape)
$pts = New-Object 'System.Drawing.Point[]' 7
$pts[0] = New-Object System.Drawing.Point(82, 48)
$pts[1] = New-Object System.Drawing.Point(71, 80)
$pts[2] = New-Object System.Drawing.Point(81, 78)
$pts[3] = New-Object System.Drawing.Point(74, 112)
$pts[4] = New-Object System.Drawing.Point(96, 72)
$pts[5] = New-Object System.Drawing.Point(84, 75)
$pts[6] = New-Object System.Drawing.Point(97, 48)
$boltBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 99, 102, 241))
$g2.FillPolygon($boltBrush, $pts)

# Glow ring around bolt
$penGlow = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(60, 99, 102, 241), 10)
$g2.DrawEllipse($penGlow, 60, 44, 44, 72)

# VAJRA text
$sf = New-Object System.Drawing.StringFormat
$sf.Alignment = 'Center'
$fontApp = New-Object System.Drawing.Font('Segoe UI', 16, [System.Drawing.FontStyle]::Bold)
$whiteBrush2 = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 248, 250, 252))
$g2.DrawString('VAJRA', $fontApp, $whiteBrush2, 82.0, 122.0, $sf)

# Version tag
$fontVer = New-Object System.Drawing.Font('Segoe UI', 7, [System.Drawing.FontStyle]::Regular)
$grayBrush2 = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(200, 100, 116, 139))
$g2.DrawString('v0.2.0-beta', $fontVer, $grayBrush2, 82.0, 144.0, $sf)

# Horizontal divider
$penLine = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(60, 99, 102, 241), 1)
$g2.DrawLine($penLine, 24, 168, 140, 168)

# Tagline
$fontTag = New-Object System.Drawing.Font('Segoe UI', 7, [System.Drawing.FontStyle]::Italic)
$mutedBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(160, 148, 163, 184))
$g2.DrawString('Fast. Reliable. Unstoppable.', $fontTag, $mutedBrush, 82.0, 178.0, $sf)

$g2.Dispose()
$outPath2 = 'D:\Project\Project-Vajra\vajra-ui-tauri\src-tauri\assets\sidebar.bmp'
$sidebar.Save($outPath2, [System.Drawing.Imaging.ImageFormat]::Bmp)
$sidebar.Dispose()
Write-Host "sidebar.bmp saved ($outPath2)"
