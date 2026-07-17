param(
    [switch]$Release
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$WorkspaceRoot = Resolve-Path "$ProjectRoot/../../.."

Write-Host "Building rgpui Component Story Web..." -ForegroundColor Green

# Step 1: Build WASM
Write-Host "Step 1: Building WASM..." -ForegroundColor Green
if ($Release) {
    cargo build --target wasm32-unknown-unknown --release
    $BuildMode = "release"
} else {
    cargo build --target wasm32-unknown-unknown
    $BuildMode = "debug"
}

# Step 2: Generate JavaScript bindings
$WasmPath = "$WorkspaceRoot/target/wasm32-unknown-unknown/$BuildMode/rgpui_component_story_web.wasm"
Write-Host "WASM_PATH: $WasmPath" -ForegroundColor Yellow

if (-not (Test-Path $WasmPath)) {
    Write-Host "Error: WASM file not found at: $WasmPath" -ForegroundColor Red
    exit 1
}

Write-Host "Step 2: Generating JavaScript bindings..." -ForegroundColor Green
wasm-bindgen $WasmPath `
    --out-dir "$ProjectRoot/www/src/wasm" `
    --target web `
    --no-typescript

Write-Host "✓ Build completed successfully!" -ForegroundColor Green

# Step 3: Start dev server
Write-Host "Starting dev server..." -ForegroundColor Yellow
Set-Location "$ProjectRoot/www"
bun install
bun run dev
