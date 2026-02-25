$path = "d:/Rust/RustAndSocket/Rust_WebSocket_RelayServer-EC2-V2-Windows/backup"
$items = Get-ChildItem -Path $path
# Register encoding provider if needed (for .NET Core)
try {
    [System.Text.Encoding]::RegisterProvider([System.Text.CodePagesEncodingProvider]::Instance)
} catch {}

$enc874 = [System.Text.Encoding]::GetEncoding(874)

foreach ($item in $items) {
    try {
        $name = $item.Name
        # If the name is already clean (ASCII), skip
        # if ($name -match "^[a-zA-Z0-9_\-\.]+$") { continue }

        $bytes = New-Object System.Collections.Generic.List[Byte]
        $valid = $true

        foreach ($char in $name.ToCharArray()) {
            # specific fixups for CP1252 range 0x80-0x9F which might appear as Unicode chars
            # TIS-620 doesn't map these, but corrupted filenames often show these from CP1252.
            $mapped = $false
            if ($char -eq [char]0x20AC) { $bytes.Add(0x80); $mapped=$true } # Euro
            elseif ($char -eq [char]0x201A) { $bytes.Add(0x82); $mapped=$true }
            elseif ($char -eq [char]0x0192) { $bytes.Add(0x83); $mapped=$true }
            elseif ($char -eq [char]0x201E) { $bytes.Add(0x84); $mapped=$true }
            elseif ($char -eq [char]0x2026) { $bytes.Add(0x85); $mapped=$true }
            elseif ($char -eq [char]0x2020) { $bytes.Add(0x86); $mapped=$true }
            elseif ($char -eq [char]0x2021) { $bytes.Add(0x87); $mapped=$true }
            elseif ($char -eq [char]0x02C6) { $bytes.Add(0x88); $mapped=$true }
            elseif ($char -eq [char]0x2030) { $bytes.Add(0x89); $mapped=$true }
            elseif ($char -eq [char]0x0160) { $bytes.Add(0x8A); $mapped=$true }
            elseif ($char -eq [char]0x2039) { $bytes.Add(0x8B); $mapped=$true }
            elseif ($char -eq [char]0x0152) { $bytes.Add(0x8C); $mapped=$true }
            elseif ($char -eq [char]0x017D) { $bytes.Add(0x8E); $mapped=$true }
            elseif ($char -eq [char]0x2018) { $bytes.Add(0x91); $mapped=$true }
            elseif ($char -eq [char]0x2019) { $bytes.Add(0x92); $mapped=$true }
            elseif ($char -eq [char]0x201C) { $bytes.Add(0x93); $mapped=$true }
            elseif ($char -eq [char]0x201D) { $bytes.Add(0x94); $mapped=$true }
            elseif ($char -eq [char]0x2022) { $bytes.Add(0x95); $mapped=$true }
            elseif ($char -eq [char]0x2013) { $bytes.Add(0x96); $mapped=$true }
            elseif ($char -eq [char]0x2014) { $bytes.Add(0x97); $mapped=$true }
            elseif ($char -eq [char]0x02DC) { $bytes.Add(0x98); $mapped=$true }
            elseif ($char -eq [char]0x2122) { $bytes.Add(0x99); $mapped=$true }
            elseif ($char -eq [char]0x0161) { $bytes.Add(0x9A); $mapped=$true }
            elseif ($char -eq [char]0x203A) { $bytes.Add(0x9B); $mapped=$true }
            elseif ($char -eq [char]0x0153) { $bytes.Add(0x9C); $mapped=$true }
            elseif ($char -eq [char]0x017E) { $bytes.Add(0x9E); $mapped=$true }
            elseif ($char -eq [char]0x0178) { $bytes.Add(0x9F); $mapped=$true }
            
            if (-not $mapped) {
               # Try 874
               try {
                   $b = $enc874.GetBytes($char.ToString())
                   if ($b.Length -eq 1) {
                       $bytes.Add($b[0])
                   } else {
                       # ASCII fallback?
                       if ([int]$char -lt 128) {
                           $bytes.Add([byte][int]$char)
                       } else {
                           Write-Host "Char unmappable: $char in $name"
                           $valid = $false; break
                       }
                   }
               } catch {
                   $valid = $false; break
               }
            }
        }
        
        if ($valid) {
            $newBytes = $bytes.ToArray()
            $decoded = [System.Text.Encoding]::UTF8.GetString($newBytes)
            
            Write-Host "ORIG: $name"
            Write-Host "NEW : $decoded"
            Write-Host "---"
        }
    } catch {
        Write-Host "Error processing $($item.Name): $_"
    }
}
