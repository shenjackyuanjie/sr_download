# SR 存档下载计划

## V2

Rewritten in Rust !

(其实最早 v1 就是用 rust 写的, 不过我 2023 年 6 月 可才刚学 rust, 代码写的很烂, 所以还是回到了 python 小脚本)

可以使用 `cargo build --release --target x86_64-unknown-linux-musl` 来构建 musl 版本的二进制文件

### API

现在支持提供 api 了

具体 API 请参考 [这个页面](./pages.md)

## V1

> 2023 06

其实就是目录底下的 `get_sr_save.py` 和 `parse.py` 两个文件，分别用来下载存档和解析存档。

也没啥东西就是了, python 随手写的脚本下载器

> 使用 NTFS 数据库系统，"存储文件信息，方便检索"
