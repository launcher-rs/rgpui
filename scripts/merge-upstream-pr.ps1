<#
.SYNOPSIS
  Auto-merge upstream PRs into rgpui with crate path/name r-prefix mapping.

.DESCRIPTION
  Reads UPSTREAM-PRS.json config, supports multiple upstreams (zed, gpui-component, yororen-ui).
  Automatically clones/pulls upstream, fetches PR changes, maps paths/content, applies, verifies.

.PARAMETER PR
  Upstream PR number to merge.

.PARAMETER All
  Process all PRs with status "pending".

.PARAMETER Scan
  Scan upstream recent commits for unmerged PRs.

.PARAMETER Upstream
  Upstream repo name (default: auto-detect from PR config).

.PARAMETER DryRun
  Analyze and print changes only, no file writes.

.EXAMPLE
  .\scripts\merge-upstream-pr.ps1 -PR 58291

.EXAMPLE
  .\scripts\merge-upstream-pr.ps1 -Scan

.EXAMPLE
  .\scripts\merge-upstream-pr.ps1 -Scan -Upstream gpui-component

.EXAMPLE
  .\scripts\merge-upstream-pr.ps1 -All

.EXAMPLE
  .\scripts\merge-upstream-pr.ps1 -PR 58291 -DryRun
#>

param(
    [Parameter(ParameterSetName = 'PR', Mandatory)]
    [int]$PR,

    [Parameter(ParameterSetName = 'All', Mandatory)]
    [switch]$All,

    [Parameter(ParameterSetName = 'Scan', Mandatory)]
    [switch]$Scan,

    [Parameter(ParameterSetName = 'UpdateList', Mandatory)]
    [switch]$UpdateList,

    [string]$Upstream,

    [switch]$DryRun
)

$ErrorActionPreference = 'Continue'
$RepoRoot = Split-Path -Parent $PSScriptRoot
$PrConfigPath = Join-Path $RepoRoot 'UPSTREAM-PRS.json'
$RulesPath = Join-Path $RepoRoot '.opencode\upstream-rules.json'

# ---------- colored output helpers ----------
function Write-Info  { Write-Host "[INFO]  $args" -ForegroundColor Cyan }
function Write-Ok   { Write-Host "[OK]    $args" -ForegroundColor Green }
function Write-Warn { Write-Host "[WARN]  $args" -ForegroundColor Yellow }
function Write-Err  { Write-Host "[ERROR] $args" -ForegroundColor Red }
function Write-Step { Write-Host "`n==> $args" -ForegroundColor Magenta }

# ---------- read config ----------
if (-not (Test-Path $PrConfigPath)) {
    Write-Err "PR status file not found: $PrConfigPath"
    exit 1
}
$PrConfig = Get-Content $PrConfigPath -Raw | ConvertFrom-Json

if (-not (Test-Path $RulesPath)) {
    Write-Err "Upstream rules file not found: $RulesPath"
    exit 1
}
$Rules = Get-Content $RulesPath -Raw | ConvertFrom-Json

# ---------- resolve upstream config ----------
function Get-UpstreamConfig {
    param([string]$name)
    if ($Rules.$name) {
        return $Rules.$name
    }
    Write-Err "Upstream config not found: $name"
    exit 1
}

function Resolve-PrUpstream {
    param([int]$prNumber)
    if ($Upstream) { return $Upstream }
    $prEntry = $PrConfig.prs | Where-Object { $_.number -eq $prNumber }
    if ($prEntry -and $prEntry.upstream) {
        return $prEntry.upstream
    }
    return 'zed'
}

# ---------- build mapping tables ----------
function Build-Mappings {
    param($upstreamConfig)

    $pathMappings = [ordered]@{}
    $contentMappings = [ordered]@{}

    foreach ($m in $upstreamConfig.mappings) {
        $pathMappings[$m.from] = $m.to

        $fromTrim = $m.from.TrimEnd('/')
        $toTrim = $m.to.TrimEnd('/')

        # 完整路径映射（用于 Cargo.toml 中的路径引用）
        $contentMappings[$fromTrim] = $toTrim

        # crate 短名称：从 crates/gpui_windows/ 提取 gpui_windows
        $crateFrom = $fromTrim -replace '^crates/', ''
        $crateTo = $toTrim -replace '^crates/', ''
        if ($crateFrom -ne $crateTo) {
            $contentMappings[$crateFrom] = $crateTo
        }
    }

    # 叠加显式 content_mappings
    if ($upstreamConfig.content_mappings) {
        foreach ($kv in $upstreamConfig.content_mappings.PSObject.Properties) {
            $contentMappings[$kv.Name] = $kv.Value
        }
    }

    # 兜底 gpui -> rgpui
    if (-not $contentMappings.Contains('gpui')) {
        $contentMappings['gpui'] = 'rgpui'
    }

    return @{ PathMappings = $pathMappings; ContentMappings = $contentMappings }
}

# ---------- path mapping function ----------
function Map-UpstreamPath {
    param([string]$upstreamPath, $pathMappings)
    $path = $upstreamPath.Replace('\', '/')
    foreach ($from in $pathMappings.Keys) {
        $fromNorm = $from.Replace('\', '/')
        if ($path -like ($fromNorm + '*') -or $path -eq $fromNorm.TrimEnd('/')) {
            $suffix = if ($fromNorm.Length -le $path.Length) { $path.Substring($fromNorm.Length) } else { '' }
            return $pathMappings[$from] + $suffix
        }
    }
    return $null
}

# ---------- content replacement function ----------
function Map-Content {
    param([string]$content, $contentMappings)
    $sortedKeys = $contentMappings.Keys | Sort-Object -Descending
    foreach ($from in $sortedKeys) {
        $to = $contentMappings[$from]
        $content = [regex]::Replace($content, "(?<=^|[^a-zA-Z_])$from(?=[^a-zA-Z_]|$)", { param($m) $to })
    }
    return $content
}

# ---------- ensure upstream clone ----------
function Ensure-UpstreamClone {
    param($upstreamConfig)
    $worktree = $upstreamConfig.worktree
    $url = $upstreamConfig.url
    $branch = $upstreamConfig.branch
    $absWorktree = Join-Path $RepoRoot $worktree

    if (Test-Path (Join-Path $absWorktree '.git')) {
        Write-Step "Updating upstream repo: $worktree"
        Push-Location $absWorktree
        try {
            git checkout $branch 2>&1 | Out-Null
            git pull --ff-only 2>&1 | Out-Null
            Write-Ok "Upstream repo is up to date"
        } finally {
            Pop-Location
        }
    } else {
        Write-Step "Cloning upstream repo to: $worktree"
        $parent = Split-Path $absWorktree -Parent
        if (-not (Test-Path $parent)) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }
        git clone $url $absWorktree 2>&1 | Out-Null
        Write-Ok "Upstream repo cloned"
    }
    return $absWorktree
}

# ---------- get PR changes ----------
function Get-PrChanges {
    param(
        [string]$upstreamDir,
        [int]$prNumber,
        $upstreamConfig,
        $pathMappings
    )

    Push-Location $upstreamDir
    try {
        $branchName = "pr-$prNumber"

        Write-Info "Fetching PR #$prNumber ..."
        $fetchOutput = git fetch origin "pull/$prNumber/head:$branchName" 2>&1
        if ($LASTEXITCODE -ne 0) {
            Write-Err "Failed to fetch PR #${prNumber}: $fetchOutput"
            return $null
        }

        $baseSha = (git rev-parse $upstreamConfig.branch 2>&1).Trim()
        $headSha = (git rev-parse $branchName 2>&1).Trim()
        $baseShort = if ($baseSha.Length -ge 8) { $baseSha.Substring(0, 8) } else { $baseSha }
        $headShort = if ($headSha.Length -ge 8) { $headSha.Substring(0, 8) } else { $headSha }

        Write-Info "  Base: $($upstreamConfig.branch) @ $baseShort"
        Write-Info "  Head: $branchName @ $headShort"

        $mergeBase = (git merge-base $baseSha $headSha 2>&1).Trim()

        $changedFiles = git diff --name-status --diff-filter=ACMR $mergeBase $headSha 2>&1
        Write-Info "Changed files:"
        $changedFiles | ForEach-Object { Write-Info "  $_" }

        $files = @()
        $changedFiles | ForEach-Object {
            $line = $_
            if ($line -match '^([ACMR])\s+(.+)$') {
                $status = $matches[1]
                $filePath = $matches[2]
                $rgpuiPath = Map-UpstreamPath -upstreamPath $filePath -pathMappings $pathMappings
                if ($rgpuiPath) {
                    if ($DryRun) {
                        $content = ""
                    } else {
                        $contentLines = git show "$headSha`:$filePath" 2>&1
                        if ($LASTEXITCODE -ne 0) {
                            Write-Warn "  Cannot read $filePath, may be binary, skipping"
                            return
                        }
                        $content = ($contentLines -join "`n")
                    }
                    $files += [PSCustomObject]@{
                        Status       = $status
                        UpstreamPath = $filePath
                        RgpuiPath    = $rgpuiPath
                        Content      = $content
                    }
                } else {
                    Write-Warn "  Skipping (not in mapping): $filePath"
                }
            }
        }

        return [PSCustomObject]@{
            PR       = $prNumber
            BaseSha  = $baseSha
            HeadSha  = $headSha
            Files    = $files
        }
    } finally {
        Pop-Location
    }
}

# ---------- apply changes to rgpui ----------
function Apply-Changes {
    param($prChanges, $contentMappings)

    $modifiedCount = 0
    $createdCount = 0

    Push-Location $RepoRoot
    try {
        foreach ($file in $prChanges.Files) {
            $absPath = Join-Path $RepoRoot $file.RgpuiPath
            $parentDir = Split-Path $absPath -Parent

            switch ($file.Status) {
                'A' {
                    if (-not (Test-Path $parentDir)) {
                        New-Item -ItemType Directory -Path $parentDir -Force | Out-Null
                    }
                    $mappedContent = Map-Content -content $file.Content -contentMappings $contentMappings
                    Set-Content -Path $absPath -Value $mappedContent.TrimEnd() -NoNewline
                    Write-Ok "  Created: $($file.RgpuiPath)"
                    $createdCount++
                }
                'M' {
                    if (-not (Test-Path $parentDir)) {
                        New-Item -ItemType Directory -Path $parentDir -Force | Out-Null
                    }
                    $mappedContent = Map-Content -content $file.Content -contentMappings $contentMappings
                    Set-Content -Path $absPath -Value $mappedContent.TrimEnd() -NoNewline
                    Write-Ok "  Updated: $($file.RgpuiPath)"
                    $modifiedCount++
                }
                'C' {
                    if (-not (Test-Path $parentDir)) {
                        New-Item -ItemType Directory -Path $parentDir -Force | Out-Null
                    }
                    $mappedContent = Map-Content -content $file.Content -contentMappings $contentMappings
                    Set-Content -Path $absPath -Value $mappedContent.TrimEnd() -NoNewline
                    Write-Ok "  Copied: $($file.RgpuiPath)"
                    $createdCount++
                }
                'R' {
                    Write-Warn "  Skipping rename: $($file.RgpuiPath)"
                }
            }
        }
    } finally {
        Pop-Location
    }

    return [PSCustomObject]@{
        Modified = $modifiedCount
        Created  = $createdCount
    }
}

# ---------- show change summary ----------
function Show-ChangeSummary {
    param($prChanges)

    Write-Step "Change summary - PR #$($prChanges.PR)"
    Write-Info "  Base SHA: $($prChanges.BaseSha)"
    Write-Info "  Head SHA: $($prChanges.HeadSha)"
    Write-Info "  Files:"
    foreach ($file in $prChanges.Files) {
        $statusStr = switch ($file.Status) {
            'A' { 'NEW' }
            'M' { 'MOD' }
            'C' { 'CPY' }
            'R' { 'MOV' }
            default { $file.Status }
        }
        Write-Info "    [$statusStr] $($file.UpstreamPath) -> $($file.RgpuiPath)"
    }
    Write-Info "  Total $($prChanges.Files.Count) files"
}

# ---------- update PR status in config ----------
function Update-PrStatus {
    param([int]$prNumber, [string]$status, [string]$title, [string]$upstreamName)

    $configObj = Get-Content $PrConfigPath -Raw | ConvertFrom-Json
    $existing = $configObj.prs | Where-Object { $_.number -eq $prNumber }

    if ($existing) {
        $existing.status = $status
        if ($title) { $existing.title = $title }
        if ($status -eq 'merged') {
            if ($existing.PSObject.Properties.Name -contains 'merged_at') {
                $existing.merged_at = (Get-Date -Format 'yyyy-MM-dd')
            } else {
                $existing | Add-Member -NotePropertyName 'merged_at' -NotePropertyValue (Get-Date -Format 'yyyy-MM-dd')
            }
        }
    } else {
        $newPr = [PSCustomObject]@{
            number    = $prNumber
            upstream  = $upstreamName
            title     = $title
            status    = $status
            merged_at = if ($status -eq 'merged') { (Get-Date -Format 'yyyy-MM-dd') } else { $null }
        }
        $configObj.prs += $newPr
    }

    $configObj | ConvertTo-Json -Depth 10 | Set-Content $PrConfigPath -Encoding UTF8
    Write-Ok "Updated UPSTREAM-PRS.json: PR #$prNumber -> $status"
}

# ---------- record merged PR in docs/merged-prs.md ----------
$MergedPrsDoc = Join-Path $RepoRoot 'docs\merged-prs.md'

function Record-MergedPr {
    param([int]$prNumber, [string]$title, [string]$upstreamName)

    if (-not (Test-Path $MergedPrsDoc)) { return }

    $today = Get-Date -Format 'yyyy-MM-dd'
    $line = "| #$prNumber | $today | $title |"
    $content = Get-Content $MergedPrsDoc -Encoding UTF8

    # 找到对应上游的表格，在表头后插入一行
    $header = "## $upstreamName"
    $insertAt = -1
    for ($i = 0; $i -lt $content.Count; $i++) {
        if ($content[$i] -eq $header) {
            # 跳过分隔行和表头行
            $j = $i + 1
            while ($j -lt $content.Count -and $content[$j] -notmatch '^\|') { $j++ }
            while ($j -lt $content.Count -and $content[$j] -match '^\|') { $j++ }
            $insertAt = $j
            break
        }
    }

    # 检查是否已存在
    $alreadyExists = $false
    foreach ($l in $content) {
        if ($l -match "^\| #$prNumber ") { $alreadyExists = $true; break }
    }

    if (-not $alreadyExists -and $insertAt -ge 0) {
        $content = $content[0..($insertAt - 1)] + @($line) + $content[$insertAt..($content.Count - 1)]
        $content | Set-Content $MergedPrsDoc -Encoding UTF8
        Write-Ok "Recorded in docs/merged-prs.md"
    }
}

# ---------- load merged PR numbers from docs ----------
function Get-MergedPrNumbers {
    if (-not (Test-Path $MergedPrsDoc)) { return @{} }

    $merged = @{}
    $content = Get-Content $MergedPrsDoc -Encoding UTF8
    foreach ($line in $content) {
        if ($line -match '^\| #(\d+) ') {
            $merged[[int]$matches[1]] = $true
        }
    }
    return $merged
}

# ---------- get PR title ----------
function Get-PrTitle {
    param([string]$upstreamDir, [int]$prNumber)

    Push-Location $upstreamDir
    try {
        $title = git log --format="%s" -1 "pr-$prNumber" 2>&1
        if ($LASTEXITCODE -eq 0) { return $title.Trim() }
    } finally {
        Pop-Location
    }
    return $null
}

# ---------- scan upstream for new PRs ----------
function Scan-NewPrs {
    param($upstreamConfig, $upstreamName)

    $upstreamDir = Ensure-UpstreamClone $upstreamConfig

    # 从映射中提取上游目录路径
    $mappedPaths = @()
    foreach ($m in $upstreamConfig.mappings) {
        $path = $m.from.TrimEnd('/')
        if ($path -ne '') { $mappedPaths += $path }
    }
    if ($mappedPaths.Count -eq 0) { $mappedPaths += '.' }

    Write-Step "Scanning upstream commits (last 200)..."
    Push-Location $upstreamDir
    try {
        $commitLog = git log $upstreamConfig.branch --oneline -200 2>&1
        Write-Info "Found $($commitLog.Count) commits"

        $foundPrs = [ordered]@{}
        foreach ($line in $commitLog) {
            $parts = $line -split '\s+', 2
            $shortSha = $parts[0]
            $msg = if ($parts.Count -gt 1) { $parts[1] } else { '' }

            # 提取 PR 编号
            $prNum = $null
            if ($msg -match '\(#(\d+)\)') {
                $prNum = [int]$matches[1]
            } elseif ($msg -match 'Merge pull request #(\d+)') {
                $prNum = [int]$matches[1]
            }
            if (-not $prNum) { continue }

            # 检查提交是否触及映射路径
            $touchedFiles = git diff-tree --no-commit-id -r --name-only $shortSha 2>&1
            $touchesMapped = $false
            foreach ($file in $touchedFiles) {
                if ($file -isnot [string]) { continue }
                $fileNorm = $file.Replace('\', '/')
                foreach ($mp in $mappedPaths) {
                    if ($fileNorm -like "$mp*") { $touchesMapped = $true; break }
                }
                if ($touchesMapped) { break }
            }
            if (-not $touchesMapped) { continue }

            # 提取标题
            $title = $msg
            if ($msg -match '^(.+?)\s+\(#\d+\)$') {
                $title = $matches[1].Trim()
            } elseif ($msg -match 'Merge pull request #\d+ from .+?$') {
                $title = $msg
            }

            if (-not $foundPrs.Contains([string]$prNum)) {
                $foundPrs[[string]$prNum] = @{ Title = $title; Sha = $shortSha }
            }
        }
    } finally {
        Pop-Location
    }

    # 去重：过滤已在 UPSTREAM-PRS.json 和 docs/merged-prs.md 中的 PR
    $configObj = Get-Content $PrConfigPath -Raw | ConvertFrom-Json
    $existingNums = @{}
    foreach ($p in $configObj.prs) { $existingNums[[string]$p.number] = $true }
    $mergedDocs = Get-MergedPrNumbers
    foreach ($num in $mergedDocs.Keys) { $existingNums[[string]$num] = $true }

    $newPrs = @()
    foreach ($prNum in $foundPrs.Keys) {
        if (-not $existingNums.ContainsKey($prNum)) {
            $info = $foundPrs[$prNum]
            $newPrs += [PSCustomObject]@{
                Number = $prNum
                Title  = $info.Title
                Sha    = $info.Sha
            }
        }
    }

    return $newPrs
}

# =====================================================================
# 主流程
# =====================================================================

if ($UpdateList) {
    $upstreamName = if ($Upstream) { $Upstream } else { 'zed' }
    $upstreamConfig = Get-UpstreamConfig $upstreamName
    Write-Step "Updating PR list (upstream: $upstreamName)"
    $upstreamDir = Ensure-UpstreamClone $upstreamConfig
    Push-Location $upstreamDir
    try {
        $recentCommits = git log $upstreamConfig.branch --oneline -50 -- 'crates/gpui/' 'crates/gpui_*/' 2>&1
        Write-Info "Recent gpui-related commits:"
        $recentCommits | ForEach-Object { Write-Info "  $_" }
    } finally {
        Pop-Location
    }
    exit 0
}

if ($Scan) {
    $upstreamName = if ($Upstream) { $Upstream } else { 'zed' }
    $upstreamConfig = Get-UpstreamConfig $upstreamName

    Write-Step "Scanning upstream: $upstreamName ($($upstreamConfig.url))"
    $newPrs = Scan-NewPrs -upstreamConfig $upstreamConfig -upstreamName $upstreamName

    if ($newPrs.Count -eq 0) {
        Write-Ok "No new unmerged PRs found (last 200 commits)"
        exit 0
    }

    Write-Step "Found $($newPrs.Count) unmerged PRs"
    $newPrs | ForEach-Object { Write-Info "  #$($_.Number) - $($_.Title) [commit $($_.Sha)]" }

    Write-Host "`nTo add:" -ForegroundColor Yellow
    Write-Host "  1. Copy the PR numbers to UPSTREAM-PRS.json prs array with status 'pending'" -ForegroundColor Yellow
    Write-Host "  2. Run .\scripts\merge-upstream-pr.ps1 -All" -ForegroundColor Yellow
    Write-Host "  3. Or run .\scripts\merge-upstream-pr.ps1 -PR <number>" -ForegroundColor Yellow
    exit 0
}

# 收集要处理的 PR 列表
$prList = @()
$configObj = Get-Content $PrConfigPath -Raw | ConvertFrom-Json

if ($PR) {
    $prEntry = $configObj.prs | Where-Object { $_.number -eq $PR }
    $prList = @(@{ Number = $PR; UpstreamName = if ($prEntry -and $prEntry.upstream) { $prEntry.upstream } else { $Upstream ?? 'zed' } })
} elseif ($All) {
    $prList = $configObj.prs | Where-Object { $_.status -eq 'pending' } | ForEach-Object {
        @{ Number = $_.number; UpstreamName = if ($_.upstream) { $_.upstream } else { 'zed' } }
    }
    if ($prList.Count -eq 0) {
        Write-Info "No pending PRs"
        exit 0
    }
    Write-Info "Pending PRs: $($prList | ForEach-Object { "$($_.Number)($($_.UpstreamName))" } -join ', ')"
} else {
    Write-Err "Please specify -PR <num>, -All, -Scan, or -UpdateList"
    exit 1
}

# 处理每个 PR
$allResults = @()
foreach ($prItem in $prList) {
    $prNum = $prItem.Number
    $upstreamName = if ($Upstream) { $Upstream } else { $prItem.UpstreamName }
    $upstreamConfig = Get-UpstreamConfig $upstreamName
    $mappings = Build-Mappings $upstreamConfig

    Write-Step "Processing PR #$prNum (upstream: $upstreamName)"

    # 确保上游已克隆
    $upstreamDir = Ensure-UpstreamClone $upstreamConfig

    $prChanges = Get-PrChanges -upstreamDir $upstreamDir -prNumber $prNum -upstreamConfig $upstreamConfig -pathMappings $mappings.PathMappings
    if (-not $prChanges) {
        Write-Err "Skipping PR #$prNum"
        continue
    }

    $title = Get-PrTitle -upstreamDir $upstreamDir -prNumber $prNum

    Show-ChangeSummary $prChanges

    if ($DryRun) {
        Write-Warn "Dry-Run mode, no files written"
        Update-PrStatus -prNumber $prNum -status 'analyzed' -title $title -upstreamName $upstreamName
        $allResults += $prChanges
        continue
    }

    # 应用变更
    $stats = Apply-Changes -prChanges $prChanges -contentMappings $mappings.ContentMappings
    Write-Ok "PR #$prNum merged: $($stats.Modified) modified, $($stats.Created) created"

    # 更新状态
    Update-PrStatus -prNumber $prNum -status 'merged' -title $title -upstreamName $upstreamName

    # 记录到 docs/merged-prs.md
    Record-MergedPr -prNumber $prNum -title $title -upstreamName $upstreamName

    # 运行 cargo check
    Write-Step "Verifying compilation: cargo check -p rgpui"
    Push-Location $RepoRoot
    try {
        $checkOutput = cargo check -p rgpui 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-Ok "cargo check passed"
        } else {
            Write-Err "cargo check failed, please check manually"
            Write-Host $checkOutput -ForegroundColor Red
            Update-PrStatus -prNumber $prNum -status 'check-failed' -title $title -upstreamName $upstreamName
        }
    } finally {
        Pop-Location
    }

    $allResults += $prChanges
}

Write-Step "All done"
Write-Info "Processed $($allResults.Count) PR(s)"
if (-not $DryRun) {
    Write-Info "Run 'cargo check --workspace --examples' for full verification"
    Write-Info "Then run 'cargo fmt' to format code"
}
