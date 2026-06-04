<#
.SYNOPSIS
  从 Zed 上游仓库自动合并 PR 到 rgpui，处理 crate 路径和名称的 r- 前缀映射。

.DESCRIPTION
  读取 UPSTREAM-PRS.json 配置，克隆/拉取上游仓库，获取指定 PR 的变更，
  自动将 gpui_* 路径和引用映射为 rgpui_*，然后应用到本地仓库。

.PARAMETER PR
  要合并的上游 PR 编号。

.PARAMETER All
  处理所有 status 为 "pending" 的 PR。

.PARAMETER UpdateList
  仅更新 PR 列表（从上游合并基础分支获取最近 PR），不执行合并。

.PARAMETER DryRun
  仅分析并输出变更内容，不写入文件。

.EXAMPLE
  .\scripts\merge-upstream-pr.ps1 -PR 58291
  合并 PR #58291

.EXAMPLE
  .\scripts\merge-upstream-pr.ps1 -All
  合并所有待处理的 PR
#>

param(
    [Parameter(ParameterSetName = 'PR', Mandatory)]
    [int]$PR,

    [Parameter(ParameterSetName = 'All', Mandatory)]
    [switch]$All,

    [Parameter(ParameterSetName = 'UpdateList', Mandatory)]
    [switch]$UpdateList,

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

# ---------- 路径替换表（从长到短，避免误替换） ----------
$PathMappings = [ordered]@{}
$ContentMappings = [ordered]@{}
foreach ($m in $Config.mappings) {
    $PathMappings[$m.from] = $m.to
    # ������·����ȡ crate ���ƣ��� crates/gpui_windows/ �� gpui_windows
    $fromTrim = $m.from.TrimEnd('/')
    $toTrim = $m.to.TrimEnd('/')
    # ·�������ԣ��� crates/gpui/ → crates/rgpui/��;
    $ContentMappings[$fromTrim] = $toTrim
    # crate ������ƣ��� gpui → rgpui��gpui_windows → rgpui_windows
    $crateFrom = $fromTrim -replace '^crates/', ''
    $crateTo = $toTrim -replace '^crates/', ''
    if ($crateFrom -ne $crateTo) {
        $ContentMappings[$crateFrom] = $crateTo
    }
}
# ȷ�� gpui → rgpui ���ڣ��� crates/gpui/ ӳ���ṩ�ˣ�
if (-not $ContentMappings.ContainsKey('gpui')) {
    $ContentMappings['gpui'] = 'rgpui'
}

# ---------- 路径映射函数 ----------
function Map-UpstreamPath {
    param([string]$upstreamPath)
    # 标准化为正斜杠
    $path = $upstreamPath.Replace('\', '/')
    foreach ($from in $PathMappings.Keys) {
        $fromNorm = $from.Replace('\', '/')
        if ($path -like ($fromNorm + '*') -or $path -eq $fromNorm.TrimEnd('/')) {
            $result = $PathMappings[$from] + $path.Substring($fromNorm.Length)
            Write-Info "  路径映射: $upstreamPath → $result"
            return $result
        }
    }
    # 不在映射中的文件，跳过
    return $null
}

# ---------- 内容替换函数 ----------
function Map-Content {
    param([string]$content)
    # 优先匹配带下划线的（如 gpui_platform → rgpui_platform）
    $sortedKeys = $ContentMappings.Keys | Sort-Object -Descending
    foreach ($from in $sortedKeys) {
        $to = $ContentMappings[$from]
        # 仅替换作为完整标识符出现的情况：边界为非字母数字下划线
        $content = [regex]::Replace($content, "(?<=^|[^a-zA-Z_])$from(?=[^a-zA-Z_]|$)", { param($m) $to })
    }
    return $content
}

# ---------- 获取上游仓库 ----------
function Ensure-UpstreamClone {
    $worktree = $Config.worktree
    $absWorktree = Join-Path $RepoRoot $worktree

    if (Test-Path (Join-Path $absWorktree '.git')) {
        Write-Step "更新上游仓库: $worktree"
        Push-Location $absWorktree
        try {
            git checkout $Config.upstream.branch 2>&1 | Out-Null
            git pull --ff-only 2>&1 | Out-Null
            Write-Ok "上游仓库已更新到最新"
        } finally {
            Pop-Location
        }
    } else {
        Write-Step "克隆上游仓库到: $worktree"
        $parent = Split-Path $absWorktree -Parent
        if (-not (Test-Path $parent)) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }
        git clone $Config.upstream.url $absWorktree 2>&1 | Out-Null
        Write-Ok "上游仓库克隆完成"
    }
    return $absWorktree
}

# ---------- 获取 PR 变更文件列表 ----------
function Get-PrChanges {
    param(
        [string]$upstreamDir,
        [int]$prNumber
    )

    Push-Location $upstreamDir
    try {
        $branchName = "pr-$prNumber"

        # 获取 PR 的远程引用
        Write-Info "获取 PR #$prNumber 的提交..."
        $fetchOutput = git fetch origin "pull/$prNumber/head:$branchName" 2>&1
        if ($LASTEXITCODE -ne 0) {
            Write-Err "获取 PR #$prNumber 失败: $fetchOutput"
            return $null
        }

        # 获取 base 分支的最新提交
        $baseSha = git rev-parse $Config.upstream.branch 2>&1
        $headSha = git rev-parse $branchName 2>&1

        Write-Info "  Base: $($Config.upstream.branch) @ $baseSha.Substring(0,8)"
        Write-Info "  Head: $branchName @ $headSha.Substring(0,8)"

        # 获取 merge-base（共同祖先）
        $mergeBase = git merge-base $baseSha $headSha 2>&1

        # 列出变更文件
        $changedFiles = git diff $mergeBase..$headSha --name-status --diff-filter=ACMR 2>&1
        Write-Info "变更文件:"
        $changedFiles | ForEach-Object { Write-Info "  $_" }

        # 收集每个文件的内容
        $files = @()
        $changedFiles | ForEach-Object {
            $line = $_
            if ($line -match '^([ACMR])\s+(.+)$') {
                $status = $matches[1]
                $filePath = $matches[2]
                $rgpuiPath = Map-UpstreamPath $filePath
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
                        Status      = $status
                        UpstreamPath = $filePath
                        RgpuiPath   = $rgpuiPath
                        Content     = $content
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
    param($prChanges)

    $modifiedCount = 0
    $createdCount = 0

    Push-Location $RepoRoot
    try {
        foreach ($file in $prChanges.Files) {
            $absPath = Join-Path $RepoRoot $file.RgpuiPath
            $parentDir = Split-Path $absPath -Parent

            switch ($file.Status) {
                'A' { # Added
                    if (-not (Test-Path $parentDir)) {
                        New-Item -ItemType Directory -Path $parentDir -Force | Out-Null
                    }
                    $mappedContent = Map-Content $file.Content
                    # 移除空行尾部
                    Set-Content -Path $absPath -Value $mappedContent.TrimEnd() -NoNewline
                    Write-Ok "  创建: $($file.RgpuiPath)"
                    $createdCount++
                }
                'M' { # Modified
                    if (-not (Test-Path $parentDir)) {
                        New-Item -ItemType Directory -Path $parentDir -Force | Out-Null
                    }
                    $mappedContent = Map-Content $file.Content
                    Set-Content -Path $absPath -Value $mappedContent.TrimEnd() -NoNewline
                    Write-Ok "  更新: $($file.RgpuiPath)"
                    $modifiedCount++
                }
                'C' { # Copied
                    if (-not (Test-Path $parentDir)) {
                        New-Item -ItemType Directory -Path $parentDir -Force | Out-Null
                    }
                    $mappedContent = Map-Content $file.Content
                    Set-Content -Path $absPath -Value $mappedContent.TrimEnd() -NoNewline
                    Write-Ok "  复制创建: $($file.RgpuiPath)"
                    $createdCount++
                }
                'R' { # Renamed
                    # 在 rgpui 中重命名文件不在本次处理范围内
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
    param(
        [int]$prNumber,
        [string]$status,
        [string]$title
    )

    $configObj = Get-Content $ConfigPath -Raw | ConvertFrom-Json
    $existing = $configObj.prs | Where-Object { $_.number -eq $prNumber }

    if ($existing) {
        $existing.status = $status
        if ($title) { $existing.title = $title }
        if ($status -eq 'merged') { $existing.merged_at = (Get-Date -Format 'yyyy-MM-dd') }
    } else {
        $newPr = [PSCustomObject]@{
            number     = $prNumber
            title      = $title
            status     = $status
            merged_at  = if ($status -eq 'merged') { (Get-Date -Format 'yyyy-MM-dd') } else { $null }
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
    Write-Step "更新 PR 列表（从上游获取最近 PR）"
    $upstreamDir = Ensure-UpstreamClone
    Push-Location $upstreamDir
    try {
        # 获取最近涉及 gpui crate 的提交
        $recentCommits = git log $Config.upstream.branch --oneline -50 -- 'crates/gpui/' 'crates/gpui_*/' 2>&1
        Write-Info "最近涉及 gpui 的提交:"
        $recentCommits | ForEach-Object { Write-Info "  $_" }
    } finally {
        Pop-Location
    }
    exit 0
}

# 收集要处理的 PR 列表
$prList = @()
if ($PR) {
    $prList = @($PR)
} elseif ($All) {
    $configObj = Get-Content $ConfigPath -Raw | ConvertFrom-Json
    $prList = $configObj.prs | Where-Object { $_.status -eq 'pending' } | ForEach-Object { $_.number }
    if ($prList.Count -eq 0) {
        Write-Info "没有待处理的 PR"
        exit 0
    }
    Write-Info "待处理 PR: $($prList -join ', ')"
} else {
    Write-Err "请指定 -PR <编号> 或 -All"
    exit 1
}

# 确保上游代码已克隆
$upstreamDir = Ensure-UpstreamClone

# 处理每个 PR
$allResults = @()
foreach ($prNum in $prList) {
    Write-Step "处理 PR #$prNum"

    $prChanges = Get-PrChanges -upstreamDir $upstreamDir -prNumber $prNum
    if (-not $prChanges) {
        Write-Err "跳过 PR #$prNum"
        continue
    }

    $title = Get-PrTitle -upstreamDir $upstreamDir -prNumber $prNum

    Show-ChangeSummary $prChanges

    if ($DryRun) {
        Write-Warn "Dry-Run 模式，不写入文件"
        Update-PrStatus -prNumber $prNum -status 'analyzed' -title $title
        $allResults += $prChanges
        continue
    }

    # 应用变更
    $stats = Apply-Changes $prChanges
    Write-Ok "PR #$prNum 合并完成: $($stats.Modified) 个修改, $($stats.Created) 个新建"

    # 更新状态
    Update-PrStatus -prNumber $prNum -status 'merged' -title $title

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
            Update-PrStatus -prNumber $prNum -status 'check-failed' -title $title
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
