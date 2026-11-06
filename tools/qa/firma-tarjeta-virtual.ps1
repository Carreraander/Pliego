<#
.SYNOPSIS
  Prueba de firma de Pliego con una «tarjeta inteligente» virtual TPM en Windows (sin lector ni DNIe).

.DESCRIPTION
  Pliego firma por CNG (WindowsStoreSigner), así que una tarjeta virtual TPM ejerce el mismo camino que un DNIe.
  El script, de principio a fin:
    1. crea la tarjeta con `tpmvscmgr create` (PIN conocido);
    2. matricula en ella un certificado autofirmado con `certreq` y un INF (proveedor «Microsoft Smart Card Key
       Storage Provider»; `New-SelfSignedCertificate` NO sirve: falla con SCARD_E_UNEXPECTED);
    3. firma con `pliego sign --store <huella>` rellenando el cuadro de PIN de Windows por UI Automation
       (ValuePattern/InvokePattern dirigidos al propio cuadro; nunca se envían teclas);
    4. comprueba `pliego verify --trust <cer> --strict` (debe pasar) y sin --trust con --strict (debe fallar);
    5. altera un bit del PDF firmado y comprueba que la firma queda rota;
    6. cancela el cuadro de PIN y comprueba que pliego falla con «cancelada» (Error::Cancelled) sin dejar salida;
    7. lo limpia todo (certificado, tarjeta, procesos auxiliares y temporales) aunque algo falle.

  Requisitos: Windows con TPM listo, sesión de escritorio interactiva y terminal ELEVADA (tpmvscmgr lo exige).
  Un pliego.exe compilado necesita sus DLL en el PATH: ejecute antes `. .\tools\pc-env.ps1` (o acr12-env.ps1).
  Con un PIN erróneo Windows vuelve a pedir el PIN y pliego se queda esperando (no se prueba aquí).

.PARAMETER Pliego
  Ruta de pliego.exe.
.PARAMETER Pdf
  PDF de entrada (sin firmar; no se modifica).
.PARAMETER Pin
  PIN de la tarjeta virtual (por defecto 12345678, el que fija `/pin default`).
.PARAMETER Salida
  Carpeta de trabajo y evidencias (por defecto C:\dev-cache\qa\firma-tarjeta-virtual). Se vacían las firmas anteriores.
.PARAMETER ConservarSalida
  No borra los PDF generados (los certificados y la tarjeta se borran siempre).

.EXAMPLE
  . .\tools\pc-env.ps1
  .\tools\qa\firma-tarjeta-virtual.ps1 -Pliego C:\dev-cache\target\compartido\debug\pliego.exe -Pdf C:\dev-cache\w43\hola.pdf

  Código de salida: 0 = todas las comprobaciones pasan; 1 = alguna falla; 2 = no se pudo preparar la prueba.
#>
[CmdletBinding()]
param(
  [Parameter(Mandatory)][string]$Pliego,
  [Parameter(Mandatory)][string]$Pdf,
  [string]$Pin = '12345678',
  [string]$Salida = 'C:\dev-cache\qa\firma-tarjeta-virtual',
  [switch]$ConservarSalida
)
Set-StrictMode -Version 2
# 'Continue' a propósito: en Windows PowerShell 5.1, `2>&1` sobre un ejecutable nativo con salida por stderr lanzaría
# una excepción con 'Stop'. Los cmdlets que deben abortar llevan -ErrorAction Stop; el resto se comprueba a mano.
$ErrorActionPreference = 'Continue'
$script:noPreparada = $false
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$resultados = [System.Collections.Generic.List[object]]::new()
function Anota([string]$nombre, [bool]$ok, [string]$detalle = '') {
  $resultados.Add([pscustomobject]@{ Prueba = $nombre; Resultado = $(if ($ok) { 'OK' } else { 'FALLA' }); Detalle = $detalle })
  Write-Host ("[{0}] {1} {2}" -f $(if ($ok) { ' OK ' } else { 'FALLA' }), $nombre, $detalle)
}

# ---- Rellenador del cuadro de PIN de Windows (UI Automation, sin teclas) ---------------------------------------------
# Se ejecuta en un trabajo aparte mientras pliego espera. Busca el cuadro por clase (el texto depende del idioma).
$rellenador = {
  param([string]$Modo, [string]$Pin, [int]$Espera)
  Add-Type -AssemblyName UIAutomationClient
  Add-Type -AssemblyName UIAutomationTypes
  $AE = [System.Windows.Automation.AutomationElement]
  $root = $AE::RootElement
  $cond = New-Object System.Windows.Automation.PropertyCondition($AE::ClassNameProperty, 'Credential Dialog Xaml Host')
  $fin = (Get-Date).AddSeconds($Espera)
  $w = $null
  while ((Get-Date) -lt $fin -and -not $w) { $w = $root.FindFirst('Children', $cond); if (-not $w) { Start-Sleep -Milliseconds 250 } }
  if (-not $w) { return 'no aparecio el cuadro de PIN' }
  Start-Sleep -Milliseconds 900   # el arbol UIA tarda un poco en tener el campo y los botones
  $buscar = {
    param($id)
    $c = New-Object System.Windows.Automation.PropertyCondition($AE::AutomationIdProperty, $id)
    $w.FindFirst('Descendants', $c)
  }
  try {
    if ($Modo -eq 'cancelar') {
      (& $buscar 'CancelButton').GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
      return 'cuadro de PIN: pulsado Cancelar'
    }
    (& $buscar 'PasswordField_0').GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($Pin)
    Start-Sleep -Milliseconds 300
    (& $buscar 'OkButton').GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    return 'cuadro de PIN: PIN escrito y pulsado OK'
  } catch {
    return "cuadro de PIN: error de automatizacion: $($_.Exception.Message)"
  }
}

# Ejecuta una orden con el cuadro de PIN atendido en paralelo. Devuelve exit code, stdout, stderr y el informe del relleno.
function Invoke-ConPin([string]$Exe, [string[]]$Argumentos, [string]$Modo, [int]$MaxSeg = 90) {
  $job = Start-Job -ScriptBlock $rellenador -ArgumentList $Modo, $Pin, ($MaxSeg - 5)
  $so = Join-Path $Salida 'stdout.tmp'; $se = Join-Path $Salida 'stderr.tmp'
  try {
    $p = Start-Process -FilePath $Exe -ArgumentList $Argumentos -PassThru -NoNewWindow -RedirectStandardOutput $so -RedirectStandardError $se
    $null = $p.Handle   # sin esto, ExitCode puede salir vacio tras WaitForExit(ms)
    $colgado = -not $p.WaitForExit($MaxSeg * 1000)
    if ($colgado) { try { $p.Kill() } catch {} ; $p.WaitForExit(5000) | Out-Null }
    $relleno = if (Wait-Job $job -Timeout 10) { (Receive-Job $job) -join '; ' } else { 'el rellenador no termino' }
    [pscustomobject]@{
      Exit = $(if ($colgado) { -999 } else { $p.ExitCode }); Colgado = $colgado
      Out = (Get-Content $so -Raw -Encoding UTF8 -ErrorAction SilentlyContinue); Err = (Get-Content $se -Raw -Encoding UTF8 -ErrorAction SilentlyContinue)
      Relleno = $relleno
    }
  } finally {
    Stop-Job $job -ErrorAction SilentlyContinue; Remove-Job $job -Force -ErrorAction SilentlyContinue
  }
}

# ---- Comprobaciones previas ------------------------------------------------------------------------------------------
$preparada = $false
$instancia = $null
$huella = $null
try {
  $esAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
  if (-not $esAdmin) { throw 'La terminal no esta elevada (tpmvscmgr exige administrador).' }
  if (-not (Get-Command tpmvscmgr.exe -ErrorAction SilentlyContinue)) { throw 'No existe tpmvscmgr.exe (hace falta Windows con tarjetas virtuales TPM).' }
  $tpm = Get-Tpm -ErrorAction Stop
  if (-not $tpm.TpmReady) { throw 'El TPM no esta listo (Get-Tpm).' }
  if (-not [Environment]::UserInteractive -or -not (Get-Process explorer -ErrorAction SilentlyContinue)) { throw 'No hay sesion de escritorio interactiva (el cuadro de PIN la necesita).' }
  if (-not (Test-Path $Pliego)) { throw "No existe pliego.exe en $Pliego" }
  if (-not (Test-Path $Pdf)) { throw "No existe el PDF $Pdf" }
  $Pliego = (Resolve-Path $Pliego).Path; $Pdf = (Resolve-Path $Pdf).Path
  & $Pliego --version | Out-Null
  if ($LASTEXITCODE -ne 0) { throw "pliego.exe no arranca (codigo $LASTEXITCODE; ¿falta '. .\tools\pc-env.ps1' para las DLL en el PATH?)" }
  if (Get-Process -Name pliego -ErrorAction SilentlyContinue) { Write-Warning 'Hay otro proceso pliego en marcha; no se toca.' }

  New-Item -ItemType Directory -Force $Salida -ErrorAction Stop | Out-Null
  Get-ChildItem $Salida -File -ErrorAction SilentlyContinue | Remove-Item -Force -ErrorAction SilentlyContinue
  $firmado = Join-Path $Salida 'firmado.pdf'
  $manipulado = Join-Path $Salida 'firmado-manipulado.pdf'
  $cancelado = Join-Path $Salida 'firmado-cancelado.pdf'
  $cer = Join-Path $Salida 'tarjeta-virtual.cer'
  $inf = Join-Path $Salida 'tarjeta-virtual.inf'
  $nombreTarjeta = 'PliegoQA'

  # ---- 1. Tarjeta virtual TPM --------------------------------------------------------------------------------------
  $antes = @(Get-PnpDevice -ErrorAction SilentlyContinue | Where-Object { $_.InstanceId -like 'ROOT\SMARTCARDREADER*' } | ForEach-Object InstanceId)
  if ($Pin -ne '12345678') { throw 'tpmvscmgr /pin default fija el PIN 12345678; use ese valor (o adapte el script con /pin prompt).' }
  $crear = & tpmvscmgr.exe create /name $nombreTarjeta /pin default /adminkey random /generate 2>&1 | Out-String
  # La salida de tpmvscmgr viene en UTF-16 con huecos: se quitan espacios y nulos antes de buscar la instancia.
  $limpio = $crear -replace '[\0\s]', ''
  if ($limpio -match 'InstanceID=(ROOT\\SMARTCARDREADER\\\d+)') { $instancia = $Matches[1] }
  if (-not $instancia) {
    $despues = @(Get-PnpDevice -ErrorAction SilentlyContinue | Where-Object { $_.InstanceId -like 'ROOT\SMARTCARDREADER*' } | ForEach-Object InstanceId)
    $instancia = $despues | Where-Object { $antes -notcontains $_ } | Select-Object -First 1
  }
  if (-not $instancia) { throw "No se pudo crear la tarjeta virtual. Salida: $limpio" }
  Anota 'Crear tarjeta virtual TPM' $true $instancia

  # ---- 2. Certificado autofirmado en la tarjeta (certreq; New-SelfSignedCertificate falla con SCARD_E_UNEXPECTED) -----
  @'
[Version]
Signature="$Windows NT$"

[NewRequest]
Subject = "CN=Pliego Prueba TPM VSC"
KeyLength = 2048
KeySpec = 2
KeyUsage = 0x80
Exportable = FALSE
MachineKeySet = FALSE
UserProtected = FALSE
ProviderName = "Microsoft Smart Card Key Storage Provider"
RequestType = Cert
HashAlgorithm = SHA256
ValidityPeriod = Days
ValidityPeriodUnits = 30
'@ | Set-Content -Path $inf -Encoding ASCII
  # Windows tarda en reconocer la tarjeta recién creada; sin esperar, certreq falla a veces con SCARD_E_UNEXPECTED
  # (0x8010001f), y lo hace mostrando un cuadro de error que hay que cerrar. Se espera a que el lector la vea y se
  # reintenta (matando certreq si sale ese cuadro).
  $listo = $false
  for ($i = 0; $i -lt 30 -and -not $listo; $i++) {
    $sc = (& certutil.exe -scinfo 2>&1 | Out-String) -replace '[\0]', ''
    if ($sc -match 'Microsoft Virtual Smart Card[\s\S]*?available for use|Microsoft Virtual Smart Card[\s\S]*?SCARD_STATE_PRESENT') { $listo = $true } else { Start-Sleep -Seconds 1 }
  }
  Start-Sleep -Seconds 3
  $req = ''
  for ($intento = 1; $intento -le 3 -and -not $huella; $intento++) {
    $job = Start-Job -ScriptBlock $rellenador -ArgumentList 'pin', $Pin, 60
    $so = Join-Path $Salida 'certreq.tmp'
    $cr = Start-Process certreq.exe -ArgumentList @('-new', '-f', "`"$inf`"", "`"$(Join-Path $Salida 'tarjeta-virtual-req.cer')`"") -PassThru -WindowStyle Hidden -RedirectStandardOutput $so
    $null = $cr.Handle
    $limite = (Get-Date).AddSeconds(75)
    while (-not $cr.HasExited -and (Get-Date) -lt $limite) {
      # cuadro de error propio de certreq (clase #32770): se lee y se cierra matando el proceso
      $dlg = [System.Windows.Automation.AutomationElement]::RootElement.FindFirst('Children',
        (New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ProcessIdProperty, $cr.Id)))
      if ($dlg -and $dlg.Current.ClassName -eq '#32770') {
        $req = 'cuadro de error de certreq: ' + ((($dlg.FindAll('Descendants', [System.Windows.Automation.Condition]::TrueCondition) | ForEach-Object { $_.Current.Name } | Where-Object { $_ -and $_ -ne 'OK' }) -join ' ') )
        break
      }
      Start-Sleep -Milliseconds 500
    }
    if (-not $cr.HasExited) { try { $cr.Kill() } catch {} }
    $cr.WaitForExit(5000) | Out-Null
    $null = Wait-Job $job -Timeout 5; Stop-Job $job -ErrorAction SilentlyContinue; Remove-Job $job -Force -ErrorAction SilentlyContinue
    $req += ' ' + (Get-Content $so -Raw -ErrorAction SilentlyContinue)
    if ($req -match 'Thumbprint:\s*([0-9a-fA-F]{40})') { $huella = $Matches[1].ToUpper() }
    elseif ($intento -lt 3) { Write-Host "certreq intento $intento fallido ($($req.Trim())); reintento"; Start-Sleep -Seconds 5; $req = '' }
  }
  if (-not $huella -or -not (Test-Path "Cert:\CurrentUser\My\$huella")) { throw "certreq no matriculo el certificado. Salida: $req" }
  Export-Certificate -Cert "Cert:\CurrentUser\My\$huella" -FilePath $cer -ErrorAction Stop | Out-Null
  Anota 'Matricular certificado (certreq)' $true $huella
  $preparada = $true

  # ---- 3. Firma con PIN correcto -----------------------------------------------------------------------------------
  $r = Invoke-ConPin $Pliego @('sign', $Pdf, '-o', $firmado, '--store', $huella, '--reason', 'Prueba_QA_tarjeta_virtual') 'pin'
  Anota 'pliego sign --store (PIN correcto)' (($r.Exit -eq 0) -and (Test-Path $firmado)) "exit=$($r.Exit) $($r.Relleno)"

  # ---- 4. Verificacion ---------------------------------------------------------------------------------------------
  if (Test-Path $firmado) {
    & $Pliego verify $firmado --trust $cer --strict | Out-Null
    Anota 'verify --trust --strict (debe pasar)' ($LASTEXITCODE -eq 0) "exit=$LASTEXITCODE"
    & $Pliego verify $firmado --strict | Out-Null
    Anota 'verify --strict sin --trust (debe fallar: raiz sin confianza)' ($LASTEXITCODE -ne 0) "exit=$LASTEXITCODE"

    # ---- 5. Alterar un bit del contenido original ----------------------------------------------------------------
    $bytes = [IO.File]::ReadAllBytes($firmado)
    $limite = [IO.File]::ReadAllBytes($Pdf).Length - 1
    $pos = -1
    for ($i = 0; $i -lt $limite - 1; $i++) {   # primera letra tras un '(' del contenido original (texto de una cadena)
      if ($bytes[$i] -eq 0x28 -and (($bytes[$i + 1] -ge 0x41 -and $bytes[$i + 1] -le 0x5A) -or ($bytes[$i + 1] -ge 0x61 -and $bytes[$i + 1] -le 0x7A))) { $pos = $i + 1; break }
    }
    if ($pos -lt 0) { $pos = [int]($limite / 2) }
    $bytes[$pos] = $bytes[$pos] -bxor 0x01
    [IO.File]::WriteAllBytes($manipulado, $bytes)
    $salidaV = & $Pliego verify $manipulado --trust $cer --strict 2>&1 | Out-String
    Anota 'verify tras alterar 1 bit (debe fallar)' ($LASTEXITCODE -ne 0) "byte $pos, exit=$LASTEXITCODE"
    Write-Verbose $salidaV
  }

  # ---- 6. Cancelar el cuadro de PIN --------------------------------------------------------------------------------
  $r = Invoke-ConPin $Pliego @('sign', $Pdf, '-o', $cancelado, '--store', $huella) 'cancelar'
  $okCancel = ($r.Exit -ne 0) -and -not $r.Colgado -and -not (Test-Path $cancelado) -and ($r.Err -match 'cancelad')
  Anota 'Cancelar el PIN da Error::Cancelled y sin salida' $okCancel "exit=$($r.Exit) stderr='$((($r.Err -replace '\s+', ' ')).Trim())' $($r.Relleno)"
}
catch {
  Anota 'Preparacion/ejecucion' $false $_.Exception.Message
  if (-not $preparada) { $script:noPreparada = $true }
}
finally {
  # ---- 7. Limpieza (siempre) ---------------------------------------------------------------------------------------
  Write-Host '--- Limpieza'
  Get-Job | Where-Object { $_.State -in 'Running', 'NotStarted' } | Stop-Job -ErrorAction SilentlyContinue
  Get-Job | Remove-Job -Force -ErrorAction SilentlyContinue
  if ($huella -and (Test-Path "Cert:\CurrentUser\My\$huella")) {
    try { Remove-Item "Cert:\CurrentUser\My\$huella" -ErrorAction Stop; Write-Host "certificado $huella borrado del almacen" }
    catch { Write-Warning "no se pudo borrar el certificado: $($_.Exception.Message)" }
  }
  if ($instancia) {
    $d = & tpmvscmgr.exe destroy /instance $instancia 2>&1 | Out-String
    $ok = ($d -replace '[\0\s]', '') -match 'destroyed'
    Write-Host "tarjeta $instancia $(if ($ok) { 'destruida' } else { 'NO destruida: ' + ($d -replace '[\0]', '') })"
    if (-not $ok) { Anota 'Destruir tarjeta virtual' $false "ejecute: tpmvscmgr.exe destroy /instance $instancia" }
  }
  Get-Process -Name certreq -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  if ($Salida -and (Test-Path $Salida)) {
    Remove-Item (Join-Path $Salida '*.tmp'), (Join-Path $Salida '*-req.cer') -Force -ErrorAction SilentlyContinue
    if (-not $ConservarSalida) {
      Remove-Item (Join-Path $Salida '*.pdf') -Force -ErrorAction SilentlyContinue
    }
  }
}

Write-Host ''
$resultados | Format-Table -AutoSize -Wrap | Out-String -Width 220 | Write-Host
$fallos = @($resultados | Where-Object Resultado -eq 'FALLA').Count
if ($script:noPreparada) { Write-Host 'No se pudo preparar la prueba.'; exit 2 }
if ($fallos -gt 0) { Write-Host "$fallos comprobacion(es) FALLAN"; exit 1 }
Write-Host 'Todas las comprobaciones pasan.'
exit 0
