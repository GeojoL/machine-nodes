# 12h 文件握手压力测（soak）

soak = 让一个 node 连续 12 小时、每 60 秒跑一回合**双向读确认握手**，用真实 ok/fail 证跨机通讯持续可靠——而不是「我在发心跳所以应该没事」。

## 怎么跑

node-agent 本身就是 soak 引擎，无需额外脚本。装 node 时由 env 控制：

| env | 默认 | soak 含义 |
|-----|------|-----------|
| `HS_DURATION` | `43200` | 总时长（秒）= **12h**。跑满写 `DONE` 退出 |
| `HS_INTERVAL` | `60` | 回合间隔（秒）。12h ÷ 60s ≈ **720 回合** |

即默认装一个 node 就是一场 12h soak：
```bash
CENTER=http://192.168.50.50:8770 ./nodes-installer.sh
# 或自定义时长(如 1h 快测):
CENTER=http://192.168.50.50:8770 HS_DURATION=3600 ./nodes-installer.sh
```
循环逻辑：每回合 `do_cycle(seq)` 跑完一次两方向握手 → 写 `handshake.log` + `handshake.stat` → `sleep HS_INTERVAL` → 直到累计 `HS_DURATION` 写 `DONE`，或检测到 `handshake.stop` 写 `STOPPED`。

## 怎么读真实 ok/fail（两机对账）

soak 的价值在**两机各自独立记账、互相印证**：

- **node 侧**（自记）：`cat ~/machine-nodes-node/handshake.stat`
  ```
  node=ken-mac ok=712 fail=0 seq=712 el=42720s last=2026-06-20T...
  ```
  逐回合明细在 `handshake.log`（`OK` / `FAIL step=...`）。
- **center 侧**（被动记）：`curl -s http://<center>:8770/hs/status | python3 -m json.tool`
  ```json
  { "ken-mac": { "persona": "jade-wren-c0", "ok": 712, "fail": 0, "last_seen": "...", "last_rtt_ms": 23 } }
  ```

**对账规则**：node `handshake.stat` 的 `ok/fail` 应与 center `/hs/status` 里该节点的 `ok/fail` **一致**。两机在不同机器上各自数，对上 = 跨机对账通过，结果可信。允许的唯一差值是「最后一回合时序错位」（node 已发但 center 还没记完，差 1）。差值持续 >1 或方向相反 = 有回合被一方判 FAIL 另一方没记到，需查（多半是 confirm POST 在路上断了，见 [TROUBLESHOOTING.md](TROUBLESHOOTING.md)）。

## 为什么比心跳强

心跳/发送端 `fail=0` 只证「我在发」，**藏丢包**（实测抓到过 macjol→najol 静默丢 8 条）。本 soak 用读确认：每回合两个方向都要对方**读到 nonce 内容并原样带回**才算过。任一环节断 = 该回合**硬 FAIL，且带 step**（`post-challenge` / `DIR1-mismatch` / `post-confirm` / `DIR2-center-rejected`，见 [PROTOCOL.md](PROTOCOL.md)），落进 `handshake.log` 和 center.log。一个丢包就是一条 FAIL 记录，**无处藏**。

| 心跳法 | 本 soak |
|--------|---------|
| 只证「我在发」 | 证「对方收到 + 读了」 |
| 丢包发送端看不出 | 丢包 = 硬 FAIL 带 step 入双方 log |
| 单边自说 | 两机 nonce 互证 + ok/fail 对账 |

## 起 / 停

- **起**：装 node 即开始（见上）。重启机器后 systemd `Restart=always` / launchd `KeepAlive` 会自动拉起续跑。
- **停**：`touch ~/machine-nodes-node/handshake.stop` → 下一轮（≤`HS_INTERVAL` 秒）写 `STOPPED` 干净退出。
  > 服务是常驻的，停进程后会被拉起重跑。要彻底停整场 soak，禁用服务（见 [INSTALL.md](INSTALL.md) 卸载/禁用）。

## 健康 vs 退化 长啥样

**健康（12h 全绿）**：
```
node=ken-mac ok=720 fail=0 seq=720 el=43200s ...
```
`handshake.log` 尾部 `DONE el=43200s ok=720 fail=0`，center `/hs/status` 同节点 `ok=720 fail=0`，两边一致。rtt 稳定（`last_rtt_ms` 个位/十位毫秒）。

**退化（有掉链）**：
```
node=ken-mac ok=705 fail=15 seq=720 ...
```
`handshake.log` 里能看到具体哪些回合 FAIL、卡在哪步：
```
[..] seq=331 FAIL step=post-confirm[URLError] 20013ms
[..] seq=332 FAIL step=post-challenge[timeout] ...
```
连续 FAIL + rtt 飙高（接近 20000ms 超时上限）= 网络抖动/center 短暂不可达；零星单条 FAIL = 偶发丢包。对照 center 侧 `ok/fail` 确认是否两机都记到，再按 step 含义定位（[TROUBLESHOOTING.md](TROUBLESHOOTING.md)）。
