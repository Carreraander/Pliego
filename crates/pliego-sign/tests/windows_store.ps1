<#
.SYNOPSIS
  Prueba manual (solo Windows) de `pliego sign --store`: crea certificados autofirmados de prueba en
  CurrentUser\My (RSA-2048 y ECDSA P-256), firma un PDF con cada uno, lo valida con `pliego verify --strict`,
  comprueba que alterar un byte rompe la firma y borra los certificados y sus claves al terminar.

.EXAMPLE
  . .\tools\acr12-env.ps1; cargo build -p pliego-cli
  .\crates\pliego-sign\tests\windows_store.ps1 -Pliego .\target\debug\pliego.exe
#>
param(
    [Parameter(Mandatory)] [string] $Pliego,
    [string] $Pdf = (Join-Path $PSScriptRoot '..\..\..\tests\corpus\basic\hola.pdf')
)
$ErrorActionPreference = 'Stop'
$work = Join-Path ([IO.Path]::GetTempPath()) ("pliego-store-" + [guid]::NewGuid())
New-Item -ItemType Directory $work | Out-Null
$failed = $false
$kinds = @(
    @{ Name = 'RSA'; Args = @{ KeyAlgorithm = 'RSA'; KeyLength = 2048 } },
    @{ Name = 'P256'; Args = @{ KeyAlgorithm = 'ECDSA_nistP256' } }
)
foreach ($k in $kinds) {
    $a = $k.Args
    $cert = New-SelfSignedCertificate @a -Subject "CN=Prueba Pliego W05 $($k.Name)" -CertStoreLocation Cert:\CurrentUser\My `
        -KeyUsage DigitalSignature, NonRepudiation -KeyExportPolicy NonExportable -HashAlgorithm SHA256 `
        -Provider 'Microsoft Software Key Storage Provider' -NotAfter (Get-Date).AddDays(2)
    try {
        $thumb = $cert.Thumbprint
        $cer = Join-Path $work "$($k.Name).cer"
        Export-Certificate -Cert $cert -FilePath $cer | Out-Null
        $out = Join-Path $work "firmado-$($k.Name).pdf"
        & $Pliego sign $Pdf -o $out --store $thumb --reason "Prueba almacén $($k.Name)"
        if ($LASTEXITCODE -ne 0) { throw "falló la firma $($k.Name)" }
        & $Pliego verify $out --trust $cer --strict
        if ($LASTEXITCODE -ne 0) { throw "la firma $($k.Name) no se validó" }
        # Alterar un byte dentro del contenido firmado debe romper la firma.
        $bytes = [IO.File]::ReadAllBytes($out)
        $bytes[20] = $bytes[20] -bxor 0x01
        $broken = Join-Path $work "roto-$($k.Name).pdf"
        [IO.File]::WriteAllBytes($broken, $bytes)
        & $Pliego verify $broken --trust $cer | Out-Host
        if ($LASTEXITCODE -eq 0) { throw "la firma $($k.Name) alterada se dio por buena" }
        Write-Host "OK $($k.Name) ($thumb)"
    } catch {
        Write-Warning $_
        $failed = $true
    } finally {
        Remove-Item "Cert:\CurrentUser\My\$($cert.Thumbprint)" -DeleteKey
    }
}
Remove-Item -Recurse -Force $work
if ($failed) { exit 1 }
exit 0
