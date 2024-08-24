# SR 存档下载计划

## V2

Rewritten in Rust !

(其实最早 v1 就是用 rust 写的, 不过我 2023 年 6 月 可才刚学 rust, 代码写的很烂, 所以还是回到了 python 小脚本)

### API

现在支持提供 api 了

- `GET /last_data` 获取最新的数据信息
  - 返回范例:

    ```json
    {
        "save_id": 1322269,
        "save_type": "save",
        "len": 3404,
        "blake_hash": "0b4758dbda98fea0ab6ad58fd589ccc7bb14c29ab8b22e6e49b670db8fec8da9"
    }
    ```

- `GET /last_save` 获取最新的存档信息
  - 返回范例:

    ```json
    {
        "save_id": 1322269,
        "len": 3404,
        "blake_hash": "0b4758dbda98fea0ab6ad58fd589ccc7bb14c29ab8b22e6e49b670db8fec8da9"
    }
    ```

- `GET /last_ship` 获取最新的船只信息
  - 返回范例:

    ```json
    {
        "save_id": 1322267,
        "len": 38967,
        "blake_hash": "9474267203155e5cf31e0e7e34ec014773f8f89c78d262f5bd57b6e27fdc25b2"
    }
    ```

## V1

> 2023 06

其实就是目录底下的 `get_sr_save.py` 和 `parse.py` 两个文件，分别用来下载存档和解析存档。

也没啥东西就是了, python 随手写的脚本下载器

> 使用 NTFS 数据库系统，"存储文件信息，方便检索"
