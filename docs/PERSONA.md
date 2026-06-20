# 节点人格（persona）

每个 node 装机时拿到一个**随机命名的人格**，作为它在集群里的身份标识。人格由 `nodes-installer.sh` 生成，登记进 center，写进本地身份文件。

## 命名规则：`adjective-noun-hex`

`nodes-installer.sh` 从两组词表 + 一个十六进制后缀拼出名字：

- **形容词**（15 个）：`swift calm bright bold quiet keen lucid amber jade onyx coral slate vivid noble brisk`
- **名词**（15 个，都是小动物/鸟）：`otter lynx heron raven ibis koi crane fox marten egret tern vole stoat finch wren`
- **十六进制后缀**：`00`–`ff`（`printf '%02x' $((RANDOM%256))`）

格式 `adj-noun-hex`，例：`jade-wren-c0`、`swift-otter-1f`、`bold-koi-a3`。词表 15×15×256 ≈ 5.7 万组合，碰撞概率低。

## 唯一性校验（防撞名）

随机名不是盲取——installer 会**比对 center 现有人格**确保唯一：

1. `curl $CENTER/hs/status` 拉全节点状态，提取所有已用 `persona`。
2. 最多重摇 8 次，若候选名已在 center 出现就重摇。
3. 8 次都撞（极罕见）→ 退化成 `node-<时间戳尾5位>` 兜底。

> 显式传 `PERSONA=<名>` 给 installer 可跳过随机生成，直接用指定名（此时不做唯一性重摇，由你自己保证不撞）。

## 身份文件 `persona.txt`

installer 把人格落到 `~/machine-nodes-node/persona.txt`，内容如：
```
persona=jade-wren-c0
node=ken-mac
center=http://192.168.50.50:8770
machine=Darwin arm64
created=2026-06-20T14:32:10
role=machine-nodes 节点级人格(随机命名);职责=本机节点 agent + 与 center 读确认握手
```
字段：`persona`（人格名）、`node`（节点名 = center 里的 key）、`center`（中心地址）、`machine`（`uname -s` + `uname -m`）、`created`（创建时间）、`role`（职责说明）。

## 生命周期

1. **创建**：装 node 时，installer 随机生成（带唯一性校验）并写 `persona.txt`。
2. **登记**：node-agent 启动调 `POST /register {node,persona,machine}`，center 把 persona 记进 `registry.json`，之后出现在 `/hs/status` 和 `/` 总览里。
3. **存活**：人格 = 该节点的身份，贯穿其全部握手回合（`center.log` / `handshake.log` 都带它）。同一节点重装若不显式保号，会换一个新随机人格。

> **保留原人格**：从 `~/machine-nodes-node/persona.txt` 读出 `persona=` 的值，重跑 installer 时传 `PERSONA=` 即可。

## 关于「活的 AI 会话」（v0 范围说明）

人格名沿用了集群里 AI 人格的命名风格，但 **v0 里 installer 只做两件事：建身份（`persona.txt`）+ 向 center 登记**。是否在该节点真起一个常驻 Claude/AI 会话来「扮演」这个人格，是**可选的、且不在 v0 范围内**。当前人格纯粹是节点的标识 + 握手身份；起活会话留给后续版本/上层编排，不影响读确认握手本身。
