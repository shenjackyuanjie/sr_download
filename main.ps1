# 用于批量下载 SR1 存档，然后存储到本地的文件夹中

# 主程序

$save_path = './save'

$ship_path = './ship'

# 创建文件夹

if (!(Test-Path $save_path -PathType Container)) {
    New-Item -ItemType Directory -Path $save_path
}

if (!(Test-Path $ship_path -PathType Container)) {
    New-Item -ItemType Directory -Path $ship_path
}

# 从命令行输入获取范围

# main.ps1 -s int -e int
# -s int
# -e int
# --no-ship
# --no-save

if ($args.Count -gt 0) {
    $range = @()
    $no_ship = $false
    $no_save = $false
    $start = 200709
    $end = 300000
    for ($i = 0; $i -lt $args.Count; $i++) {
        switch ($args[$i]) {
            '-s' {
                $start = $args[$i + 1]
                $i += 1
            }
            '-e' {
                $end = $args[$i + 1]
                $i += 1
            }
            '--no-ship' {
                $no_ship = $true
            }
            '--no-save' {
                $no_save = $true
            }
        }
    }
    $range = $start..$end
}

# 多线程下载
function Main {
    param ($range, $no_ship, $no_save)

    if ($no_ship -and $no_save) {
        Write-Output "无效参数"
        return
    }

    $range | ForEach-Object -Parallel {

        function Download {
            param ($Id)
            $using:no_save
            $using:no_ship
            # 飞船 API http://jundroo.com/service/SimpleRockets/DownloadRocket?id=
            # 存档 API http://jundroo.com/service/SimpleRockets/DownloadSandBox?id=
            # (api直接在Content中返回内容，id无效则返回0)
            # 先判断 ship 和 save 中有没有已经下载的文件，有则跳过
            if (Test-Path "./ship/$Id.xml" -PathType Leaf) {
                Write-Host "ID $Id 飞船已存在,跳过"
                return
            }
            if (Test-Path "./save/$Id.xml" -PathType Leaf) {
                Write-Host "ID $Id 存档已存在,跳过"
                return
            }
            if ($no_ship) {
                $data = Invoke-WebRequest "http://jundroo.com/service/SimpleRockets/DownloadSandBox?id=$Id" -MaximumRetryCount 3
                if ($data.Content -eq "0") {
                    Write-Host "ID $Id 无效,跳过"
                }
                else {
                    $data.Content | Out-File "./save/$Id.xml" -Encoding ASCII
                    Write-Host "-----ID $Id 存档下载成功-----"
                }
                return
            }
            $data = Invoke-WebRequest "http://jundroo.com/service/SimpleRockets/DownloadRocket?id=$Id" -MaximumRetryCount 3
            if ($data.Content -eq "0") {
                Write-Host "ID $Id 非飞船,尝试存档中"
                if ($no_save) {
                    Write-Host "ID $Id 无效,跳过"
                    return
                }
                $data = Invoke-WebRequest "http://jundroo.com/service/SimpleRockets/DownloadSandBox?id=$Id"
                if ($data.Content -eq "0") {
                    Write-Host "ID $Id 无效,跳过"
                }
                else {
                    $data.Content | Out-File "./save/$Id.xml" -Encoding ASCII
                    Write-Host "-----ID $Id 存档下载成功-----"
                }
            }
            else {
                $data.Content | Out-File "./ship/$Id.xml" -Encoding ASCII
                Write-Host "=====ID $Id 飞船下载成功====="
            }
        }

        Download $_

    } -ThrottleLimit 10
}

Main $range $no_ship $no_save

Write-Host "下载完成"
