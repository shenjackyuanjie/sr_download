# sr-download 网络 API

serve 模式启动后暴露下列页面与接口。所有 JSON 响应使用同一信封：

```json
{
    "code": 200,
    "msg": "ok",
    "data": {}
}
```

`code` 与 HTTP 状态码一致，`msg` 为状态消息，`data` 为实际数据。记录不存在时返回 `404` 与 `msg: "data not found"`；ID 非法时返回 `400` 并在 `msg` 中说明原因。

## 设置

接口地址与开关由 `config.toml` 的 `[serve]` 决定：

```toml
[serve]
# 服务的地址和端口
host_with_port = "0.0.0.0:10002"
# 数据库最大连接数
db_max_connect = 10
# 是否启用 serve 模式
enable = true
# 重同步令牌，必须换成非空且非占位值
resync_token = "change-me-before-enabling-server"
# 缓存刷新间隔（毫秒）
refresh_interval = 10_000
```

## 页面

| 路由 | 说明 |
| --- | --- |
| `GET /dashboard` | 仪表板页面 |
| `GET /` | 301 重定向到 `/dashboard` |
| `GET /favicon.ico` | 站点图标 |
| `GET /assets/info.css` | 仪表板样式表 |
| `GET /assets/info.js` | 仪表板脚本 |
| `GET /assets/dark.js` | 深浅色切换脚本 |
| `GET /ua_display` | 回显请求 User-Agent，便于排查客户端问题 |

静态资源在构建时通过 `include_str!` 嵌入二进制，修改前端后需要重新构建。其他未匹配路径一律重定向到 `/dashboard`。

## 查询接口

### GET `/last/data`

获取最新一条数据的信息。

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

获取最新一条存档的信息，字段与 `/last/data` 相同，但不含 `save_type`。

### GET `/last/ship`

获取最新一条船只的信息，字段与 `/last/save` 相同。

### GET `/info/{id}`

按 ID 获取数据信息，`data` 结构与 `/last/data` 相同。ID 必须落在 PostgreSQL `integer` 范围内。

### GET `/download/{id}`

按 ID 下载原始数据。

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

这些接口同时接受 `POST`，行为与 `GET` 一致；`/info` 与 `/resync` 不带 ID 时返回空数据。

## 仪表板数据

### GET `/api/overview`

返回仪表板首页所需的聚合信息：最新数据、最新船只、最新存档与服务状态。

```json
{
    "code": 200,
    "msg": "ok",
    "data": {
        "latest_data": { "save_id": 1322273, "save_type": "save", "len": 2955, "blake_hash": "…", "xml_tested": true },
        "latest_ship": { "save_id": 1322271, "len": 13721, "blake_hash": "…", "xml_tested": true },
        "latest_save": { "save_id": 1322273, "len": 2955, "blake_hash": "…", "xml_tested": true },
        "service": { "version": "1.3.3", "web_request_count": 12, "api_request_count": 8, "uptime_human": "2h 5m", "uptime_seconds": 7500, "min_lookup_id": 76858 }
    }
}
```

### GET `/api/service`

只返回服务状态，`data` 为上面 `service` 字段的内容：版本号、网页与 API 请求计数、运行时长和可查询的最小 ID。

## 记录

### GET `/api/records/{id}`

记录详情，用于档案页展示。

```json
{
    "code": 200,
    "msg": "ok",
    "data": {
        "info": { "save_id": 1322271, "save_type": "ship", "len": 13721, "blake_hash": "…", "xml_tested": true },
        "xml_status": "verified ship",
        "raw_data": "<Ship version=\"1\" liftedOff ..."
    }
}
```

`xml_status` 取值与校验分类一致：`not xml`、`not ship`、`fake ship`、`broken ship`、`verified ship`。

### GET `/api/records/{id}/raw`

记录详情加上原始 XML，`data` 结构与 `/download/{id}` 相同。

### GET `/api/records/{id}/analysis`

获取已通过 XML 与飞船结构校验的飞船工程分析。只有 `save_type=ship`、`xml_tested=true` 且 `xml_status` 为 `verified ship` 的记录可用；不合格记录返回 `422`，不存在返回 `404`。

返回内容包括：

- 飞船状态、部件数量、激活/爆炸/未知部件统计
- 基于 PartList.xml 的部件清单与质量估算
- 当前燃料、燃料容量、引擎/RCS/太阳能板统计
- Pod 级序和触发部件
- 普通连接、Dock 连接、断开组和布局包围盒

质量字段同时提供 `catalog_units` 和兼容旧分析脚本的 `scaled_units`，后者使用 `scale=500`。

### GET `/api/market`

分页查询已入库的数据，用于网页市场页。

查询参数：

- `page`：页码，从 1 开始，默认 1
- `limit`：每页数量，默认 25，最大 100
- `type`：`all`、`ship`、`save` 或 `unknown`
- `sort`：排序字段，`id`（默认）、`len` 或 `time`
- `dir`：排序方向，`asc` 或 `desc`（默认）
- `min_len` / `max_len`：按数据长度（字节）过滤，同时给出时不能 min 大于 max
- `xml`：`all`（默认）、`valid` 或 `invalid`
- `logic`：高级条件之间的组合方式，`and`（默认）或 `or`
- `filters`：JSON 数组，每项形如 `{"field":"len","op":"gte","value":1024}`

`filters` 支持的字段：`save_id`、`save_type`、`len`、`xml_tested`、`blake_hash`、`time`；
支持的操作符：`eq`、`ne`、`ilike`、`not_ilike`、`gt`、`lt`、`gte`、`lte`、`is_null`、`is_not_null`。
`ilike` 只适用于文本字段，`is_null` / `is_not_null` 不需要 `value`，时间取 `YYYY-MM-DD`、`YYYY-MM-DD HH:MM:SS` 或 RFC3339（按 UTC 处理）。
一次请求最多 16 个条件，参数非法时返回 `400` 并带有具体原因。

示例：

```http
GET /api/market?page=2&limit=25&type=ship&sort=len&dir=desc&min_len=1024&filters=[{"field":"time","op":"gte","value":"2026-01-01"}]
```

返回内容：

```json
{
    "code": 200,
    "msg": "ok",
    "data": {
        "records": [
            {
                "save_id": 1329431,
                "save_type": "ship",
                "len": 287,
                "blake_hash": "…",
                "xml_tested": true,
                "recorded_at": "2026-10-28T08:12:00+00:00"
            }
        ],
        "page": 2,
        "limit": 25,
        "total": 843824,
        "total_len": 7452021120,
        "has_more": true
    }
}
```

`total` 是当前筛选条件下的记录总数，`total_len` 是对应记录的长度总和，`has_more` 表示后面还有数据。

## 重同步

### GET `/resync/{id}`

重新下载指定 ID，并更新数据库：

- 需要 `x-resync-token` 请求头，缺失返回 `400`，无效返回 `401`
- 同步执行，返回响应时已完成抓取与写入
- hash 未变化时不写库，`msg` 为 `Data unchanged, no update needed`
- 返回结构与 `/download/{id}` 相同

```http
GET /resync/1322271
x-resync-token: <your_resync_token>
```

数据更新成功：

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

数据未变化：

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

缺少令牌：

```json
{
    "code": 400,
    "msg": "Bad Request",
    "data": "Missing token"
}
```

令牌无效：

```json
{
    "code": 401,
    "msg": "Unauthorized",
    "data": "Invalid token"
}
```

### GET / POST `/resync`

不带 ID 时返回空数据，用于连通性检查。
