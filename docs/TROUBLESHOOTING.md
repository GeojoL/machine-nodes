# 排障（常见故障 + 定位）

先看两处真相：node 的 `~/machine-nodes-node/handshake.log`（逐回合，带 FAIL step）和 center 的 `curl http://<center>:8770/hs/status`（聚合 ok/fail）。两者对账 + step 含义基本能定位一切。

---

## 1) node 连不上 center

**现象**：装机时 `连不上 center ... 查中心是否在跑、端口/防火墙` 直接退；或 `handshake.log` 满是 `REGISTER FAIL` / `FAIL step=post-challenge[...]`。

**测**（在 node 机上）：
```bash
curl -fsS -m6 http://<center-ip>:8770/hs/status   # 通 = 回 JSON；不通 = 卡住/拒连
```
**查**：
- center 没在跑 → 去中心机 `curl http://127.0.0.1:8770/hs/status`，不通就重起 center（见下「服务起不来」）。
- 端口/防火墙挡入站 → center 机放行 `CENTER_PORT`（默认 8770）的入站；node 机确认能出站到该端口。
- 地址/端口写错 → 核对 node 的 `CENTER=`（`~/machine-nodes-node/persona.txt` 里有记），必须 `http://<ip>:<port>` 且端口与 center 实际监听一致。
- DNS/网段不通 → 用 IP 而非主机名；确认两机在同一可达网络（LAN / ZeroTier 等）。

---

## 2) 握手 FAIL step 含义（来自 PROTOCOL.md）

`handshake.log` 里 `FAIL step=<step>` 指明断在哪一环：

| step | 含义 | 多半原因 |
|------|------|----------|
| `post-challenge[<Err>]` | 挑战 POST 失败（方向1请求没发到/没回）| 网络断、center 挂、超时（20s）。`[timeout]`/`[URLError]` 看类型 |
| `DIR1-mismatch[got=..]` | center 回的 `got_challenge` ≠ node 的 `na` | 内容被改/串号；正常网络几乎不会出现，出现要查 center 文件读写是否异常 |
| `DIR1-no-center-nonce` | center 回里没 `center_nonce` | center 异常（磁盘满写不了 resp 文件？查 center.out / journalctl）|
| `post-confirm[<Err>]` | 确认 POST 失败（方向2请求没发到/没回）| 同 post-challenge：网络断/center 挂/超时 |
| `DIR2-center-rejected` | center 验 `confirm` ≠ `center_nonce` | node 读错 center 的 nonce / 串号 / center 的 resp 文件被清掉（极端慢回合超过保留窗）|

**对账线索**：`post-*` 类失败时 center 侧那一回合通常**没记到**（请求没到）；`DIR2-center-rejected` 时 center 侧会记一条 `fail`（请求到了但验不过）。比对两机 ok/fail 能区分「没送达」还是「送达了对不上」。

---

## 3) 时钟问题

握手**不校验时间窗**（nonce 比对，不依赖时间戳同步），所以时钟偏差**不会直接判 FAIL**。但：
- 各机时间偏差会让 `handshake.log` / `center.log` / `/hs/status` 的 `last_seen` 时间戳对不上，**排障读 log 时容易看花眼**——建议各机开 NTP。
- 若看到 rtt 异常（负数/巨大），先怀疑本机时钟被回拨。

---

## 4) python3 缺失 / 权限

- **python3 缺失**：installer 起手就 `command -v python3 || 需要 python3` 退出。装 python3（系统包管理器）后重跑 installer。
- **权限**：全程 user 级、不需要 root。若 `~/.local/bin`、`~/machine-nodes-{center,node}` 写不进，查这些目录的属主/权限（installer 用 `mkdir -p` 建，正常不会有问题）。LXC 等环境注意 `$HOME` 是否可写。
- **`~/.local/bin` 不在 PATH**：不影响——service unit 用的是绝对路径（`$PY $BIN/mn-*-agent.py`），不靠 PATH。

---

## 5) 服务起不来

按平台查：
```bash
# Linux (systemd user)
systemctl --user status machine-nodes-center.service   # 或 -node
journalctl --user -u machine-nodes-center.service -n 50 -f

# macOS (launchd)
launchctl list | grep machine-nodes
# 日志看 nohup/兜底输出或 plist 进程

# nohup 兜底模式
tail -f ~/machine-nodes-center/center.out      # center
tail -f ~/machine-nodes-node/node.out          # node
```
常见原因：
- 端口被占（center）→ 换 `CENTER_PORT` 重跑 center-installer，并同步更新 node 的 `CENTER=`。
- 无 systemd user（容器/精简系统）→ installer 自动落 nohup 兜底，**重启机器不会自动拉起**，需重跑 installer。
- launchd 没加载 → `launchctl unload` 后再 `launchctl load` 对应 plist（installer 已做，手动重试同样）。

---

## 6) node 登记了但不跑回合（agent 崩了）

**现象**：`/hs/status` 有该节点（registered 过），但 `ok/fail` 不再涨、`last_seen` 停在很久以前；`handshake.stat` 不更新。

**查**：
```bash
tail -n 30 ~/machine-nodes-node/handshake.log
```
- 看到 `DONE el=43200s` → soak 正常跑满 12h 结束了（不是故障）。要续跑：重跑 installer 或拉长 `HS_DURATION`。
- 看到 `STOPPED` → 有人 `touch handshake.stop` 停了（`rm` 掉哨兵 + 重起服务即续）。
- log 停在中途、无 DONE/STOPPED、且服务显示 active 但不动 → agent 卡死/异常。重起服务：
  ```bash
  systemctl --user restart machine-nodes-node.service      # Linux
  launchctl kickstart -k gui/$(id -u)/com.geojol.machine-nodes-node   # macOS（或 unload+load）
  pkill -f mn-node-agent.py                                 # nohup 模式（KeepAlive/无→需重跑 installer）
  ```
- log 里 register 之后一条回合都没有 → center 在 register 后变不可达，回 §1 测连通。

> 别只看 `/hs/status` 的「曾登记」就以为在跑——以 `last_seen` 在动、`ok/fail` 在涨为准。
