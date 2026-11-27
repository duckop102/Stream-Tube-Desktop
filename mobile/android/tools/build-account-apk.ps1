[CmdletBinding()]
param(
    [string]$CargoFerry = 'cargo-ferry',
    [string]$Gradle = 'gradle',
    # CI passes a build number so every published APK can update the one before it.
    [int]$VersionCode = 0,
    [string]$VersionName = ''
)

$ErrorActionPreference = 'Stop'
$mobile = Split-Path -Parent $PSScriptRoot
$sdk = if ($env:ANDROID_HOME) { $env:ANDROID_HOME } else { $env:ANDROID_SDK_ROOT }
if (-not $sdk) { throw 'Set ANDROID_HOME to the Android SDK.' }

$tools = Join-Path $sdk 'build-tools/36.0.0'
$signer = Join-Path $tools 'apksigner.bat'
$aapt = Join-Path $tools 'aapt2.exe'
$align = Join-Path $tools 'zipalign.exe'
$inputs = Join-Path $mobile 'target/google-play'
$googleProject = Join-Path $mobile 'google-play'

$build = & $CargoFerry --json build android --release --project-dir $mobile | ConvertFrom-Json
if ($LASTEXITCODE -ne 0 -or $build.status -ne 'ok') { throw ('RustFerry build failed. ' + $build.error.message) }

$plan = & $CargoFerry --json --dry-run build android --release --project-dir $mobile | ConvertFrom-Json
if ($LASTEXITCODE -ne 0 -or $plan.status -ne 'ok') { throw 'Could not read the RustFerry build plan.' }
$manifest = ($plan.data.plan | Where-Object stage -EQ 'generate Android manifest and resources').outputs[0]
$generated = Split-Path -Parent ($manifest -replace '^\\\\\?\\', '')
$base = $build.data.artifact -replace '^\\\\\?\\', ''

# AGP supplies these values from its namespace, release build type, and native packaging settings.
New-Item -ItemType Directory -Force -Path $inputs | Out-Null
$androidNamespace = 'http://schemas.android.com/apk/res/android'
$gradleManifest = [System.Xml.XmlDocument]::new()
$gradleManifest.Load((Join-Path $generated 'AndroidManifest.xml'))
$gradleManifest.DocumentElement.RemoveAttribute('package')
$app = $gradleManifest.SelectSingleNode('/manifest/application')
$app.RemoveAttribute('debuggable', $androidNamespace)
$app.RemoveAttribute('extractNativeLibs', $androidNamespace)
# In-app updates hand the downloaded APK to PackageInstaller.
$installPermission = 'android.permission.REQUEST_INSTALL_PACKAGES'
if (-not $gradleManifest.SelectSingleNode("/manifest/uses-permission[@*[local-name()='name']='$installPermission']")) {
    $permission = $gradleManifest.CreateElement('uses-permission')
    $permission.SetAttribute('name', $androidNamespace, $installPermission) | Out-Null
    $gradleManifest.DocumentElement.InsertBefore($permission, $app) | Out-Null
}
$gradleManifest.Save((Join-Path $inputs 'AndroidManifest.xml'))

$metadata = cargo metadata --manifest-path (Join-Path $mobile 'Cargo.toml') --locked --filter-platform aarch64-linux-android --format-version 1 | ConvertFrom-Json
if ($LASTEXITCODE -ne 0) { throw 'Could not resolve the locked Slint Android helper.' }
$androidSlint = $metadata.packages | Where-Object { $_.name -eq 'i-slint-backend-android-activity' -and $_.version -eq '1.17.1' }
$slintJava = Join-Path (Split-Path -Parent $androidSlint.manifest_path) 'java'

$nativeLibraries = @(
    @{ abi = 'arm64-v8a'; target = 'aarch64-linux-android' },
    @{ abi = 'x86_64'; target = 'x86_64-linux-android' }
)
foreach ($library in $nativeLibraries) {
    $destination = Join-Path $inputs "jniLibs/$($library.abi)"
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    $native = Join-Path $mobile "target/$($library.target)/release/libstream_tube_android.so"
    Copy-Item -LiteralPath $native -Destination $destination
}

$version = & $aapt dump badging $base | Select-String "^package:"
if ($LASTEXITCODE -ne 0 -or "$version" -notmatch "versionCode='(\d+)' versionName='([^']+)'") {
    throw 'Could not read the Android version from the RustFerry APK.'
}
$versionCode = if ($VersionCode -gt 0) { $VersionCode } else { $Matches[1] }
$versionName = if ($VersionName) { $VersionName } else { $Matches[2] }
$gradleArguments = @(
    '--no-daemon', '--project-dir', $googleProject, "--project-cache-dir=$inputs/gradle-cache",
    "-PferryInputs=$inputs", "-PferryGenerated=$generated", "-PslintJava=$slintJava",
    "-PstreamTubeVersionCode=$versionCode", "-PstreamTubeVersion=$versionName", 'assembleRelease'
)
& $Gradle @gradleArguments
if ($LASTEXITCODE -ne 0) { throw 'Google Play services packaging failed.' }

$unsigned = Join-Path $googleProject 'build/outputs/apk/release/StreamTube-release-unsigned.apk'
if (-not (Test-Path -LiteralPath $unsigned)) { throw 'Missing unsigned APK.' }
$aligned = Join-Path $inputs 'stream-tube-aligned.apk'
$release = Join-Path $inputs "Stream-Tube-Android-$versionName.apk"
& $align -P 16 -f 4 $unsigned $aligned
if ($LASTEXITCODE -ne 0) { throw 'APK alignment failed.' }

if ($env:STREAMTUBE_KEYSTORE) {
    # Release identity: every update must be signed with the same key to install over the last one.
    & $signer sign --ks $env:STREAMTUBE_KEYSTORE --ks-pass env:STREAMTUBE_KEYSTORE_PASSWORD `
        --ks-key-alias $env:STREAMTUBE_KEY_ALIAS --key-pass env:STREAMTUBE_KEYSTORE_PASSWORD --out $release $aligned
} else {
    $identity = Join-Path $env:APPDATA 'cargo-ferry/android'
    $keystore = Join-Path $identity 'debug.keystore'
    $passwordFile = Join-Path $identity 'debug-keystore.pass'
    & $signer sign --ks $keystore --ks-pass "file:$passwordFile" --ks-key-alias rustferrydebugkey --out $release $aligned
}
if ($LASTEXITCODE -ne 0) { throw 'APK signing failed.' }
& $signer verify --verbose --print-certs $release
if ($LASTEXITCODE -ne 0) { throw 'APK signature verification failed.' }
& $align -c -P 16 4 $release
if ($LASTEXITCODE -ne 0) { throw 'APK alignment verification failed.' }
Get-Item -LiteralPath $release
