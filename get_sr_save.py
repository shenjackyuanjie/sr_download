import os
import sys
import time
import requests
import traceback
import threading

# 飞船 API http://jundroo.com/service/SimpleRockets/DownloadRocket?id=
# 存档 API http://jundroo.com/service/SimpleRockets/DownloadSandBox?id=
# (api直接在 text 中返回内容，id无效则返回0)

version = "1.0.1"

# 命令行参数:
CLI = f"""
By shenjackyuanjie 3695888@qq.com
SR1 存档下载脚本 版本 {version}
飞船保存目录: ./ship
沙盒保存目录: ./save

命令行参数:
download.py get <id>    下载指定 ID 的飞船/存档
download.py list        列出所有飞船/存档
download.py help        显示帮助信息
download.py version     显示版本信息
download.py serve <id>  间隔一定时间尝试从 <id> 开始下载新的飞船/存档 (默认 60)
    参数说明:
        -t --time <time> 间隔时间 (默认 10)

download.py gets <start> <end> 下载指定范围的飞船/存档
    参数说明:
        -nsp --no-ship   不下载飞船
        -nsb --no-save   不下载存档
        -vv  --void-void 无效 ID 也下载
"""


reset_code = '\033[0m'
threaded = True


class DiskStatus:
    def __init__(self, save: list[int], ship: list[int], void: list[int] = None):
        self.save = save
        self.ship = ship
        self.void = void or []



def download(id, id_type: int) -> bool:
    # 1 ship
    # 2 sandbox
    url = f"http://jundroo.com/service/SimpleRockets/Download{'Rocket' if id_type == 1 else 'SandBox'}?id={id}"
    try:
        raw_data = requests.get(url, timeout=10)
    except:
        # just try again
        try:
            raw_data = requests.get(url, timeout=10)
        except requests.exceptions.RequestException as e:
            # 记录当前错误 ID
            traceback.print_exc()
            with open("error.csv", "a", encoding='utf-8') as f:
                f.write(f"{id},{id_type},{e}\n")
            return False
        if raw_data.status_code != 200:
            print(f"ID {id} {'飞船' if id_type == 1 else '沙盒'} 下载失败\n")
            return False
    raw_data = raw_data.text
    if raw_data == "0" or raw_data == "" or len(raw_data) < 1:
        return False
    with open(f"./{'ship' if id_type == 1 else 'save'}/{id}.xml", "w", encoding='utf-8') as f:
        f.write(raw_data)
    return True


def download_any(id) -> bool | int:
    # 首先尝试飞船
    if not download(id, 1):
        if not download(id, 2):
            # print(f"ID {id} 无效")
            with open("void.txt", "a", encoding='utf-8') as f:
                f.write(f"{id}\n")
            return False
        return 2
    return 1


def thread_download(id, len, start, end, no_ship, no_save, status: DiskStatus):
    color_code = f'\033[3{str(id % 7 + 1)}m'
    print(f"{color_code}线程 {threading.current_thread().name}({len}) 开始下载 {start} - {end} {reset_code}")
    for i in range(start, end + 1):
        # 判定是否已经下载
        if i in status.ship or i in status.save or i in status.void:
            print(f"{color_code}  ID {i} 已存在 {reset_code}")
            continue
        if no_ship:
            if not download(i, 2):
                print(f"{color_code}  ID {i} 无效-存档 {reset_code}")
        elif no_save:
            if not download(i, 1):
                print(f"{color_code}  ID {i} 无效-飞船 {reset_code}")
        else:
            get_status = download_any(i)
            if not get_status:
                print(f"{color_code}  ID {i} 无效 {reset_code}")
            elif get_status == 1:
                print(f"{color_code} {time.strftime('[%H:%M:%S]', time.gmtime())} ----------ID {i} 飞船下载完成---------- {reset_code}")
            else:
                print(f"{color_code} {time.strftime('[%H:%M:%S]', time.gmtime())} ++++++++++ID {i} 存档下载完成++++++++++ {reset_code}")
    print(f"{color_code}线程 {threading.current_thread().name} 下载完成 {start} - {end} {reset_code}")


def exit_with_help():
    print(CLI)
    sys.exit()


def check_disk_status() -> DiskStatus:
    ships = []
    if not os.path.exists("./ship"):
        os.mkdir("./ship")
    for file in os.listdir("./ship"):
        if file.endswith(".xml"):
            ships.append(int(file[:-4]))
    saves = []
    if not os.path.exists("./save"):
        os.mkdir("./save")
    for file in os.listdir("./save"):
        if file.endswith(".xml"):
            saves.append(int(file[:-4]))
    voids = []
    if os.path.exists("./void.txt"):
        with open("./void.txt", "r", encoding='utf-8') as f:
            voids = f.readlines()
        # 重新写入回去，排序, 去重
        voids = [int(i) for i in voids if i != "\n"]
        voids = list(set(voids))
        # 先检查是否已经存在
        # voids = [i for i in voids if i not in ships and i not in saves]
        voids.sort()
        with open("./void.txt", "w", encoding='utf-8') as f:
            for i in voids:
                f.write(f"{i}\n")
    else:
        with open("./void.txt", "x", encoding='utf-8') as f:
            ...
    return DiskStatus(saves, ships, voids)


def download_range():
    # 参数检查
    if len(sys.argv) < 4:
        exit_with_help()
    no_ship = False
    no_save = False
    void_void = False
    start = int(sys.argv[2])
    end = int(sys.argv[3])
    if start > end:
        exit_with_help()
    if '--void-void' in sys.argv or '-vv' in sys.argv:
        print('尝试下载已忽略的 ID')
        void_void = True
    if "--no-ship" in sys.argv or "-nsp" in sys.argv:
        print("不下载飞船")
        no_ship = True
        if "--no-save" in sys.argv or "-nsb" in sys.argv:
            print("错误参数")
            exit_with_help()
    if "--no-save" in sys.argv or "-nsb" in sys.argv:
        print("不下载存档")
        no_save = True
    print(f"开始下载 {start} - {end}, 不下载飞船: {no_ship}, 不下载存档: {no_save}, 尝试下载已忽略的 ID: {void_void}")
    # 多线程下载  每个线程下载 10 个
    # 最后一个线程下载剩余的
    status = check_disk_status()
    # 如果尝试下载已忽略的 ID
    # 清空 status.void
    if void_void:
        status.void = []
            
    thread_pool = []
    id = 0
    for i in range(start, end + 1, 10):
        # 如果这次循环所有存档都已经存在，则跳过
        if all((x in status.save or x in status.ship or x in status.void) for x in range(i, min(i+10, end+1))):
            print(f"==========ID {i} - {min(i+10, end+1)} 已存在==========")
            continue
        
        for thread in thread_pool:
            if not thread.is_alive():
                thread_pool.pop(thread_pool.index(thread))
        if i + 10 > end:
            thread_download(id, len(thread_pool), i, end + 1, no_ship, no_save, status)
        else:
            if threaded:
                cache = threading.Thread(target=thread_download, args=(id, len(thread_pool), i, i+10, no_ship, no_save, status, ))
                while threading.active_count() >= 48:
                    time.sleep(0.01)
                cache.start()
                thread_pool.append(cache)
            else:
                thread_download(id, len(thread_pool), i, i+10, no_ship, no_save, status)
        id += 1
    # 开始所有线程
    # 等待所有线程结束
    for thread in threading.enumerate():
        if not thread == threading.current_thread() and not thread == threading.main_thread() and thread.is_alive():
            thread.join()
    print("下载完成")
    check_disk_status()


def serve_download(start_id: int, wait_time: int | None = 60) -> None:
    start_status = check_disk_status()
    wait_time = wait_time or 10
    next_id = start_id
    trys = 0
    print(f"开始下载 ID {next_id} 之后的飞船和存档，每 {wait_time} 秒检查一次")
    while True:
        # 检查等候下载的 id 状态
        if next_id in start_status.ship or next_id in start_status.save:
            print(f"ID {next_id} 已存在")
            next_id += 1
            continue
        status = download_any(next_id)
        if status == 1:
            print(f"  {time.strftime('[%H:%M:%S]', time.gmtime())} ------ ID {next_id} 飞船下载完成 -----")
            next_id += 1
            trys = 0
            continue
        elif status == 2:
            print(f"  {time.strftime('[%H:%M:%S]', time.gmtime())} ++++++ ID {next_id} 存档下载完成 +++++")
            next_id += 1
            trys = 0
            continue
        else:
            trys += 1
            if trys == 1:
                print(f"尝试下载 ID {next_id} 次数:    1", end="", flush=True)
            else:
                back = "\b" * len(str(trys))
                print(back, end="")
                print(f"{trys}", end="", flush=True)
        time.sleep(wait_time)

 
if __name__ == "__main__":

    # 如果直接运行脚本 输出帮助信息
    if len(sys.argv) == 1 or sys.argv[1] == "help":
        exit_with_help()

    # 如果运行脚本时带有参数
    if sys.argv[1] == "version":
        print(version)
        sys.exit()

    if sys.argv[1] == "list":
        print("飞船列表:")
        print(os.listdir("./ship"))
        print("存档列表:")
        print(os.listdir("./save"))
        
    print(len(sys.argv))

    if len(sys.argv) >= 3:
        match sys.argv[1]:
            case "get":
                id = int(sys.argv[2])
                status = download_any(id)
                if status == 1:
                    print(f"ID {id} 飞船下载完成")
                elif status == 2:
                    print(f"ID {id} 存档下载完成")
                else:
                    print(f"ID {id} 无效")
                sys.exit()
            case "gets":
                download_range()
            case "serve":
                id = int(sys.argv[2])
                wait_time = None
                if '-t' in sys.argv or '--time' in sys.argv:
                    wait_time = float(sys.argv[sys.argv.index('-t') + 1])
                serve_download(id, wait_time)
            case _:
                print("未知参数", sys.argv)
                exit_with_help()
