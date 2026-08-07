
# sr-download 提供的网络 API

## 设置

要想设置这部分内容，请编辑 `config.toml` 文件：

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

### GET `/dashboard`
展示当前信息的仪表板页面

### GET `/`
重定向到 `/dashboard`

### GET `/favicon.ico`
获取网站图标

### GET `/info.js`
获取仪表板页面所需的 JavaScript 文件

### GET `/dark.js`
获取深色模式 JavaScript 文件

### GET `/info.css`
获取仪表板页面样式表

## API

### GET `/last/data`
获取最新的数据信息

```json
{
    "code": 200,
    "msg": "ok",
    "data": {
        "save_id": 1322273,
        "save_type": "save",
        "len": 2955,
        "blake_hash": "1e327361ae30604f7828f3e1a0987098a61a16df0ce830352237e60c9db434fe",
        "xml_tested": true
    }
}
```

### GET `/last/save`
获取最新的保存信息

```json
{
    "code": 200,
    "msg": "ok",
    "data": {
        "save_id": 1322273,
        "len": 2955,
        "blake_hash": "1e327361ae30604f7828f3e1a0987098a61a16df0ce830352237e60c9db434fe",
        "xml_tested": true
    }
}
```

### GET `/last/ship`
获取最新的船只信息

```json
{
    "code": 200,
    "msg": "ok",
    "data": {
        "save_id": 1322271,
        "len": 13721,
        "blake_hash": "79c97ca4fe9fa982209e58d1e11df6ebf22cf2e96a2fc8cc48f9316982e6d7d5",
        "xml_tested": true
    }
}
```

### GET `/info/:id`
根据 ID 获取数据信息

```json
{
    "code": 200,
    "msg": "ok",
    "data": {
        "save_id": 1322271,
        "save_type": "ship",
        "len": 13721,
        "blake_hash": "79c97ca4fe9fa982209e58d1e11df6ebf22cf2e96a2fc8cc48f9316982e6d7d5",
        "xml_tested": true
    }
}
```

### GET `/download/:id`
根据 ID 下载原始数据

```json
{
    "code": 200,
    "msg": "ok",
    "data": {
        "info": {
            "save_id": 1322271,
            "save_type": "ship",
            "len": 13721,
            "blake_hash": "79c97ca4fe9fa982209e58d1e11df6ebf22cf2e96a2fc8cc48f9316982e6d7d5",
            "xml_tested": true
        },
        "raw_data": "<Ship version=\"1\" liftedOff ..."
    }
}
```

### GET /api/records/:id/analysis
获取已通过后端 XML 和飞船结构校验的飞船工程分析。

只有 save_type=ship、xml_tested=true 且 xml_status=verified ship 的记录可用。
非合格记录返回 422，不存在的记录返回 404。

返回数据包括：

- 飞船状态、部件数量、激活/爆炸/未知部件统计
- 基于 PartList.xml 的部件清单与质量估算
- 当前燃料、燃料容量、引擎/RCS/太阳能板统计
- Pod 级序和触发部件
- 普通连接、Dock 连接、断开组和布局包围盒

质量字段同时提供目录单位 catalog_units 和兼容旧分析脚本的 scaled_units，
后者使用 scale=500。

### GET `/api/market`
获取最近入库的数据列表，用于网页“市场”页面。

查询参数：

- `limit`：每页数量，默认 48，最大 100
- `type`：`all`、`ship`、`save` 或 `unknown`
- `before`：只返回记录 ID 小于该值的数据，用于继续加载更早记录

返回的每条记录包含记录 ID、数据类型、长度、Blake3、XML 是否通过和入库时间。

### GET `/resync`
触发数据重新同步操作

**功能说明**:
- 强制程序重新扫描和同步最新数据
- 需要提供有效的 `x-resync-token` 请求头进行身份验证
- 会向下载器发送重新同步指令
- 操作是异步执行的，API 会立即返回响应

**请求头**:
```http
x-resync-token: <your_resync_token>
```

**成功响应**:
根据操作结果返回两种可能的成功响应：

1. 数据成功更新:
```json
{
    "code": 200,
    "msg": "ok",
    "data": {
        "info": {
            "save_id": 1322271,
            "save_type": "ship",
            "len": 13721,
            "blake_hash": "79c97ca4fe9fa982209e58d1e11df6ebf22cf2e96a2fc8cc48f9316982e6d7d5",
            "xml_tested": true
        },
        "raw_data": "<Ship version=\"1\" liftedOff ..."
    }
}
```

2. 数据未变化(无需更新):
```json
{
    "code": 200,
    "msg": "Data unchanged, no update needed",
    "data": {
        "info": {
            "save_id": 1322271,
            "save_type": "ship",
            "len": 13721,
            "blake_hash": "79c97ca4fe9fa982209e58d1e11df6ebf22cf2e96a2fc8cc48f9316982e6d7d5",
            "xml_tested": true
        },
        "raw_data": "<Ship version=\"1\" liftedOff ..."
    }
}
```

**功能说明更新**:
- 端点路径为 `/resync/:id` 而非 `/resync`
- 需要指定要重新同步的数据ID
- 重新下载指定ID的数据并更新数据库
- 如果数据哈希值未变化则不更新
- 返回重新下载后的完整数据
- 操作是同步执行的，返回响应时已完成更新

**错误响应**:
1. 缺少 token:
```json
{
    "code": 400,
    "msg": "Bad Request",
    "data": "Missing token"
}
```

2. 无效 token:
```json
{
    "code": 401,
    "msg": "Unauthorized",
    "data": "Invalid token"
}
```

**配置说明**:
在 `config.toml` 中需要设置 resync token:
```toml
[serve]
resync_token = "your_secure_token_here"  # 用于API重新同步的认证令牌

## 响应结构

所有 API 响应都遵循以下结构：
```json
{
    "code": 200,      // HTTP 状态码
    "msg": "ok",      // 状态消息
    "data": {}        // 实际返回的数据
}
```
