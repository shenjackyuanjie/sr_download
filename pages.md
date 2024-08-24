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

## 页面

`/dashboard`

展示当前信息

## API

### GET `/last/data`

```json
{
    "code": 200,
    "msg": "ok",
    "data": {
        "save_id": 1322273,
        "save_type": "save",
        "len": 2955,
        "blake_hash": "1e327361ae30604f7828f3e1a0987098a61a16df0ce830352237e60c9db434fe"
    }
}
```

### GET `/last/save`

```json
{
    "code": 200,
    "msg": "ok",
    "data": {
        "save_id": 1322273,
        "len": 2955,
        "blake_hash": "1e327361ae30604f7828f3e1a0987098a61a16df0ce830352237e60c9db434fe"
    }
}
```

### GET `/last/ship`

```json
{
    "code": 200,
    "msg": "ok",
    "data": {
        "save_id": 1322271,
        "len": 13721,
        "blake_hash": "79c97ca4fe9fa982209e58d1e11df6ebf22cf2e96a2fc8cc48f9316982e6d7d5"
    }
}
```

### GET `/info/:id`

```json
{
    "code": 200,
    "msg": "ok",
    "data": {
        "save_id": 1322271,
        "save_type": "ship",
        "len": 13721,
        "blake_hash": "79c97ca4fe9fa982209e58d1e11df6ebf22cf2e96a2fc8cc48f9316982e6d7d5"
    }
}
```

### GET `/download/:id`

```json
{
    "code": 200,
    "msg": "ok",
    "data": {
        "info": {
            "save_id": 1322271,
            "save_type": "ship",
            "len": 13721,
            "blake_hash": "79c97ca4fe9fa982209e58d1e11df6ebf22cf2e96a2fc8cc48f9316982e6d7d5"
        },
        "raw_data": "<Ship version=\"1\" liftedOff ..."
    }
}
```
