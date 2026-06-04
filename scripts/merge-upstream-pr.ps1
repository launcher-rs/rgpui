<#
.SYNOPSIS
  从上游仓库自动合并 PR 到 rgpui，处理 crate 路径和名称的 r- 前缀映射。

.DESCRIPTION
  读取 UPSTREAM-PRS.json 配置，支持多个上游仓库（zed、gpui-component、yororen-ui）。
  自动克隆/拉取上游，获取 PR 变更，映射路径和内容中的 crate 名称，应用后验证编译。

.PARAMETER PR
  要合并的上游 PR 编号。

.PARAMETER All
  处理所有 status 为 "pending" 的 PR。

.PARAMETER Upstream
  指定上游仓库名称（默认自动从 PR 配置读取）。

.PARAMETER UpdateList
  仅更新 PR 列表，不执行合并。

.PARAMETER DryRun
  仅分析并输出变更内容，不写入文件。

.EXAMPLE
  .\scripts\merge-upstream-pr.ps1 -PR 58291
  合并 PR #58291（自动查找其 upstream）

.EXAMPLE
  .\scripts\merge-upstream-pr.ps1 -All
  合并所有待处理的 PR

.EXAMPLE
  .\scripts\merge-upstream-pr.ps1 -PR 58291 -DryRun
  仅分析 PR #58291
#>

param(
    [Parameter(ParameterSetName = 'PR', Mandatory)]
    [int]$PR,

    [Parameter(ParameterSetName = 'All', Mandatory)]
    [switch]$All,

    [Parameter(ParameterSetName = 'UpdateList', Mandatory)]
    [switch]$UpdateList,

    [string]$Upstream,

    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'
$RepoRoot = Split-Path -Parent $PSScriptRoot
$ConfigPath = Join-Path $RepoRoot 'UPSTREAM-PRS.json'

# ---------- 颜色输出辅助 ----------
function Write-Info  { Write-Host "[INFO]  $args" -ForegroundColor Cyan }
function Write-Ok   { Write-Host "[OK]    $args" -ForegroundColor Green }
function Write-Warn { Write-Host "[WARN]  $args" -ForegroundColor Yellow }
function Write-Err  { Write-Host "[ERROR] $args" -ForegroundColor Red }
function Write-Step { Write-Host "`n==> $args" -ForegroundColor Magenta }

# ---------- 读取配置 ----------
if (-not (Test-Path $ConfigPath)) {
    Write-Err "找不到配置文件: $ConfigPath"
    exit 1
}
$Config = Get-Content $ConfigPath -Raw | ConvertFrom-Json

# ---------- 解析上游配置 ----------
function Get-UpstreamConfig {
    param([string]$name)

    # 新版：从 upstreams 字典中查找
    $upstreams = $Config.upstreams
    if ($upstreams -and $upstreams.$name) {
        return $upstreams.$name
    }

    # 旧版兼容：顶层 upstream 对象
    if ($Config.upstream) {
        return $Config.upstream
    }

    Write-Err "找不到上游仓库配置: $name"
    exit 1
}

function Resolve-PrUpstream {
    param([int]$prNumber)

    if ($Upstream) { return $Upstream }

    $configObj = Get-Content $ConfigPath -Raw | ConvertFrom-Json
    $prEntry = $configObj.prs | Where-Object { $_.number -eq $prNumber }
    if ($prEntry -and $prEntry.upstream) {
        return $prEntry.upstream
    }
    return 'zed'  # 默认
}

# ---------- 构建映射表 ----------
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

    # 兜底 gpui → rgpui
    if (-not $contentMappings.ContainsKey('gpui')) {
        $contentMappings['gpui'] = 'rgpui'
    }

    return @{ PathMappings = $pathMappings; ContentMappings = $contentMappings }
}

# ---------- 路径映射函数 ----------
function Map-UpstreamPath {
    param([string]$upstreamPath, $pathMappings)
    $path = $upstreamPath.Replace('\', '/')
    foreach ($from in $pathMappings.Keys) {
        $fromNorm = $from.Replace('\', '/')
        if ($path -like ($fromNorm + '*') -or $path -eq $fromNorm.TrimEnd('/')) {
            return $pathMappings[$from] + $path.Substring($fromNorm.Length)
        }
    }
    return $null
}

# ---------- 内容替换函数 ----------
function Map-Content {
    param([string]$content, $contentMappings)
    $sortedKeys = $contentMappings.Keys | Sort-Object -Descending
    foreach ($from in $sortedKeys) {
        $to = $contentMappings[$from]
        $content = [regex]::Replace($content, "(?<=^|[^a-zA-Z_])$from(?=[^a-zA-Z_]|$)", { param($m) $to })
    }
    return $content
}

# ---------- 获取上游仓库 ----------
function Ensure-UpstreamClone {
    param($upstreamConfig)
    $worktree = $upstreamConfig.worktree
    $url = $upstreamConfig.url
    $branch = $upstreamConfig.branch
    $absWorktree = Join-Path $RepoRoot $worktree

    if (Test-Path (Join-Path $absWorktree '.git')) {
        Write-Step "更新上游仓库: $worktree"
        Push-Location $absWorktree
        try {
            git checkout $branch 2>&1 | Out-Null
            git pull --ff-only 2>&1 | Out-Null
            Write-Ok "上游仓库已更新到最新"
        } finally {
            Pop-Location
        }
    } else {
        Write-Step "克隆上游仓库到: $worktree"
        $parent = Split-Path $absWorktree -Parent
        if (-not (Test-Path $parent)) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }
        git clone $url $absWorktree 2>&1 | Out-Null
        Write-Ok "上游仓库克隆完成"
    }
    return $absWorktree
}

# ---------- 获取 PR 变更文件列表 ----------
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

        Write-Info "获取 PR #$prNumber 的提交..."
        $fetchOutput = git fetch origin "pull/$prNumber/head:$branchName" 2>&1
        if ($LASTEXITCODE -ne 0) {
            Write-Err "获取 PR #$prNumber 失败: $fetchOutput"
            return $null
        }

        $baseSha = git rev-parse $upstreamConfig.branch 2>&1
        $headSha = git rev-parse $branchName 2>&1

        Write-Info "  Base: $($upstreamConfig.branch) @ $($baseSha.Substring(0,8))"
        Write-Info "  Head: $branchName @ $($headSha.Substring(0,8))"

        $mergeBase = git merge-base $baseSha $headSha 2>&1

        $changedFiles = git diff $mergeBase..$headSha --name-status --diff-filter=ACMR 2>&1
        Write-Info "变更文件:"
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
                        $content = git show "$headSha`:$filePath" 2>&1
                        if ($LASTEXITCODE -ne 0) {
                            Write-Warn "  无法读取 $filePath，可能是二进制文件，跳过"
                            return
                        }
                    }
                    $files += [PSCustomObject]@{
                        Status       = $status
                        UpstreamPath = $filePath
                        RgpuiPath    = $rgpuiPath
                        Content      = $content
                    }
                } else {
                    Write-Warn "  跳过（不在映射中）: $filePath"
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

# ---------- 应用变更到 rgpui 仓库 ----------
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
                    Write-Ok "  创建: $($file.RgpuiPath)"
                    $createdCount++
                }
                'M' {
                    if (-not (Test-Path $parentDir)) {
                        New-Item -ItemType Directory -Path $parentDir -Force | Out-Null
                    }
                    $mappedContent = Map-Content -content $file.Content -contentMappings $contentMappings
                    Set-Content -Path $absPath -Value $mappedContent.TrimEnd() -NoNewline
                    Write-Ok "  更新: $($file.RgpuiPath)"
                    $modifiedCount++
                }
                'C' {
                    if (-not (Test-Path $parentDir)) {
                        New-Item -ItemType Directory -Path $parentDir -Force | Out-Null
                    }
                    $mappedContent = Map-Content -content $file.Content -contentMappings $contentMappings
                    Set-Content -Path $absPath -Value $mappedContent.TrimEnd() -NoNewline
                    Write-Ok "  复制创建: $($file.RgpuiPath)"
                    $createdCount++
                }
                'R' {
                    Write-Warn "  跳过重命名: $($file.RgpuiPath)"
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

# ---------- 分析并打印变更摘要 ----------
function Show-ChangeSummary {
    param($prChanges)

    Write-Step "变更摘要 - PR #$($prChanges.PR)"
    Write-Info "  Base SHA: $($prChanges.BaseSha)"
    Write-Info "  Head SHA: $($prChanges.HeadSha)"
    Write-Info "  文件列表:"
    foreach ($file in $prChanges.Files) {
        $statusStr = switch ($file.Status) {
            'A' { '新增' }
            'M' { '修改' }
            'C' { '复制' }
            'R' { '重命名' }
            default { $file.Status }
        }
        Write-Info "    [$statusStr] $($file.UpstreamPath) → $($file.RgpuiPath)"
    }
    Write-Info "  总计 $($prChanges.Files.Count) 个文件"
}

# ---------- 更新配置文件 PR 状态 ----------
function Update-PrStatus {
    param([int]$prNumber, [string]$status, [string]$title, [string]$upstreamName)

    $configObj = Get-Content $ConfigPath -Raw | ConvertFrom-Json
    $existing = $configObj.prs | Where-Object { $_.number -eq $prNumber }

    if ($existing) {
        $existing.status = $status
        if ($title) { $existing.title = $title }
        if ($status -eq 'merged') { $existing.merged_at = (Get-Date -Format 'yyyy-MM-dd') }
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

    $configObj | ConvertTo-Json -Depth 10 | Set-Content $ConfigPath -Encoding UTF8
    Write-Ok "已更新 UPSTREAM-PRS.json: PR #$prNumber → $status"
}

# ---------- 获取 PR 标题 ----------
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

# =====================================================================
# 主流程
# =====================================================================

if ($UpdateList) {
    $upstreamName = if ($Upstream) { $Upstream } else { 'zed' }
    $upstreamConfig = Get-UpstreamConfig $upstreamName
    Write-Step "更新 PR 列表（上游: $upstreamName）"
    $upstreamDir = Ensure-UpstreamClone $upstreamConfig
    Push-Location $upstreamDir
    try {
        $recentCommits = git log $upstreamConfig.branch --oneline -50 -- 'crates/gpui/' 'crates/gpui_*/' 2>&1
        Write-Info "最近涉及 gpui 的提交:"
        $recentCommits | ForEach-Object { Write-Info "  $_" }
    } finally {
        Pop-Location
    }
    exit 0
}

# 收集要处理的 PR 列表
$prList = @()
$configObj = Get-Content $ConfigPath -Raw | ConvertFrom-Json

if ($PR) {
    $prEntry = $configObj.prs | Where-Object { $_.number -eq $PR }
    $prList = @(@{ Number = $PR; UpstreamName = if ($prEntry -and $prEntry.upstream) { $prEntry.upstream } else { $Upstream ?? 'zed' } })
} elseif ($All) {
    $prList = $configObj.prs | Where-Object { $_.status -eq 'pending' } | ForEach-Object {
        @{ Number = $_.number; UpstreamName = if ($_.upstream) { $_.upstream } else { 'zed' } }
    }
    if ($prList.Count -eq 0) {
        Write-Info "没有待处理的 PR"
        exit 0
    }
    Write-Info "待处理 PR: $($prList | ForEach-Object { "$($_.Number)($($_.UpstreamName))" } -join ', ')"
} else {
    Write-Err "请指定 -PR <编号> 或 -All"
    exit 1
}

# 处理每个 PR
$allResults = @()
foreach ($prItem in $prList) {
    $prNum = $prItem.Number
    $upstreamName = if ($Upstream) { $Upstream } else { $prItem.UpstreamName }
    $upstreamConfig = Get-UpstreamConfig $upstreamName
    $mappings = Build-Mappings $upstreamConfig

    Write-Step "处理 PR #$prNum（上游: $upstreamName）"

    # 确保上游已克隆
    $upstreamDir = Ensure-UpstreamClone $upstreamConfig

    $prChanges = Get-PrChanges -upstreamDir $upstreamDir -prNumber $prNum -upstreamConfig $upstreamConfig -pathMappings $mappings.PathMappings
    if (-not $prChanges) {
        Write-Err "跳过 PR #$prNum"
        continue
    }

    $title = Get-PrTitle -upstreamDir $upstreamDir -prNumber $prNum

    Show-ChangeSummary $prChanges

    if ($DryRun) {
        Write-Warn "Dry-Run 模式，不写入文件"
        Update-PrStatus -prNumber $prNum -status 'analyzed' -title $title -upstreamName $upstreamName
        $allResults += $prChanges
        continue
    }

    # 应用变更
    $stats = Apply-Changes -prChanges $prChanges -contentMappings $mappings.ContentMappings
    Write-Ok "PR #$prNum 合并完成: $($stats.Modified) 个修改, $($stats.Created) 个新建"

    # 更新状态
    Update-PrStatus -prNumber $prNum -status 'merged' -title $title -upstreamName $upstreamName

    # 运行 cargo check
    Write-Step "验证编译: cargo check -p rgpui"
    Push-Location $RepoRoot
    try {
        $checkOutput = cargo check -p rgpui 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-Ok "cargo check 通过"
        } else {
            Write-Err "cargo check 失败，请手动检查"
            Write-Host $checkOutput -ForegroundColor Red
            Update-PrStatus -prNumber $prNum -status 'check-failed' -title $title -upstreamName $upstreamName
        }
    } finally {
        Pop-Location
    }

    $allResults += $prChanges
}

Write-Step "全部完成"
Write-Info "处理了 $($allResults.Count) 个 PR"
if (-not $DryRun) {
    Write-Info "请执行 cargo check --workspace --examples 全面验证"
    Write-Info "然后执行 cargo fmt 格式化代码"
}
