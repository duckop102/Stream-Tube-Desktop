[CmdletBinding()]
param(
    # The fork whose Actions build and publish the Android releases.
    [string]$Repo = 'duckop102/Stream-Tube-Desktop',
    # Where the release key lives. Back this folder up: an APK signed with another key
    # cannot update the installed app.
    [string]$KeyDir = (Join-Path $HOME '.streamtube-android')
)

$ErrorActionPreference = 'Stop'
$alias = 'streamtube'
$keystore = Join-Path $KeyDir 'release.jks'
$passwordFile = Join-Path $KeyDir 'release.pass'

function Find-Keytool {
    if ($env:JAVA_HOME -and (Test-Path (Join-Path $env:JAVA_HOME 'bin/keytool.exe'))) {
        return Join-Path $env:JAVA_HOME 'bin/keytool.exe'
    }
    $unity = Get-ChildItem 'C:\Program Files\Unity\Hub\Editor\*\Editor\Data\PlaybackEngines\AndroidPlayer\OpenJDK\bin\keytool.exe' -ErrorAction SilentlyContinue |
        Sort-Object FullName | Select-Object -Last 1
    if ($unity) { return $unity.FullName }
    $onPath = Get-Command keytool -ErrorAction SilentlyContinue
    if ($onPath) { return $onPath.Source }
    throw 'keytool not found. Install JDK 17 or set JAVA_HOME.'
}

if (-not (Test-Path -LiteralPath $keystore)) {
    New-Item -ItemType Directory -Force -Path $KeyDir | Out-Null
    $bytes = New-Object byte[] 24
    [Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
    $password = [Convert]::ToBase64String($bytes) -replace '[+/=]', 'x'
    Set-Content -LiteralPath $passwordFile -Value $password -NoNewline -Encoding ascii
    $keytool = Find-Keytool
    & $keytool -genkeypair -v -keystore $keystore -storetype PKCS12 -alias $alias -keyalg RSA -keysize 4096 `
        -validity 10000 -storepass $password -keypass $password -dname 'CN=Stream Tube Android, O=Personal build'
    if ($LASTEXITCODE -ne 0) { throw 'keytool failed' }
    Write-Host "Created $keystore. Back up $KeyDir somewhere safe."
} else {
    Write-Host "Using the existing key in $keystore"
}

$password = Get-Content -LiteralPath $passwordFile -Raw
[Convert]::ToBase64String([IO.File]::ReadAllBytes($keystore)) | gh secret set ANDROID_KEYSTORE_BASE64 --repo $Repo
if ($LASTEXITCODE -ne 0) { throw 'Could not set ANDROID_KEYSTORE_BASE64' }
$password | gh secret set ANDROID_KEYSTORE_PASSWORD --repo $Repo
if ($LASTEXITCODE -ne 0) { throw 'Could not set ANDROID_KEYSTORE_PASSWORD' }
$alias | gh secret set ANDROID_KEY_ALIAS --repo $Repo
if ($LASTEXITCODE -ne 0) { throw 'Could not set ANDROID_KEY_ALIAS' }
Write-Host "Signing secrets are set on $Repo."
