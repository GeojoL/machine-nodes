# 安装 / 卸载 / 升级

零依赖（python3 stdlib + bash），幂等可重跑，user 级（不要 root）。先装 **center**（中心机），再装 **node**（任意节点机）。

## 前置
- `python3`（installer 缺它直接退）。
- `curl`（远程拉取 / 探活用）。
- node 机能**出站访问 center 的端口**（默认 8770）。center 无需能连回 node。

---

## 一、装 center（中心机，如 najol）

把本机装成集群中心：握手对端 + 节点登记 + ok/fail 聚合。

### 本地跑（已克隆仓库）
```bash
cd machine-nodes
./center-installer.sh                 # 默认端口 8770
CENTER_PORT=9000 ./center-installer.sh # 自定义端口
```

### 远程一键
```bash
curl -fsSL https://raw.githubusercontent.com/GeojoL/machine-nodes/main/center-installer.sh | bash
# 自定义端口:
curl -fsSL <raw>/center-installer.sh | CENTER_PORT=9000 bash
```
> 远程模式会自动从 `MN_RAW`（默认 GitHub raw）拉 `center/center-agent.py`。

### env
| env | 默认 | 说明 |
|-----|------|------|
| `CENTER_PORT` | `8770` | center 监听端口 |
| `CENTER_HOME` | `~/machine-nodes-center` | 数据目录（registry.json + nodes/）|
| `MN_RAW` | GitHub raw 地址 | 远程拉源码的基址 |

### installer 做了什么
1. 拷 `center-agent.py` → `~/.local/bin/mn-center-agent.py`（本地仓库优先，否则远程拉）。
2. 按平台起服务：systemd user（Linux）/ launchd（macOS）/ nohup（兜底），env 烤进 unit。
3. 起后 `curl http://127.0.0.1:$PORT/hs/status` 自检，并打印节点装机要用的 `CENTER=http://<center-ip>:$PORT`。

### 验证 center
```bash
curl -s http://127.0.0.1:8770/hs/status | python3 -m json.tool   # 每节点 JSON（刚装是 {}）
curl -s http://127.0.0.1:8770/                                   # 文本总览
```
nohup 兜底模式日志看 `~/machine-nodes-center/center.out`。

---

## 二、装 node（节点机，如 Mac / 树莓派）

把本机装成节点：握手 agent + 随机命名人格 + 加入 center 握手。

### 远程一键（在节点机上跑）
```bash
curl -fsSL https://raw.githubusercontent.com/GeojoL/machine-nodes/main/nodes-installer.sh \
  | CENTER=http://<center-ip>:8770 bash
```

### 本地跑（已克隆仓库）
```bash
CENTER=http://192.168.50.50:8770 ./nodes-installer.sh
```

### env
| env | 默认 | 说明 |
|-----|------|------|
| `CENTER` | **必填** | center 地址，如 `http://192.168.50.50:8770` |
| `NODE` | `hostname`（去域名）| 节点名（center 里的 key）|
| `PERSONA` | 随机 `adj-noun-hex` | 指定人格名（默认随机生成，撞 center 已有名会重摇，见 [PERSONA.md](PERSONA.md)）|
| `HS_INTERVAL` | `60` | 回合间隔（秒）|
| `HS_DURATION` | `43200` | 总时长（秒，12h，见 [SOAK.md](SOAK.md)）|
| `MN_RAW` | GitHub raw 地址 | 远程拉源码的基址 |

### installer 做了什么
1. 拷 `node-agent.py` → `~/.local/bin/mn-node-agent.py`。
2. **探活 center**：`curl $CENTER/hs/status` 不通直接退（提示查中心/端口/防火墙）。
3. 生成随机人格（带唯一性校验），写 `~/machine-nodes-node/persona.txt`。
4. 按平台起 node-agent 服务（systemd / launchd / nohup），env 烤进 unit。
5. 自检该节点已出现在 `$CENTER/hs/status`，并提示本机/中心两处查法。

### 验证 node
```bash
cat ~/machine-nodes-node/handshake.stat          # node 自记：node ok fail seq el last
curl -s http://<center-ip>:8770/hs/status \
  | python3 -m json.tool                          # center 侧该节点 ok/fail（应与上面一致）
```
`handshake.stat` 与 `/hs/status` 一致 = 跨机对账通过。逐回合明细看 `~/machine-nodes-node/handshake.log`。

---

## 三、停 / 卸载 / 升级

### 临时停一个 node（不卸载）
node-agent 内置停止哨兵——touch 一个文件，下一轮（≤`HS_INTERVAL` 秒）它检测到即写 `STOPPED` 干净退出：
```bash
touch ~/machine-nodes-node/handshake.stop
```
> 若 service 是 `Restart=always`/`KeepAlive`，进程退出后会被拉起、重新建文件并删掉 stop 哨兵再跑。要长期停，用下面的「禁用服务」。

### 卸载 node
```bash
# Linux (systemd user)
systemctl --user disable --now machine-nodes-node.service
rm -f ~/.config/systemd/user/machine-nodes-node.service
systemctl --user daemon-reload

# macOS (launchd)
launchctl unload ~/Library/LaunchAgents/com.geojol.machine-nodes-node.plist
rm -f ~/Library/LaunchAgents/com.geojol.machine-nodes-node.plist

# nohup 兜底模式
pkill -f mn-node-agent.py

# 清数据 + 二进制（可选）
rm -rf ~/machine-nodes-node ~/.local/bin/mn-node-agent.py
```
> center 侧该节点在 `registry.json` 里的记录不会自动消失（保留历史 ok/fail）；要清就编辑 `~/machine-nodes-center/registry.json` 删掉对应 key。

### 卸载 center
```bash
# Linux
systemctl --user disable --now machine-nodes-center.service
rm -f ~/.config/systemd/user/machine-nodes-center.service
systemctl --user daemon-reload

# macOS
launchctl unload ~/Library/LaunchAgents/com.geojol.machine-nodes-center.plist
rm -f ~/Library/LaunchAgents/com.geojol.machine-nodes-center.plist

# nohup
pkill -f mn-center-agent.py

# 清数据 + 二进制（可选）
rm -rf ~/machine-nodes-center ~/.local/bin/mn-center-agent.py
```

### 升级
installer 幂等——直接**重跑同一条命令**即可：会覆盖 `~/.local/bin/mn-*-agent.py`、重写 service unit、daemon-reload 后重起服务。数据目录（`registry.json` / `handshake.*`）保留。
```bash
# center
./center-installer.sh
# node（重跑会复用已有 persona.txt 里的逻辑；如要保号显式传 PERSONA）
CENTER=http://192.168.50.50:8770 PERSONA=jade-wren-c0 ./nodes-installer.sh
```
> node 重跑默认会**重新随机一个人格**（除非显式传 `PERSONA=`）。要保留原人格，从 `~/machine-nodes-node/persona.txt` 读出旧名传进去。
