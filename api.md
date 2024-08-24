# sr-download 提供的网络 api

## 设置

要想设置这部分内容

请编辑 `config.toml` 文件

```toml
# 这个部分
[serve]
# 服务的地址和端口
host_with_port = "0.0.0.0:10002"
# 数据库最大连接数
db_max_connect = 10
# 是否启用 serve 模式
enable = true
```

## API

### GET `/last/data`

