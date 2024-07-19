import traceback
from pathlib import Path

count = 0
num = 0

ship = """<Ship version="1" liftedOff="0" touchingGround="0">
    <Parts>
        <Part partType="pod-1" id="1" x="0.000000" y="0.750000" angle="0.000000" angleV="0.000000" editorAngle="0">
            <Pod throttle="0.000000" name="">
                <Staging currentStage="0"/>
            </Pod>
        </Part>
    </Parts>
    <Connections/>
</Ship>
"""

def check_ship():
    for file in Path("./ship").iterdir():
        count += 1
        write = False
        with open(file, "r", encoding="utf-8") as f:
            # print(file.name, end=" ")
            try:
                first_line = f.readline()
            except UnicodeDecodeError:
                traceback.print_exc()
                continue
            if first_line == "empty_ship-v0\n" or first_line == "empty_ship-v0":
                print(file, end=" ")
                print(count, num)
                num += 1
                write = True
        if write:
            with open(file, "w", encoding="utf-8") as f:
                f.write(ship)


def remove_save():
    save_list = [int(path.name[:-4]) for path in Path("./save").iterdir()]
    save_list.sort()
    print(save_list)
    print("go!")
    count = 0
    for file in Path("./ship").iterdir():
        if int(file.name[:-4]) in save_list:
            print(file)
            file.unlink()
        count += 1
        if count >= 50:
            print(file.name[:-4])
            count = 0

remove_save()
