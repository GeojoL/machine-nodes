# 握手协议（read-confirmed handshake）

machine-nodes 的健康验证**不看心跳**，看**读确认挑战-应答**：一方写出一份带 nonce 的内容，另一方**必须读到该内容**才能算出正确应答；任一环节断或对不上 = 该回合**硬 FAIL**。

## 角色

- **node**：主动方。持续出站 HTTP 打 center。
- **center**：被动方。持有每节点的握手状态 + ok/fail 计数。

> 为什么 node 主动 / HTTP：只要求节点**出站**可达 center，不要求 center 能反向连进节点（很多节点在 NAT 后 / 无公钥 / 是 Mac）。这是最低部署门槛。

## 一个回合（cycle）= 证两个方向

每 `HS_INTERVAL` 秒一回合，node 执行：

```
1. node 生成 nonce na;  写本地文件 chal-<seq>(na);  读回 na_read
2. node ──POST /hs/challenge {node,seq,challenge:na_read}──▶ center
3. center 把 challenge【写进文件】chal-<seq>，再【从文件读回】得 got；生成 center nonce cn；
   写文件 resp-<seq>(got,cn)；──▶ 回 {got_challenge:got, center_nonce:cn}
4. node 验  got_challenge == na   →  ✅【方向1】center 真读到了 node 的文件内容(node→center 送达确认)
5. node 把 cn【写进文件】resp-<seq>，读回 cn_read
6. node ──POST /hs/confirm {node,seq,confirm:cn_read,rtt_ms}──▶ center
7. center 读自己的 resp-<seq> 得 cn，验 confirm == cn → ✅【方向2】node 真读到了 center 的文件内容(center→node 送达确认)
8. center 记 DIR2_OK/FAIL；回 {ok}
9. node 记 OK / FAIL(带失败步骤)
```

**两个方向都用 nonce 读确认**：方向1 证 center 读了 node 的内容（node 出的 nonce 被原样读回），方向2 证 node 读了 center 的内容（center 出的 nonce 被原样回带）。

## 失败语义（无处藏）

任一步失败 = 该回合 `FAIL step=<步骤>`：

| step | 含义 |
|------|------|
| `post-challenge[...]` | node→center 的挑战 POST 失败（网络断/center 挂）|
| `DIR1-mismatch[got=..]` | center 回的 got_challenge ≠ na（内容被改/串号）|
| `DIR1-no-center-nonce` | center 没回 nonce（异常）|
| `post-confirm[...]` | 确认 POST 失败 |
| `DIR2-center-rejected` | center 验 confirm≠cn（node 读错/串号）|

**对账**：node 本地 `ok/fail` 与 center 的 `/hs/status` 里该节点 `ok/fail` 应一致（两机各自记，互证）。差值只应是「最后一回合时序错位」。

## 与旧法对比（为什么换）

| 旧（心跳 / 发送端 fail=0）| 新（本协议）|
|---|---|
| 只证「我在发」| 证「对方收到+读了」|
| 丢包发送端看不出（实测藏过 8 条）| 丢包=硬 FAIL 入 log |
| 单边自说 | 两机 nonce 互证 |

## 参数

| env | 默认 | 说明 |
|-----|------|------|
| `HS_INTERVAL` | 60 | 回合间隔（秒）|
| `HS_DURATION` | 43200 | 总时长（秒，12h）|
| `CENTER` | — | 节点必填，如 `http://192.168.50.50:8770` |
