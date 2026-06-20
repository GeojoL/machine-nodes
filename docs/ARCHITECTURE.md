# 架构（center / node 组件 + 数据流 + 目录布局）

machine-nodes 是 **中心机 ↔ 分布节点** 的极简集群：center 一个零依赖 HTTP 服务，node 一个零依赖出站 agent。健康靠**文件级读确认握手**证（见 [PROTOCOL.md](PROTOCOL.md)），不看心跳。

## 两个组件

### center-agent（`center/center-agent.py`）
中心机的**被动方**。零依赖（python3 stdlib `http.server`）。`ThreadingHTTPServer` 绑 `0.0.0.0:CENTER_PORT`。职责：

- **登记节点**：`POST /register` `{node,persona,machine}` → 写入 `registry.json`、建该节点的 `in/`、`out/` 目录。
- **握手对端**：
  - `POST /hs/challenge` `{node,seq,challenge}` → 把 node 的挑战**写进文件 `nodes/<node>/in/chal-<seq>.txt`、再从文件读回** `got`（=证 center 真读了 node 的内容），生成 `center_nonce` 写 `nodes/<node>/out/resp-<seq>.txt`，回 `{got_challenge, center_nonce}`。
  - `POST /hs/confirm` `{node,seq,confirm,rtt_ms}` → **读自己的 `resp-<seq>.txt`** 取 `center_nonce`，验 `confirm==center_nonce`（=证 node 真读了 center 的内容），记 `DIR2_OK/FAIL`、累加该节点 `ok/fail`、更新 `last_seen/last_rtt_ms`，追加 `nodes/<node>/center.log`，回 `{ok}`。
- **聚合 / 查询**：
  - `GET /hs/status` → 整张 `registry.json`（每节点 `persona/machine/ok/fail/last_seen/last_rtt_ms` 等，JSON）。
  - `GET /`（含 `/health`）→ 文本总览（节点数、总 ok/fail、每节点一行）。
- 所有写注册表/握手状态走 `LOCK`（线程锁）串行化；`registry.json` 用 `tmp + os.replace` 原子落盘。
- 每回合 confirm 后清理该节点 `in/`、`out/`，**各留最近 80 个文件**。

### node-agent（`node/node-agent.py`）
节点机的**主动方**。零依赖。**只对 center 出站 HTTP**（`urllib.request`），不要求 center 能反向连进来。职责：

1. 启动写 `handshake.log`（`NODE START ...`），调一次 `POST /register`。
2. 进 `while` 循环，每 `HS_INTERVAL` 秒跑一个 `do_cycle(seq)`（一回合证两个方向，见 PROTOCOL）。
3. 每回合把结果写 `handshake.log`（逐行 `OK`/`FAIL step=...`）+ 覆盖写 `handshake.stat`（`node ok fail seq el last`）。
4. 跑满 `HS_DURATION` 秒 → 写 `DONE` 退出；或检测到 `handshake.stop` → 写 `STOPPED` 退出。
5. 每回合清理本地 `out/`、`in/`，各留最近 80 个文件。

## 一回合（cycle）的数据流

node 每 `HS_INTERVAL` 秒发起一回合，跨两个 HTTP 请求证两个方向：

```
node                                            center
 │ ① 写 chal-<seq>(na) + 读回 na_read
 │ ──POST /hs/challenge {node,seq,challenge=na_read}──▶
 │                          ② 写 in/chal-<seq>(na)→读回 got；
 │                             生成 cn；写 out/resp-<seq>(got,cn)
 │ ◀──{got_challenge=got, center_nonce=cn}────────────
 │ ③ 验 got==na ? ✅ 方向1（center 读到了 node 的文件）
 │ ④ 写 resp-<seq>(cn) + 读回 cn_read
 │ ──POST /hs/confirm {node,seq,confirm=cn_read,rtt_ms}─▶
 │                          ⑤ 读 out/resp-<seq> 得 cn；
 │                             验 confirm==cn → DIR2_OK/FAIL；
 │                             累加 ok/fail；写 center.log
 │ ◀──{ok}────────────────────────────────────────────
 │ ⑥ 记 OK / FAIL(step) → handshake.log + handshake.stat
```

任一步断/对不上 = 该回合**硬 FAIL（带 step）**入 log，丢包无处藏。两机各自记 `ok/fail`，`/hs/status` 与 node `handshake.stat` 应一致 = 跨机对账。

## 端口

- **8770**（默认）。center 绑 `0.0.0.0:CENTER_PORT`，`CENTER_PORT` 可改（installer 透传到 service unit）。
- node 侧靠 `CENTER=http://<center-ip>:<port>` 指向 center，端口随 center 配置走，不写死。
- **只需 node→center 一个方向的出站连通**（node 能打到 center 的端口即可）。center 无需能连回 node。

## 为什么 node→center 出站 HTTP（而非 ssh）

ssh 文件握手要求 center 能**反向连进 node**（双向公钥）。很多节点做不到：Mac 无 najol 公钥、NAT 后、防火墙挡入站。**node→center 出站 HTTP 是最低部署门槛**——节点只要能访问 center 的端口（它本来就要上报）就能装。读确认语义不变：被读的「文件」就是那份必须读到内容才能正确应答的 nonce 载荷。

## 目录布局

### center（`CENTER_HOME`，默认 `~/machine-nodes-center/`）
```
~/machine-nodes-center/
├── registry.json                 # 全节点状态(persona/machine/ok/fail/last_seen/last_rtt_ms)
├── center.out                    # nohup 兜底模式的 stdout/stderr
└── nodes/
    └── <node>/
        ├── in/chal-<seq>.txt     # center 收到的 node 挑战(写入再读回)
        ├── out/resp-<seq>.txt    # center 发出的应答(got_challenge + center_nonce)
        └── center.log            # 该节点逐回合 DIR2_OK/FAIL log
```
> `in/`、`out/` 各只留最近 80 个文件（每回合 confirm 后自动清理）。

### node（`NODE_HOME`，默认 `~/machine-nodes-node/`）
```
~/machine-nodes-node/
├── persona.txt        # nodes-installer 写的身份文件(persona/node/center/machine/created/role)
├── handshake.log      # 逐回合 log(START / OK / FAIL step=.. / DONE / STOPPED)
├── handshake.stat     # 覆盖式当前快照(node ok fail seq el last)
├── handshake.stop     # touch 它 → agent 下一轮检测到即退出
├── node.out           # nohup 兜底模式的 stdout/stderr
├── out/chal-<seq>.txt # node 发出的挑战(na)
└── in/resp-<seq>.txt  # node 收到的 center 应答(center_nonce)
```
> 同样各只留最近 80 个文件。

二进制本体由 installer 拷到 `~/.local/bin/`：center 为 `mn-center-agent.py`，node 为 `mn-node-agent.py`。

## 服务（持久化）

installer 按平台三选一拉起，**统统 user 级**（无需 root）：

| 平台 | 机制 | unit / plist | 重启策略 |
|------|------|--------------|----------|
| Linux（有 systemd user）| systemd user service | `~/.config/systemd/user/machine-nodes-{center,node}.service` | `Restart=always`（center RestartSec=3 / node 5）|
| macOS | launchd LaunchAgent | `~/Library/LaunchAgents/com.geojol.machine-nodes-{center,node}.plist` | `KeepAlive=true` + `RunAtLoad` |
| 其它 / 无 systemd user | `nohup` 兜底 | 无持久化 unit，日志进 `center.out` / `node.out` | 无自动重启（重启机器后需重跑 installer）|

env（端口、CENTER、NODE、PERSONA、HS_INTERVAL、HS_DURATION）由 installer 烤进 service unit / plist，不写死在代码里。
