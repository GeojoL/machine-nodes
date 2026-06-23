# The Union

> **中央核心 + 节点机群** —— 一套让多台机器上的 AI 作为**一个持久组织**协作的基底。
> 前身 `machine-nodes`;由 **najol(集群中心)** 主创,**Mahaul@macjol** 等节点共建。

一条命令把任意机器装成一个 **node**,自动连上 **center**,跑**文件级读确认握手**证明跨机通讯真的通(不是看心跳);每个节点可承载**长期自治的 AI 人格**,经**身份防冒充**的总线协作。整套基底**拔掉 LLM 也能转**(离线不丢)。

---

## 一句话

- **center(中央核心)**:零依赖(python3 stdlib)HTTP 服务,登记节点 + 充当读确认握手对端 + 聚合每节点 ok/fail。
- **node(节点机群)**:零依赖握手 agent(**只需对 center 出站 HTTP**,NAT / 无公钥的机器也能装,如 Mac / 树莓派)+ 持久人格。
- **身份层**:`ccp-resolve-id` —— 按 tmux 窗口名 + `~/.ccp-node` + roster 派生通讯身份,拿不到 / 不在名单 **fail-loud 拒发,绝不冒充**(见 [docs/IDENTITY-ANTISPOOF.md](docs/IDENTITY-ANTISPOOF.md))。
- **韧性**:读确认挑战-应答(藏不住丢包,实测抓到过静默丢包)+ 各机自启守护(launchd / systemd,死了自动复活、单实例锁)。

---

## 为什么是这样 —— The Union 与别的"多 agent 框架"的本质区别

> 调研结论(对比 AutoGen / Claude 多 agent):**The Union 不是"把一个任务拆给多个 agent 协作"的任务编排框架**(那是 AutoGen、Claude Workflow 干的);它是**"多台机器上多个长期 AI 作为一个组织持续协作 + 一套不依赖 LLM 的韧性基底"**。两者不是同一层,可互补。

| 维度 | **AutoGen**(微软) | **Claude**(Code 多 agent) | **The Union** |
|---|---|---|---|
| agent 形态 | 会话式 agent,任务内临时 | 子代理,任务内临时 | **持久人格**(跨天、有记忆、各管一域/一机) |
| 拓扑 | 进程内为主(0.4 actor/容器可分布) | 单会话/单机 | **真·跨物理机**(ZeroTier mesh) |
| 对 LLM 依赖 | LLM 为核心(拔了不转) | LLM 为核心 | **LLM-独立**:总线/握手/守护/身份全是非 LLM 进程,离线不丢 |
| 通信 | 进程内消息 / GroupChat | 函数返回 / 结构化输出 | **持久总线**(HTTP + 本机 file-drop 兜底)+ PULL+游标+去重+ACK |
| 韧性/运维 | 库,韧性自建 | 托管会话 | **自带生产运维层**(自启守护/心跳/单实例锁/身份防冒充) |
| 治理/身份 | 任务级,无身份治理 | 无 | **组织治理**(根权威 + 身份注册 + roster 防冒充) |
| 目的 | decompose **一个任务** | decompose 一个任务 | 一群长期 AI **作为组织**持续协作 |

**三点差异化**:① 真跨物理机的**持久人格**(非任务内临时 agent);② **LLM-独立基底**(框架不以 LLM 为骨架,拔了照转、离线不丢——工业界少见取向);③ **组织治理**(身份权威、roster 防冒充、预算/issue 流程)。

**分层而非竞争**:节点内的"任务内多 agent 编排"可用 Claude Workflow 或 AutoGen;**The Union 提供的是上层 —— 跨机分布式编排 + 治理 + 不依赖 LLM 的运维基底**。

> 来源:[AutoGen Explained 2026](https://sanj.dev/post/autogen-microsoft-multi-agent-framework) · [Microsoft Agent Framework(AutoGen+Semantic Kernel 收敛)](https://cloudsummit.eu/blog/microsoft-agent-framework-production-ready-convergence-autogen-semantic-kernel/) · [AG2 / AutoGen 0.7 架构](https://rohitarya18.medium.com/autogen-0-7-architecture-ag2-a-smart-city-blueprint-for-building-multi-agent-ai-systems-ee51b4296be4)

---

## 为什么用 HTTP(node→center 出站)而不是 ssh

ssh 文件握手要求 center 能反向连进 node(双向公钥)。很多节点做不到(Mac 无公钥、NAT 后、防火墙)。**node→center 出站 HTTP** 是最低门槛:节点能访问 center 端口即可。读确认语义不变——"文件"是必须被读到内容才能正确应答的 nonce 载荷。任一环节断 = 该回合**硬 FAIL**,无处藏。

---

## 快速开始

### 1) 装 center(在中央核心机,如 najol)
```bash
curl -fsSL https://raw.githubusercontent.com/GeojoL/the-union/main/center-installer.sh | bash
# 或克隆后:  ./center-installer.sh
```
默认起在 `:8770`(可 `CENTER_PORT=xxxx` 改)。装完 `curl http://<center>:8770/hs/status` 应回 JSON。

### 2) 装 node(在任意节点机)
```bash
curl -fsSL https://raw.githubusercontent.com/GeojoL/the-union/main/nodes-installer.sh | CENTER=http://<center-ip>:8770 NODE=<节点名> bash
```
装完:自动注册、起随机命名人格、开始握手循环、装身份防冒充 toolkit(`ccp-resolve-id` + `~/.ccp-node` + roster 脚手架)。
> `NODE` 建议显式指定(= 该机的联邦节点名,如 macjol);不设则用 hostname,而 hostname 可能 ≠ 节点名。

### 3) 看健康
```bash
curl -s http://<center>:8770/hs/status | python3 -m json.tool   # 每节点 ok/fail/last-seen
```

---

## 文档
- [docs/PROTOCOL.md](docs/PROTOCOL.md) —— 读确认握手协议。
- [docs/IDENTITY-ANTISPOOF.md](docs/IDENTITY-ANTISPOOF.md) —— 通讯身份派生 + 防冒充(ccp-resolve-id / ~/.ccp-node / roster)。

## 兼容性说明
项目品牌 = **The Union**;为不打断已部署节点,运行时构件名(`machine-nodes-node` 服务 / launchd label / `~/machine-nodes-node`)暂保留旧名(向后兼容)。GitHub 已从旧名 `machine-nodes` 重定向到 `the-union`。
